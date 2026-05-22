use std::fmt::Write;

use super::{PathMode, Rust2Zig};
use super::name::camel_to_snake;

impl Rust2Zig {
    pub fn translate_expr(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Array(ea) => self.translate_array(ea),
            syn::Expr::Assign(ea) => self.translate_assign(ea),
            syn::Expr::Binary(eb) => self.translate_binary(eb),
            syn::Expr::Break(eb) => self.translate_break(eb),
            syn::Expr::Call(ec) => self.translate_call(ec),
            syn::Expr::Continue(ec) => self.translate_continue(ec),
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
            syn::Expr::Unary(eu) => self.translate_unary(eu),
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
        use syn::spanned::Spanned;
        let types = self.scip.binary_type_at(&eb.op.span().into());
        let (left_ref, right_ref) = match &types {
            Some((l, r)) => (matches!(l, syn::Type::Reference(_)), matches!(r, syn::Type::Reference(_))),
            None => (false, false),
        };
        if matches!(eb.op, syn::BinOp::Rem(_)) {
            let signed = match &types {
                Some((l, _)) => is_signed_int(peel_ref(l)),
                None => false,
            };
            if signed {
                write!(self.out, "@rem(").unwrap();
                self.translate_operand(&eb.left, left_ref);
                write!(self.out, ", ").unwrap();
                self.translate_operand(&eb.right, right_ref);
                write!(self.out, ")").unwrap();
                return;
            }
        }
        self.translate_operand(&eb.left, left_ref);
        let op = self.translate_binop(&eb.op);
        write!(self.out, " {} ", op).unwrap();
        self.translate_operand(&eb.right, right_ref);
    }

    fn translate_operand(&mut self, expr: &syn::Expr, deref: bool) {
        self.translate_expr(expr);
        if deref {
            write!(self.out, ".*").unwrap();
        }
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

    fn translate_field(&mut self, ef: &syn::ExprField) {
        self.translate_expr(&ef.base);
        match &ef.member {
            syn::Member::Named(ident) => write!(self.out, ".{}", ident).unwrap(),
            syn::Member::Unnamed(index) => write!(self.out, ".{}", index.index).unwrap(),
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
        let deref = self.match_needs_deref(em);
        self.translate_expr(&em.expr);
        if deref {
            write!(self.out, ".*").unwrap();
        }
        write!(self.out, ") ").unwrap();
        self.translate_match_arms(&em.arms);
    }

    fn match_needs_deref(&self, em: &syn::ExprMatch) -> bool {
        if em.arms.iter().any(|arm| matches!(arm.pat, syn::Pat::Reference(_))) {
            return false;
        }
        let syn::Expr::Path(ep) = &*em.expr else { return false };
        let Some(ident) = ep.path.get_ident() else { return false };
        let Some(ty) = self.scip.type_at(&ident.span().into()) else { return false };
        matches!(ty, syn::Type::Reference(_))
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

    fn translate_unary(&mut self, eu: &syn::ExprUnary) {
        match eu.op {
            syn::UnOp::Deref(_) => {
                self.translate_expr(&eu.expr);
                write!(self.out, ".*").unwrap();
            }
            _ => {
                write!(self.out, "/* TODO: unary */").unwrap();
            }
        }
    }
}

fn peel_ref(ty: &syn::Type) -> &syn::Type {
    match ty {
        syn::Type::Reference(tr) => &tr.elem,
        _ => ty,
    }
}

fn is_signed_int(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else { return false };
    let Some(seg) = tp.path.segments.last() else { return false };
    matches!(seg.ident.to_string().as_str(), "i8" | "i16" | "i32" | "i64" | "i128" | "isize")
}
