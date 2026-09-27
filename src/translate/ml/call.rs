use crate::ast::ml::Expression;
use crate::translate::name::escape_ml;
use super::integer::{IntRepr, WRAPPING, integer_width, mask, shift_in_range};
use super::{Translator, apply, qualified, unit};

impl Translator {
    pub fn translate_call(&self, ec: &syn::ExprCall) -> Expression {
        if let syn::Expr::Path(ep) = &*ec.func {
            if self.is_variant(&ep.path) {
                return self.translate_call_constructor(ec, &ep.path);
            }
            if self.check_moniker(&ep.path, "alloc::boxed::Box::new") {
                return self.translate_expr(&ec.args[0]);
            }
        }
        let func = self.translate_expr(&ec.func);
        let mut args = vec![];
        for arg in &ec.args {
            let arg = self.translate_expr(arg);
            args.push(arg);
        }
        if args.is_empty() {
            args.push(unit());
        }
        Expression::Apply(Box::new(func), args)
    }

    fn translate_call_constructor(&self, ec: &syn::ExprCall, path: &syn::Path) -> Expression {
        let arg = match ec.args.len() {
            0 => None,
            1 => Some(Box::new(self.translate_expr(&ec.args[0]))),
            _ => {
                let mut elements = vec![];
                for arg in &ec.args {
                    let element = self.translate_expr(arg);
                    elements.push(element);
                }
                Some(Box::new(Expression::Tuple(elements)))
            }
        };
        Expression::Construct(self.variant_name(path), arg)
    }

    pub fn translate_method_call(&self, emc: &syn::ExprMethodCall) -> Expression {
        let receiver = self.translate_expr(&emc.receiver);
        let str_len = self.check_moniker_ident(&emc.method, "core::str::len");
        if str_len || self.check_moniker_ident(&emc.method, "core::slice::len") {
            let string = str_len || self.is_string_expr(&emc.receiver);
            let module = if string { "String" } else { "Array" };
            return Expression::Apply(Box::new(qualified(module, "length")), vec![receiver]);
        }
        if self.check_moniker_ident(&emc.method, "alloc::vec::Vec::len") {
            return Expression::Apply(Box::new(qualified("Dynarray", "length")), vec![receiver]);
        }
        if self.check_moniker_ident(&emc.method, "alloc::vec::Vec::pop") {
            let func = qualified("Dynarray", "pop_last_opt");
            return Expression::Apply(Box::new(func), vec![receiver]);
        }
        if self.check_moniker_ident(&emc.method, "alloc::vec::Vec::push") {
            let value = self.translate_expr(&emc.args[0]);
            let func = qualified("Dynarray", "add_last");
            return Expression::Apply(Box::new(func), vec![receiver, value]);
        }
        if self.check_moniker_ident(&emc.method, "core::str::as_bytes") {
            return receiver;
        }
        if self.check_moniker_ident(&emc.method, "core::char::len_utf8") {
            let func = qualified("Uchar", "utf_8_byte_length");
            return Expression::Apply(Box::new(func), vec![receiver]);
        }
        if let Some(expr) = self.translate_char_at(emc) {
            return expr;
        }
        if self.check_moniker_ident(&emc.method, "core::option::Option::unwrap") {
            return Expression::Apply(Box::new(qualified("Option", "get")), vec![receiver]);
        }
        if self.check_moniker_ident(&emc.method, "core::result::Result::unwrap") {
            return Expression::Apply(Box::new(qualified("Result", "get_ok")), vec![receiver]);
        }
        if let Some(expr) = self.translate_wrapping(emc) {
            return expr;
        }
        if let Some(expr) = self.translate_wrapping_shl(emc) {
            return expr;
        }
        if let Some(expr) = self.translate_rotate(emc) {
            return expr;
        }
        let module = self.expr_module(&emc.receiver);
        let method = escape_ml(&emc.method.to_string());
        let func = Expression::Ident(self.qualify(module, method));
        let mut args = vec![receiver];
        for arg in &emc.args {
            let arg = self.translate_expr(arg);
            args.push(arg);
        }
        Expression::Apply(Box::new(func), args)
    }

    fn translate_wrapping(&self, emc: &syn::ExprMethodCall) -> Option<Expression> {
        let repr = self.name_repr(&self.self_integer_type(&emc.method)?);
        let module = repr.module()?;
        let (_, op) = WRAPPING.iter().find(|(m, _)| self.check_moniker_ident(&emc.method, m))?;
        let left = self.translate_expr(&emc.receiver);
        let right = self.translate_int_operand(emc.args.first()?, repr);
        Some(Expression::Apply(Box::new(qualified(module, op)), vec![left, right]))
    }

    fn translate_wrapping_shl(&self, emc: &syn::ExprMethodCall) -> Option<Expression> {
        if !self.check_moniker_ident(&emc.method, "core::num::wrapping_shl") {
            return None;
        }
        let name = self.self_integer_type(&emc.method)?;
        let bits = integer_width(&name)?;
        let arg = &emc.args[0];
        let value = self.translate_expr(&emc.receiver);
        let mut amount = self.shift_amount(arg);
        if !shift_in_range(arg, bits) {
            amount = mask(amount, IntRepr::Int, bits.trailing_zeros());
        }
        match self.name_repr(&name).module() {
            Some(module) => Some(Expression::Apply(Box::new(qualified(module, "shift_left")), vec![value, amount])),
            None => Some(apply("lsl", vec![value, amount])),
        }
    }
}
