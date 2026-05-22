use std::collections::{HashMap, HashSet};

use crate::scip::{Range, Scip};
use super::Rust2Zig;

impl Rust2Zig {
    pub fn rename_ident(&self, ident: &syn::Ident) -> String {
        let range: Range = ident.span().into();
        if let Some(symbol) = self.scip.symbol_at(&range) {
            if let Some(name) = self.renames.get(symbol) {
                return name.clone();
            }
        }
        ident.to_string()
    }

    pub fn collect_renames(&mut self, file: &syn::File) {
        use syn::visit::Visit;

        let mut collector = Collector {
            scip: &self.scip,
            renames: HashMap::new(),
            stack: Vec::new(),
        };
        collector.visit_file(file);
        self.renames = collector.renames;
    }
}

struct Collector<'a> {
    scip: &'a Scip,
    renames: HashMap<String, String>,
    stack: Vec<HashSet<String>>,
}

impl Collector<'_> {
    fn bind_ident(&mut self, ident: &syn::Ident) {
        let original = ident.to_string();
        let range: Range = ident.span().into();
        let Some(symbol) = self.scip.symbol_at(&range) else { return };
        let symbol = symbol.to_string();
        let mut name = original.clone();
        let mut n = 2;
        while self.stack.iter().any(|s| s.contains(&name)) {
            name = format!("{original}{n}");
            n += 1;
        }
        self.stack.last_mut().unwrap().insert(name.clone());
        if name != original {
            self.renames.insert(symbol, name);
        }
    }

    fn bind_pat(&mut self, pat: &syn::Pat) {
        match pat {
            syn::Pat::Ident(pi) => self.bind_ident(&pi.ident),
            syn::Pat::Reference(pr) => self.bind_pat(&pr.pat),
            syn::Pat::Struct(ps) => {
                for f in &ps.fields {
                    self.bind_pat(&f.pat);
                }
            }
            syn::Pat::Tuple(pt) => {
                for e in &pt.elems {
                    self.bind_pat(e);
                }
            }
            syn::Pat::TupleStruct(pts) => {
                for e in &pts.elems {
                    self.bind_pat(e);
                }
            }
            syn::Pat::Type(pt) => self.bind_pat(&pt.pat),
            _ => {}
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for Collector<'_> {
    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        self.stack.push(HashSet::new());
        for arg in &f.sig.inputs {
            if let syn::FnArg::Typed(pt) = arg {
                self.bind_pat(&pt.pat);
            }
        }
        syn::visit::visit_block(self, &f.block);
        self.stack.pop();
    }

    fn visit_impl_item_fn(&mut self, m: &'ast syn::ImplItemFn) {
        self.stack.push(HashSet::new());
        for arg in &m.sig.inputs {
            if let syn::FnArg::Typed(pt) = arg {
                self.bind_pat(&pt.pat);
            }
        }
        syn::visit::visit_block(self, &m.block);
        self.stack.pop();
    }

    fn visit_block(&mut self, b: &'ast syn::Block) {
        self.stack.push(HashSet::new());
        syn::visit::visit_block(self, b);
        self.stack.pop();
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some(init) = &local.init {
            syn::visit::visit_expr(self, &init.expr);
        }
        self.bind_pat(&local.pat);
    }

    fn visit_expr_for_loop(&mut self, efl: &'ast syn::ExprForLoop) {
        syn::visit::visit_expr(self, &efl.expr);
        self.stack.push(HashSet::new());
        self.bind_pat(&efl.pat);
        syn::visit::visit_block(self, &efl.body);
        self.stack.pop();
    }

    fn visit_expr_closure(&mut self, ec: &'ast syn::ExprClosure) {
        self.stack.push(HashSet::new());
        for input in &ec.inputs {
            self.bind_pat(input);
        }
        syn::visit::visit_expr(self, &ec.body);
        self.stack.pop();
    }

    fn visit_expr_match(&mut self, em: &'ast syn::ExprMatch) {
        syn::visit::visit_expr(self, &em.expr);
        for arm in &em.arms {
            self.stack.push(HashSet::new());
            self.bind_pat(&arm.pat);
            if let Some((_, guard)) = &arm.guard {
                syn::visit::visit_expr(self, guard);
            }
            syn::visit::visit_expr(self, &arm.body);
            self.stack.pop();
        }
    }

    fn visit_expr_if(&mut self, ei: &'ast syn::ExprIf) {
        self.stack.push(HashSet::new());
        if let syn::Expr::Let(el) = &*ei.cond {
            syn::visit::visit_expr(self, &el.expr);
            self.bind_pat(&el.pat);
        } else {
            syn::visit::visit_expr(self, &ei.cond);
        }
        syn::visit::visit_block(self, &ei.then_branch);
        self.stack.pop();
        if let Some((_, else_expr)) = &ei.else_branch {
            syn::visit::visit_expr(self, else_expr);
        }
    }
}
