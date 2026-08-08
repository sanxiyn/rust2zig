use std::collections::{HashMap, HashSet};

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::ast::ml::{RecFlag, StructureItem, ValueBinding};
use crate::scip::Scip;
use super::Translator;

pub struct Def {
    pub symbols: HashSet<String>,
    pub refs: HashSet<String>,
    pub item: StructureItem,
}

pub fn order(defs: Vec<Def>) -> Vec<StructureItem> {
    let mut graph: DiGraph<(), ()> = DiGraph::with_capacity(defs.len(), defs.len());
    for _ in &defs {
        graph.add_node(());
    }
    let mut owner: HashMap<&str, usize> = Default::default();
    for (i, def) in defs.iter().enumerate() {
        for symbol in &def.symbols {
            owner.insert(symbol, i);
        }
    }
    for (i, def) in defs.iter().enumerate() {
        for symbol in &def.refs {
            if let Some(&j) = owner.get(symbol.as_str()) {
                graph.add_edge(NodeIndex::new(i), NodeIndex::new(j), ());
            }
        }
    }

    let components = tarjan_scc(&graph);
    let mut defs: Vec<Option<Def>> = defs.into_iter().map(Some).collect();
    let mut out = vec![];
    for component in components {
        let mut members: Vec<usize> = component.iter().map(|node| node.index()).collect();
        members.sort();
        let recursive = members.len() > 1
            || members.iter().any(|&i| graph.find_edge(NodeIndex::new(i), NodeIndex::new(i)).is_some());
        let items: Vec<StructureItem> = members.iter()
            .filter_map(|&i| defs[i].take())
            .map(|def| def.item)
            .collect();
        if recursive {
            out.extend(recursive_group(items));
        } else {
            out.extend(items);
        }
    }
    out
}

fn recursive_group(items: Vec<StructureItem>) -> Vec<StructureItem> {
    let bindings: Vec<ValueBinding> = items.into_iter()
        .flat_map(|item| match item {
            StructureItem::Value(_, bindings) => bindings,
            _ => panic!("mutually recursive component contains a non-value definition"),
        })
        .collect();
    vec![StructureItem::Value(RecFlag::Recursive, bindings)]
}

impl Translator {
    pub fn refs(&self, visit: impl FnOnce(&mut Refs)) -> HashSet<String> {
        let mut refs = Refs { scip: &self.scip, out: Default::default() };
        visit(&mut refs);
        refs.out
    }

    pub fn def_symbol(&self, ident: &syn::Ident) -> Option<String> {
        self.scip.symbol_at(&ident.span().into()).map(str::to_string)
    }
}

pub struct Refs<'a> {
    scip: &'a Scip,
    out: HashSet<String>,
}

impl<'ast> syn::visit::Visit<'ast> for Refs<'_> {
    fn visit_expr_method_call(&mut self, emc: &'ast syn::ExprMethodCall) {
        if let Some(symbol) = self.scip.symbol_at(&emc.method.span().into()) {
            self.out.insert(symbol.to_string());
        }
        syn::visit::visit_expr_method_call(self, emc);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        if let Some(segment) = path.segments.last() {
            if let Some(symbol) = self.scip.symbol_at(&segment.ident.span().into()) {
                self.out.insert(symbol.to_string());
            }
        }
        syn::visit::visit_path(self, path);
    }
}
