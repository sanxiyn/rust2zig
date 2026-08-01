use std::collections::HashMap;

use syn::visit_mut::{self, VisitMut};

use crate::scip::Scip;

pub fn run(scip: &Scip, file: &mut syn::File) {
    let aliases = collect(scip, file);
    if aliases.is_empty() {
        return;
    }
    Desugar { scip, aliases }.visit_file_mut(file);
    file.items.retain(|item| !matches!(item, syn::Item::Type(_)));
}

fn collect(scip: &Scip, file: &syn::File) -> HashMap<String, syn::ItemType> {
    let mut aliases: HashMap<String, syn::ItemType> = Default::default();
    for item in &file.items {
        let syn::Item::Type(it) = item else { continue };
        let Some(symbol) = scip.symbol_at(&it.ident.span().into()) else { continue };
        aliases.insert(symbol.to_string(), it.clone());
    }
    aliases
}

struct Desugar<'a> {
    scip: &'a Scip,
    aliases: HashMap<String, syn::ItemType>,
}

impl VisitMut for Desugar<'_> {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        visit_mut::visit_type_mut(self, ty);
        let Some(expanded) = self.expand(ty) else { return };
        *ty = expanded;
        self.visit_type_mut(ty);
    }
}

impl Desugar<'_> {
    fn expand(&self, ty: &syn::Type) -> Option<syn::Type> {
        let syn::Type::Path(tp) = ty else { return None };
        if tp.qself.is_some() {
            return None;
        }
        let segment = tp.path.segments.last()?;
        let symbol = self.scip.symbol_at(&segment.ident.span().into())?;
        let alias = self.aliases.get(symbol)?;
        let params: Vec<String> = alias.generics.params.iter().filter_map(|param| match param {
            syn::GenericParam::Type(tp) => Some(tp.ident.to_string()),
            _ => None,
        }).collect();
        let args: Vec<syn::Type> = match &segment.arguments {
            syn::PathArguments::AngleBracketed(ab) => ab.args.iter().filter_map(|arg| match arg {
                syn::GenericArgument::Type(ty) => Some(ty.clone()),
                _ => None,
            }).collect(),
            _ => vec![],
        };
        if args.len() < params.len() {
            return None;
        }
        let mut expanded = (*alias.ty).clone();
        let bindings = params.into_iter().zip(args).collect();
        Substitute { bindings }.visit_type_mut(&mut expanded);
        Some(expanded)
    }
}

struct Substitute {
    bindings: HashMap<String, syn::Type>,
}

impl VisitMut for Substitute {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        if let syn::Type::Path(tp) = ty {
            if tp.qself.is_none() {
                if let Some(ident) = tp.path.get_ident() {
                    if let Some(binding) = self.bindings.get(&ident.to_string()) {
                        *ty = binding.clone();
                        return;
                    }
                }
            }
        }
        visit_mut::visit_type_mut(self, ty);
    }
}
