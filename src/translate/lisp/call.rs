use lexpr::{Value, sexp};

use crate::translate::ty::unsigned_bits;
use super::{Translator, call, is_place, todo, type_ident};

impl Translator {
    pub fn translate_method_call(&self, emc: &syn::ExprMethodCall) -> Value {
        let receiver = self.translate_expr(&emc.receiver);
        if self.check_moniker_ident(&emc.method, "core::cell::Cell::get") {
            return receiver;
        }
        if self.check_moniker_ident(&emc.method, "core::cell::Cell::set") {
            let value = self.translate_expr(&emc.args[0]);
            return sexp!((setf ,receiver ,value));
        }
        if self.check_moniker_ident(&emc.method, "core::option::Option::unwrap") {
            let message = Value::string("called Option::unwrap() on a None value");
            let panic = call("error", vec![message]);
            return sexp!((or ,receiver ,panic));
        }
        if self.check_moniker_ident(&emc.method, "core::slice::len")
            || self.check_moniker_ident(&emc.method, "alloc::vec::Vec::len")
        {
            return sexp!((length ,receiver));
        }
        if self.check_moniker_ident(&emc.method, "alloc::vec::Vec::pop") {
            return sexp!((# "vector-pop" ,receiver));
        }
        if self.check_moniker_ident(&emc.method, "alloc::vec::Vec::push") {
            let value = self.translate_expr(&emc.args[0]);
            return sexp!((# "vector-push-extend" ,value ,receiver));
        }
        if let Some(value) = self.translate_wrapping(emc) {
            return value;
        }
        if let Some(value) = self.translate_rotate(emc) {
            return value;
        }
        if let Some(ty) = self.expr_ty(&emc.receiver).as_ref().and_then(type_ident) {
            if self.structs.contains_key(&ty) || self.enums.contains_key(&ty) {
                let mut args = vec![receiver];
                for arg in &emc.args {
                    let arg = self.translate_expr(arg);
                    args.push(arg);
                }
                return call(&self.method_name(&ty, &emc.method), args);
            }
        }
        todo("method call")
    }

    fn translate_wrapping(&self, emc: &syn::ExprMethodCall) -> Option<Value> {
        let bits = unsigned_bits(&self.expr_ty(&emc.receiver)?)?;
        let op = self.wrapping_op(&emc.method)?;
        let left = self.translate_expr(&emc.receiver);
        let right = self.translate_expr(&emc.args[0]);
        let value = call(op, vec![left, right]);
        Some(sexp!((ldb (byte ,bits 0) ,value)))
    }

    fn translate_rotate(&self, emc: &syn::ExprMethodCall) -> Option<Value> {
        if !self.check_moniker_ident(&emc.method, "core::num::rotate_right") {
            return None;
        }
        if !is_place(&emc.receiver) || !is_place(&emc.args[0]) {
            return None;
        }
        let bits = unsigned_bits(&self.expr_ty(&emc.receiver)?)?;
        let value = self.translate_expr(&emc.receiver);
        let amount = self.translate_expr(&emc.args[0]);
        let low = {
            let (value, amount) = (value.clone(), amount.clone());
            sexp!((ash ,value (- ,amount)))
        };
        let high = {
            let shifted = sexp!((ash ,value (- ,bits ,amount)));
            sexp!((ldb (byte ,bits 0) ,shifted))
        };
        Some(sexp!((logior ,low ,high)))
    }

    pub fn wrapping_op(&self, method: &syn::Ident) -> Option<&'static str> {
        const WRAPPING: &[(&str, &str)] = &[
            ("core::num::wrapping_add", "+"),
            ("core::num::wrapping_mul", "*"),
            ("core::num::wrapping_shl", "ash"),
            ("core::num::wrapping_sub", "-"),
        ];
        WRAPPING.iter()
            .find(|(moniker, _)| self.check_moniker_ident(method, moniker))
            .map(|(_, op)| *op)
    }
}
