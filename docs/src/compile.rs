use std::process::exit;
use crate::typst::{build, run};

pub mod typst;

fn main() {
    build();
    exit(run("compile"));
}
