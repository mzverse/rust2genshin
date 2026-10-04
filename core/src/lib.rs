#![feature(rustc_private)]
#![feature(type_changing_struct_update)]

pub mod asset;
pub mod compile;
pub mod node;
pub mod value;
pub mod structure;

#[allow(unused_extern_crates)]
extern crate rustc_abi;
extern crate rustc_apfloat;
extern crate rustc_ast;
extern crate rustc_attr_ir;
extern crate rustc_codegen_ssa;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_metadata;
extern crate rustc_middle;
extern crate rustc_monomorphize;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_structures;
extern crate rustc_target;
extern crate rustc_ty_utils;

#[macro_export]
macro_rules! unwrap {
    ($e:expr, $variant:path) => {
        if let $variant(x) = $e {
            x
        } else {
            panic!("expected {}, got {:?}", stringify!($variant), $e);
        }
    };
}
