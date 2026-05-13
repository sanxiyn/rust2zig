use std::fmt::Write;

use super::{PathMode, Rust2Zig, peel_type};
use super::name::{camel_to_snake, escape_zig, snake_to_camel};

impl Rust2Zig {
    pub fn translate_call(&mut self, ec: &syn::ExprCall) {
        if let syn::Expr::Path(ep) = &*ec.func {
            if matches!(self.path_mode(&ep.path), PathMode::EnumVariant) {
                let name = ep.path.segments.last().unwrap().ident.to_string();
                let variant = camel_to_snake(&name);
                write!(self.out, ".{{ .{} = ", variant).unwrap();
                let multi = ec.args.len() > 1;
                if multi {
                    write!(self.out, ".{{ ").unwrap();
                }
                for (i, arg) in ec.args.iter().enumerate() {
                    if i > 0 {
                        write!(self.out, ", ").unwrap();
                    }
                    self.translate_expr(arg);
                }
                if multi {
                    write!(self.out, " }}").unwrap();
                }
                write!(self.out, " }}").unwrap();
                return;
            }
            if self.check_moniker(&ep.path, "core::option::Option::Some") {
                self.translate_expr(&ec.args[0]);
                return;
            }
        }
        self.translate_expr(&ec.func);
        write!(self.out, "(").unwrap();
        let mut first = true;
        if let syn::Expr::Path(ep) = &*ec.func {
            let ident = &ep.path.segments.last().unwrap().ident;
            let refs: Vec<(usize, Vec<usize>)> = self.scip.symbol_at(&ident.span().into())
                .and_then(|s| self.generic_fns.get(s))
                .map(|gf| gf.param_arg_index.iter().map(|r| (r.arg, r.path.clone())).collect())
                .unwrap_or_default();
            for (arg_idx, path) in refs {
                let syn::Expr::Path(ap) = &ec.args[arg_idx] else { continue };
                let aident = &ap.path.segments.last().unwrap().ident;
                let Some(ty) = self.scip.type_at(&aident.span().into()) else { continue };
                let Some(peeled) = peel_type(&ty, &path) else { continue };
                if !first {
                    write!(self.out, ", ").unwrap();
                }
                first = false;
                let peeled = peeled.clone();
                self.translate_type(&peeled);
            }
        }
        for arg in ec.args.iter() {
            if !first {
                write!(self.out, ", ").unwrap();
            }
            first = false;
            self.translate_expr(arg);
        }
        write!(self.out, ")").unwrap();
    }

    pub fn translate_method_call(&mut self, emc: &syn::ExprMethodCall) {
        if self.check_moniker_ident(&emc.method, "core::slice::len") {
            self.translate_expr(&emc.receiver);
            write!(self.out, ".len").unwrap();
            return;
        }
        self.translate_expr(&emc.receiver);
        write!(self.out, ".{}", escape_zig(&snake_to_camel(&emc.method.to_string()))).unwrap();
        write!(self.out, "(").unwrap();
        let refs: Vec<(usize, Vec<usize>)> = self.scip.symbol_at(&emc.method.span().into())
            .and_then(|s| self.generic_fns.get(s))
            .map(|gf| gf.param_arg_index.iter().map(|r| (r.arg, r.path.clone())).collect())
            .unwrap_or_default();
        let mut first = true;
        for (arg_idx, path) in refs {
            let syn::Expr::Path(ap) = &emc.args[arg_idx] else { continue };
            let aident = &ap.path.segments.last().unwrap().ident;
            let Some(ty) = self.scip.type_at(&aident.span().into()) else { continue };
            let Some(peeled) = peel_type(&ty, &path) else { continue };
            if !first {
                write!(self.out, ", ").unwrap();
            }
            first = false;
            let peeled = peeled.clone();
            self.translate_type(&peeled);
        }
        for arg in emc.args.iter() {
            if !first {
                write!(self.out, ", ").unwrap();
            }
            first = false;
            self.translate_expr(arg);
        }
        write!(self.out, ")").unwrap();
    }
}
