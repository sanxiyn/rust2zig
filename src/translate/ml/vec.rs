use crate::translate::ty::{expr_type, peel_ref};
use super::Translator;

pub fn is_vec_type(ty: &syn::Type) -> bool {
    match peel_ref(ty) {
        syn::Type::Path(tp) => match tp.path.segments.last() {
            Some(segment) => segment.ident == "Vec",
            None => false,
        },
        _ => false,
    }
}

impl Translator {
    pub fn is_vec_expr(&self, expr: &syn::Expr) -> bool {
        expr_type(&self.scip, expr).is_some_and(|ty| is_vec_type(&ty))
    }
}
