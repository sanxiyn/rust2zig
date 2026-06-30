use crate::ast::zig::Node;
use crate::translate::name::{camel_to_snake, escape_zig, snake_to_camel};
use super::{PathMode, Translator};
use super::generic::peel_type;

impl Translator {
    pub fn translate_call(&self, ec: &syn::ExprCall) -> Node {
        if let syn::Expr::Path(ep) = &*ec.func {
            if matches!(self.path_mode(&ep.path), PathMode::EnumVariant) {
                return self.translate_call_constructor(ec, &ep.path);
            }
            if self.check_moniker(&ep.path, "core::option::Option::Some") {
                return self.translate_expr(&ec.args[0]);
            }
        }
        let func = self.translate_callee(&ec.func);
        let mut args = self.generic_type_args(&ec.func, &ec.args);
        for arg in &ec.args {
            let arg = self.translate_expr(arg);
            args.push(arg);
        }
        Node::Call(Box::new(func), args)
    }

    fn translate_call_constructor(&self, ec: &syn::ExprCall, path: &syn::Path) -> Node {
        let name = path.segments.last().unwrap().ident.to_string();
        let variant = camel_to_snake(&name);
        let multi = ec.args.len() > 1;
        let value = if multi {
            let mut elements = vec![];
            for arg in &ec.args {
                let element = self.translate_expr(arg);
                elements.push(element);
            }
            Node::ArrayInit(None, elements)
        } else {
            self.translate_expr(&ec.args[0])
        };
        Node::StructInit(None, vec![(variant, value)])
    }

    fn translate_callee(&self, func: &syn::Expr) -> Node {
        let node = self.translate_expr(func);
        let syn::Expr::Path(ep) = func else { return node };
        let ident = &ep.path.segments.last().unwrap().ident;
        match self.scip.type_at(&ident.span().into()) {
            Some(ty) if self.is_closure_type(&ty) => {
                Node::FieldAccess(Box::new(node), "call".to_string())
            }
            _ => node,
        }
    }

    fn generic_type_args(
        &self,
        func: &syn::Expr,
        args: &syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>,
    ) -> Vec<Node> {
        let syn::Expr::Path(ep) = func else { return Vec::new() };
        self.generic_args_for(&ep.path.segments.last().unwrap().ident, args)
    }

    pub fn generic_args_for(
        &self,
        ident: &syn::Ident,
        args: &syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>,
    ) -> Vec<Node> {
        let Some(gf) = self.scip.symbol_at(&ident.span().into()).and_then(|s| self.generic_fns.get(s)) else {
            return Vec::new();
        };
        gf.param_arg_index.iter().filter_map(|reference| {
            let syn::Expr::Path(ap) = &args[reference.arg] else { return None };
            let aident = &ap.path.segments.last().unwrap().ident;
            let ty = self.scip.type_at(&aident.span().into())?;
            let peeled = peel_type(&ty, &reference.path)?;
            Some(self.translate_type(peeled))
        }).collect()
    }

    pub fn translate_method_call(&self, emc: &syn::ExprMethodCall) -> Node {
        if self.check_moniker_ident(&emc.method, "core::slice::len") {
            let base = self.translate_expr(&emc.receiver);
            return Node::FieldAccess(
                Box::new(base),
                "len".to_string(),
            );
        }
        let base = self.translate_expr(&emc.receiver);
        let method = escape_zig(&snake_to_camel(&emc.method.to_string()));
        let func = Node::FieldAccess(Box::new(base), method);
        let mut args = self.generic_args_for(&emc.method, &emc.args);
        for arg in &emc.args {
            let arg = self.translate_expr(arg);
            args.push(arg);
        }
        Node::Call(Box::new(func), args)
    }
}
