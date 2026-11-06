use crate::ast::ml::{Case, Constant, Expression, Longident, Pattern};
use crate::translate::name::escape_ml;
use super::{Translator, apply, qualified, todo, unit, unit_pat};
use super::integer::{IntRepr, literal_digits};
use super::pat::pat_guard;

impl Translator {
    pub fn translate_expr(&self, expr: &syn::Expr) -> Expression {
        match expr {
            syn::Expr::Array(ea) => self.translate_array(ea),
            syn::Expr::Assign(ea) => self.translate_assign(ea),
            syn::Expr::Binary(eb) => self.translate_binary(eb),
            syn::Expr::Block(eb) => self.translate_block(&eb.block),
            syn::Expr::Break(eb) => self.translate_break(eb),
            syn::Expr::Call(ec) => self.translate_call(ec),
            syn::Expr::Cast(ec) => self.translate_cast(ec),
            syn::Expr::Continue(ec) => self.translate_continue(ec),
            syn::Expr::Closure(ec) => {
                let (params, body) = self.closure_parts(ec);
                Expression::Function(params, Box::new(body))
            }
            syn::Expr::Field(ef) => self.translate_field(ef),
            syn::Expr::ForLoop(ef) => self.translate_for_loop(ef),
            syn::Expr::If(ei) => self.translate_if(ei),
            syn::Expr::Index(ei) => self.translate_index(ei),
            syn::Expr::Lit(el) => self.translate_lit(el),
            syn::Expr::Loop(el) => self.translate_loop(el),
            syn::Expr::Macro(em) => self.translate_macro(&em.mac)
                .unwrap_or_else(|| todo("macro")),
            syn::Expr::Match(em) => self.translate_match(em),
            syn::Expr::MethodCall(emc) => self.translate_method_call(emc),
            syn::Expr::Paren(ep) => self.translate_paren(ep),
            syn::Expr::Path(ep) => self.translate_path(&ep.path),
            syn::Expr::Reference(er) => self.translate_reference(er),
            syn::Expr::Return(er) => self.translate_return(er),
            syn::Expr::Struct(es) => self.translate_struct_expr(es),
            syn::Expr::Tuple(et) => self.translate_tuple(et),
            syn::Expr::Unary(eu) => self.translate_unary(eu),
            syn::Expr::While(ew) => self.translate_while(ew),
            _ => todo("expr"),
        }
    }

    fn translate_array(&self, ea: &syn::ExprArray) -> Expression {
        let mut elements = vec![];
        for elem in &ea.elems {
            let element = self.translate_expr(elem);
            elements.push(element);
        }
        Expression::Array(elements)
    }

    fn translate_assign(&self, ea: &syn::ExprAssign) -> Expression {
        let right = self.translate_expr(&ea.right);
        match &*ea.left {
            syn::Expr::Field(ef) => {
                let base = self.translate_expr(&ef.base);
                let field = member_name(&ef.member);
                Expression::SetField(Box::new(base), Longident::Lident(field), Box::new(right))
            }
            syn::Expr::Path(ep) => {
                let ident = &ep.path.segments.last().unwrap().ident;
                let left = Expression::Ident(Longident::Lident(escape_ml(&ident.to_string())));
                apply(":=", vec![left, right])
            }
            _ => apply(":=", vec![self.translate_expr(&ea.left), right]),
        }
    }

    fn translate_binary(&self, eb: &syn::ExprBinary) -> Expression {
        if let Some(expr) = self.translate_int_binary(eb) {
            return expr;
        }
        let left = self.translate_expr(&eb.left);
        let right = self.translate_expr(&eb.right);
        let op = match eb.op {
            syn::BinOp::Add(_) => "+",
            syn::BinOp::And(_) => "&&",
            syn::BinOp::BitAnd(_) => "land",
            syn::BinOp::BitOr(_) => "lor",
            syn::BinOp::BitXor(_) => "lxor",
            syn::BinOp::Div(_) => "/",
            syn::BinOp::Eq(_) => "=",
            syn::BinOp::Ge(_) => ">=",
            syn::BinOp::Gt(_) => ">",
            syn::BinOp::Le(_) => "<=",
            syn::BinOp::Lt(_) => "<",
            syn::BinOp::Mul(_) => "*",
            syn::BinOp::Ne(_) => "<>",
            syn::BinOp::Or(_) => "||",
            syn::BinOp::Rem(_) => "mod",
            syn::BinOp::Shl(_) => "lsl",
            syn::BinOp::Sub(_) => "-",
            _ => return todo("binop"),
        };
        apply(op, vec![left, right])
    }

    fn translate_cast(&self, ec: &syn::ExprCast) -> Expression {
        if let Some(expr) = self.translate_int_cast(ec) {
            return expr;
        }
        todo("cast")
    }

    pub fn closure_parts(&self, ec: &syn::ExprClosure) -> (Vec<Pattern>, Expression) {
        let mut params: Vec<Pattern> = ec.inputs.iter().map(|param| self.translate_pat(param)).collect();
        if params.is_empty() {
            params.push(unit_pat());
        }
        let body = match &*ec.body {
            syn::Expr::Block(eb) => self.translate_block(&eb.block),
            other => self.translate_expr(other),
        };
        (params, body)
    }

    fn translate_field(&self, ef: &syn::ExprField) -> Expression {
        let base = self.translate_expr(&ef.base);
        Expression::Field(Box::new(base), Longident::Lident(member_name(&ef.member)))
    }

    fn translate_index(&self, ei: &syn::ExprIndex) -> Expression {
        let base = self.translate_expr(&ei.expr);
        let index = self.translate_expr(&ei.index);
        if self.is_string_expr(&ei.expr) {
            return self.string_get(base, index);
        }
        if self.is_vec_expr(&ei.expr) {
            let get = qualified("Dynarray", "get");
            return Expression::Apply(Box::new(get), vec![base, index]);
        }
        Expression::ArrayGet(Box::new(base), Box::new(index))
    }

    fn translate_lit(&self, el: &syn::ExprLit) -> Expression {
        match &el.lit {
            syn::Lit::Bool(b) => Expression::Construct(Longident::Lident(b.value.to_string()), None),
            syn::Lit::Char(c) => {
                let c = c.value();
                let (name, arg) = if (c as u32) <= 0xFF {
                    ("of_char", Expression::Constant(Constant::Char(c)))
                } else {
                    let digits = format!("0x{:04X}", c as u32);
                    ("of_int", Expression::Constant(Constant::Integer(digits, None)))
                };
                Expression::Apply(Box::new(qualified("Uchar", name)), vec![arg])
            }
            syn::Lit::Int(i) => Expression::Constant(Constant::Integer(literal_digits(i, IntRepr::Int), None)),
            syn::Lit::Str(s) => Expression::Constant(Constant::String(s.value())),
            _ => todo("lit"),
        }
    }

    fn translate_match(&self, em: &syn::ExprMatch) -> Expression {
        let scrutinee = self.translate_expr(&em.expr);
        let mut cases = vec![];
        for arm in &em.arms {
            let (pat, guard) = pat_guard(&arm.pat);
            let case = Case {
                lhs: self.translate_pat(pat),
                guard: guard.map(|guard| self.translate_expr(guard)),
                rhs: self.translate_expr(&arm.body),
            };
            cases.push(case);
        }
        Expression::Match(Box::new(scrutinee), cases)
    }

    fn translate_paren(&self, ep: &syn::ExprParen) -> Expression {
        self.translate_expr(&ep.expr)
    }

    fn translate_reference(&self, er: &syn::ExprReference) -> Expression {
        self.translate_expr(&er.expr)
    }

    fn translate_return(&self, er: &syn::ExprReturn) -> Expression {
        let payload = if let Some(expr) = &er.expr {
            self.translate_expr(expr)
        } else {
            unit()
        };
        apply("raise", vec![Expression::Construct(Longident::Lident("Return".to_string()), Some(Box::new(payload)))])
    }

    fn translate_struct_expr(&self, es: &syn::ExprStruct) -> Expression {
        let mut fields = vec![];
        for field in &es.fields {
            if let syn::Member::Named(ident) = &field.member {
                let name = ident.to_string();
                let value = match self.declared_repr(ident) {
                    Some(repr) => self.translate_int_operand(&field.expr, repr),
                    None => self.translate_expr(&field.expr),
                };
                fields.push((Longident::Lident(escape_ml(&name)), value));
            }
        }
        let base = es.rest.as_ref().map(|rest| Box::new(self.translate_expr(rest)));
        if self.is_variant(&es.path) {
            let record = Expression::Record(fields, base);
            return Expression::Construct(self.variant_name(&es.path), Some(Box::new(record)));
        }
        let ident = &es.path.segments.last().unwrap().ident;
        if let Some((label, _)) = fields.first_mut() {
            if let Longident::Lident(name) = label {
                *label = self.qualify(self.type_module(ident), name.clone());
            }
        }
        Expression::Record(fields, base)
    }

    fn translate_tuple(&self, et: &syn::ExprTuple) -> Expression {
        if et.elems.is_empty() {
            return unit();
        }
        let mut elements = vec![];
        for elem in &et.elems {
            let element = self.translate_expr(elem);
            elements.push(element);
        }
        Expression::Tuple(elements)
    }

    fn translate_unary(&self, eu: &syn::ExprUnary) -> Expression {
        let expr = self.translate_expr(&eu.expr);
        match eu.op {
            syn::UnOp::Deref(_) => expr,
            syn::UnOp::Not(_) => {
                if let Some(expr) = self.translate_int_not(&eu.expr) {
                    expr
                } else {
                    apply("not", vec![expr])
                }
            }
            _ => todo("unary"),
        }
    }
}

fn member_name(member: &syn::Member) -> String {
    match member {
        syn::Member::Named(ident) => escape_ml(&ident.to_string()),
        syn::Member::Unnamed(index) => index.index.to_string(),
    }
}
