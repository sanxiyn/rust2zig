use crate::ast::zig::Node;
use super::{PathMode, Translator};

pub enum Accessor {
    Field(String),
    Index(usize),
}

pub struct Capture {
    pub name: String,
    pub accessor: Accessor,
    pub by_ref: bool,
}

impl Translator {
    pub fn pat_name(&self, pat: &syn::Pat) -> String {
        match pat {
            syn::Pat::Ident(pi) => self.rename_ident(&pi.ident),
            _ => "_".to_string(),
        }
    }

    pub fn translate_match_pat(&self, pat: &syn::Pat) -> (Node, Vec<Capture>) {
        match pat {
            syn::Pat::Path(pp) => {
                let node = self.translate_path(&pp.path, PathMode::EnumVariant);
                (node, Default::default())
            }
            syn::Pat::Struct(ps) => {
                let node = self.translate_path(&ps.path, PathMode::EnumVariant);
                let mut captures: Vec<Capture> = Default::default();
                for field in &ps.fields {
                    if let syn::Member::Named(ident) = &field.member {
                        if let syn::Pat::Ident(pi) = &*field.pat {
                            captures.push(Capture {
                                name: self.rename_ident(&pi.ident),
                                accessor: Accessor::Field(ident.to_string()),
                                by_ref: pi.by_ref.is_some(),
                            });
                        }
                    }
                }
                (node, captures)
            }
            syn::Pat::TupleStruct(pts) => {
                let node = self.translate_path(&pts.path, PathMode::EnumVariant);
                let mut captures: Vec<Capture> = Default::default();
                for (i, elem) in pts.elems.iter().enumerate() {
                    if let syn::Pat::Ident(pi) = elem {
                        captures.push(Capture {
                            name: self.rename_ident(&pi.ident),
                            accessor: Accessor::Index(i),
                            by_ref: pi.by_ref.is_some(),
                        });
                    }
                }
                (node, captures)
            }
            _ => {
                let node = Node::Todo("match pat".to_string());
                (node, Default::default())
            }
        }
    }
}
