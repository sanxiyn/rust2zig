use std::env;
use std::fs;
use std::path::Path;

mod lsif;
mod translate;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: rust2zig <package-dir>");
        std::process::exit(1);
    }
    let package_dir = Path::new(&args[1]);
    let cargo_toml = fs::read_to_string(package_dir.join("Cargo.toml")).expect("failed to read Cargo.toml");
    let cargo: toml::Table = cargo_toml.parse().expect("failed to parse Cargo.toml");
    let package_name = cargo["package"]["name"].as_str().expect("missing package.name");
    let lsif = lsif::load(package_dir, package_name);
    let source = fs::read_to_string(package_dir.join("src/main.rs")).expect("failed to read source");
    let file = syn::parse_file(&source).expect("failed to parse");
    let mut rust2zig = translate::Rust2Zig::new(lsif);
    rust2zig.analyze(&file);
    let output = rust2zig.translate_file(&file);
    print!("{}", output);
}
