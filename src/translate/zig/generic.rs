use crate::scip::Range;
use super::Translator;

pub struct GenericFn {
    pub type_params: Vec<String>,
}

impl Translator {
    pub fn register_generic(&mut self, signature: &syn::Signature) {
        let type_params: Vec<String> = signature.generics.params.iter().filter_map(|p| {
            if let syn::GenericParam::Type(tp) = p {
                Some(tp.ident.to_string())
            } else {
                None
            }
        }).collect();
        if type_params.is_empty() {
            return;
        }
        let range: Range = signature.ident.span().into();
        let Some(symbol) = self.scip.symbol_at(&range) else { return };
        self.generic_fns.insert(symbol.to_string(), GenericFn { type_params });
    }

    pub fn type_params(&self, ident: &syn::Ident) -> Vec<String> {
        self.scip.symbol_at(&ident.span().into())
            .and_then(|symbol| self.generic_fns.get(symbol))
            .map(|gf| gf.type_params.clone())
            .unwrap_or_default()
    }
}
