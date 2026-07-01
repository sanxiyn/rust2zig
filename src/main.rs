use std::env;
use std::fs;
use std::path::Path;

mod ast;
mod desugar;
mod print;
mod scip;
mod translate;

const BACKENDS: &[&str] = &["zig"];

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 || !BACKENDS.contains(&args[1].as_str()) {
        eprintln!("Usage: rust2zig <backend> <source-dir> <target-dir>");
        std::process::exit(1);
    }
    let backend = &args[1];
    let source_dir = Path::new(&args[2]);
    let target_dir = Path::new(&args[3]);
    let name = source_dir
        .file_name()
        .expect("source directory has no name")
        .to_str()
        .expect("source directory name is not UTF-8");
    let scip = scip::load(source_dir);
    let source = fs::read_to_string(source_dir.join("src/lib.rs")).expect("failed to read source");
    let file = syn::parse_file(&source).expect("failed to parse");
    let file = desugar::desugar(&scip, file);
    fs::create_dir_all(target_dir).expect("failed to create target directory");
    match backend.as_str() {
        "zig" => {
            let mut translator = translate::zig::Translator::new(scip);
            translator.analyze(&file);
            let root = translator.translate_file(&file);
            let output = print::zig::print(&root);
            fs::write(target_dir.join(format!("{name}.zig")), output)
                .expect("failed to write output");
        }
        _ => unreachable!(),
    }
}
