use crate::ast::zig::Node;
use crate::scip::Kind;
use super::{PathMode, Translator};

pub enum Accessor {
    Field(String),
    Index(usize),
    Whole,
}

pub struct Capture {
    pub name: String,
    pub accessor: Accessor,
    pub by_ref: bool,
}

impl Translator {
    pub fn pat_name(&self, pat: &syn::Pat) -> String {
        match pat {
            syn::Pat::Ident(pi) => pi.ident.to_string(),
            _ => "_".to_string(),
        }
    }

    pub fn translate_match_pat(&self, pat: &syn::Pat) -> (Option<Node>, Vec<Capture>) {
        match pat {
            syn::Pat::Ident(pi) if pi.subpat.is_none() => {
                if self.scip.kind_at(&pi.ident.span().into()) == Some(Kind::EnumMember) {
                    let path = syn::Path::from(pi.ident.clone());
                    let node = self.translate_path(&path, PathMode::EnumVariant);
                    return (Some(node), Default::default());
                }
                let capture = Capture {
                    name: pi.ident.to_string(),
                    accessor: Accessor::Whole,
                    by_ref: pi.by_ref.is_some(),
                };
                (None, vec![capture])
            }
            syn::Pat::Lit(pl) => {
                let node = match &pl.lit {
                    syn::Lit::Bool(b) => Node::Identifier(b.value.to_string()),
                    syn::Lit::Int(i) => Node::NumberLiteral(i.base10_digits().to_string()),
                    syn::Lit::Str(s) => Node::StringLiteral(s.value()),
                    _ => Node::Todo("match lit".to_string()),
                };
                (Some(node), Default::default())
            }
            syn::Pat::Path(pp) => {
                let node = self.translate_path(&pp.path, PathMode::EnumVariant);
                (Some(node), Default::default())
            }
            syn::Pat::Struct(ps) => {
                let node = self.translate_path(&ps.path, PathMode::EnumVariant);
                let mut captures: Vec<Capture> = Default::default();
                for field in &ps.fields {
                    if let syn::Member::Named(ident) = &field.member {
                        if let syn::Pat::Ident(pi) = &*field.pat {
                            captures.push(Capture {
                                name: pi.ident.to_string(),
                                accessor: Accessor::Field(ident.to_string()),
                                by_ref: pi.by_ref.is_some(),
                            });
                        }
                    }
                }
                (Some(node), captures)
            }
            syn::Pat::TupleStruct(pts) => {
                let node = self.translate_path(&pts.path, PathMode::EnumVariant);
                let mut captures: Vec<Capture> = Default::default();
                for (i, elem) in pts.elems.iter().enumerate() {
                    if let syn::Pat::Ident(pi) = elem {
                        captures.push(Capture {
                            name: pi.ident.to_string(),
                            accessor: Accessor::Index(i),
                            by_ref: pi.by_ref.is_some(),
                        });
                    }
                }
                (Some(node), captures)
            }
            syn::Pat::Wild(_) => (None, Default::default()),
            _ => {
                let node = Node::Todo("match pat".to_string());
                (Some(node), Default::default())
            }
        }
    }
}
