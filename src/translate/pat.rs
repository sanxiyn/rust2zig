use std::fmt::Write;

use super::{PathMode, Rust2Zig, camel_to_snake};

impl Rust2Zig {
    pub fn translate_pat(&mut self, pat: &syn::Pat) {
        match pat {
            syn::Pat::Ident(pi) => {
                write!(self.out, "{}", pi.ident).unwrap();
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

    pub fn translate_match_pat(&mut self, pat: &syn::Pat) -> Vec<(String, String)> {
        match pat {
            syn::Pat::Ident(pi) => {
                write!(self.out, "{}", pi.ident).unwrap();
                Default::default()
            }
            syn::Pat::Path(pp) => {
                self.translate_path(&pp.path, PathMode::EnumVariant);
                Default::default()
            }
            syn::Pat::Struct(ps) => {
                let variant = ps.path.segments.last().unwrap().ident.to_string();
                write!(self.out, ".{}", camel_to_snake(&variant)).unwrap();
                let mut captures: Vec<(String, String)> = Default::default();
                for field in &ps.fields {
                    if let syn::Member::Named(ident) = &field.member {
                        if let syn::Pat::Ident(pi) = &*field.pat {
                            captures.push((pi.ident.to_string(), format!(".{}", ident)));
                        }
                    }
                }
                captures
            }
            syn::Pat::TupleStruct(pts) => {
                let variant = pts.path.segments.last().unwrap().ident.to_string();
                write!(self.out, ".{}", camel_to_snake(&variant)).unwrap();
                let mut captures: Vec<(String, String)> = Default::default();
                for (i, elem) in pts.elems.iter().enumerate() {
                    if let syn::Pat::Ident(pi) = elem {
                        captures.push((pi.ident.to_string(), format!("[{}]", i)));
                    }
                }
                captures
            }
            syn::Pat::Wild(_) => {
                write!(self.out, "_").unwrap();
                Default::default()
            }
            _ => {
                write!(self.out, "/* TODO: match pat */").unwrap();
                Default::default()
            }
        }
    }
}
