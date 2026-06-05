use syn::visit_mut::{self, VisitMut};

use crate::scip::Scip;

pub fn run(scip: &Scip, file: &mut syn::File) {
    Desugar { scip }.visit_file_mut(file);
}

struct Desugar<'a> {
    scip: &'a Scip,
}

impl Desugar<'_> {
    fn ergonomic_mutability(&self, em: &syn::ExprMatch) -> Option<bool> {
        if em.arms.iter().any(|arm| matches!(arm.pat, syn::Pat::Reference(_))) {
            return None;
        }
        let syn::Expr::Path(ep) = &*em.expr else { return None };
        let ident = ep.path.get_ident()?;
        let syn::Type::Reference(tr) = self.scip.type_at(&ident.span().into())? else {
            return None;
        };
        Some(tr.mutability.is_some())
    }
}

impl VisitMut for Desugar<'_> {
    fn visit_expr_match_mut(&mut self, em: &mut syn::ExprMatch) {
        visit_mut::visit_expr_match_mut(self, em);
        let Some(mutable) = self.ergonomic_mutability(em) else { return };
        let expr = em.expr.clone();
        *em.expr = syn::parse_quote!(*#expr);
        for arm in &mut em.arms {
            mark_binding_refs(&mut arm.pat, mutable);
        }
    }
}

fn mark_binding_refs(pat: &mut syn::Pat, mutable: bool) {
    match pat {
        syn::Pat::TupleStruct(pts) => {
            for elem in &mut pts.elems {
                set_ref(elem, mutable);
            }
        }
        syn::Pat::Struct(ps) => {
            for field in &mut ps.fields {
                set_ref(&mut field.pat, mutable);
            }
        }
        _ => set_ref(pat, mutable),
    }
}

fn set_ref(pat: &mut syn::Pat, mutable: bool) {
    if !matches!(pat, syn::Pat::Ident(_)) {
        return;
    }
    *pat = if mutable {
        syn::parse_quote!(ref mut #pat)
    } else {
        syn::parse_quote!(ref #pat)
    };
}
