use std::collections::HashMap;
use std::fs;
use std::path::Path;

use proc_macro2::Span;
use prost::Message;

mod proto {
    #![allow(unused)]
    include!(concat!(env!("OUT_DIR"), "/scip.rs"));
}

pub use proto::SymbolRole;
pub use proto::symbol_information::Kind;

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct Range {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

impl Range {
    pub fn contains(&self, other: &Range) -> bool {
        let outer_start = (self.start_line, self.start_character);
        let outer_end = (self.end_line, self.end_character);
        let inner_start = (other.start_line, other.start_character);
        let inner_end = (other.end_line, other.end_character);
        outer_start <= inner_start && inner_end <= outer_end
    }
}

impl From<Span> for Range {
    fn from(span: Span) -> Self {
        let start = span.start();
        let end = span.end();
        Self {
            start_line: (start.line - 1) as u32,
            start_character: start.column as u32,
            end_line: (end.line - 1) as u32,
            end_character: end.column as u32,
        }
    }
}

pub struct SymbolInfo {
    pub kind: Kind,
    pub range: Option<Range>,
    pub signature: Option<String>,
}

pub struct Scip {
    occurrences: HashMap<Range, String>,
    symbols: HashMap<String, SymbolInfo>,
}

impl Scip {
    pub fn symbol_at(&self, range: &Range) -> Option<&str> {
        self.occurrences.get(range).map(|s| s.as_str())
    }

    pub fn symbol_info(&self, symbol: &str) -> Option<&SymbolInfo> {
        self.symbols.get(symbol)
    }

    pub fn kind_at(&self, range: &Range) -> Option<Kind> {
        let symbol = self.symbol_at(range)?;
        Some(self.symbol_info(symbol)?.kind)
    }

    pub fn type_at(&self, range: &Range) -> Option<syn::Type> {
        let symbol = self.symbol_at(range)?;
        let info = self.symbol_info(symbol)?;
        if !matches!(info.kind, Kind::Variable | Kind::Parameter | Kind::SelfParameter) {
            return None;
        }
        let sig = info.signature.as_deref()?;
        let (_, ty) = sig.split_once(": ")?;
        syn::parse_str(ty).ok()
    }

    pub fn signature_at(&self, range: &Range) -> Option<syn::Signature> {
        let symbol = self.symbol_at(range)?;
        let info = self.symbol_info(symbol)?;
        if !matches!(info.kind, Kind::Function | Kind::Method | Kind::StaticMethod) {
            return None;
        }
        struct Signature {
            #[allow(unused)]
            visibility: syn::Visibility,
            signature: syn::Signature,
        }
        impl syn::parse::Parse for Signature {
            fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
                let visibility: syn::Visibility = input.parse()?;
                let signature: syn::Signature = input.parse()?;
                Ok(Self { visibility, signature })
            }
        }
        let signature = info.signature.as_deref()?;
        let signature = syn::parse_str::<Signature>(signature).ok()?;
        Some(signature.signature)
    }

    pub fn return_type_at(&self, range: &Range) -> Option<syn::Type> {
        let signature = self.signature_at(range)?;
        let syn::ReturnType::Type(_, ty) = signature.output else { return None; };
        Some(*ty)
    }

    pub fn binary_type_at(&self, range: &Range) -> Option<(syn::Type, syn::Type)> {
        let symbol = self.symbol_at(range)?;
        let idx = symbol.find("/impl#[")?;
        let rest = &symbol[idx + "/impl#[".len()..];
        let (left_str, rest) = if let Some(rest) = rest.strip_prefix('`') {
            let end = rest.find('`')?;
            (&rest[..end], &rest[end + 1..])
        } else {
            let end = rest.find(']')?;
            (&rest[..end], &rest[end..])
        };
        let rest = rest.strip_prefix("][`")?;
        let end = rest.find("`]")?;
        let trait_part = &rest[..end];
        let lt = trait_part.find('<')?;
        let gt = trait_part.rfind('>')?;
        let right_str = &trait_part[lt + 1..gt];
        let right_str = if right_str == "Self" { left_str } else { right_str };
        let left_ty: syn::Type = syn::parse_str(left_str).ok()?;
        let right_ty: syn::Type = syn::parse_str(right_str).ok()?;
        Some((left_ty, right_ty))
    }
}

fn decode_range(range: &[i32]) -> Option<Range> {
    let (sl, sc, el, ec) = match range {
        [sl, sc, ec] => (*sl, *sc, *sl, *ec),
        [sl, sc, el, ec] => (*sl, *sc, *el, *ec),
        _ => return None,
    };
    Some(Range {
        start_line: sl as u32,
        start_character: sc as u32,
        end_line: el as u32,
        end_character: ec as u32,
    })
}

pub fn load(package_dir: &Path) -> Scip {
    let path = package_dir.join("index.scip");
    let bytes = fs::read(&path).expect("failed to read SCIP file");
    let index = proto::Index::decode(bytes.as_slice()).expect("failed to decode SCIP");

    let mut occurrences: HashMap<Range, String> = Default::default();
    let mut definitions: HashMap<String, Range> = Default::default();
    let mut symbols: HashMap<String, SymbolInfo> = Default::default();

    for document in &index.documents {
        for occurrence in &document.occurrences {
            let Some(range) = decode_range(&occurrence.range) else { continue };
            if occurrence.symbol.is_empty() {
                continue;
            }
            if occurrence.symbol_roles & SymbolRole::Definition as i32 != 0 {
                definitions.insert(occurrence.symbol.clone(), range.clone());
            }
            occurrences.insert(range, occurrence.symbol.clone());
        }
        for symbol in &document.symbols {
            let range = definitions.get(&symbol.symbol).cloned();
            let signature = symbol
                .signature_documentation
                .as_ref()
                .map(|d| d.text.clone());
            symbols.insert(
                symbol.symbol.clone(),
                SymbolInfo { kind: symbol.kind(), range, signature },
            );
        }
    }

    Scip { occurrences, symbols }
}
