use lexpr::{Value, sexp};

use super::{Translator, call, list_of, symbol, todo};

impl Translator {
    pub fn translate_macro(&self, mac: &syn::Macro) -> Option<Value> {
        if self.check_moniker(&mac.path, "std::macros::assert") {
            Some(self.translate_assert(mac))
        } else if self.check_moniker(&mac.path, "core::macros::assert_eq") {
            Some(self.translate_assert_eq(mac))
        } else if self.check_moniker(&mac.path, "std::macros::panic") {
            Some(self.translate_panic(mac))
        } else if self.check_moniker(&mac.path, "std::macros::println") {
            Some(self.translate_println(mac))
        } else if self.check_moniker(&mac.path, "alloc::macros::vec") {
            Some(self.translate_vec(mac))
        } else {
            None
        }
    }

    fn translate_assert(&self, mac: &syn::Macro) -> Value {
        let args = macro_args(mac);
        let value = self.translate_expr(&args[0]);
        sexp!((assert ,value))
    }

    fn translate_assert_eq(&self, mac: &syn::Macro) -> Value {
        let args = macro_args(mac);
        let (left, right) = (&args[0], &args[1]);
        let op = self.equality(left, right);
        let left = self.translate_expr(left);
        let right = self.translate_expr(right);
        let equality = call(op, vec![left, right]);
        sexp!((assert ,equality))
    }

    fn translate_panic(&self, mac: &syn::Macro) -> Value {
        let args = macro_args(mac);
        if args.len() != 1 {
            return todo("panic");
        }
        let syn::Expr::Lit(el) = &args[0] else { return todo("panic") };
        let syn::Lit::Str(ls) = &el.lit else { return todo("panic") };
        call("error", vec![Value::string(ls.value().replace('~', "~~"))])
    }

    fn translate_println(&self, _mac: &syn::Macro) -> Value {
        todo("println")
    }

    fn translate_vec(&self, mac: &syn::Macro) -> Value {
        let args = macro_args(mac);
        let size = args.len() as u64;
        let mut items = vec![];
        items.extend(sexp!((,size # ":adjustable" t # ":fill-pointer" t)).to_vec().unwrap());
        if !args.is_empty() {
            let elements = args.iter().map(|arg| self.translate_expr(arg)).collect();
            items.push(symbol(":initial-contents"));
            items.push(list_of("list", elements));
        }
        call("make-array", items)
    }
}

fn macro_args(mac: &syn::Macro) -> Vec<syn::Expr> {
    use syn::parse::Parser;
    use syn::punctuated::Punctuated;
    let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
    let args = parser.parse2(mac.tokens.clone()).expect("failed to parse macro args");
    args.into_iter().collect()
}
