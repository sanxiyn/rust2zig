use crate::ast::zig::Node;
use crate::scip::Kind;
use super::{PathMode, Translator};

pub enum Accessor {
    Field(String),
    Index(usize),
    Whole,
}

pub struct Capture {
    pub pat: syn::PatIdent,
    pub accessor: Accessor,
}

impl Capture {
    pub fn name(&self) -> String {
        self.pat.ident.to_string()
    }

    pub fn by_ref(&self) -> bool {
        self.pat.by_ref.is_some()
    }
}

impl Translator {
    pub fn pat_captures(&self, pat: &syn::Pat) -> Vec<Capture> {
        match pat {
            syn::Pat::Ident(pi) => vec![Capture { pat: pi.clone(), accessor: Accessor::Whole }],
            syn::Pat::Struct(ps) => ps.fields.iter().filter_map(|field| {
                let syn::Member::Named(ident) = &field.member else { return None };
                let syn::Pat::Ident(pi) = &*field.pat else { return None };
                Some(Capture { pat: pi.clone(), accessor: Accessor::Field(ident.to_string()) })
            }).collect(),
            syn::Pat::TupleStruct(pts) => pts.elems.iter().enumerate().filter_map(|(i, elem)| {
                let syn::Pat::Ident(pi) = elem else { return None };
                Some(Capture { pat: pi.clone(), accessor: Accessor::Index(i) })
            }).collect(),
            _ => Default::default(),
        }
    }

    pub fn capture_access(&self, base: Node, capture: &Capture) -> Node {
        let access = match &capture.accessor {
            Accessor::Field(field) => Node::FieldAccess(Box::new(base), field.clone()),
            Accessor::Index(index) => {
                Node::ArrayAccess(Box::new(base), Box::new(Node::NumberLiteral(index.to_string())))
            }
            Accessor::Whole => base,
        };
        if capture.by_ref() {
            Node::AddressOf(Box::new(access))
        } else {
            access
        }
    }

    pub fn pat_name(&self, pat: &syn::Pat) -> String {
        match pat {
            syn::Pat::Ident(pi) => pi.ident.to_string(),
            _ => "_".to_string(),
        }
    }

    pub fn translate_match_pat(&self, pat: &syn::Pat) -> (Vec<Node>, Vec<Capture>) {
        match pat {
            syn::Pat::Ident(pi) if pi.subpat.is_none() => {
                if self.scip.kind_at(&pi.ident.span().into()) == Some(Kind::EnumMember) {
                    let path = syn::Path::from(pi.ident.clone());
                    let node = self.translate_path(&path, PathMode::EnumVariant);
                    return (vec![node], Default::default());
                }
                (vec![], self.pat_captures(pat))
            }
            syn::Pat::Lit(pl) => {
                let node = match &pl.lit {
                    syn::Lit::Bool(b) => Node::Identifier(b.value.to_string()),
                    syn::Lit::Int(i) => Node::NumberLiteral(i.base10_digits().to_string()),
                    syn::Lit::Str(s) => Node::StringLiteral(s.value()),
                    _ => Node::Todo("match lit".to_string()),
                };
                (vec![node], Default::default())
            }
            syn::Pat::Or(po) => {
                let mut nodes = vec![];
                for case in &po.cases {
                    let (node, captures) = self.translate_match_pat(case);
                    if !captures.is_empty() {
                        return (vec![Node::Todo("match or".to_string())], Default::default());
                    }
                    nodes.extend(node);
                }
                (nodes, Default::default())
            }
            syn::Pat::Path(pp) => {
                let node = self.translate_path(&pp.path, PathMode::EnumVariant);
                (vec![node], Default::default())
            }
            syn::Pat::Struct(ps) => {
                let node = self.translate_path(&ps.path, PathMode::EnumVariant);
                (vec![node], self.pat_captures(pat))
            }
            syn::Pat::TupleStruct(pts) => {
                let node = self.translate_path(&pts.path, PathMode::EnumVariant);
                (vec![node], self.pat_captures(pat))
            }
            syn::Pat::Wild(_) => (vec![], Default::default()),
            _ => {
                let node = Node::Todo("match pat".to_string());
                (vec![node], Default::default())
            }
        }
    }
}

pub fn pat_guard(pat: &syn::Pat) -> (&syn::Pat, Option<&syn::Expr>) {
    if let syn::Pat::Guard(pg) = pat {
        (&*pg.pat, Some(&*pg.guard))
    } else {
        (pat, None)
    }
}
