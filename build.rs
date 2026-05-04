fn main() {
    prost_build::compile_protos(&["proto/scip.proto"], &["proto"]).unwrap();
}
