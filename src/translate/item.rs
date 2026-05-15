use std::fmt::Write;

use super::Rust2Zig;
use super::name::{camel_to_snake, escape_zig, snake_to_camel};

impl Rust2Zig {
    pub fn translate_item(&mut self, item: &syn::Item) {
        match item {
            syn::Item::Enum(e) => self.translate_enum(e),
            syn::Item::Struct(s) => self.translate_struct(s),
            syn::Item::Fn(f) => self.translate_fn(f),
            syn::Item::Impl(_) => (),
            _ => writeln!(self.out, "// TODO: item").unwrap(),
        }
    }

    fn translate_enum(&mut self, e: &syn::ItemEnum) {
        let name = e.ident.to_string();
        let enum_ = &self.enums[&name];
        let has_data = enum_.has_data;
        let is_generic = enum_.is_generic;
        let impls = enum_.impls.clone();

        if is_generic {
            write!(self.out, "fn {}(", name).unwrap();
            for (i, param) in e.generics.params.iter().enumerate() {
                if i > 0 {
                    write!(self.out, ", ").unwrap();
                }
                if let syn::GenericParam::Type(tp) = param {
                    write!(self.out, "comptime {}: type", tp.ident).unwrap();
                }
            }
            writeln!(self.out, ") type {{").unwrap();
            self.indent();
            let pad = self.pad();
            if has_data {
                writeln!(self.out, "{}return union(enum) {{", pad).unwrap();
            } else {
                writeln!(self.out, "{}return enum {{", pad).unwrap();
            }
            self.indent();
        } else {
            if has_data {
                writeln!(self.out, "const {} = union(enum) {{", name).unwrap();
                self.indent();
            } else {
                writeln!(self.out, "const {} = enum {{", name).unwrap();
                self.indent();
            }
        }

        if !impls.is_empty() {
            let pad = self.pad();
            writeln!(self.out, "{}const Self = @This();", pad).unwrap();
            writeln!(self.out).unwrap();
        }

        for variant in &e.variants {
            let pad = self.pad();
            let vname = camel_to_snake(&variant.ident.to_string());
            match &variant.fields {
                syn::Fields::Unit => {
                    writeln!(self.out, "{}{},", pad, vname).unwrap();
                }
                syn::Fields::Unnamed(fields) => {
                    write!(self.out, "{}{}: ", pad, vname).unwrap();
                    if fields.unnamed.len() == 1 {
                        self.translate_type(&fields.unnamed[0].ty);
                    } else {
                        write!(self.out, "struct {{ ").unwrap();
                        for (i, field) in fields.unnamed.iter().enumerate() {
                            if i > 0 {
                                write!(self.out, ", ").unwrap();
                            }
                            self.translate_type(&field.ty);
                        }
                        write!(self.out, " }}").unwrap();
                    }
                    writeln!(self.out, ",").unwrap();
                }
                syn::Fields::Named(fields) => {
                    write!(self.out, "{}{}: struct {{ ", pad, vname).unwrap();
                    for (i, field) in fields.named.iter().enumerate() {
                        if i > 0 {
                            write!(self.out, ", ").unwrap();
                        }
                        let fname = field.ident.as_ref().unwrap();
                        write!(self.out, "{}: ", fname).unwrap();
                        self.translate_type(&field.ty);
                    }
                    writeln!(self.out, " }},").unwrap();
                }
            }
        }

        for i in &impls {
            for ii in &i.items {
                match ii {
                    syn::ImplItem::Fn(method) => {
                        writeln!(self.out).unwrap();
                        self.translate_method(method);
                    }
                    _ => {
                        let pad = self.pad();
                        writeln!(self.out, "{}// TODO: impl item", pad).unwrap();
                    }
                }
            }
        }

        if is_generic {
            self.dedent();
            writeln!(self.out, "{}}};", self.pad()).unwrap();
            self.dedent();
            writeln!(self.out, "{}}}", self.pad()).unwrap();
        } else {
            self.dedent();
            writeln!(self.out, "{}}};", self.pad()).unwrap();
        }
        writeln!(self.out).unwrap();
    }

    fn translate_struct(&mut self, s: &syn::ItemStruct) {
        let name = s.ident.to_string();
        let impls = self.structs[&name].impls.clone();

        writeln!(self.out, "const {} = struct {{", name).unwrap();
        self.indent();

        if !impls.is_empty() {
            let pad = self.pad();
            writeln!(self.out, "{}const Self = @This();", pad).unwrap();
            writeln!(self.out).unwrap();
        }

        for field in &s.fields {
            let pad = self.pad();
            let fname = field.ident.as_ref().unwrap().to_string();
            write!(self.out, "{}{}: ", pad, fname).unwrap();
            self.translate_type(&field.ty);
            writeln!(self.out, ",").unwrap();
        }

        for i in &impls {
            for ii in &i.items {
                if let syn::ImplItem::Fn(method) = ii {
                    writeln!(self.out).unwrap();
                    self.translate_method(method);
                }
            }
        }

        self.dedent();
        writeln!(self.out, "}};").unwrap();
        writeln!(self.out).unwrap();
    }

    fn translate_method(&mut self, method: &syn::ImplItemFn) {
        let pad = self.pad();
        let name = escape_zig(&snake_to_camel(&method.sig.ident.to_string()));
        write!(self.out, "{}fn {}(", pad, name).unwrap();
        let type_params: Vec<String> = self.scip.symbol_at(&method.sig.ident.span().into())
            .and_then(|s| self.generic_fns.get(s))
            .map(|gf| gf.type_params.clone())
            .unwrap_or_default();
        let mut first = true;
        for arg in method.sig.inputs.iter() {
            if let syn::FnArg::Receiver(_) = arg {
                if !first {
                    write!(self.out, ", ").unwrap();
                }
                first = false;
                write!(self.out, "self: Self").unwrap();
            }
        }
        for tp in &type_params {
            if !first {
                write!(self.out, ", ").unwrap();
            }
            first = false;
            write!(self.out, "comptime {}: type", tp).unwrap();
        }
        for arg in method.sig.inputs.iter() {
            if let syn::FnArg::Typed(pat_type) = arg {
                if !first {
                    write!(self.out, ", ").unwrap();
                }
                first = false;
                self.translate_pat(&pat_type.pat);
                write!(self.out, ": ").unwrap();
                self.translate_type(&pat_type.ty);
            }
        }
        write!(self.out, ") ").unwrap();
        self.translate_return_type(&method.sig.output);
        write!(self.out, " ").unwrap();
        self.translate_block(&method.block);
        writeln!(self.out).unwrap();
    }

    fn translate_fn(&mut self, f: &syn::ItemFn) {
        let name = &f.sig.ident;
        let is_test = f.attrs.iter().any(|a| a.path().is_ident("test"));

        if is_test {
            let n = name.to_string();
            let test_name = n.strip_prefix("test_").unwrap_or(&n);
            write!(self.out, "test \"{}\" ", test_name).unwrap();
            self.translate_block(&f.block);
            writeln!(self.out).unwrap();
            writeln!(self.out).unwrap();
            return;
        }

        let mut mut_params: Vec<String> = Default::default();
        for arg in &f.sig.inputs {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pi) = &*pat_type.pat {
                    if pi.mutability.is_some() {
                        mut_params.push(pi.ident.to_string());
                    }
                }
            }
        }

        write!(self.out, "fn {}", escape_zig(&snake_to_camel(&name.to_string()))).unwrap();
        write!(self.out, "(").unwrap();
        let mut first = true;
        let type_params: Vec<String> = self.scip.symbol_at(&name.span().into())
            .and_then(|s| self.generic_fns.get(s))
            .map(|gf| gf.type_params.clone())
            .unwrap_or_default();
        for tp in &type_params {
            if !first {
                write!(self.out, ", ").unwrap();
            }
            first = false;
            write!(self.out, "comptime {}: type", tp).unwrap();
        }
        for arg in f.sig.inputs.iter() {
            if !first {
                write!(self.out, ", ").unwrap();
            }
            first = false;
            self.translate_fn_arg(arg, &mut_params);
        }
        write!(self.out, ") ").unwrap();
        self.translate_return_type(&f.sig.output);

        write!(self.out, " ").unwrap();
        let preamble: Vec<String> = mut_params
            .iter()
            .map(|name| format!("var {name} = _{name};"))
            .collect();
        self.translate_block_with_preamble(&f.block, &preamble);
        writeln!(self.out).unwrap();
        writeln!(self.out).unwrap();
    }

    fn translate_fn_arg(&mut self, arg: &syn::FnArg, mut_params: &[String]) {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                if let syn::Pat::Ident(pi) = &*pat_type.pat {
                    let name = pi.ident.to_string();
                    if mut_params.contains(&name) {
                        write!(self.out, "_{}", name).unwrap();
                    } else {
                        write!(self.out, "{}", name).unwrap();
                    }
                }
                write!(self.out, ": ").unwrap();
                self.translate_type(&pat_type.ty);
            }
            _ => write!(self.out, "/* TODO: self */").unwrap(),
        }
    }
}
