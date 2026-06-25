use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use crate::scip::{Kind, Range, Scip};
use super::Rust2Zig;

impl Rust2Zig {
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

    pub fn translate_closure_local(&mut self, pi: &syn::PatIdent, ec: &syn::ExprClosure) {
        let pad = self.pad();
        let local_name = self.rename_ident(&pi.ident);
        let captures = self.collect_captures(ec);
        writeln!(self.out, "{}const {} = struct {{", pad, local_name).unwrap();
        self.indent();
        let mut capture_fields: Vec<String> = Vec::new();
        for (ident, ty) in &captures {
            let field = self.rename_ident(ident);
            let pad = self.pad();
            write!(self.out, "{}{}: ", pad, field).unwrap();
            self.translate_type(ty);
            writeln!(self.out, ",").unwrap();
            capture_fields.push(field);
        }
        let self_name = if captures.is_empty() { "_" } else { "self" };
        write!(self.out, "{}fn call({}: @This()", self.pad(), self_name).unwrap();
        for input in ec.inputs.iter() {
            write!(self.out, ", ").unwrap();
            if let syn::Pat::Ident(pi) = input {
                write!(self.out, "{}", self.rename_ident(&pi.ident)).unwrap();
                if let Some(ty) = self.scip.type_at(&pi.ident.span().into()) {
                    write!(self.out, ": ").unwrap();
                    self.translate_type(&ty);
                }
            } else {
                self.translate_pat(input);
            }
        }
        write!(self.out, ") ").unwrap();
        if let Some(ret) = self.closure_return_type(&pi.ident) {
            self.translate_type(&ret);
        } else {
            write!(self.out, "void").unwrap();
        }
        write!(self.out, " ").unwrap();
        let mut map: HashMap<String, String> = Default::default();
        for ((ident, _), field) in captures.iter().zip(capture_fields.iter()) {
            if let Some(symbol) = self.scip.symbol_at(&ident.span().into()) {
                map.insert(symbol.to_string(), field.clone());
            }
        }
        self.capture_stack.push(map);
        if let syn::Expr::Block(eb) = &*ec.body {
            self.translate_block(&eb.block);
            writeln!(self.out).unwrap();
        } else {
            writeln!(self.out, "{{").unwrap();
            self.indent();
            write!(self.out, "{}return ", self.pad()).unwrap();
            self.translate_expr(&ec.body);
            writeln!(self.out, ";").unwrap();
            self.dedent();
            writeln!(self.out, "{}}}", self.pad()).unwrap();
        }
        self.capture_stack.pop();
        self.dedent();
        if capture_fields.is_empty() {
            writeln!(self.out, "{}}}{{}};", pad).unwrap();
        } else {
            write!(self.out, "{}}}{{ ", pad).unwrap();
            for (i, field) in capture_fields.iter().enumerate() {
                if i > 0 {
                    write!(self.out, ", ").unwrap();
                }
                write!(self.out, ".{} = {}", field, field).unwrap();
            }
            writeln!(self.out, " }};").unwrap();
        }
    }
}
