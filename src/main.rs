use std::env;
use std::fs;
use std::path::Path;

mod ast;
mod desugar;
mod print;
mod scip;
mod translate;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 || (args[1] != "old" && args[1] != "new") {
        eprintln!("Usage: rust2zig <old|new> <package-dir>");
        std::process::exit(1);
    }
    let mode = &args[1];
    let package_dir = Path::new(&args[2]);
    let scip = scip::load(package_dir);
    let source = fs::read_to_string(package_dir.join("src/lib.rs")).expect("failed to read source");
    let file = syn::parse_file(&source).expect("failed to parse");
    let file = desugar::desugar(&scip, file);
    let output = if mode == "new" {
        let mut translator = translate::zig::Translator::new(scip);
        translator.analyze(&file);
        let root = translator.translate_file(&file);
        print::zig::print(&root)
    } else {
        let mut rust2zig = translate::Rust2Zig::new(scip);
        rust2zig.analyze(&file);
        rust2zig.translate_file(&file)
    };
    print!("{}", output);
}
