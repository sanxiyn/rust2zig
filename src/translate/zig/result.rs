use syn::visit::Visit;

use crate::ast::zig::{Node, Var};
use super::Translator;

pub enum ResultPat {
    Ok(String),
    Err(String),
}

impl Translator {
    pub fn collect_error_types(&mut self, file: &syn::File) {
        let mut collector = Collector { translator: self, symbols: Default::default() };
        collector.visit_file(file);
        let symbols = collector.symbols;
        for symbol in symbols {
            if self.is_payload_free(&symbol) {
                self.error_types.insert(symbol);
            }
        }
    }

    fn is_payload_free(&self, symbol: &str) -> bool {
        if let Some(e) = self.enums.get(symbol) {
            return !e.has_data && e.impls.is_empty();
        }
        if let Some(s) = self.structs.get(symbol) {
            return !s.has_fields && s.impls.is_empty();
        }
        false
    }

    pub fn is_error_set(&self, ty: &syn::Type) -> bool {
        self.type_symbol(ty).is_some_and(|symbol| self.error_types.contains(&symbol))
    }

    pub fn result_types(&self, ty: &syn::Type) -> Option<(syn::Type, syn::Type)> {
        let syn::Type::Path(tp) = ty else { return None };
        if !self.check_moniker(&tp.path, "core::result::Result") {
            return None;
        }
        let syn::PathArguments::AngleBracketed(ab) = &tp.path.segments.last()?.arguments else {
            return None;
        };
        let mut args = ab.args.iter().filter_map(|arg| match arg {
            syn::GenericArgument::Type(ty) => Some(ty.clone()),
            _ => None,
        });
        Some((args.next()?, args.next()?))
    }

    pub fn type_symbol(&self, ty: &syn::Type) -> Option<String> {
        let syn::Type::Path(tp) = ty else { return None };
        let ident = &tp.path.segments.last()?.ident;
        Some(self.scip.symbol_at(&ident.span().into())?.to_string())
    }

    pub fn push_error_scope(&self, output: &syn::ReturnType) {
        let symbol = match output {
            syn::ReturnType::Type(_, ty) => {
                self.result_types(ty).and_then(|(_, error)| self.type_symbol(&error))
            }
            syn::ReturnType::Default => None,
        };
        *self.error_scope.borrow_mut() = symbol;
    }

    pub fn pop_error_scope(&self) {
        self.error_scope.borrow_mut().take();
    }

    pub fn translate_try(&self, et: &syn::ExprTry) -> Node {
        let range = et.question_token.spans[0].into();
        let expr = self.translate_expr(&et.expr);
        if self.check_moniker_at(&range,"core::option::branch") {
            let null = Node::Identifier("null".to_string());
            return Node::Orelse(Box::new(expr), Box::new(Node::Return(Some(Box::new(null)))));
        }
        if !self.check_moniker_at(&range, "core::result::branch") {
            return Node::Todo("try".to_string());
        }
        if !self.propagates_error(&et.expr) {
            return Node::Todo("try".to_string());
        }
        Node::Try(Box::new(expr))
    }

    fn propagates_error(&self, expr: &syn::Expr) -> bool {
        let Some(scope) = self.error_scope.borrow().clone() else { return false };
        let Some(ty) = self.expr_type(expr) else { return true };
        let Some((_, error)) = self.result_types(&ty) else { return true };
        self.type_symbol(&error).is_some_and(|symbol| symbol == scope)
    }

    pub fn result_pat(&self, pat: &syn::Pat) -> Option<ResultPat> {
        let syn::Pat::TupleStruct(pts) = pat else { return None };
        if pts.elems.len() != 1 {
            return None;
        }
        let name = self.pat_name(&pts.elems[0]);
        if self.check_moniker(&pts.path, "core::result::Result::Ok") {
            Some(ResultPat::Ok(name))
        } else if self.check_moniker(&pts.path, "core::result::Result::Err") {
            Some(ResultPat::Err(name))
        } else {
            None
        }
    }

    pub fn translate_match_result(&self, em: &syn::ExprMatch) -> Node {
        let mut ok = None;
        let mut err = None;
        for arm in &em.arms {
            if arm.guard.is_some() {
                return Node::Todo("match".to_string());
            }
            match self.result_pat(&arm.pat) {
                Some(ResultPat::Ok(name)) => ok = Some((name, &arm.body)),
                Some(ResultPat::Err(name)) => err = Some((name, &arm.body)),
                None => return Node::Todo("match".to_string()),
            }
        }
        let (Some((ok_name, ok_body)), Some((err_name, err_body))) = (ok, err) else {
            return Node::Todo("match".to_string());
        };
        Node::If {
            cond: Box::new(self.translate_expr(&em.expr)),
            capture: Some(ok_name),
            then_branch: Box::new(self.translate_expr(ok_body)),
            else_capture: Some(err_name),
            else_branch: Some(Box::new(self.translate_expr(err_body))),
        }
    }

    pub fn translate_err(&self, expr: &syn::Expr) -> Node {
        let syn::Expr::Path(ep) = expr else { return Node::Todo("expr".to_string()) };
        let name = ep.path.segments.last().unwrap().ident.to_string();
        Node::ErrorValue(name)
    }

    pub fn translate_error_set(&self, name: &str, members: Vec<String>) -> Node {
        Node::SimpleVarDecl {
            var: Var { is_const: true, name: name.to_string(), ty: None },
            expr: Some(Box::new(Node::ErrorSetDecl(members))),
        }
    }
}

struct Collector<'a> {
    translator: &'a Translator,
    symbols: Vec<String>,
}

impl<'ast> Visit<'ast> for Collector<'_> {
    fn visit_type(&mut self, ty: &'ast syn::Type) {
        if let Some((_, error)) = self.translator.result_types(ty) {
            if let Some(symbol) = self.translator.type_symbol(&error) {
                self.symbols.push(symbol);
            }
        }
        syn::visit::visit_type(self, ty);
    }
}
