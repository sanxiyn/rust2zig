use super::Translator;

#[derive(PartialEq)]
enum Sort {
    Number,
    Aggregate,
    Other,
    Unknown,
}

impl Translator {
    pub fn equality(&self, left: &syn::Expr, right: &syn::Expr) -> &'static str {
        let left = self.expr_sort(left);
        let right = self.expr_sort(right);
        match (left, right) {
            (Sort::Number, _)
            | (_, Sort::Number) => "=",
            (Sort::Aggregate, _)
            | (_, Sort::Aggregate) => "equalp",
            _ => "equal",
        }
    }

    fn expr_sort(&self, expr: &syn::Expr) -> Sort {
        match expr {
            syn::Expr::Lit(el) => match el.lit {
                syn::Lit::Float(_) | syn::Lit::Int(_) => Sort::Number,
                _ => Sort::Other,
            },
            syn::Expr::Call(ec) => match &*ec.func {
                syn::Expr::Path(ep)
                    if self.check_moniker(&ep.path, "core::option::Option::Some") =>
                {
                    Sort::Other
                }
                _ => {
                    match self.expr_ty(expr) {
                        Some(ty) => self.type_sort(&ty),
                        None => Sort::Unknown,
                    }
                }
            },
            syn::Expr::Path(ep)
                if self.check_moniker(&ep.path, "core::option::Option::None") =>
            {
                Sort::Other
            }
            syn::Expr::Paren(ep) => self.expr_sort(&ep.expr),
            syn::Expr::Reference(er) => self.expr_sort(&er.expr),
            syn::Expr::Unary(eu) if matches!(eu.op, syn::UnOp::Deref(_)) => self.expr_sort(&eu.expr),
            _ => {
                match self.expr_ty(expr) {
                    Some(ty) => self.type_sort(&ty),
                    None => Sort::Unknown,
                }
            }
        }
    }

    fn type_sort(&self, ty: &syn::Type) -> Sort {
        match ty {
            syn::Type::Array(_) | syn::Type::Slice(_) => Sort::Aggregate,
            syn::Type::Paren(tp) => self.type_sort(&tp.elem),
            syn::Type::Path(tp) => {
                let name = tp.path.segments.last().unwrap().ident.to_string();
                if is_number(&name) {
                    return Sort::Number;
                }
                match name.as_str() {
                    "Vec" => Sort::Aggregate,
                    "Option" | "Result" | "String" | "bool" | "char" | "str" => Sort::Other,
                    _ if self.structs.contains_key(&name)
                        || self.is_data_enum(&name) =>
                    {
                        Sort::Aggregate
                    }
                    _ => Sort::Unknown
                }
            }
            syn::Type::Reference(tr) => self.type_sort(&tr.elem),
            syn::Type::Tuple(_) => Sort::Other,
            _ => Sort::Unknown
        }
    }

    fn is_data_enum(&self, name: &str) -> bool {
        self.enums.get(name).is_some_and(|variants| {
            variants.iter().any(|variant| self.structs.contains_key(variant))
        })
    }
}

fn is_number(name: &str) -> bool {
    matches!(
        name,
        "f32" | "f64"
            | "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
            | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
    )
}
