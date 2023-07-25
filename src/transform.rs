use std::borrow::Cow;
use std::fmt::Write;

use swc_core::common::util::take::Take;
use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::*;
use swc_core::ecma::atoms::{Atom, JsWord};

use swc_core::ecma::atoms::js_word;
// use swc_core::ecma::parser::{Syntax, TsConfig};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

pub struct TransformVisitor {
    pub templator: Box<Expr>,
    pub quasis: Vec<String>,
    pub exprs: Vec<Box<Expr>>,
    pub spread: Ident,
    pub child: Ident,
    pub children: Ident,
}

#[derive(Default)]
pub struct StaticPropVisitor {
    pub results: Vec<()>,
}

// const TSX: Syntax = Syntax::Typescript(TsConfig {
//     tsx: true,
//     decorators: true,
//     dts: false,
//     no_early_errors: false,
//     disallow_ambiguous_jsx_like: true,
// });

impl Default for TransformVisitor {
    fn default() -> Self {
        Self {
            templator: Box::new(Expr::Member(MemberExpr {
                span: DUMMY_SP,
                obj: Box::new(Expr::Ident(Ident::new(js_word!("String"), DUMMY_SP))),
                prop: MemberProp::Ident(Ident::new("raw".into(), DUMMY_SP)),
            })),
            child: Ident::new("$$child".into(), DUMMY_SP),
            children: Ident::new("$$children".into(), DUMMY_SP),
            spread: Ident::new("$$spread".into(), DUMMY_SP),
            quasis: vec![],
            exprs: vec![],
        }
    }
}

impl TransformVisitor {
    #[inline]
    fn quasi_last_mut(&mut self) -> &mut String {
        self.quasis.last_mut().unwrap()
    }

    fn fold_jsx_element(&mut self, elt: JSXElement) {
        let JSXOpeningElement { name, attrs, .. } = elt.opening;
        let name = match name {
            JSXElementName::JSXNamespacedName(name) => format!("{}:{}", name.ns.sym, name.name.sym),
            JSXElementName::JSXMemberExpr(..) => unreachable!(),
            JSXElementName::Ident(ident) => ident.sym.to_string(),
        };
        fn extract_static_attr_pair(
            attr: &JSXAttrOrSpread,
        ) -> Option<(&JSXAttrName, Option<&str>)> {
            match attr {
                JSXAttrOrSpread::JSXAttr(JSXAttr { name, value, .. }) => match value {
                    Some(JSXAttrValue::Lit(Lit::Str(Str { value, .. }))) => {
                        Some((name, Some(value.as_ref())))
                    }
                    Some(JSXAttrValue::Lit(other_lit)) => {
                        unreachable!("JSX attribute value {other_lit:?}")
                    }
                    Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
                        expr: JSXExpr::Expr(expr),
                        ..
                    })) => Some((
                        name,
                        Some(TransformVisitor::try_extract_tpl_from_expr(expr.as_ref())?),
                    )),
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
                let name = Self::jsx_attr_name_as_str(name);
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
        for attr in attrs.into_iter().skip(leading_static_attrs_count) {
            if let Some((name, value)) = extract_static_attr_pair(&attr) {
                let name = Self::jsx_attr_name_as_str(name);
                let last = self.quasi_last_mut();
                if let Some(value) = value.as_deref() {
                    _ = write!(last, "{name}=\"{value}\" ");
                } else {
                    _ = write!(last, "{name} ");
                }
                continue;
            }
            match attr {
                JSXAttrOrSpread::JSXAttr(JSXAttr { name, value, .. }) => {
                    let name = Self::jsx_attr_name_as_str(&name);
                    if let Some(value) = value {
                        let value = match value {
                            JSXAttrValue::Lit(lit) => Box::new(Expr::Lit(lit)),
                            JSXAttrValue::JSXExprContainer(JSXExprContainer { expr, .. }) => {
                                match expr {
                                    JSXExpr::JSXEmptyExpr(empty) => Box::new(Expr::JSXEmpty(empty)),
                                    JSXExpr::Expr(expr) => expr,
                                }
                            }
                            JSXAttrValue::JSXElement(elt) => Box::new(Expr::JSXElement(elt)),
                            JSXAttrValue::JSXFragment(frag) => Box::new(Expr::JSXFragment(frag)),
                        };
                        props.push((JsWord::from(name), value));
                    } else {
                        _ = write!(self.quasi_last_mut(), "{name} ");
                    }
                }
                JSXAttrOrSpread::SpreadElement(SpreadElement { mut expr, .. }) => {
                    if let Expr::Object(ObjectLit {
                        props: obj_props, ..
                    }) = expr.as_mut()
                    {
                        let mut extractor = ExtractStaticProps {
                            buffer: self.quasi_last_mut(),
                        };
                        obj_props.visit_mut_with(&mut extractor);
                        if !obj_props.is_empty() {
                            self.quasis.push(" ".to_string());
                            self.exprs.push(Box::new(Expr::Object(ObjectLit {
                                span: DUMMY_SP,
                                props: vec![PropOrSpread::Prop(Box::new(Prop::KeyValue(
                                    KeyValueProp {
                                        key: PropName::Ident(self.spread.clone()),
                                        value: expr,
                                    },
                                )))],
                            })));
                        }
                        continue;
                    }
                    self.quasis.push(" ".to_string());
                    self.exprs.push(Box::new(Expr::Object(ObjectLit {
                        span: DUMMY_SP,
                        props: vec![PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                            key: PropName::Ident(self.spread.clone()),
                            value: expr,
                        })))],
                    })))
                }
            }
        }
        if !props.is_empty() {
            // TODO: Explore more opportunities for collapsing statics
            self.quasis.push(" ".to_string());
            self.exprs.push(Box::new(Expr::Object(ObjectLit {
                span: DUMMY_SP,
                props: props
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
                    .collect(),
            })));
        }
        if elt.children.is_empty() {
            self.quasi_last_mut().push_str("/>");
            return;
        }
        {
            let last = self.quasi_last_mut();
            last.pop();
            last.push('>');
        }
        for child in elt.children {
            self.fold_jsx_child(child);
        }
        _ = write!(self.quasi_last_mut(), "</{name}>")
    }

    #[inline]
    fn fold_jsx_child(&mut self, child: JSXElementChild) {
        match child {
            JSXElementChild::JSXText(JSXText { value, .. }) => {
                _ = self.quasi_last_mut().write_str(value.trim());
            }
            JSXElementChild::JSXElement(elt) => {
                self.fold_jsx_element(*elt);
            }
            JSXElementChild::JSXFragment(JSXFragment { children, .. }) => {
                for child in children {
                    self.fold_jsx_child(child);
                }
            }
            JSXElementChild::JSXExprContainer(JSXExprContainer {
                expr: JSXExpr::Expr(mut expr),
                ..
            }) => match expr.as_mut() {
                Expr::JSXElement(elt) => {
                    self.fold_jsx_element(*elt.take());
                }
                Expr::JSXFragment(frag) => {
                    for child in frag.take().children {
                        self.fold_jsx_child(child)
                    }
                }
                _ => todo!(),
            },
            JSXElementChild::JSXSpreadChild(..) => todo!(),
            _ => {}
        }
    }

    fn replace_jsx_element(&mut self, elt: JSXElement) -> Expr {
        self.fold_jsx_element(elt);
        let mut quasis = core::mem::take(&mut self.quasis)
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
        Expr::TaggedTpl(TaggedTpl {
            span: DUMMY_SP,
            tag: self.templator.clone(),
            type_params: None,
            tpl: Box::new(Tpl {
                span: DUMMY_SP,
                exprs: core::mem::take(&mut self.exprs),
                quasis,
            }),
        })
    }
    #[inline]
    fn jsx_attr_name_as_str(attr: &JSXAttrName) -> Cow<str> {
        match attr {
            JSXAttrName::Ident(ident) => Cow::Borrowed(&ident.sym),
            JSXAttrName::JSXNamespacedName(name) => {
                Cow::Owned(format!("{}:{}", name.ns.sym, name.name.sym))
            }
        }
    }
    #[inline]
    fn try_extract_tpl_from_expr(expr: &Expr) -> Option<&Atom> {
        match expr {
            Expr::Tpl(Tpl { quasis, .. }) => match &quasis[..] {
                [TplElement { cooked, raw, .. }] => Some(cooked.as_ref().unwrap_or(raw)),
                _ => None,
            },
            _ => None,
        }
    }

    fn needs_replace(&self, n: &Expr) -> bool {
        if let Expr::JSXElement(elt) = n {
            return match &elt.opening.name {
                JSXElementName::Ident(ident) => {
                    ident.sym.starts_with(|p: char| p.is_ascii_lowercase())
                }
                JSXElementName::JSXNamespacedName(..) => true,
                JSXElementName::JSXMemberExpr(..) => false,
            };
        }
        false
    }
}

impl VisitMut for TransformVisitor {
    fn visit_mut_expr(&mut self, n: &mut Expr) {
        if self.needs_replace(n) {
            let Expr::JSXElement(elt) = n.take() else {unreachable!()};
            let quasis = core::mem::take(&mut self.quasis);
            let exprs = core::mem::take(&mut self.exprs);
            *n = self.replace_jsx_element(*elt);
            let leftover_quasis = core::mem::replace(&mut self.quasis, quasis);
            assert_eq!(leftover_quasis.as_slice(), &[] as &[String]);
            let leftover_exprs = core::mem::replace(&mut self.exprs, exprs);
            assert_eq!(leftover_exprs.as_slice(), &[] as &[_]);
        }
    }
}

pub struct ExtractStaticProps<'a> {
    pub buffer: &'a mut String,
}

impl VisitMut for ExtractStaticProps<'_> {
    fn visit_mut_prop_or_spreads(&mut self, n: &mut Vec<PropOrSpread>) {
        VisitMutWith::visit_mut_children_with(n, self);
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
            lit @ Expr::Lit(Lit::Str(..) | Lit::Bool(..) | Lit::Num(..)) => match lit.take() {
                Expr::Lit(Lit::Str(str)) => {
                    _ = write!(self.buffer, "{name}=\"{}\" ", str.value);
                }
                Expr::Lit(Lit::Bool(bool)) => {
                    if bool.value {
                        _ = write!(self.buffer, "{name} ");
                    }
                }
                Expr::Lit(Lit::Num(value)) => {
                    _ = write!(self.buffer, "{name}=\"{}\"", value.value);
                }
                _ => todo!(),
            },
            _ => return,
        }
    }
}
