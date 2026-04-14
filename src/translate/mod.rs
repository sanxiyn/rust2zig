mod expr;
mod item;
mod pat;

use std::collections::HashMap;
use std::fmt::Write;

use crate::lsif::Lsif;

const INDENT_SIZE: usize = 4;

pub fn camel_to_snake(s: &str) -> String {
    let mut result: String = Default::default();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

pub fn snake_to_camel(s: &str) -> String {
    let mut result: String = Default::default();
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

pub enum PathMode {
    Normal,
    EnumVariant,
}

pub struct Enum {
    pub has_data: bool,
    pub is_generic: bool,
    pub impls: Vec<syn::ItemImpl>,
}

pub struct Rust2Zig {
    pub enums: HashMap<String, Enum>,
    pub lsif: Lsif,
    out: String,
    indent: usize,
}

impl Rust2Zig {
    pub fn new(lsif: Lsif) -> Self {
        Rust2Zig { enums: Default::default(), lsif, out: Default::default(), indent: Default::default() }
    }

    pub fn check_moniker(&self, path: &syn::Path, expected: &str) -> bool {
        let ident = &path.segments.last().unwrap().ident;
        let range = ident.span().into();
        self.lsif.monikers.get(&range) == Some(expected)
    }

    fn indent(&mut self) {
        self.indent += INDENT_SIZE;
    }

    fn dedent(&mut self) {
        self.indent -= INDENT_SIZE;
    }

    fn pad(&self) -> String {
        " ".repeat(self.indent)
    }

    pub fn analyze(&mut self, file: &syn::File) {
        for item in &file.items {
            if let syn::Item::Enum(e) = item {
                let name = e.ident.to_string();
                let has_data = e.variants.iter().any(|v| !v.fields.is_empty());
                let is_generic = !e.generics.params.is_empty();
                self.enums.insert(name, Enum { has_data, is_generic, impls: Default::default() });
            }
        }

        for item in &file.items {
            if let syn::Item::Impl(i) = item {
                if let syn::Type::Path(tp) = &*i.self_ty {
                    let name = tp.path.segments.last().unwrap().ident.to_string();
                    if let Some(e) = self.enums.get_mut(&name) {
                        e.impls.push(i.clone());
                    }
                }
            }
        }
    }

    pub fn path_mode(&self, path: &syn::Path) -> PathMode {
        if path.segments.len() > 1 {
            let first = path.segments[0].ident.to_string();
            if self.enums.contains_key(&first) {
                return PathMode::EnumVariant;
            }
        }
        PathMode::Normal
    }

    pub fn translate_file(&mut self, file: &syn::File) -> String {
        self.out.clear();
        writeln!(self.out, "const std = @import(\"std\");").unwrap();
        writeln!(self.out).unwrap();
        for item in &file.items {
            self.translate_item(item);
        }
        std::mem::take(&mut self.out)
    }

    pub fn translate_path(&mut self, path: &syn::Path, mode: PathMode) {
        let ident = &path.segments.last().unwrap().ident;
        match mode {
            PathMode::Normal => {
                write!(self.out, "{}", snake_to_camel(&ident.to_string())).unwrap();
            }
            PathMode::EnumVariant => {
                write!(self.out, ".{}", camel_to_snake(&ident.to_string())).unwrap();
            }
        }
    }

    pub fn translate_type(&mut self, ty: &syn::Type) {
        match ty {
            syn::Type::Path(tp) => {
                let segment = tp.path.segments.last().unwrap();
                let ident = &segment.ident;
                let name = ident.to_string();
                match name.as_str() {
                    "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => {
                        write!(self.out, "{}", ident).unwrap();
                    }
                    "str" => write!(self.out, "[]const u8").unwrap(),
                    _ if self.check_moniker(&tp.path, "core::option::Option") => {
                        write!(self.out, "?").unwrap();
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                self.translate_type(inner_ty);
                            }
                        }
                    }
                    _ => {
                        write!(self.out, "{}", ident).unwrap();
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            write!(self.out, "(").unwrap();
                            for (i, arg) in args.args.iter().enumerate() {
                                if i > 0 {
                                    write!(self.out, ", ").unwrap();
                                }
                                if let syn::GenericArgument::Type(arg_ty) = arg {
                                    self.translate_type(arg_ty);
                                }
                            }
                            write!(self.out, ")").unwrap();
                        }
                    }
                }
            }
            syn::Type::Reference(tr) => {
                self.translate_type(&tr.elem);
            }
            _ => write!(self.out, "/* TODO: type */").unwrap(),
        }
    }

    pub fn translate_return_type(&mut self, ret: &syn::ReturnType) {
        match ret {
            syn::ReturnType::Default => write!(self.out, "void").unwrap(),
            syn::ReturnType::Type(_, ty) => self.translate_type(ty),
        }
    }

    pub fn translate_block(&mut self, block: &syn::Block) {
        self.translate_block_with_mut_params(block, &[]);
    }

    pub fn translate_block_with_mut_params(&mut self, block: &syn::Block, mut_params: &[String]) {
        writeln!(self.out, "{{").unwrap();
        self.indent();
        for name in mut_params {
            let pad = self.pad();
            writeln!(self.out, "{}var {name} = _{name};", pad, name = name).unwrap();
        }
        let stmts = &block.stmts;
        for (i, stmt) in stmts.iter().enumerate() {
            let is_last = i == stmts.len() - 1;
            self.translate_stmt(stmt, is_last);
        }
        self.dedent();
        write!(self.out, "{}}}", self.pad()).unwrap();
    }
}
