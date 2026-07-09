use crate::ast::zig::{Capture, Node, SwitchArm, SwitchBody, Var};
use crate::translate::name::camel_to_snake;
use super::{PathMode, Translator};
use super::pat::Accessor;

impl Translator {
    pub fn translate_expr(&self, expr: &syn::Expr) -> Node {
        match expr {
            syn::Expr::Array(ea) => self.translate_array(ea),
            syn::Expr::Assign(ea) => self.translate_assign(ea),
            syn::Expr::Binary(eb) => self.translate_binary(eb),
            syn::Expr::Block(eb) => self.translate_block_expr(&eb.block),
            syn::Expr::Break(eb) => self.translate_break(eb),
            syn::Expr::Call(ec) => self.translate_call(ec),
            syn::Expr::Continue(ec) => self.translate_continue(ec),
            syn::Expr::Field(ef) => self.translate_field(ef),
            syn::Expr::ForLoop(efl) => self.translate_for_loop(efl),
            syn::Expr::If(ei) => self.translate_if(ei),
            syn::Expr::Index(ei) => self.translate_index(ei),
            syn::Expr::Lit(el) => self.translate_lit(el),
            syn::Expr::Macro(em) => self.translate_macro(&em.mac)
                .unwrap_or_else(|| Node::Todo("macro".to_string())),
            syn::Expr::Match(em) => self.translate_match(em),
            syn::Expr::MethodCall(emc) => self.translate_method_call(emc),
            syn::Expr::Path(ep) => {
                let mode = self.path_mode(&ep.path);
                if matches!(mode, PathMode::EnumVariant) {
                    self.translate_path(&ep.path, mode)
                } else {
                    if self.check_moniker(&ep.path, "core::option::Option::None") {
                        Node::Identifier("null".to_string())
                    } else {
                        self.translate_path(&ep.path, mode)
                    }
                }
            }
            syn::Expr::Reference(er) => self.translate_reference(er),
            syn::Expr::Repeat(er) => self.translate_repeat(er),
            syn::Expr::Return(er) => self.translate_return(er),
            syn::Expr::Struct(es) => self.translate_struct_expr(es),
            syn::Expr::Tuple(et) => self.translate_tuple(et),
            syn::Expr::Unary(eu) => self.translate_unary(eu),
            syn::Expr::Unsafe(eu) => self.translate_block_expr(&eu.block),
            syn::Expr::While(ew) => self.translate_while(ew),
            _ => {
                Node::Todo("expr".to_string())
            }
        }
    }

    fn translate_array(&self, ea: &syn::ExprArray) -> Node {
        let mut elements = vec![];
        for elem in &ea.elems {
            let element = self.translate_expr(elem);
            elements.push(element);
        }
        Node::ArrayInit(None, elements)
    }

    fn translate_assign(&self, ea: &syn::ExprAssign) -> Node {
        let left = self.translate_expr(&ea.left);
        let right = self.translate_expr(&ea.right);
        Node::Assign(Box::new(left), Box::new(right))
    }

    fn translate_binary(&self, eb: &syn::ExprBinary) -> Node {
        if matches!(eb.op, syn::BinOp::Rem(_)) && self.rem_is_signed(eb) {
            let left = self.translate_expr(&eb.left);
            let right = self.translate_expr(&eb.right);
            return Node::BuiltinCall(
                "rem".to_string(),
                vec![left, right],
            );
        }
        let left = Box::new(self.translate_expr(&eb.left));
        let right = Box::new(self.translate_expr(&eb.right));
        match eb.op {
            syn::BinOp::Add(_) => Node::Add(left, right),
            syn::BinOp::AddAssign(_) => Node::AssignAdd(left, right),
            syn::BinOp::Div(_) => Node::Div(left, right),
            syn::BinOp::DivAssign(_) => Node::AssignDiv(left, right),
            syn::BinOp::Eq(_) => Node::EqualEqual(left, right),
            syn::BinOp::Ge(_) => Node::GreaterOrEqual(left, right),
            syn::BinOp::Gt(_) => Node::GreaterThan(left, right),
            syn::BinOp::Le(_) => Node::LessOrEqual(left, right),
            syn::BinOp::Lt(_) => Node::LessThan(left, right),
            syn::BinOp::Mul(_) => Node::Mul(left, right),
            syn::BinOp::MulAssign(_) => Node::AssignMul(left, right),
            syn::BinOp::Ne(_) => Node::BangEqual(left, right),
            syn::BinOp::Rem(_) => Node::Mod(left, right),
            syn::BinOp::RemAssign(_) => Node::AssignMod(left, right),
            syn::BinOp::Sub(_) => Node::Sub(left, right),
            syn::BinOp::SubAssign(_) => Node::AssignSub(left, right),
            _ => Node::Todo("binop".to_string()),
        }
    }

    fn rem_is_signed(&self, eb: &syn::ExprBinary) -> bool {
        use syn::spanned::Spanned;
        match self.scip.binary_type_at(&eb.op.span().into()) {
            Some((left, _)) => is_signed_int(peel_ref(&left)),
            None => false,
        }
    }

    fn translate_field(&self, ef: &syn::ExprField) -> Node {
        let base = self.translate_expr(&ef.base);
        let field = match &ef.member {
            syn::Member::Named(ident) => ident.to_string(),
            syn::Member::Unnamed(index) => index.index.to_string(),
        };
        Node::FieldAccess(Box::new(base), field)
    }

    fn translate_index(&self, ei: &syn::ExprIndex) -> Node {
        let base = self.translate_expr(&ei.expr);
        let index = self.translate_expr(&ei.index);
        Node::ArrayAccess(Box::new(base), Box::new(index))
    }

    fn translate_lit(&self, el: &syn::ExprLit) -> Node {
        match &el.lit {
            syn::Lit::Bool(b) => Node::Identifier(b.value.to_string()),
            syn::Lit::Int(i) => Node::NumberLiteral(i.base10_digits().to_string()),
            syn::Lit::Str(s) => Node::StringLiteral(s.value()),
            _ => Node::Todo("lit".to_string()),
        }
    }

    fn translate_match(&self, em: &syn::ExprMatch) -> Node {
        let cond = Box::new(self.translate_expr(&em.expr));
        let arms = em.arms.iter().map(|arm| self.translate_match_arm(arm)).collect();
        Node::Switch { cond, arms }
    }

    fn translate_match_arm(&self, arm: &syn::Arm) -> SwitchArm {
        let variant = match &arm.pat {
            syn::Pat::Struct(ps) => {
                Some(camel_to_snake(&ps.path.segments.last().unwrap().ident.to_string()))
            }
            syn::Pat::TupleStruct(pts) => {
                Some(camel_to_snake(&pts.path.segments.last().unwrap().ident.to_string()))
            }
            _ => None,
        };
        let (pattern, captures) = self.translate_match_pat(&arm.pat);
        let by_ref = captures.iter().any(|capture| capture.by_ref);
        let use_block = captures.len() > 1
            || captures.iter().any(|capture| matches!(capture.accessor, Accessor::Field(_)));
        let clears = self.prelude_clear_flags(&arm.body);
        let result = self.translate_expr(&arm.body);
        if use_block {
            let payload = format!("_{}", variant.unwrap());
            let mut bindings: Vec<Node> = captures.iter().map(|capture| {
                let mut access = match &capture.accessor {
                    Accessor::Index(index) => Node::ArrayAccess(
                        Box::new(Node::Identifier(payload.clone())),
                        Box::new(Node::NumberLiteral(index.to_string())),
                    ),
                    Accessor::Field(field) => Node::FieldAccess(
                        Box::new(Node::Identifier(payload.clone())),
                        field.clone(),
                    ),
                };
                if capture.by_ref {
                    access = Node::AddressOf(Box::new(access));
                }
                Node::SimpleVarDecl { var: Var { is_const: true, name: capture.name.clone(), ty: None }, expr: Some(Box::new(access)) }
            }).collect();
            bindings.extend(clears);
            SwitchArm {
                pattern,
                capture: Some(Capture { name: payload, by_ref }),
                body: SwitchBody::Block { bindings, result },
            }
        } else if !clears.is_empty() {
            let capture = captures.first().map(|capture| Capture { name: capture.name.clone(), by_ref });
            SwitchArm {
                pattern,
                capture,
                body: SwitchBody::Block { bindings: clears, result },
            }
        } else {
            let capture = captures.first().map(|capture| Capture { name: capture.name.clone(), by_ref });
            SwitchArm { pattern, capture, body: SwitchBody::Expr(result) }
        }
    }

    fn translate_reference(&self, er: &syn::ExprReference) -> Node {
        let expr = self.translate_expr(&er.expr);
        Node::AddressOf(Box::new(expr))
    }

    fn translate_repeat(&self, er: &syn::ExprRepeat) -> Node {
        let value = self.translate_expr(&er.expr);
        let len = self.translate_expr(&er.len);
        Node::ArrayRepeat(Box::new(value), Box::new(len))
    }

    fn translate_return(&self, er: &syn::ExprReturn) -> Node {
        if let Some(expr) = &er.expr {
            let expr = self.translate_expr(expr);
            Node::Return(Some(Box::new(expr)))
        } else {
            Node::Return(None)
        }
    }

    fn translate_struct_expr(&self, es: &syn::ExprStruct) -> Node {
        if matches!(self.path_mode(&es.path), PathMode::EnumVariant) {
            return self.translate_struct_constructor(es);
        }
        let ty = Box::new(self.translate_path(&es.path, PathMode::Normal));
        let mut fields = vec![];
        for field in &es.fields {
            if let syn::Member::Named(ident) = &field.member {
                let name = ident.to_string();
                let value = self.translate_expr(&field.expr);
                fields.push((name, value));
            }
        }
        Node::StructInit(Some(ty), fields)
    }

    fn translate_struct_constructor(&self, es: &syn::ExprStruct) -> Node {
        let variant = camel_to_snake(&es.path.segments.last().unwrap().ident.to_string());
        let mut fields = vec![];
        for field in &es.fields {
            if let syn::Member::Named(ident) = &field.member {
                let name = ident.to_string();
                let value = self.translate_expr(&field.expr);
                fields.push((name, value));
            }
        }
        Node::StructInit(None, vec![(variant, Node::StructInit(None, fields))])
    }

    fn translate_tuple(&self, et: &syn::ExprTuple) -> Node {
        let mut elements = vec![];
        for elem in &et.elems {
            let element = self.translate_expr(elem);
            elements.push(element);
        }
        Node::ArrayInit(None, elements)
    }

    fn translate_unary(&self, eu: &syn::ExprUnary) -> Node {
        match eu.op {
            syn::UnOp::Deref(_) => {
                let expr = self.translate_expr(&eu.expr);
                Node::Deref(Box::new(expr))
            }
            _ => {
                Node::Todo("unary".to_string())
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
    let Some(segment) = tp.path.segments.last() else { return false };
    matches!(segment.ident.to_string().as_str(), "i8" | "i16" | "i32" | "i64" | "i128" | "isize")
}
