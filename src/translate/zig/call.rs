use crate::ast::zig::Node;
use crate::translate::name::{camel_to_snake, escape_zig, snake_to_camel};
use super::{PathMode, Translator};

impl Translator {
    pub fn translate_call(&self, ec: &syn::ExprCall) -> Node {
        if let syn::Expr::Path(ep) = &*ec.func {
            if matches!(self.path_mode(&ep.path), PathMode::EnumVariant) {
                return self.translate_call_constructor(ec, &ep.path);
            }
            if self.check_moniker(&ep.path, "core::option::Option::Some") {
                return self.translate_expr(&ec.args[0]);
            }
            if self.check_moniker(&ep.path, "core::mem::drop") {
                let arg = self.translate_expr(&ec.args[0]);
                return Node::Call(
                    Box::new(Node::FieldAccess(Box::new(arg), "drop".to_string())),
                    vec![],
                );
            }
        }
        let func = self.translate_callee(&ec.func);
        let mut args = vec![];
        if let syn::Expr::Path(ep) = &*ec.func {
            let segment = ep.path.segments.last().unwrap();
            if let syn::PathArguments::AngleBracketed(generic_args) = &segment.arguments {
                for arg in &generic_args.args {
                    let syn::GenericArgument::Type(ty) = arg else { continue };
                    let arg = self.translate_type(ty);
                    args.push(arg);
                }
            }
        }
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
        let mut args = vec![];
        if let Some(generic_args) = &emc.turbofish {
            for arg in &generic_args.args {
                let syn::GenericArgument::Type(ty) = arg else { continue };
                let arg = self.translate_type(ty);
                args.push(arg);
            }
        }
        for arg in &emc.args {
            let arg = self.translate_expr(arg);
            args.push(arg);
        }
        Node::Call(Box::new(func), args)
    }
}
