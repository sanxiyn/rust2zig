use std::collections::HashMap;
use std::fs;
use std::path::Path;

use proc_macro2::Span;
use prost::Message;

mod proto {
    #![allow(unused)]
    include!(concat!(env!("OUT_DIR"), "/scip.rs"));
}

pub use proto::symbol_information::Kind;

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct Range {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
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
    let mut symbols: HashMap<String, SymbolInfo> = Default::default();

    for document in &index.documents {
        for occurrence in &document.occurrences {
            let Some(range) = decode_range(&occurrence.range) else { continue };
            if !occurrence.symbol.is_empty() {
                occurrences.insert(range, occurrence.symbol.clone());
            }
        }
        for symbol in &document.symbols {
            symbols.insert(
                symbol.symbol.clone(),
                SymbolInfo { kind: symbol.kind() },
            );
        }
    }

    Scip { occurrences, symbols }
}
