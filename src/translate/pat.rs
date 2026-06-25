use std::fmt::Write;

use super::{PathMode, Rust2Zig};

pub struct Capture {
    pub name: String,
    pub accessor: String,
    pub by_ref: bool,
}

impl Rust2Zig {
    pub fn translate_pat(&mut self, pat: &syn::Pat) {
        match pat {
            syn::Pat::Ident(pi) => {
                write!(self.out, "{}", self.rename_ident(&pi.ident)).unwrap();
            }
            syn::Pat::Type(pt) => {
                self.translate_pat(&pt.pat);
                write!(self.out, ": ").unwrap();
                self.translate_type(&pt.ty);
            }
            syn::Pat::Wild(_) => {
                write!(self.out, "_").unwrap();
            }
            _ => {
                write!(self.out, "/* TODO: pat */").unwrap();
            }
        }
    }

    pub fn translate_match_pat(&mut self, pat: &syn::Pat) -> Vec<Capture> {
        match pat {
            syn::Pat::Path(pp) => {
                self.translate_path(&pp.path, PathMode::EnumVariant);
                Default::default()
            }
            syn::Pat::Struct(ps) => {
                self.translate_path(&ps.path, PathMode::EnumVariant);
                let mut captures: Vec<Capture> = Default::default();
                for field in &ps.fields {
                    if let syn::Member::Named(ident) = &field.member {
                        if let syn::Pat::Ident(pi) = &*field.pat {
                            captures.push(Capture {
                                name: self.rename_ident(&pi.ident),
                                accessor: format!(".{}", ident),
                                by_ref: pi.by_ref.is_some(),
                            });
                        }
                    }
                }
                captures
            }
            syn::Pat::TupleStruct(pts) => {
                self.translate_path(&pts.path, PathMode::EnumVariant);
                let mut captures: Vec<Capture> = Default::default();
                for (i, elem) in pts.elems.iter().enumerate() {
                    if let syn::Pat::Ident(pi) = elem {
                        captures.push(Capture {
                            name: self.rename_ident(&pi.ident),
                            accessor: format!("[{}]", i),
                            by_ref: pi.by_ref.is_some(),
                        });
                    }
                }
                captures
            }
            _ => {
                write!(self.out, "/* TODO: match pat */").unwrap();
                Default::default()
            }
        }
    }
}
