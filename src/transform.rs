use core::mem;
use serde::Deserialize;
use std::borrow::Cow;
use std::fmt::Write;
use swc_core::common::util::take::Take;
use swc_core::common::{Mark, Spanned, DUMMY_SP};
use swc_core::ecma::ast::*;
use swc_core::ecma::atoms::js_word;
use swc_core::ecma::atoms::{Atom, JsWord};
use swc_core::ecma::utils::IdentExt;
use swc_core::ecma::visit::{noop_visit_mut_type, VisitMut, VisitMutWith};
use swc_core::plugin::errors::HANDLER;
use swc_core::trace_macro::swc_trace;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransformVisitor {
	#[serde(deserialize_with = "de::expr", default = "default_template_fn")]
	template: Option<Box<Expr>>,
	#[serde(deserialize_with = "de::str", default)]
	import_source: Option<Str>,
	#[serde(deserialize_with = "de::ident", default = "default_spread")]
	spread: Ident,
	#[serde(deserialize_with = "de::ident", default = "default_child")]
	child: Ident,
	#[serde(deserialize_with = "de::ident", default = "default_children")]
	children: Ident,
	#[serde(skip)]
	quasis: Vec<String>,
	#[serde(skip)]
	#[allow(clippy::vec_box)]
	exprs: Vec<Box<Expr>>,
}

#[inline]
fn default_spread() -> Ident {
	Ident::new("$$spread".into(), DUMMY_SP)
}
#[inline]
fn default_child() -> Ident {
	Ident::new("$$child".into(), DUMMY_SP)
}
#[inline]
fn default_children() -> Ident {
	Ident::new("$$children".into(), DUMMY_SP)
}
#[inline]
fn default_template_fn() -> Option<Box<Expr>> {
	Some(Box::new(Expr::Member(MemberExpr {
		span: DUMMY_SP,
		obj: Box::new(Expr::Ident(Ident::new(js_word!("String"), DUMMY_SP))),
		prop: MemberProp::Ident(Ident::new("raw".into(), DUMMY_SP)),
	})))
}

impl Default for TransformVisitor {
	fn default() -> Self {
		Self {
			template: default_template_fn(),
			child: default_child(),
			children: default_children(),
			spread: default_spread(),
			import_source: None,
			quasis: vec![],
			exprs: vec![],
		}
	}
}

#[swc_trace]
impl TransformVisitor {
	#[inline]
	fn quasi_last_mut(&mut self) -> &mut String {
		self.quasis.last_mut().unwrap()
	}

	#[inline]
	fn push(&mut self, expr: Box<Expr>) {
		self.quasis.push(" ".to_string());
		self.exprs.push(expr);
	}

	fn fold_jsx_element(&mut self, elt: &mut JSXElement) {
		let JSXOpeningElement { span, name, attrs, .. } = &mut elt.opening;
		let name = match name {
			JSXElementName::JSXNamespacedName(name) => format!("{}:{}", name.ns.sym, name.name.sym),
			JSXElementName::JSXMemberExpr(..) => unreachable(span.take()),
			JSXElementName::Ident(ident) => ident.sym.to_string(),
		};

		fn extract_static_attr_pair(attr: &JSXAttrOrSpread) -> Option<(&JSXAttrName, Option<Cow<str>>)> {
			match attr {
				JSXAttrOrSpread::JSXAttr(JSXAttr { name, value, .. }) => match value {
					Some(JSXAttrValue::Lit(Lit::Str(Str { value, .. }))) => Some((name, Some(value.as_ref().into()))),
					Some(JSXAttrValue::Lit(other_lit)) => {
						HANDLER.with(|handler| handler.span_bug(other_lit.span(), "Impossible JSX attribute value"))
					}
					Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
						expr: JSXExpr::Expr(expr),
						..
					})) => match expr.as_ref() {
						Expr::Tpl(tpl) if tpl.exprs.is_empty() => {
							let [TplElement { cooked, raw, .. }] = &tpl.quasis[..] else {
								unreachable(tpl.span)
							};
							Some((name, Some(cooked.as_deref().unwrap_or(raw.trim()).into())))
						}
						Expr::Lit(Lit::Bool(value)) if value.value => Some((name, None)),
						Expr::Lit(Lit::Str(Str { value, .. })) => Some((name, Some(value.as_ref().into()))),
						Expr::Lit(Lit::Num(Number { value, .. })) => Some((name, Some(value.to_string().into()))),
						_ => None,
					},
					Some(_) => None,
					None => Some((name, None)),
				},
				_ => None,
			}
		}

		let leading_static_attrs = attrs
			.iter()
			.map_while(extract_static_attr_pair)
			.map(|(name, value)| {
				let name = jsx_attr_name_as_str(name);
				match value {
					Some(value) => format!("{name}=\"{value}\""),
					None => name.to_string(),
				}
			})
			.collect::<Vec<_>>();
		let leading_static_attrs_count = leading_static_attrs.len();
		let leading_static_attrs = leading_static_attrs.join(" ");
		let first = if leading_static_attrs.is_empty() {
			format!("<{name} ")
		} else {
			format!("<{name} {} ", leading_static_attrs)
		};

		if self.quasis.is_empty() {
			self.quasis.push(first);
		} else {
			_ = self.quasi_last_mut().write_str(&first);
		}

		let mut props = vec![];
		for attr in attrs.iter_mut().skip(leading_static_attrs_count) {
			if let Some((name, value)) = extract_static_attr_pair(attr) {
				let name = jsx_attr_name_as_str(name);
				let last = self.quasi_last_mut();
				if let Some(value) = value {
					_ = write!(last, "{name}=\"{value}\" ");
				} else {
					_ = write!(last, "{name} ");
				}
				continue;
			}
			match attr {
				JSXAttrOrSpread::JSXAttr(JSXAttr { name, value, .. }) => {
					let name = jsx_attr_name_as_str(name);
					if let Some(value) = value {
						let value = match value {
							JSXAttrValue::Lit(lit) => Box::new(Expr::Lit(take_lit(lit))),
							JSXAttrValue::JSXExprContainer(JSXExprContainer { expr, .. }) => match expr {
								JSXExpr::JSXEmptyExpr(empty) => Box::new(Expr::JSXEmpty(*empty)),
								JSXExpr::Expr(expr) => expr.take(),
							},
							JSXAttrValue::JSXElement(elt) => Box::new(Expr::JSXElement(elt.take())),
							JSXAttrValue::JSXFragment(frag) => Box::new(Expr::JSXFragment(frag.take())),
						};
						props.push((JsWord::from(name), value));
					} else {
						_ = write!(self.quasi_last_mut(), "{name} ");
					}
				}
				JSXAttrOrSpread::SpreadElement(SpreadElement { expr, .. }) => {
					if let Expr::Object(ObjectLit { props: obj_props, .. }) = expr.as_mut() {
						let mut extractor = ExtractStaticProps {
							buffer: self.quasi_last_mut(),
						};
						obj_props.visit_mut_with(&mut extractor);
						if !obj_props.is_empty() {
							self.push(Box::new(Expr::Object(ObjectLit {
								span: DUMMY_SP,
								props: vec![PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
									key: PropName::Ident(self.spread.clone()),
									value: expr.take(),
								})))],
							})));
						}
						continue;
					}
					self.push(Box::new(Expr::Object(ObjectLit {
						span: DUMMY_SP,
						props: vec![PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
							key: PropName::Ident(self.spread.clone()),
							value: expr.take(),
						})))],
					})));
				}
			}
		}
		if !props.is_empty() {
			let props = props
				.into_iter()
				.map(|(key, value)| {
					PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
						key: PropName::Str(Str {
							span: DUMMY_SP,
							value: key,
							raw: None,
						}),
						value,
					})))
				})
				.collect();
			self.push(Box::new(Expr::Object(ObjectLit { span: DUMMY_SP, props })));
		}

		static VOID_ELEMENTS: phf::Set<&str> = phf::phf_set!(
			"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track",
			"wbr"
		);

		if elt.children.is_empty() {
			if VOID_ELEMENTS.contains(&name) {
				self.quasi_last_mut().push('>');
			} else {
				self.quasi_last_mut().push_str("/>");
			}
			return;
		}
		{
			let last = self.quasi_last_mut();
			last.pop();
			last.push('>');
		}
		for child in &mut elt.children {
			self.fold_jsx_child(child);
		}
		_ = write!(self.quasi_last_mut(), "</{name}>")
	}

	fn fold_jsx_child(&mut self, child: &mut JSXElementChild) {
		match child {
			JSXElementChild::JSXText(JSXText { value, .. }) => {
				_ = self.quasi_last_mut().write_str(value.trim());
			}
			JSXElementChild::JSXElement(elt) => {
				self.fold_jsx_element(elt);
			}
			JSXElementChild::JSXFragment(JSXFragment { children, .. }) => {
				for child in children {
					self.fold_jsx_child(child);
				}
			}
			JSXElementChild::JSXExprContainer(JSXExprContainer {
				expr: JSXExpr::Expr(expr),
				..
			}) => match expr.as_mut() {
				Expr::JSXElement(elt) => self.fold_jsx_element(elt),
				Expr::JSXFragment(frag) => {
					for child in &mut frag.children {
						self.fold_jsx_child(child)
					}
				}
				Expr::Lit(Lit::Str(str)) => {
					_ = write!(self.quasi_last_mut(), "{} ", str.value.trim());
				}
				Expr::Lit(Lit::Num(value)) => {
					_ = write!(self.quasi_last_mut(), "{} ", value.value);
				}
				Expr::Tpl(Tpl { exprs, quasis, span }) if exprs.is_empty() => {
					let [TplElement { cooked, raw, .. }] = &quasis[..] else {
						unreachable(*span)
					};
					let value = cooked.as_ref().unwrap_or(&raw);
					let last = self.quasi_last_mut();
					for line in value.lines().map(str::trim).filter(|line| !line.is_empty()) {
						_ = writeln!(last, "{line}");
					}
				}
				other => {
					eprintln!("{other:?}");
					self.push(Box::new(Expr::Object(ObjectLit {
						span: DUMMY_SP,
						props: vec![PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
							key: PropName::Ident(self.child.clone()),
							value: expr.take(),
						})))],
					})));
				}
			},
			JSXElementChild::JSXSpreadChild(JSXSpreadChild { expr, .. }) => {
				self.push(Box::new(Expr::Object(ObjectLit {
					span: DUMMY_SP,
					props: vec![PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
						key: PropName::Ident(self.children.clone()),
						value: expr.take(),
					})))],
				})));
			}
			_ => {}
		}
	}

	fn replace_jsx_element(&mut self, elt: Option<&mut JSXElement>) -> Expr {
		if let Some(elt) = elt {
			self.fold_jsx_element(elt);
		}
		let mut quasis = mem::take(&mut self.quasis)
			.into_iter()
			.map(|raw| {
				let raw = Atom::new(raw);
				TplElement {
					span: DUMMY_SP,
					tail: false,
					cooked: None,
					raw,
				}
			})
			.collect::<Vec<_>>();
		quasis.last_mut().unwrap().tail = true;
		match self.template.clone() {
			Some(tag) => Expr::TaggedTpl(TaggedTpl {
				span: DUMMY_SP,
				tag,
				type_params: None,
				tpl: Box::new(Tpl {
					span: DUMMY_SP,
					exprs: mem::take(&mut self.exprs),
					quasis,
				}),
			}),
			None => Expr::Tpl(Tpl {
				span: DUMMY_SP,
				exprs: mem::take(&mut self.exprs),
				quasis,
			}),
		}
	}

	fn swap_state<T>(&mut self, blk: impl FnOnce(&mut Self) -> T) -> T {
		let quasis = mem::take(&mut self.quasis);
		let exprs = mem::take(&mut self.exprs);
		let ret = blk(self);
		let leftover_quasis = mem::replace(&mut self.quasis, quasis);
		assert_eq!(leftover_quasis.as_slice(), &[] as &[String]);
		let leftover_exprs = mem::replace(&mut self.exprs, exprs);
		assert_eq!(leftover_exprs.as_slice(), &[] as &[_]);
		ret
	}
}

#[swc_trace]
impl VisitMut for TransformVisitor {
	noop_visit_mut_type!();
	fn visit_mut_expr(&mut self, n: &mut Expr) {
		if let Some(mut elt) = expr_as_jsx_elt(n) {
			self.swap_state(|me| *n = me.replace_jsx_element(Some(&mut elt)));
			return;
		}
		if let Some(mut frag) = expr_as_jsx_fragment(n) {
			self.swap_state(|me| {
				me.quasis.push(String::new());
				for child in frag.children.iter_mut() {
					me.fold_jsx_child(child);
				}
				*n = me.replace_jsx_element(None);
			});
			return;
		}
		n.visit_mut_children_with(self)
	}
	fn visit_mut_module(&mut self, n: &mut Module) {
		if let Some(src) = self.import_source.clone() {
			let Some(Expr::Ident(ident)) = self.template.as_mut().map(|tpl| tpl.unwrap_parens_mut()) else {
				HANDLER.with(|handler| {
					handler
						.struct_err("[swc-plugin-static-jsx] incompatible template function")
						.note("expected `template` to be an identifier because `importSource` was specified")
						.emit()
				});
				return;
			};
			let import_ident = ident.clone();
			*ident = ident.prefix("_");
			ident.span = ident.span.apply_mark(Mark::new());
			let import = ImportSpecifier::Named(ImportNamedSpecifier {
				span: DUMMY_SP,
				local: ident.clone(),
				imported: Some(ModuleExportName::Ident(import_ident)),
				is_type_only: false,
			});
			n.body.insert(
				0,
				ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
					span: DUMMY_SP,
					specifiers: vec![import],
					src: Box::new(src),
					type_only: false,
					asserts: None,
				})),
			);
		} else if let Some(Expr::Ident(ident)) = self.template.as_mut().map(|tpl| tpl.unwrap_parens_mut()) {
			ident.span = ident.span.apply_mark(Mark::from_u32(2));
		}
		n.visit_mut_children_with(self);
	}
}

pub struct ExtractStaticProps<'a> {
	pub buffer: &'a mut String,
}

#[swc_trace]
impl VisitMut for ExtractStaticProps<'_> {
	noop_visit_mut_type!();
	fn visit_mut_prop_or_spreads(&mut self, n: &mut Vec<PropOrSpread>) {
		n.visit_mut_children_with(self);
		n.retain(|elt| match elt {
			PropOrSpread::Spread(SpreadElement { expr, .. }) => match expr.as_ref() {
				Expr::Object(ObjectLit { props, .. }) => !props.is_empty(),
				_ => true,
			},
			PropOrSpread::Prop(prop) => match prop.as_ref() {
				Prop::KeyValue(KeyValueProp { value, .. }) => !value.is_invalid(),
				_ => true,
			},
		})
	}
	fn visit_mut_key_value_prop(&mut self, n: &mut KeyValueProp) {
		let name = match &n.key {
			PropName::Ident(ident) => ident.sym.as_ref(),
			PropName::Str(str) => str.value.as_ref(),
			_ => return,
		};
		match n.value.as_mut() {
			lit @ Expr::Lit(Lit::Str(..) | Lit::Bool(..) | Lit::Num(..) | Lit::BigInt(..)) => match lit.take() {
				Expr::Lit(Lit::Str(str)) => {
					_ = write!(self.buffer, "{name}=\"{}\" ", str.value);
				}
				Expr::Lit(Lit::Bool(bool)) => {
					if bool.value {
						_ = write!(self.buffer, "{name} ");
					}
				}
				Expr::Lit(Lit::Num(value)) => {
					_ = write!(self.buffer, "{name}=\"{}\" ", value.value);
				}
				Expr::Lit(Lit::BigInt(value)) => {
					_ = write!(self.buffer, "{name}=\"{}\" ", value.value.to_str_radix(10));
				}
				_ => unreachable(n.span()),
			},
			Expr::Tpl(tpl) if tpl.exprs.is_empty() => {
				let [TplElement { cooked, raw, .. }] = &tpl.quasis[..] else {
					unreachable(tpl.span)
				};
				let text = cooked.as_deref().unwrap_or(raw.trim());
				_ = self.buffer.write_str(text);
			}
			_ => n.visit_mut_children_with(self),
		}
	}
}

pub use utils::*;
mod utils {
	use std::borrow::Cow;
	use swc_core::common::util::take::Take;
	use swc_core::common::{Span, Spanned};
	use swc_core::ecma::ast::*;
	use swc_core::plugin::errors::HANDLER;

	#[cold]
	#[inline(never)]
	pub fn unreachable(span: Span) -> ! {
		HANDLER.with(|handler| handler.span_bug(span, "Encountered unreachable code"))
	}

	pub fn take_lit(lit: &mut Lit) -> Lit {
		match lit {
			Lit::Str(str) => Lit::Str(str.take()),
			Lit::Bool(bool) => Lit::Bool(bool.take()),
			Lit::Num(num) => Lit::Num(Number {
				span: num.span,
				value: num.value,
				raw: num.raw.clone(),
			}),
			Lit::BigInt(bigint) => Lit::BigInt(BigInt {
				span: bigint.span,
				value: bigint.value.clone(),
				raw: bigint.raw.clone(),
			}),
			Lit::JSXText(text) => Lit::JSXText(JSXText {
				span: text.span,
				value: text.value.clone(),
				raw: text.raw.clone(),
			}),
			Lit::Null(null) => Lit::Null(null.take()),
			Lit::Regex(regex) => Lit::Regex(Regex {
				span: regex.span,
				exp: regex.exp.clone(),
				flags: regex.flags.clone(),
			}),
		}
	}

	pub fn expr_as_jsx_elt(n: &mut Expr) -> Option<Box<JSXElement>> {
		let span_n = n.span();
		match n {
			Expr::JSXElement(elt) => match &elt.opening.name {
				JSXElementName::Ident(ident) => ident.sym.starts_with(|p: char| p.is_ascii_lowercase()).then(|| {
					let Expr::JSXElement(elt) = n.take() else {
						unreachable(span_n)
					};
					elt
				}),
				JSXElementName::JSXNamespacedName(..) => {
					let Expr::JSXElement(elt) = n.take() else {
						unreachable(span_n)
					};
					Some(elt)
				}
				_ => None,
			},
			_ => None,
		}
	}

	pub fn expr_as_jsx_fragment(n: &mut Expr) -> Option<JSXFragment> {
		let span_n = n.span();
		if let Expr::JSXFragment(..) = n {
			let Expr::JSXFragment(frag) = n.take() else {
				unreachable(span_n)
			};
			return Some(frag);
		}
		None
	}

	pub fn jsx_attr_name_as_str(attr: &JSXAttrName) -> Cow<str> {
		match attr {
			JSXAttrName::Ident(ident) => Cow::Borrowed(&ident.sym),
			JSXAttrName::JSXNamespacedName(name) => Cow::Owned(format!("{}:{}", name.ns.sym, name.name.sym)),
		}
	}
}

mod de {
	use serde::{de::Visitor, Deserializer};
	use swc_core::common::{BytePos, DUMMY_SP};
	use swc_core::ecma::ast::{Expr, Ident, Str};
	use swc_core::ecma::parser::{Parser, StringInput, Syntax};
	use swc_core::plugin::errors::HANDLER;

	pub fn expr<'de, D>(de: D) -> Result<Option<Box<Expr>>, D::Error>
	where
		D: Deserializer<'de>,
	{
		struct ExprVisitor;
		impl<'de> Visitor<'de> for ExprVisitor {
			type Value = Option<Box<Expr>>;
			fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
				formatter.write_str("an expression")
			}
			fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Parser::new(
					Syntax::Typescript(Default::default()),
					StringInput::new(v, BytePos::DUMMY, BytePos::DUMMY),
					None,
				)
				.parse_expr()
				.map_err(|err| {
					let kind = err.kind().msg();
					HANDLER.with(|handler| err.into_diagnostic(handler).emit());
					E::custom(kind)
				})
				.map(Some)
			}
			fn visit_some<D>(self, de: D) -> Result<Self::Value, D::Error>
			where
				D: Deserializer<'de>,
			{
				de.deserialize_str(Self)
			}
			fn visit_none<E>(self) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Ok(None)
			}
		}
		de.deserialize_option(ExprVisitor)
	}

	pub fn ident<'de, D>(de: D) -> Result<Ident, D::Error>
	where
		D: Deserializer<'de>,
	{
		struct IdentVisitor;
		impl<'de> Visitor<'de> for IdentVisitor {
			type Value = Ident;
			fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
				formatter.write_str("an identifier")
			}
			#[inline]
			fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Ident::new(v.into(), DUMMY_SP))
			}
		}
		de.deserialize_str(IdentVisitor)
	}

	pub fn str<'de, D>(de: D) -> Result<Option<Str>, D::Error>
	where
		D: Deserializer<'de>,
	{
		struct OptStrVisitor;
		impl<'de> Visitor<'de> for OptStrVisitor {
			type Value = Option<Str>;

			fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
				formatter.write_str("a string")
			}

			fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Some(Str {
					span: DUMMY_SP,
					value: v.into(),
					raw: None,
				}))
			}
		}
		de.deserialize_str(OptStrVisitor)
	}
}
