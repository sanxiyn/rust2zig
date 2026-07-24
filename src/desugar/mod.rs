mod binary;
#[allow(unused)]
mod compound_assignment;
mod generic;
mod integer_literal;
mod match_ergonomics;

use crate::scip::Scip;

pub fn desugar(scip: &Scip, mut file: syn::File) -> syn::File {
    binary::run(scip, &mut file);
    integer_literal::run(scip, &mut file);
    match_ergonomics::run(scip, &mut file);
    generic::run(scip, &mut file);
    file
}
