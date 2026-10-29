use std::collections::HashSet;

use syn::visit::Visit;

use crate::ast::ml::{Constant, Expression};
use crate::translate::ty::{expr_type, peel_ref};
use super::{Translator, apply, qualified, todo};

pub const WRAPPING: &[(&str, &str)] = &[
    ("core::num::wrapping_add", "add"),
    ("core::num::wrapping_mul", "mul"),
    ("core::num::wrapping_sub", "sub"),
];

const SELF_RETURNING: &[&str] = &[
    "core::num::wrapping_add",
    "core::num::wrapping_mul",
    "core::num::wrapping_shl",
    "core::num::wrapping_sub",
];

#[derive(Clone, Copy, PartialEq)]
pub enum IntRepr {
    Int,
    Int32,
    Int64,
}

impl IntRepr {
    pub fn module(self) -> Option<&'static str> {
        match self {
            IntRepr::Int => None,
            IntRepr::Int32 => Some("Int32"),
            IntRepr::Int64 => Some("Int64"),
        }
    }

    pub fn ocaml_type(self) -> &'static str {
        match self {
            IntRepr::Int => "int",
            IntRepr::Int32 => "int32",
            IntRepr::Int64 => "int64",
        }
    }

    fn suffix(self) -> Option<char> {
        match self {
            IntRepr::Int => None,
            IntRepr::Int32 => Some('l'),
            IntRepr::Int64 => Some('L'),
        }
    }

    fn bits(self) -> u32 {
        match self {
            IntRepr::Int => 63,
            IntRepr::Int32 => 32,
            IntRepr::Int64 => 64,
        }
    }
}

pub fn integer_type_name(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(tp) = peel_ref(ty) else { return None };
    let name = tp.path.segments.last()?.ident.to_string();
    is_integer_type(&name).then_some(name)
}

pub fn is_integer_type(name: &str) -> bool {
    matches!(name,
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
        | "u8" | "u16" | "u32" | "u64" | "u128" | "usize")
}

fn is_signed(name: &str) -> bool {
    name.starts_with('i')
}

pub fn integer_width(name: &str) -> Option<u32> {
    match name {
        "i8" | "u8" => Some(8),
        "i16" | "u16" => Some(16),
        "i32" | "u32" => Some(32),
        "i64" | "u64" => Some(64),
        "i128" | "u128" => Some(128),
        _ => None,
    }
}

pub fn literal_digits(li: &syn::LitInt, repr: IntRepr) -> String {
    let token = li.token().to_string();
    let digits = token.strip_suffix(li.suffix()).unwrap_or(&token).to_string();
    if repr == IntRepr::Int || digits.starts_with("0x") || digits.starts_with("0o") || digits.starts_with("0b") {
        return digits;
    }
    let Ok(value) = li.base10_parse::<u128>() else { return digits };
    if value >= 1 << (repr.bits() - 1) {
        return format!("0x{:x}", value);
    }
    digits
}

enum Form {
    Fun(&'static str),
    Op(&'static str),
    Compare(&'static str),
}

impl Translator {
    pub fn collect_widths(&mut self, file: &syn::File) {
        struct Collect<'a> {
            translator: &'a Translator,
            escalated: &'a mut HashSet<String>,
        }

        impl<'ast> Visit<'ast> for Collect<'_> {
            fn visit_expr_method_call(&mut self, emc: &'ast syn::ExprMethodCall) {
                let wrapping = WRAPPING.iter()
                    .any(|(m, _)| self.translator.check_moniker_ident(&emc.method, m));
                if wrapping {
                    if let Some(name) = self.translator.self_integer_type(&emc.method) {
                        self.escalated.insert(name);
                    }
                }
                syn::visit::visit_expr_method_call(self, emc);
            }
        }

        let mut escalated = HashSet::new();
        Collect { translator: self, escalated: &mut escalated }.visit_file(file);
        self.escalated = escalated;
    }

    pub fn self_integer_type(&self, method: &syn::Ident) -> Option<String> {
        integer_type_name(&self.scip.self_type_at(&method.span().into())?)
    }

    pub fn name_repr(&self, name: &str) -> IntRepr {
        match name {
            "i64" | "u64" => IntRepr::Int64,
            "isize" | "usize" => IntRepr::Int,
            _ if self.escalated.contains(name) => IntRepr::Int32,
            _ => IntRepr::Int,
        }
    }

    pub fn declared_repr(&self, ident: &syn::Ident) -> Option<IntRepr> {
        let ty = self.scip.type_at(&ident.span().into())?;
        Some(self.name_repr(&integer_type_name(&ty)?))
    }

    pub fn expr_repr(&self, expr: &syn::Expr) -> Option<(String, IntRepr)> {
        let name = expr_type(&self.scip, expr)
            .as_ref()
            .and_then(integer_type_name)
            .or_else(|| self.self_returning_type(expr))?;
        let repr = self.name_repr(&name);
        Some((name, repr))
    }

    fn self_returning_type(&self, expr: &syn::Expr) -> Option<String> {
        let syn::Expr::MethodCall(emc) = expr else { return None };
        SELF_RETURNING.iter()
            .any(|moniker| self.check_moniker_ident(&emc.method, moniker))
            .then(|| self.self_integer_type(&emc.method))?
    }

    pub(super) fn translate_int_operand(&self, expr: &syn::Expr, repr: IntRepr) -> Expression {
        if let syn::Expr::Lit(el) = expr {
            if let syn::Lit::Int(li) = &el.lit {
                return Expression::Constant(Constant::Integer(literal_digits(li, repr), repr.suffix()));
            }
        }
        self.translate_expr(expr)
    }

    pub(super) fn translate_int_binary(&self, eb: &syn::ExprBinary) -> Option<Expression> {
        let (name, repr) = self.expr_repr(&eb.left).or_else(|| self.expr_repr(&eb.right))?;
        let module = repr.module()?;
        let signed = is_signed(&name);
        let form = match eb.op {
            syn::BinOp::Add(_) => Form::Fun("add"),
            syn::BinOp::BitAnd(_) => Form::Fun("logand"),
            syn::BinOp::BitOr(_) => Form::Fun("logor"),
            syn::BinOp::BitXor(_) => Form::Fun("logxor"),
            syn::BinOp::Div(_) => Form::Fun(if signed { "div" } else { "unsigned_div" }),
            syn::BinOp::Eq(_) => Form::Op("="),
            syn::BinOp::Ge(_) if signed => Form::Op(">="),
            syn::BinOp::Ge(_) => Form::Compare(">="),
            syn::BinOp::Gt(_) if signed => Form::Op(">"),
            syn::BinOp::Gt(_) => Form::Compare(">"),
            syn::BinOp::Le(_) if signed => Form::Op("<="),
            syn::BinOp::Le(_) => Form::Compare("<="),
            syn::BinOp::Lt(_) if signed => Form::Op("<"),
            syn::BinOp::Lt(_) => Form::Compare("<"),
            syn::BinOp::Mul(_) => Form::Fun("mul"),
            syn::BinOp::Ne(_) => Form::Op("<>"),
            syn::BinOp::Rem(_) => Form::Fun(if signed { "rem" } else { "unsigned_rem" }),
            syn::BinOp::Shl(_) => Form::Fun("shift_left"),
            syn::BinOp::Shr(_) => Form::Fun(if signed { "shift_right" } else { "shift_right_logical" }),
            syn::BinOp::Sub(_) => Form::Fun("sub"),
            _ => return None,
        };
        let left = self.translate_int_operand(&eb.left, repr);
        let right = match eb.op {
            syn::BinOp::Shl(_) | syn::BinOp::Shr(_) => self.shift_amount(&eb.right),
            _ => self.translate_int_operand(&eb.right, repr),
        };
        Some(match form {
            Form::Fun(fun) => Expression::Apply(Box::new(qualified(module, fun)), vec![left, right]),
            Form::Op(op) => apply(op, vec![left, right]),
            Form::Compare(op) => {
                let compare = Expression::Apply(
                    Box::new(qualified(module, "unsigned_compare")),
                    vec![left, right],
                );
                apply(op, vec![compare, Expression::Constant(Constant::Integer("0".to_string(), None))])
            }
        })
    }

    pub(super) fn translate_int_cast(&self, ec: &syn::ExprCast) -> Option<Expression> {
        let target = integer_type_name(&ec.ty)?;
        let (source, from) = self.expr_repr(&ec.expr)?;
        let to = self.name_repr(&target);
        let mut expr = self.translate_expr(&ec.expr);
        expr = match (from.module(), to.module()) {
            (None, None) => expr,
            (Some(a), Some(b)) if a == b => expr,
            (None, Some(b)) => Expression::Apply(Box::new(qualified(b, "of_int")), vec![expr]),
            (Some(a), None) => {
                let converted = Expression::Apply(Box::new(qualified(a, "to_int")), vec![expr]);
                match (is_signed(&source), integer_width(&source)) {
                    (false, Some(bits)) if bits < IntRepr::Int.bits() => mask(converted, IntRepr::Int, bits),
                    _ => converted,
                }
            }
            (Some("Int32"), Some("Int64")) => {
                let converted = Expression::Apply(Box::new(qualified("Int64", "of_int32")), vec![expr]);
                match is_signed(&source) {
                    false => mask(converted, IntRepr::Int64, 32),
                    true => converted,
                }
            }
            (Some("Int64"), Some("Int32")) => {
                Expression::Apply(Box::new(qualified("Int64", "to_int32")), vec![expr])
            }
            _ => return None,
        };
        match integer_width(&target) {
            Some(bits) if bits < to.bits() => {
                if is_signed(&target) {
                    return None;
                }
                Some(mask(expr, to, bits))
            }
            _ => Some(expr),
        }
    }

    pub(super) fn translate_int_not(&self, expr: &syn::Expr) -> Option<Expression> {
        let (name, repr) = self.expr_repr(expr)?;
        let inner = self.translate_expr(expr);
        let complement = match repr.module() {
            Some(module) => Expression::Apply(Box::new(qualified(module, "lognot")), vec![inner]),
            None => apply("lnot", vec![inner]),
        };
        if is_signed(&name) {
            return Some(complement);
        }
        Some(match integer_width(&name) {
            Some(bits) if bits < repr.bits() => mask(complement, repr, bits),
            Some(_) => complement,
            None => todo("lnot"),
        })
    }

    pub(super) fn translate_rotate(&self, emc: &syn::ExprMethodCall) -> Option<Expression> {
        if !self.check_moniker_ident(&emc.method, "core::num::rotate_right") {
            return None;
        }
        let name = self.self_integer_type(&emc.method)?;
        let bits = integer_width(&name)?;
        let repr = self.name_repr(&name);
        if is_signed(&name) || repr != IntRepr::Int || bits >= repr.bits() {
            return None;
        }
        let arg = &emc.args[0];
        let width = Expression::Constant(Constant::Integer(bits.to_string(), None));
        let right = apply("lsr", vec![
            self.translate_expr(&emc.receiver),
            self.shift_amount(arg),
        ]);
        let left = apply("lsl", vec![
            self.translate_expr(&emc.receiver),
            apply("-", vec![width, self.shift_amount(arg)]),
        ]);
        Some(mask(apply("lor", vec![right, left]), repr, bits))
    }

    pub fn shift_amount(&self, expr: &syn::Expr) -> Expression {
        let amount = self.translate_int_operand(expr, IntRepr::Int);
        match self.expr_repr(expr) {
            Some((_, repr)) => match repr.module() {
                Some(module) => Expression::Apply(Box::new(qualified(module, "to_int")), vec![amount]),
                None => amount,
            },
            None => amount,
        }
    }
}

pub fn mask(expr: Expression, repr: IntRepr, bits: u32) -> Expression {
    match repr.module() {
        None => {
            let literal = Expression::Constant(Constant::Integer(
                format!("0x{:x}", (1u128 << bits) - 1),
                None,
            ));
            apply("land", vec![expr, literal])
        }
        Some(module) => {
            let literal = Expression::Constant(Constant::Integer(
                format!("0x{:x}", (1u128 << bits) - 1),
                repr.suffix(),
            ));
            Expression::Apply(Box::new(qualified(module, "logand")), vec![expr, literal])
        }
    }
}

pub fn shift_in_range(expr: &syn::Expr, bits: u32) -> bool {
    let syn::Expr::Lit(el) = expr else { return false };
    let syn::Lit::Int(li) = &el.lit else { return false };
    li.base10_parse::<u32>().is_ok_and(|amount| amount < bits)
}
