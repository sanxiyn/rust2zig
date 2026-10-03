use proc_macro2::Span;
use syn::visit_mut::{self, VisitMut};

pub fn run(file: &mut syn::File) {
    Desugar { counter: 0 }.visit_file_mut(file);
}

struct Desugar {
    counter: usize,
}

impl VisitMut for Desugar {
    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        visit_mut::visit_block_mut(self, block);
        let mut stmts = Vec::with_capacity(block.stmts.len());
        for stmt in std::mem::take(&mut block.stmts) {
            match self.expand(&stmt) {
                Some(expanded) => stmts.extend(expanded),
                None => stmts.push(stmt),
            }
        }
        block.stmts = stmts;
    }
}

impl Desugar {
    fn expand(&mut self, stmt: &syn::Stmt) -> Option<Vec<syn::Stmt>> {
        let syn::Stmt::Expr(syn::Expr::Assign(ea), _) = stmt else { return None };
        let syn::Expr::Tuple(tuple) = &*ea.left else { return None };
        let rhs = (*ea.right).clone();
        let places: Vec<&syn::Expr> = tuple.elems.iter().collect();
        let mut temps = vec![];
        for _ in &places {
            let ident = self.fresh();
            temps.push(ident);
        }
        let mut stmts = vec![syn::parse_quote!(let (#(#temps),*) = #rhs;)];
        for (place, temp) in places.into_iter().zip(temps) {
            stmts.push(syn::parse_quote!(#place = #temp;));
        }
        Some(stmts)
    }

    fn fresh(&mut self) -> syn::Ident {
        self.counter += 1;
        syn::Ident::new(&format!("tmp{}", self.counter), Span::call_site())
    }
}
