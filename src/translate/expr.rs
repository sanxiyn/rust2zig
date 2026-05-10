use std::fmt::Write;

use super::{PathMode, Rust2Zig, camel_to_snake, escape_zig, peel_type, snake_to_camel};

impl Rust2Zig {
    pub fn translate_expr(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Array(ea) => self.translate_array(ea),
            syn::Expr::Assign(ea) => self.translate_assign(ea),
            syn::Expr::Binary(eb) => self.translate_binary(eb),
            syn::Expr::Call(ec) => self.translate_call(ec),
            syn::Expr::Field(ef) => self.translate_field(ef),
            syn::Expr::ForLoop(efl) => self.translate_for_loop(efl),
            syn::Expr::If(ei) => self.translate_if(ei),
            syn::Expr::Lit(el) => self.translate_lit(el),
            syn::Expr::Macro(em) => {
                if !self.translate_macro(&em.mac) {
                    write!(self.out, "/* TODO: macro */").unwrap();
                }
            }
            syn::Expr::Match(em) => self.translate_match(em),
            syn::Expr::MethodCall(emc) => self.translate_method_call(emc),
            syn::Expr::Reference(er) => {
                write!(self.out, "&").unwrap();
                self.translate_expr(&er.expr);
            }
            syn::Expr::Path(ep) => {
                let mode = self.path_mode(&ep.path);
                if matches!(mode, PathMode::EnumVariant) {
                    self.translate_path(&ep.path, mode);
                } else {
                    if self.check_moniker(&ep.path, "core::option::Option::None") {
                        write!(self.out, "null").unwrap();
                    } else {
                        self.translate_path(&ep.path, mode);
                    }
                }
            }
            syn::Expr::Struct(es) => self.translate_struct_expr(es),
            syn::Expr::Tuple(et) => self.translate_tuple(et),
            syn::Expr::While(ew) => self.translate_while(ew),
            _ => {
                write!(self.out, "/* TODO: expr */").unwrap();
            }
        }
    }

    fn translate_array(&mut self, ea: &syn::ExprArray) {
        write!(self.out, ".{{ ").unwrap();
        for (i, elem) in ea.elems.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ").unwrap();
            }
            self.translate_expr(elem);
        }
        write!(self.out, " }}").unwrap();
    }

    fn translate_assign(&mut self, ea: &syn::ExprAssign) {
        self.translate_expr(&ea.left);
        write!(self.out, " = ").unwrap();
        self.translate_expr(&ea.right);
    }

    fn translate_binary(&mut self, eb: &syn::ExprBinary) {
        self.translate_expr(&eb.left);
        let op = self.translate_binop(&eb.op);
        write!(self.out, " {} ", op).unwrap();
        self.translate_expr(&eb.right);
    }

    fn translate_binop(&mut self, op: &syn::BinOp) -> &'static str {
        match op {
            syn::BinOp::Add(_) => "+",
            syn::BinOp::AddAssign(_) => "+=",
            syn::BinOp::Div(_) => "/",
            syn::BinOp::DivAssign(_) => "/=",
            syn::BinOp::Eq(_) => "==",
            syn::BinOp::Ge(_) => ">=",
            syn::BinOp::Gt(_) => ">",
            syn::BinOp::Le(_) => "<=",
            syn::BinOp::Lt(_) => "<",
            syn::BinOp::Mul(_) => "*",
            syn::BinOp::MulAssign(_) => "*=",
            syn::BinOp::Ne(_) => "!=",
            syn::BinOp::Rem(_) => "%",
            syn::BinOp::RemAssign(_) => "%=",
            syn::BinOp::Sub(_) => "-",
            syn::BinOp::SubAssign(_) => "-=",
            _ => "/* TODO: binop */",
        }
    }

    fn translate_call(&mut self, ec: &syn::ExprCall) {
        if let syn::Expr::Path(ep) = &*ec.func {
            if matches!(self.path_mode(&ep.path), PathMode::EnumVariant) {
                let name = ep.path.segments.last().unwrap().ident.to_string();
                let variant = camel_to_snake(&name);
                write!(self.out, ".{{ .{} = ", variant).unwrap();
                let multi = ec.args.len() > 1;
                if multi {
                    write!(self.out, ".{{ ").unwrap();
                }
                for (i, arg) in ec.args.iter().enumerate() {
                    if i > 0 {
                        write!(self.out, ", ").unwrap();
                    }
                    self.translate_expr(arg);
                }
                if multi {
                    write!(self.out, " }}").unwrap();
                }
                write!(self.out, " }}").unwrap();
                return;
            }
            if self.check_moniker(&ep.path, "core::option::Option::Some") {
                self.translate_expr(&ec.args[0]);
                return;
            }
        }
        self.translate_expr(&ec.func);
        write!(self.out, "(").unwrap();
        let mut first = true;
        if let syn::Expr::Path(ep) = &*ec.func {
            let ident = &ep.path.segments.last().unwrap().ident;
            let refs: Vec<(usize, Vec<usize>)> = self.scip.symbol_at(&ident.span().into())
                .and_then(|s| self.generic_fns.get(s))
                .map(|gf| gf.param_arg_index.iter().map(|r| (r.arg, r.path.clone())).collect())
                .unwrap_or_default();
            for (arg_idx, path) in refs {
                let syn::Expr::Path(ap) = &ec.args[arg_idx] else { continue };
                let aident = &ap.path.segments.last().unwrap().ident;
                let Some(ty) = self.scip.type_at(&aident.span().into()) else { continue };
                let Some(peeled) = peel_type(&ty, &path) else { continue };
                if !first {
                    write!(self.out, ", ").unwrap();
                }
                first = false;
                let peeled = peeled.clone();
                self.translate_type(&peeled);
            }
        }
        for arg in ec.args.iter() {
            if !first {
                write!(self.out, ", ").unwrap();
            }
            first = false;
            self.translate_expr(arg);
        }
        write!(self.out, ")").unwrap();
    }

    fn translate_field(&mut self, ef: &syn::ExprField) {
        self.translate_expr(&ef.base);
        match &ef.member {
            syn::Member::Named(ident) => write!(self.out, ".{}", ident).unwrap(),
            syn::Member::Unnamed(index) => write!(self.out, ".{}", index.index).unwrap(),
        }
    }

    fn translate_for_loop(&mut self, efl: &syn::ExprForLoop) {
        if let syn::Expr::Range(er) = &*efl.expr {
            let (Some(start), Some(end)) = (&er.start, &er.end) else {
                write!(self.out, "/* TODO: for */").unwrap();
                return;
            };
            let pi = if let syn::Pat::Ident(pi) = &*efl.pat { pi } else {
                write!(self.out, "/* TODO: for */").unwrap();
                return;
            };
            let name = pi.ident.to_string();
            let ty = self.scip.type_at(&pi.ident.span().into());
            let mut ty_str = String::new();
            if let Some(ty) = &ty {
                let saved = std::mem::take(&mut self.out);
                self.translate_type(ty);
                ty_str = std::mem::replace(&mut self.out, saved);
            }
            let preamble = vec![if ty_str.is_empty() {
                format!("const {name} = _{name};")
            } else {
                format!("const {name}: {ty_str} = @intCast(_{name});")
            }];
            write!(self.out, "for (").unwrap();
            self.translate_expr(start);
            write!(self.out, "..").unwrap();
            if matches!(er.limits, syn::RangeLimits::Closed(_)) {
                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(n), .. }) = &**end {
                    let v: u64 = n.base10_parse().unwrap();
                    write!(self.out, "{}", v + 1).unwrap();
                } else {
                    self.translate_expr(end);
                    write!(self.out, " + 1").unwrap();
                }
            } else {
                self.translate_expr(end);
            }
            write!(self.out, ") |_{}| ", name).unwrap();
            self.translate_block_with_preamble(&efl.body, &preamble);
            return;
        }
        let is_iterable = if let syn::Expr::Path(ep) = &*efl.expr {
            let ident = &ep.path.segments.last().unwrap().ident;
            match self.scip.type_at(&ident.span().into()) {
                Some(syn::Type::Array(_)) => true,
                Some(syn::Type::Reference(tr)) => matches!(*tr.elem, syn::Type::Slice(_)),
                _ => false,
            }
        } else {
            false
        };
        if !is_iterable {
            write!(self.out, "/* TODO: for */").unwrap();
            return;
        }
        write!(self.out, "for (").unwrap();
        self.translate_expr(&efl.expr);
        write!(self.out, ") |").unwrap();
        self.translate_pat(&efl.pat);
        write!(self.out, "| ").unwrap();
        self.translate_block(&efl.body);
    }

    fn translate_if(&mut self, ei: &syn::ExprIf) {
        if let syn::Expr::Let(el) = &*ei.cond {
            if let syn::Pat::TupleStruct(pts) = &*el.pat {
                if self.check_moniker(&pts.path, "core::option::Option::Some") {
                    write!(self.out, "if (").unwrap();
                    self.translate_expr(&el.expr);
                    write!(self.out, ") |").unwrap();
                    self.translate_pat(&pts.elems[0]);
                    write!(self.out, "| ").unwrap();
                    self.translate_block(&ei.then_branch);
                    if let Some((_, else_expr)) = &ei.else_branch {
                        if let syn::Expr::Block(eb) = &**else_expr {
                            write!(self.out, " else ").unwrap();
                            self.translate_block(&eb.block);
                        }
                    }
                    return;
                }
            }
        }
        write!(self.out, "if (").unwrap();
        self.translate_expr(&ei.cond);
        write!(self.out, ") ").unwrap();
        self.translate_block(&ei.then_branch);
        if let Some((_, else_expr)) = &ei.else_branch {
            if let syn::Expr::Block(eb) = &**else_expr {
                write!(self.out, " else ").unwrap();
                self.translate_block(&eb.block);
            }
        }
    }

    fn translate_lit(&mut self, el: &syn::ExprLit) {
        match &el.lit {
            syn::Lit::Bool(b) => write!(self.out, "{}", b.value).unwrap(),
            syn::Lit::Int(i) => write!(self.out, "{}", i.base10_digits()).unwrap(),
            syn::Lit::Str(s) => write!(self.out, "\"{}\"", s.value()).unwrap(),
            _ => writeln!(self.out, "/* TODO: lit */").unwrap(),
        }
    }

    fn translate_match(&mut self, em: &syn::ExprMatch) {
        write!(self.out, "switch (").unwrap();
        self.translate_expr(&em.expr);
        write!(self.out, ") ").unwrap();
        self.translate_match_arms(&em.arms);
    }

    fn translate_match_arms(&mut self, arms: &[syn::Arm]) {
        writeln!(self.out, "{{").unwrap();
        self.indent();
        for arm in arms {
            let pad = self.pad();
            write!(self.out, "{}", pad).unwrap();
            let variant = match &arm.pat {
                syn::Pat::TupleStruct(pts) => {
                    Some(camel_to_snake(&pts.path.segments.last().unwrap().ident.to_string()))
                }
                syn::Pat::Struct(ps) => {
                    Some(camel_to_snake(&ps.path.segments.last().unwrap().ident.to_string()))
                }
                _ => None,
            };
            let captures = self.translate_match_pat(&arm.pat);
            write!(self.out, " => ").unwrap();
            let use_block = captures.len() > 1
                || captures.iter().any(|(_, acc)| acc.starts_with('.'));
            if use_block {
                let payload = format!("_{}", variant.unwrap());
                writeln!(self.out, "|{}| blk: {{", payload).unwrap();
                self.indent();
                for (capture, accessor) in &captures {
                    let pad = self.pad();
                    writeln!(self.out, "{}const {} = {}{};", pad, capture, payload, accessor).unwrap();
                }
                let pad = self.pad();
                write!(self.out, "{}break :blk ", pad).unwrap();
                self.translate_expr(&arm.body);
                writeln!(self.out, ";").unwrap();
                self.dedent();
                write!(self.out, "{}}}", self.pad()).unwrap();
            } else {
                if let Some((capture, _)) = captures.first() {
                    write!(self.out, "|{}| ", capture).unwrap();
                }
                self.translate_expr(&arm.body);
            }
            writeln!(self.out, ",").unwrap();
        }
        self.dedent();
        write!(self.out, "{}}}", self.pad()).unwrap();
    }

    fn translate_method_call(&mut self, emc: &syn::ExprMethodCall) {
        self.translate_expr(&emc.receiver);
        write!(self.out, ".{}", escape_zig(&snake_to_camel(&emc.method.to_string()))).unwrap();
        write!(self.out, "(").unwrap();
        let refs: Vec<(usize, Vec<usize>)> = self.scip.symbol_at(&emc.method.span().into())
            .and_then(|s| self.generic_fns.get(s))
            .map(|gf| gf.param_arg_index.iter().map(|r| (r.arg, r.path.clone())).collect())
            .unwrap_or_default();
        let mut first = true;
        for (arg_idx, path) in refs {
            let syn::Expr::Path(ap) = &emc.args[arg_idx] else { continue };
            let aident = &ap.path.segments.last().unwrap().ident;
            let Some(ty) = self.scip.type_at(&aident.span().into()) else { continue };
            let Some(peeled) = peel_type(&ty, &path) else { continue };
            if !first {
                write!(self.out, ", ").unwrap();
            }
            first = false;
            let peeled = peeled.clone();
            self.translate_type(&peeled);
        }
        for arg in emc.args.iter() {
            if !first {
                write!(self.out, ", ").unwrap();
            }
            first = false;
            self.translate_expr(arg);
        }
        write!(self.out, ")").unwrap();
    }

    fn translate_struct_expr(&mut self, es: &syn::ExprStruct) {
        if matches!(self.path_mode(&es.path), PathMode::EnumVariant) {
            let variant = camel_to_snake(&es.path.segments.last().unwrap().ident.to_string());
            write!(self.out, ".{{ .{} = .{{ ", variant).unwrap();
            for (i, field) in es.fields.iter().enumerate() {
                if i > 0 {
                    write!(self.out, ", ").unwrap();
                }
                if let syn::Member::Named(ident) = &field.member {
                    write!(self.out, ".{} = ", ident).unwrap();
                }
                self.translate_expr(&field.expr);
            }
            write!(self.out, " }} }}").unwrap();
            return;
        }
        self.translate_path(&es.path, PathMode::Normal);
        write!(self.out, "{{ ").unwrap();
        for (i, field) in es.fields.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ").unwrap();
            }
            match &field.member {
                syn::Member::Named(ident) => write!(self.out, ".{} = ", ident).unwrap(),
                syn::Member::Unnamed(index) => write!(self.out, ".{} = ", index.index).unwrap(),
            }
            self.translate_expr(&field.expr);
        }
        write!(self.out, " }}").unwrap();
    }

    fn translate_tuple(&mut self, et: &syn::ExprTuple) {
        write!(self.out, ".{{ ").unwrap();
        for (i, elem) in et.elems.iter().enumerate() {
            if i > 0 {
                write!(self.out, ", ").unwrap();
            }
            self.translate_expr(elem);
        }
        write!(self.out, " }}").unwrap();
    }

    fn translate_while(&mut self, ew: &syn::ExprWhile) {
        write!(self.out, "while (").unwrap();
        self.translate_expr(&ew.cond);
        write!(self.out, ") ").unwrap();
        self.translate_block(&ew.body);
    }
}
