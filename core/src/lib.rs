#![feature(rustc_private)]

pub mod asset;
pub mod backend;
pub mod parser;
pub mod compile;

#[allow(unused_extern_crates)]
extern crate rustc_abi;
extern crate rustc_attr_ir;
extern crate rustc_codegen_ssa;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_metadata;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_structures;
extern crate rustc_target;
