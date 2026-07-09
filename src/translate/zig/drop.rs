use std::collections::{HashMap, HashSet};

use syn::visit::Visit;

use crate::ast::zig::Node;
use crate::scip::Scip;
use super::Translator;

pub struct DropInfo {
    pub name: String,
    pub may_move: bool,
    pub must_move: bool,
    pub has_drop_call: bool,
}

impl DropInfo {
    pub fn needs_flag(&self) -> bool {
        self.may_move && !self.must_move
    }

    pub fn needs_defer(&self) -> bool {
        !self.must_move
    }

    pub fn needs_var(&self) -> bool {
        self.needs_defer() || self.has_drop_call
    }
}

impl Translator {
    pub fn collect_drop_infos(&mut self, file: &syn::File) {
        let mut collector = Collector {
            scip: &self.scip,
            drop_types: &self.drop_types,
            drop_infos: Default::default(),
        };
        collector.visit_file(file);
        self.drop_infos = collector.drop_infos;
    }

    pub fn drop_info_at(&self, ident: &syn::Ident) -> Option<&DropInfo> {
        let symbol = self.scip.symbol_at(&ident.span().into())?;
        self.drop_infos.get(symbol)
    }

    pub fn local_drop_flags(&self, ident: &syn::Ident) -> (bool, bool, bool) {
        let Some(info) = self.drop_info_at(ident) else {
            return (false, false, false);
        };
        (info.needs_var(), info.needs_defer(), info.needs_flag())
    }

    pub fn alive_name(zig_name: &str) -> String {
        format!("{zig_name}_alive")
    }

    pub fn conditional_defer(&self, zig_name: &str) -> Node {
        let alive = Node::Identifier(Self::alive_name(zig_name));
        let drop = self.drop_call(zig_name);
        Node::Defer(Box::new(Node::Block(vec![Node::If {
            cond: Box::new(alive),
            capture: None,
            then_branch: Box::new(Node::Block(vec![drop])),
            else_branch: None,
        }])))
    }

    pub fn prelude_clear_flags(&self, expr: &syn::Expr) -> Vec<Node> {
        if matches!(
            expr,
            syn::Expr::If(_)
                | syn::Expr::Match(_)
                | syn::Expr::Block(_)
                | syn::Expr::Unsafe(_)
        ) {
            return Vec::new();
        }
        let mut collector = ClearCollector {
            scip: &self.scip,
            drop_infos: &self.drop_infos,
            renames: &self.renames,
            seen: Default::default(),
            clears: Vec::new(),
        };
        collector.visit_expr(expr);
        collector.clears
    }
}

struct ClearCollector<'a> {
    scip: &'a Scip,
    drop_infos: &'a HashMap<String, DropInfo>,
    renames: &'a HashMap<String, String>,
    seen: HashSet<String>,
    clears: Vec<Node>,
}

impl ClearCollector<'_> {
    fn consider_path(&mut self, ep: &syn::ExprPath) {
        if ep.path.segments.len() != 1 {
            return;
        }
        let ident = &ep.path.segments[0].ident;
        let Some(symbol) = self.scip.symbol_at(&ident.span().into()) else { return };
        if !self.seen.insert(symbol.to_string()) {
            return;
        }
        let Some(info) = self.drop_infos.get(symbol) else { return };
        if !info.needs_flag() {
            return;
        }
        let zig_name = self
            .renames
            .get(symbol)
            .cloned()
            .unwrap_or_else(|| info.name.clone());
        self.clears.push(Node::Assign(
            Box::new(Node::Identifier(Translator::alive_name(&zig_name))),
            Box::new(Node::Identifier("false".to_string())),
        ));
    }
}

impl<'ast> Visit<'ast> for ClearCollector<'_> {
    fn visit_expr_assign(&mut self, ea: &'ast syn::ExprAssign) {
        self.visit_expr(&ea.right);
    }

    fn visit_expr_field(&mut self, _: &'ast syn::ExprField) {}

    fn visit_expr_method_call(&mut self, emc: &'ast syn::ExprMethodCall) {
        for arg in &emc.args {
            self.visit_expr(arg);
        }
    }

    fn visit_expr_path(&mut self, ep: &'ast syn::ExprPath) {
        self.consider_path(ep);
    }

    fn visit_expr_reference(&mut self, _: &'ast syn::ExprReference) {}
}

struct Collector<'a> {
    scip: &'a Scip,
    drop_types: &'a HashSet<String>,
    drop_infos: HashMap<String, DropInfo>,
}

impl Collector<'_> {
    fn type_needs_drop(&self, ty: &syn::Type) -> bool {
        let syn::Type::Path(tp) = ty else { return false };
        let ident = &tp.path.segments.last().unwrap().ident;
        if let Some(symbol) = self.scip.symbol_at(&ident.span().into()) {
            return self.drop_types.contains(symbol);
        }
        let name = ident.to_string();
        self.drop_types.iter().any(|s| {
            s.ends_with(&format!("/{name}#")) || s.ends_with(&format!("{name}#"))
        })
    }

    fn binding_needs_drop(&self, ident: &syn::Ident, ty: Option<&syn::Type>) -> bool {
        if let Some(ty) = ty {
            if self.type_needs_drop(ty) {
                return true;
            }
        }
        if let Some(ty) = self.scip.type_at(&ident.span().into()) {
            return self.type_needs_drop(&ty);
        }
        false
    }

    fn record_binding(&mut self, ident: &syn::Ident, ty: Option<&syn::Type>, scope: &[syn::Stmt]) {
        if !self.binding_needs_drop(ident, ty) {
            return;
        }
        let Some(symbol) = self.scip.symbol_at(&ident.span().into()) else { return };
        let name = ident.to_string();
        let (may_move, must_move) = scope_move_status(&name, scope);
        self.drop_infos.insert(
            symbol.to_string(),
            DropInfo {
                name: name.clone(),
                may_move,
                must_move,
                has_drop_call: scope_has_drop_call(&name, scope),
            },
        );
    }

    fn record_block_locals(&mut self, stmts: &[syn::Stmt]) {
        for stmt in stmts {
            if let syn::Stmt::Local(local) = stmt {
                let pat = match &local.pat {
                    syn::Pat::Type(pt) => &*pt.pat,
                    pat => pat,
                };
                if let syn::Pat::Ident(pi) = pat {
                    let ty = match &local.pat {
                        syn::Pat::Type(pt) => Some(&*pt.ty),
                        _ => None,
                    };
                    self.record_binding(&pi.ident, ty, stmts);
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for Collector<'_> {
    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        for arg in &f.sig.inputs {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pi) = &*pat_type.pat {
                    self.record_binding(&pi.ident, Some(&pat_type.ty), &f.block.stmts);
                }
            }
        }
        syn::visit::visit_item_fn(self, f);
    }

    fn visit_impl_item_fn(&mut self, f: &'ast syn::ImplItemFn) {
        for arg in &f.sig.inputs {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pi) = &*pat_type.pat {
                    self.record_binding(&pi.ident, Some(&pat_type.ty), &f.block.stmts);
                }
            }
        }
        syn::visit::visit_impl_item_fn(self, f);
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.record_block_locals(&block.stmts);
        syn::visit::visit_block(self, block);
    }
}

fn is_ident_path(expr: &syn::Expr, name: &str) -> bool {
    let syn::Expr::Path(ep) = expr else { return false };
    ep.path.segments.len() == 1 && ep.path.segments[0].ident == name
}

fn is_mem_drop_call(expr: &syn::Expr) -> bool {
    let syn::Expr::Path(ep) = expr else { return false };
    ep.path.segments.last().unwrap().ident == "drop"
}

fn expr_moves_ident(name: &str, expr: &syn::Expr) -> bool {
    let mut finder = MoveFinder { name, found: false };
    finder.visit_expr(expr);
    finder.found
}

fn scope_has_drop_call(name: &str, scope: &[syn::Stmt]) -> bool {
    let mut finder = DropCallFinder { name, found: false };
    for stmt in scope {
        finder.visit_stmt(stmt);
        if finder.found {
            return true;
        }
    }
    false
}

fn scope_move_status(name: &str, scope: &[syn::Stmt]) -> (bool, bool) {
    let exits = analyze_stmts(name, scope, false);
    if exits.is_empty() {
        return (false, false);
    }
    let may = exits.iter().any(|&m| m);
    let must = exits.iter().all(|&m| m);
    (may, must)
}

enum BranchExit {
    Fallthrough(bool),
    Terminate(bool),
}

fn analyze_stmts(name: &str, stmts: &[syn::Stmt], moved: bool) -> Vec<bool> {
    let mut exits = Vec::new();
    for exit in analyze_stmts_inner(name, stmts, moved) {
        match exit {
            BranchExit::Fallthrough(m) | BranchExit::Terminate(m) => exits.push(m),
        }
    }
    exits
}

fn analyze_stmts_inner(name: &str, stmts: &[syn::Stmt], moved: bool) -> Vec<BranchExit> {
    if stmts.is_empty() {
        return vec![BranchExit::Fallthrough(moved)];
    }
    let (head, tail) = stmts.split_first().unwrap();
    match head {
        syn::Stmt::Local(local) => {
            let moved = moved
                || local
                    .init
                    .as_ref()
                    .is_some_and(|init| expr_moves_ident(name, &init.expr));
            analyze_stmts_inner(name, tail, moved)
        }
        syn::Stmt::Expr(expr, semi) => {
            let is_tail = tail.is_empty() && semi.is_none();
            match expr {
                syn::Expr::Return(er) => {
                    let m = moved
                        || er
                            .expr
                            .as_ref()
                            .is_some_and(|e| expr_moves_ident(name, e));
                    vec![BranchExit::Terminate(m)]
                }
                syn::Expr::If(ei) => analyze_if(name, ei, moved, is_tail, tail),
                syn::Expr::Match(em) => analyze_match(name, em, moved, is_tail, tail),
                syn::Expr::Block(eb) => {
                    continue_after(name, analyze_stmts_inner(name, &eb.block.stmts, moved), is_tail, tail)
                }
                syn::Expr::Unsafe(eu) => {
                    continue_after(name, analyze_stmts_inner(name, &eu.block.stmts, moved), is_tail, tail)
                }
                other => {
                    let moved = moved || expr_moves_ident(name, other);
                    if is_tail {
                        vec![BranchExit::Terminate(moved)]
                    } else {
                        analyze_stmts_inner(name, tail, moved)
                    }
                }
            }
        }
        _ => analyze_stmts_inner(name, tail, moved),
    }
}

fn continue_after(
    name: &str,
    branch_exits: Vec<BranchExit>,
    is_tail: bool,
    tail: &[syn::Stmt],
) -> Vec<BranchExit> {
    let mut result = Vec::new();
    for exit in branch_exits {
        match exit {
            BranchExit::Terminate(m) => result.push(BranchExit::Terminate(m)),
            BranchExit::Fallthrough(m) => {
                if is_tail {
                    result.push(BranchExit::Terminate(m));
                } else {
                    result.extend(analyze_stmts_inner(name, tail, m));
                }
            }
        }
    }
    result
}

fn analyze_if(
    name: &str,
    ei: &syn::ExprIf,
    moved: bool,
    is_tail: bool,
    tail: &[syn::Stmt],
) -> Vec<BranchExit> {
    let then_exits = analyze_stmts_inner(name, &ei.then_branch.stmts, moved);
    let else_exits = match &ei.else_branch {
        None => vec![BranchExit::Fallthrough(moved)],
        Some((_, else_expr)) => analyze_branch_expr(name, else_expr, moved),
    };
    let mut result = Vec::new();
    for exit in then_exits.into_iter().chain(else_exits) {
        match exit {
            BranchExit::Terminate(m) => result.push(BranchExit::Terminate(m)),
            BranchExit::Fallthrough(m) => {
                if is_tail {
                    result.push(BranchExit::Terminate(m));
                } else {
                    result.extend(analyze_stmts_inner(name, tail, m));
                }
            }
        }
    }
    result
}

fn analyze_match(
    name: &str,
    em: &syn::ExprMatch,
    moved: bool,
    is_tail: bool,
    tail: &[syn::Stmt],
) -> Vec<BranchExit> {
    let mut result = Vec::new();
    for arm in &em.arms {
        for exit in analyze_branch_expr(name, &arm.body, moved) {
            match exit {
                BranchExit::Terminate(m) => result.push(BranchExit::Terminate(m)),
                BranchExit::Fallthrough(m) => {
                    if is_tail {
                        result.push(BranchExit::Terminate(m));
                    } else {
                        result.extend(analyze_stmts_inner(name, tail, m));
                    }
                }
            }
        }
    }
    result
}

fn analyze_branch_expr(name: &str, expr: &syn::Expr, moved: bool) -> Vec<BranchExit> {
    match expr {
        syn::Expr::Block(eb) => analyze_stmts_inner(name, &eb.block.stmts, moved),
        syn::Expr::Return(er) => {
            let m = moved
                || er
                    .expr
                    .as_ref()
                    .is_some_and(|e| expr_moves_ident(name, e));
            vec![BranchExit::Terminate(m)]
        }
        syn::Expr::If(ei) => analyze_if(name, ei, moved, true, &[]),
        other => {
            let m = moved || expr_moves_ident(name, other);
            vec![BranchExit::Fallthrough(m)]
        }
    }
}

struct MoveFinder<'a> {
    name: &'a str,
    found: bool,
}

impl<'ast> Visit<'ast> for MoveFinder<'_> {
    fn visit_expr_assign(&mut self, ea: &'ast syn::ExprAssign) {
        self.visit_expr(&ea.right);
    }

    fn visit_expr_field(&mut self, _: &'ast syn::ExprField) {}

    fn visit_expr_method_call(&mut self, emc: &'ast syn::ExprMethodCall) {
        for arg in &emc.args {
            self.visit_expr(arg);
            if self.found {
                return;
            }
        }
    }

    fn visit_expr_path(&mut self, ep: &'ast syn::ExprPath) {
        if !self.found
            && ep.path.segments.len() == 1
            && ep.path.segments[0].ident == self.name
        {
            self.found = true;
        }
    }

    fn visit_expr_reference(&mut self, _: &'ast syn::ExprReference) {}
}

struct DropCallFinder<'a> {
    name: &'a str,
    found: bool,
}

impl<'ast> Visit<'ast> for DropCallFinder<'_> {
    fn visit_expr_call(&mut self, ec: &'ast syn::ExprCall) {
        if !self.found
            && is_mem_drop_call(&ec.func)
            && ec.args.iter().any(|arg| is_ident_path(arg, self.name))
        {
            self.found = true;
            return;
        }
        syn::visit::visit_expr_call(self, ec);
    }
}
