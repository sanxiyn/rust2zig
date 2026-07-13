use std::collections::HashMap;

use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::visit_mut::{self, VisitMut};

use crate::scip::{Range, Scip};

pub struct GenericArgRef {
    pub arg: usize,
    pub path: Vec<usize>,
}

pub fn find_type_param(ty: &syn::Type, name: &str) -> Option<Vec<usize>> {
    let syn::Type::Path(tp) = ty else { return None };
    if tp.path.is_ident(name) {
        return Some(Default::default());
    }
    let last = tp.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(ab) = &last.arguments else { return None };
    for (i, ga) in ab.args.iter().enumerate() {
        let syn::GenericArgument::Type(inner) = ga else { continue };
        if let Some(mut sub) = find_type_param(inner, name) {
            let mut path = vec![i];
            path.append(&mut sub);
            return Some(path);
        }
    }
    None
}

pub fn peel_type<'a>(mut ty: &'a syn::Type, path: &[usize]) -> Option<&'a syn::Type> {
    for &idx in path {
        let syn::Type::Path(tp) = ty else { return None };
        let last = tp.path.segments.last()?;
        let syn::PathArguments::AngleBracketed(ab) = &last.arguments else { return None };
        let ga = ab.args.iter().nth(idx)?;
        let syn::GenericArgument::Type(inner) = ga else { return None };
        ty = inner;
    }
    Some(ty)
}

pub fn run(scip: &Scip, file: &mut syn::File) {
    let mut collector = Collect { scip, generics: Default::default() };
    collector.visit_file(file);
    Desugar { scip, generics: collector.generics, changed: false }.visit_file_mut(file);
}

struct Collect<'a> {
    scip: &'a Scip,
    generics: HashMap<String, Vec<GenericArgRef>>,
}

impl Collect<'_> {
    fn analyze_signature(&self, signature: &syn::Signature) -> Option<(String, Vec<GenericArgRef>)> {
        let type_params: Vec<String> = signature.generics.params.iter().filter_map(|p| {
            if let syn::GenericParam::Type(tp) = p {
                Some(tp.ident.to_string())
            } else {
                None
            }
        }).collect();
        if type_params.is_empty() {
            return None;
        }
        let mut refs = Vec::with_capacity(type_params.len());
        for tp in &type_params {
            let mut found = None;
            for (i, arg) in signature.inputs.iter().filter(|x| matches!(x, syn::FnArg::Typed(_))).enumerate() {
                let syn::FnArg::Typed(pt) = arg else { continue };
                if let Some(path) = find_type_param(&pt.ty, tp) {
                    found = Some(GenericArgRef { arg: i, path });
                    break;
                }
            }
            refs.push(found?);
        }
        let range: Range = signature.ident.span().into();
        let symbol = self.scip.symbol_at(&range)?;
        Some((symbol.to_string(), refs))
    }
}

impl<'ast> Visit<'ast> for Collect<'_> {
    fn visit_signature(&mut self, signature: &'ast syn::Signature) {
        visit::visit_signature(self, signature);
        if let Some((symbol, refs)) = self.analyze_signature(signature) {
            self.generics.insert(symbol, refs);
        }
    }
}

struct Desugar<'a> {
    scip: &'a Scip,
    generics: HashMap<String, Vec<GenericArgRef>>,
    changed: bool,
}

impl Desugar<'_> {
    fn callee_refs(&self, ident: &syn::Ident) -> Option<&Vec<GenericArgRef>> {
        let symbol = self.scip.symbol_at(&ident.span().into())?;
        self.generics.get(symbol)
    }

    fn resolve(&self, refs: &[GenericArgRef], args: &Punctuated<syn::Expr, syn::Token![,]>) -> Vec<syn::Type> {
        refs.iter().filter_map(|reference| {
            let syn::Expr::Path(ap) = &args[reference.arg] else { return None };
            let aident = &ap.path.segments.last().unwrap().ident;
            let ty = self.scip.type_at(&aident.span().into())?;
            let peeled = peel_type(&ty, &reference.path)?;
            Some(peeled.clone())
        }).collect()
    }
}

impl VisitMut for Desugar<'_> {
    fn visit_expr_call_mut(&mut self, ec: &mut syn::ExprCall) {
        visit_mut::visit_expr_call_mut(self, ec);
        let types = {
            let syn::Expr::Path(ep) = &*ec.func else { return };
            let ident = &ep.path.segments.last().unwrap().ident;
            let Some(refs) = self.callee_refs(ident) else { return };
            let types = self.resolve(refs, &ec.args);
            if types.is_empty() {
                return;
            }
            types
        };
        if let syn::Expr::Path(ep) = &mut *ec.func {
            let segment = ep.path.segments.last_mut().unwrap();
            segment.arguments = syn::PathArguments::AngleBracketed(turbofish(types));
            self.changed = true;
        }
    }

    fn visit_expr_method_call_mut(&mut self, emc: &mut syn::ExprMethodCall) {
        visit_mut::visit_expr_method_call_mut(self, emc);
        let Some(refs) = self.callee_refs(&emc.method) else { return };
        let types = self.resolve(refs, &emc.args);
        if types.is_empty() {
            return;
        }
        emc.turbofish = Some(turbofish(types));
        self.changed = true;
    }

    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        use syn::parse::Parser;
        let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        let Ok(mut args) = parser.parse2(mac.tokens.clone()) else { return };
        let outer = std::mem::replace(&mut self.changed, false);
        for arg in &mut args {
            self.visit_expr_mut(arg);
        }
        if self.changed {
            mac.tokens = quote::quote! { #args };
        }
        self.changed |= outer;
    }
}

fn turbofish(types: Vec<syn::Type>) -> syn::AngleBracketedGenericArguments {
    syn::parse_quote! { ::<#(#types),*> }
}
