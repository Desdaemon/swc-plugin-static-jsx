use std::borrow::Cow;
use std::fmt::Write;

use swc_core::common::util::take::Take;
use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::*;
use swc_core::ecma::atoms::{Atom, JsWord};

use swc_core::ecma::visit::{as_folder, FoldWith, VisitMut};
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};
// use swc_ecma_parser::{Parser, StringInput};
use swc_ecma_parser::{Syntax, TsConfig};

pub struct TransformVisitor {
    pub templator: Box<Expr>,
    pub process_attr: Box<Expr>,
    pub process_spread: Box<Expr>,
    pub quasis: Vec<String>,
    pub exprs: Vec<Box<Expr>>,
}

#[derive(Default)]
pub struct StaticPropVisitor {
    pub results: Vec<()>,
}

const TSX: Syntax = Syntax::Typescript(TsConfig {
    tsx: true,
    decorators: true,
    dts: false,
    no_early_errors: false,
    disallow_ambiguous_jsx_like: true,
});

impl Default for TransformVisitor {
    fn default() -> Self {
        Self {
            templator: Box::new(Expr::Member(MemberExpr {
                span: DUMMY_SP,
                obj: Box::new(Expr::Ident(Ident::new("String".into(), DUMMY_SP))),
                prop: MemberProp::Ident(Ident::new("raw".into(), DUMMY_SP)),
            })),
            process_attr: Box::new(Expr::Ident(Ident::new("$$attr".into(), DUMMY_SP))),
            process_spread: Box::new(Expr::Ident(Ident::new("$$spread".into(), DUMMY_SP))),
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
            JSXElementName::JSXMemberExpr(..) => todo!(),
            JSXElementName::Ident(ident) => {
                if ident.sym.chars().next().unwrap().is_ascii_lowercase() {
                    ident.sym.to_string()
                } else {
                    todo!()
                }
            }
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
                        self.quasis.push(" ".to_string());
                        let expr = match value {
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
                        self.exprs.push(Box::new(Expr::Call(CallExpr {
                            span: DUMMY_SP,
                            callee: Callee::Expr(self.process_attr.clone()),
                            type_args: None,
                            args: vec![
                                ExprOrSpread {
                                    spread: None,
                                    expr: Box::new(Expr::Lit(Lit::Str(Str {
                                        span: DUMMY_SP,
                                        value: JsWord::from(name),
                                        raw: None,
                                    }))),
                                },
                                ExprOrSpread { spread: None, expr },
                            ],
                        })))
                    } else {
                        _ = write!(self.quasi_last_mut(), "{name} ");
                    }
                }
                JSXAttrOrSpread::SpreadElement(SpreadElement { expr, .. }) => {
                    // let obj = match expr.as_ref() {
                    //     Expr::Object(..) => {
                    //         // Bit weird here, but necessary to avoid one unneeded allocation.
                    //         let Expr::Object(lit) = *expr else {unreachable!()};
                    //         lit
                    //     }
                    //     _ => {
                    self.quasis.push(" ".to_string());
                    self.exprs.push(Box::new(Expr::Call(CallExpr {
                        span: DUMMY_SP,
                        callee: Callee::Expr(self.process_spread.clone()),
                        type_args: None,
                        args: vec![ExprOrSpread { spread: None, expr }],
                    })));
                    continue;
                    //     }
                    // };
                }
            }
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
                expr: JSXExpr::Expr(expr),
                ..
            }) => match expr.as_ref() {
                Expr::JSXElement(..) => {
                    let Expr::JSXElement(elt) = *expr else {unreachable!()};
                    self.fold_jsx_element(*elt);
                }
                Expr::JSXFragment(..) => {
                    let Expr::JSXFragment(frag) = *expr else {unreachable!()};
                    for child in frag.children {
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
}

impl VisitMut for TransformVisitor {
    fn visit_mut_expr(&mut self, n: &mut Expr) {
        if matches!(n, Expr::JSXElement(..)) {
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

#[plugin_transform]
pub fn process_transform(program: Program, _meta: TransformPluginProgramMetadata) -> Program {
    program.fold_with(&mut as_folder(TransformVisitor::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_core::ecma::transforms::testing::test;

    test!(
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_all_static,
        r#"<div foo="1" foo="2" />"#,
        r#"String.raw`<div foo="1" foo="2" />`"#
    );

    test!(
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_mixed,
        r#"<div foo="1" foo={2} />"#,
        r#"String.raw`<div foo="1" ${$$attr("foo", 2)} />`"#
    );

    test!(
        ignore,
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_fold_static_long,
        r#"<div foo={"foo"} />"#,
        r#"String.raw`<div foo="foo" />`"#
    );

    test!(
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_all_dynamic,
        r#"<div foo={2} />"#,
        r#"String.raw`<div ${$$attr("foo", 2)} />`"#
    );

    test!(
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_static_after,
        r#"<div foo={2} foo="2" />"#,
        r#"String.raw`<div ${$$attr("foo", 2)} foo="2" />`"#
    );

    test!(
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_empty,
        r#"<div />"#,
        r#"String.raw`<div />`"#
    );

    test!(
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_elt_ns,
        r#"<foo:bar />"#,
        r#"String.raw`<foo:bar />`"#
    );

    test!(
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_attr_ns,
        r#"<foo foo:bar="123" foo:baz={true} />"#,
        r#"String.raw`<foo foo:bar="123" ${$$attr("foo:baz", true)} />`"#
    );

    test!(
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_spread,
        r#"<div {...foo} />"#,
        r#"String.raw`<div ${$$spread(foo)} />`"#
    );

    test!(
        ignore,
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_spread_empty,
        r#"<div {...{}} />"#,
        r#"String.raw`<div />`"#
    );

    test!(
        ignore,
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_spread_static,
        r#"<div {...{foo: 'foo', 'bar:bar': 'bar', cool: true, baz}} />"#,
        r#"String.raw`<div foo="foo" bar:bar="bar" cool ${$$spread({baz})} />`"#
    );

    test!(
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_child_text,
        r#"<div>Hello there!</div>"#,
        r#"String.raw`<div>Hello there!</div>`"#
    );

    test!(
        TSX,
        |_| as_folder(TransformVisitor::default()),
        test_child_elt,
        r#"
        <div>
            Hello there!
            <span data-foo data-bar={123}> Another good day! </span>
        </div>"#,
        concat!(
            "String.raw`<div>Hello there!",
            r#"<span data-foo ${$$attr("data-bar", 123)}>Another good day!</span>"#,
            "</div>`"
        )
    );
}
