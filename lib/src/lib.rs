#![no_std]

pub mod entity;
pub mod math;
pub mod list;
pub mod dict;

extern crate rust2genshin_lib_internal;

pub use rust2genshin_lib_internal::event_listener;

use rust2genshin_lib_internal::*;

/// same as `&'static str`
pub type String = *const str;
pub trait ToString {
    fn to_string(&self) -> String;
}
impl ToString for str {
    #[inline(always)]
    fn to_string(&self) -> String {
        self as *const str
    }
}

#[inline(always)]
pub fn log(s: &(impl ToString + ?Sized)) {
    log_(s.to_string())
}

#[native_exec(1)]
fn log_(s: String);

#[repr(transparent)]
pub struct Guid(pub i64);
