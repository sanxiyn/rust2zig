use std::collections::{HashMap, HashSet};

use crate::ast::zig::{Field, Node, Param, Var};
use crate::scip::{Kind, Range, Scip};
use super::Translator;

impl Translator {
    fn collect_captures(&self, ec: &syn::ExprClosure) -> Vec<(syn::Ident, syn::Type)> {
        use syn::spanned::Spanned;
        use syn::visit::Visit;

        struct Visitor<'a> {
            scip: &'a Scip,
            span: Range,
            seen: HashSet<String>,
            captures: Vec<(syn::Ident, syn::Type)>,
        }

        impl<'ast> Visit<'ast> for Visitor<'_> {
            fn visit_ident(&mut self, ident: &'ast syn::Ident) {
                let range: Range = ident.span().into();
                let Some(symbol) = self.scip.symbol_at(&range) else { return };
                let Some(info) = self.scip.symbol_info(symbol) else { return };
                if !matches!(info.kind, Kind::Parameter | Kind::Variable) {
                    return;
                }
                let Some(def) = info.range.as_ref() else { return };
                if self.span.contains(def) {
                    return;
                }
                if self.seen.insert(symbol.to_string()) {
                    if let Some(ty) = self.scip.type_at(&range) {
                        self.captures.push((ident.clone(), ty));
                    }
                }
            }
        }

        let span: Range = ec.span().into();
        let mut visitor = Visitor {
            scip: &self.scip,
            span,
            seen: Default::default(),
            captures: Default::default(),
        };
        visitor.visit_expr(&ec.body);
        visitor.captures
    }

    fn closure_return_type(&self, ident: &syn::Ident) -> Option<syn::Type> {
        let ty = self.scip.type_at(&ident.span().into())?;
        let syn::Type::ImplTrait(it) = ty else { return None };
        for bound in it.bounds {
            let syn::TypeParamBound::Trait(tb) = bound else { continue };
            let Some(last) = tb.path.segments.last() else { continue };
            let syn::PathArguments::Parenthesized(p) = &last.arguments else { continue };
            if let syn::ReturnType::Type(_, t) = &p.output {
                return Some((**t).clone());
            }
        }
        None
    }

    pub fn is_closure_type(&self, ty: &syn::Type) -> bool {
        let syn::Type::ImplTrait(it) = ty else { return false };
        it.bounds.iter().any(|bound| {
            let syn::TypeParamBound::Trait(tb) = bound else { return false };
            let Some(last) = tb.path.segments.last() else { return false };
            matches!(last.ident.to_string().as_str(), "Fn" | "FnMut" | "FnOnce")
        })
    }

    pub fn translate_closure_local(&self, pi: &syn::PatIdent, ec: &syn::ExprClosure) -> Node {
        let name = self.rename_ident(&pi.ident);
        let captures = self.collect_captures(ec);
        let capture_fields: Vec<Field> = captures.iter().map(|(ident, ty)| Field {
            name: self.rename_ident(ident),
            ty: self.translate_type(ty),
        }).collect();
        let has_self = !captures.is_empty();
        let params = ec.inputs.iter().filter_map(|input| {
            let syn::Pat::Ident(pi) = input else { return None };
            let ty = match self.scip.type_at(&pi.ident.span().into()) {
                Some(ty) => self.translate_type(&ty),
                None => Node::Todo("type".to_string()),
            };
            Some(Param { comptime: false, name: self.rename_ident(&pi.ident), ty })
        }).collect();
        let return_type = match self.closure_return_type(&pi.ident) {
            Some(ty) => self.translate_type(&ty),
            None => Node::Identifier("void".to_string()),
        };
        let mut map: HashMap<String, String> = Default::default();
        for ((ident, _), field) in captures.iter().zip(capture_fields.iter()) {
            if let Some(symbol) = self.scip.symbol_at(&ident.span().into()) {
                map.insert(symbol.to_string(), field.name.clone());
            }
        }
        self.capture_stack.borrow_mut().push(map);
        let body = if let syn::Expr::Block(eb) = &*ec.body {
            self.translate_block(&eb.block)
        } else {
            let expr = self.translate_expr(&ec.body);
            let stmt = Node::Return(Some(Box::new(expr)));
            Node::Block(vec![stmt])
        };
        self.capture_stack.borrow_mut().pop();
        Node::SimpleVarDecl {
            var: Var { is_const: true, name, ty: None },
            expr: Some(Box::new(Node::Closure {
                captures: capture_fields,
                has_self,
                params,
                return_type: Box::new(return_type),
                body: Box::new(body),
            })),
        }
    }
}
