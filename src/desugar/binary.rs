use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};

use crate::scip::Scip;

pub fn run(scip: &Scip, file: &mut syn::File) {
    Desugar { scip }.visit_file_mut(file);
}

struct Desugar<'a> {
    scip: &'a Scip,
}

impl VisitMut for Desugar<'_> {
    fn visit_expr_binary_mut(&mut self, eb: &mut syn::ExprBinary) {
        visit_mut::visit_expr_binary_mut(self, eb);
        let types = self.scip.binary_type_at(&eb.op.span().into());
        let Some((left, right)) = types else { return };
        if matches!(left, syn::Type::Reference(_)) {
            let left = eb.left.clone();
            *eb.left = syn::parse_quote!(*#left);
        }
        if matches!(right, syn::Type::Reference(_)) {
            let right = eb.right.clone();
            *eb.right = syn::parse_quote!(*#right);
        }
    }
}
