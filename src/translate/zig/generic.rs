use crate::scip::Range;
use super::Translator;

pub struct GenericArgRef {
    pub arg: usize,
    pub path: Vec<usize>,
}

pub struct GenericFn {
    pub type_params: Vec<String>,
    pub param_arg_index: Vec<GenericArgRef>,
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

impl Translator {
    pub fn register_generic(&mut self, sig: &syn::Signature) {
        let type_params: Vec<String> = sig.generics.params.iter().filter_map(|p| {
            if let syn::GenericParam::Type(tp) = p {
                Some(tp.ident.to_string())
            } else {
                None
            }
        }).collect();
        if type_params.is_empty() {
            return;
        }
        let mut param_arg_index = Vec::with_capacity(type_params.len());
        for tp in &type_params {
            let mut found: Option<GenericArgRef> = None;
            for (i, arg) in sig.inputs.iter().filter(|x| matches!(x, syn::FnArg::Typed(_))).enumerate() {
                let syn::FnArg::Typed(pt) = arg else { continue };
                if let Some(path) = find_type_param(&pt.ty, tp) {
                    found = Some(GenericArgRef { arg: i, path });
                    break;
                }
            }
            let Some(r) = found else { return };
            param_arg_index.push(r);
        }
        let range: Range = sig.ident.span().into();
        let Some(symbol) = self.scip.symbol_at(&range) else { return };
        self.generic_fns.insert(symbol.to_string(), GenericFn { type_params, param_arg_index });
    }

    pub fn type_params(&self, ident: &syn::Ident) -> Vec<String> {
        self.scip.symbol_at(&ident.span().into())
            .and_then(|symbol| self.generic_fns.get(symbol))
            .map(|gf| gf.type_params.clone())
            .unwrap_or_default()
    }
}
