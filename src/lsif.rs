use std::collections::HashMap;
use std::fs;
use std::path::Path;

use proc_macro2::Span;
use serde_json::Value;

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

pub struct MonikerMap {
    map: HashMap<Range, String>,
}

impl MonikerMap {
    pub fn get(&self, range: &Range) -> Option<&str> {
        self.map.get(range).map(|s| s.as_str())
    }
}

pub struct Lsif {
    pub monikers: MonikerMap,
}

pub fn load(package_dir: &Path, package_name: &str) -> Lsif {
    let path = package_dir.join(format!("{}.lsif", package_name));
    let content = fs::read_to_string(&path).expect("failed to read LSIF file");
    let entries: Vec<Value> = content
        .lines()
        .map(|line| serde_json::from_str(line).expect("failed to parse LSIF line"))
        .collect();

    let mut ranges: HashMap<u64, Range> = Default::default();
    let mut monikers: HashMap<u64, String> = Default::default();
    let mut next_edges: HashMap<u64, u64> = Default::default();
    let mut moniker_edges: HashMap<u64, u64> = Default::default();

    for entry in &entries {
        let id = entry["id"].as_u64().unwrap();
        let type_ = entry["type"].as_str().unwrap();
        let label = entry["label"].as_str().unwrap();
        match (type_, label) {
            ("vertex", "range") => {
                let start = &entry["start"];
                let end = &entry["end"];
                ranges.insert(id, Range {
                    start_line: start["line"].as_u64().unwrap() as u32,
                    start_character: start["character"].as_u64().unwrap() as u32,
                    end_line: end["line"].as_u64().unwrap() as u32,
                    end_character: end["character"].as_u64().unwrap() as u32,
                });
            }
            ("vertex", "moniker") => {
                let identifier = entry["identifier"].as_str().unwrap().to_string();
                monikers.insert(id, identifier);
            }
            ("edge", "next") => {
                let out_vertex = entry["outV"].as_u64().unwrap();
                let in_vertex = entry["inV"].as_u64().unwrap();
                next_edges.insert(out_vertex, in_vertex);
            }
            ("edge", "moniker") => {
                let out_vertex = entry["outV"].as_u64().unwrap();
                let in_vertex = entry["inV"].as_u64().unwrap();
                moniker_edges.insert(out_vertex, in_vertex);
            }
            _ => {}
        }
    }

    let mut moniker_map: HashMap<Range, String> = Default::default();
    for (range_id, range) in &ranges {
        let Some(result_set_id) = next_edges.get(range_id) else { continue };
        let Some(moniker_id) = moniker_edges.get(result_set_id) else { continue };
        let Some(identifier) = monikers.get(moniker_id) else { continue };
        moniker_map.insert(range.clone(), identifier.clone());
    }

    Lsif {
        monikers: MonikerMap { map: moniker_map },
    }
}
