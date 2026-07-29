use crate::ast::ml::{ClosedFlag, Longident, Pattern};
use super::Translator;

impl Translator {
    pub fn translate_pat(&self, pat: &syn::Pat) -> Pattern {
        match pat {
            syn::Pat::Ident(pi) => Pattern::Var(pi.ident.to_string()),
            syn::Pat::Path(pp) if self.is_variant(&pp.path) => {
                let name = pp.path.segments.last().unwrap().ident.to_string();
                Pattern::Construct(Longident::Lident(name), None)
            }
            syn::Pat::Struct(ps) if self.is_variant(&ps.path) => {
                let name = ps.path.segments.last().unwrap().ident.to_string();
                let mut fields = vec![];
                for field in &ps.fields {
                    if let syn::Member::Named(ident) = &field.member {
                        fields.push((Longident::Lident(ident.to_string()), self.translate_pat(&field.pat)));
                    }
                }
                let closed = if ps.rest.is_none() { ClosedFlag::Closed } else { ClosedFlag::Open };
                let record = Pattern::Record(fields, closed);
                Pattern::Construct(Longident::Lident(name), Some(Box::new(record)))
            }
            syn::Pat::Tuple(pt) => {
                Pattern::Tuple(pt.elems.iter().map(|elem| self.translate_pat(elem)).collect())
            }
            syn::Pat::TupleStruct(pts) if self.is_variant(&pts.path) => {
                let name = pts.path.segments.last().unwrap().ident.to_string();
                let arg = match pts.elems.len() {
                    0 => None,
                    1 => Some(Box::new(self.translate_pat(&pts.elems[0]))),
                    _ => Some(Box::new(Pattern::Tuple(
                        pts.elems.iter().map(|elem| self.translate_pat(elem)).collect(),
                    ))),
                };
                Pattern::Construct(Longident::Lident(name), arg)
            }
            syn::Pat::Type(pt) => self.translate_pat(&pt.pat),
            _ => Pattern::Var("_".to_string()),
        }
    }
}
