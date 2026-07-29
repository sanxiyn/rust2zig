use std::collections::{HashMap, HashSet};

use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::visit_mut::VisitMut;

use crate::scip::{Range, Scip};

pub fn run(scip: &Scip, file: &mut syn::File) {
    let mut collector = Collect { scip, renames: Default::default(), stack: Default::default() };
    collector.visit_file(file);
    if collector.renames.is_empty() {
        return;
    }
    Apply { scip, renames: collector.renames, changed: false }.visit_file_mut(file);
}

struct Collect<'a> {
    scip: &'a Scip,
    renames: HashMap<String, String>,
    stack: Vec<HashSet<String>>,
}

impl Collect<'_> {
    fn bind_ident(&mut self, ident: &syn::Ident) {
        let original = ident.to_string();
        let range: Range = ident.span().into();
        let Some(symbol) = self.scip.symbol_at(&range) else { return };
        let symbol = symbol.to_string();
        let mut name = original.clone();
        let mut n = 2;
        while self.stack.iter().any(|scope| scope.contains(&name)) {
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
                for field in &ps.fields {
                    self.bind_pat(&field.pat);
                }
            }
            syn::Pat::Tuple(pt) => {
                for elem in &pt.elems {
                    self.bind_pat(elem);
                }
            }
            syn::Pat::TupleStruct(pts) => {
                for elem in &pts.elems {
                    self.bind_pat(elem);
                }
            }
            syn::Pat::Type(pt) => self.bind_pat(&pt.pat),
            _ => {}
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for Collect<'_> {
    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        self.stack.push(Default::default());
        for arg in &f.sig.inputs {
            if let syn::FnArg::Typed(pt) = arg {
                self.bind_pat(&pt.pat);
            }
        }
        syn::visit::visit_block(self, &f.block);
        self.stack.pop();
    }

    fn visit_impl_item_fn(&mut self, m: &'ast syn::ImplItemFn) {
        self.stack.push(Default::default());
        for arg in &m.sig.inputs {
            if let syn::FnArg::Typed(pt) = arg {
                self.bind_pat(&pt.pat);
            }
        }
        syn::visit::visit_block(self, &m.block);
        self.stack.pop();
    }

    fn visit_block(&mut self, b: &'ast syn::Block) {
        self.stack.push(Default::default());
        syn::visit::visit_block(self, b);
        self.stack.pop();
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some(init) = &local.init {
            syn::visit::visit_expr(self, &init.expr);
        }
        self.bind_pat(&local.pat);
    }

    fn visit_expr_closure(&mut self, ec: &'ast syn::ExprClosure) {
        self.stack.push(Default::default());
        for input in &ec.inputs {
            self.bind_pat(input);
        }
        syn::visit::visit_expr(self, &ec.body);
        self.stack.pop();
    }

    fn visit_expr_for_loop(&mut self, efl: &'ast syn::ExprForLoop) {
        syn::visit::visit_expr(self, &efl.expr);
        self.stack.push(Default::default());
        self.bind_pat(&efl.pat);
        syn::visit::visit_block(self, &efl.body);
        self.stack.pop();
    }

    fn visit_expr_if(&mut self, ei: &'ast syn::ExprIf) {
        self.stack.push(Default::default());
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

    fn visit_expr_match(&mut self, em: &'ast syn::ExprMatch) {
        syn::visit::visit_expr(self, &em.expr);
        for arm in &em.arms {
            self.stack.push(Default::default());
            self.bind_pat(&arm.pat);
            if let Some((_, guard)) = &arm.guard {
                syn::visit::visit_expr(self, guard);
            }
            syn::visit::visit_expr(self, &arm.body);
            self.stack.pop();
        }
    }
}

struct Apply<'a> {
    scip: &'a Scip,
    renames: HashMap<String, String>,
    changed: bool,
}

impl syn::visit_mut::VisitMut for Apply<'_> {
    fn visit_ident_mut(&mut self, ident: &mut syn::Ident) {
        let range: Range = ident.span().into();
        if let Some(symbol) = self.scip.symbol_at(&range) {
            if let Some(name) = self.renames.get(symbol) {
                *ident = syn::Ident::new(name, ident.span());
                self.changed = true;
            }
        }
    }

    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        use syn::parse::Parser;
        let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        let Ok(mut args) = parser.parse2(mac.tokens.clone()) else { return };
        let outer = std::mem::replace(&mut self.changed, false);
        for arg in &mut args {
            self.visit_expr_mut(arg);
        }
        if self.changed {
            mac.tokens = quote::quote! { #args };
        }
        self.changed |= outer;
    }
}
