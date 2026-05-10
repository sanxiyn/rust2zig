use std::env;
use std::fs;
use std::path::Path;

mod scip;
mod translate;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: rust2zig <package-dir>");
        std::process::exit(1);
    }
    let package_dir = Path::new(&args[1]);
    let scip = scip::load(package_dir);
    let source = fs::read_to_string(package_dir.join("src/lib.rs")).expect("failed to read source");
    let file = syn::parse_file(&source).expect("failed to parse");
    let mut rust2zig = translate::Rust2Zig::new(scip);
    rust2zig.analyze(&file);
    let output = rust2zig.translate_file(&file);
    print!("{}", output);
}
