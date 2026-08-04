use std::collections::HashMap;

use crate::translate::ty::peel_ref;
use super::Translator;

impl Translator {
    pub fn collect_cell_types(&mut self, file: &syn::File) {
        let mut fields: HashMap<String, Vec<syn::Type>> = Default::default();
        for item in &file.items {
            let syn::Item::Struct(s) = item else { continue };
            let Some(symbol) = self.scip.symbol_at(&s.ident.span().into()) else { continue };
            let tys = s.fields.iter().map(|field| field.ty.clone()).collect();
            fields.insert(symbol.to_string(), tys);
        }
        loop {
            let mut found = vec![];
            for (symbol, tys) in &fields {
                if self.cell_types.contains(symbol) {
                    continue;
                }
                for ty in tys {
                    if self.owns_cell(ty) {
                        found.push(symbol.clone());
                        break;
                    }
                }
                if tys.iter().any(|ty| self.owns_cell(ty)) {
                    found.push(symbol.clone());
                }
            }
            if found.is_empty() {
                break;
            }
            self.cell_types.extend(found);
        }
    }

    fn owns_cell(&self, ty: &syn::Type) -> bool {
        match ty {
            syn::Type::Array(ta) => self.owns_cell(&ta.elem),
            syn::Type::Tuple(tt) => tt.elems.iter().any(|elem| self.owns_cell(elem)),
            syn::Type::Path(tp) => {
                self.check_moniker(&tp.path, "core::cell::Cell") || self.is_cell_bearing(ty)
            }
            _ => false,
        }
    }

    pub fn is_cell_bearing(&self, ty: &syn::Type) -> bool {
        let syn::Type::Path(tp) = ty else { return false };
        let ident = &tp.path.segments.last().unwrap().ident;
        let Some(symbol) = self.scip.symbol_at(&ident.span().into()) else { return false };
        self.cell_types.contains(symbol)
    }

    pub fn receiver_is_cell_bearing(&self, receiver: &syn::Receiver) -> bool {
        let range = receiver.self_token.span.into();
        let Some(ty) = self.scip.type_at(&range) else { return false };
        self.is_cell_bearing(peel_ref(&ty))
    }
}
