#![no_std]
#![feature(legacy_receiver_trait)]
#![feature(coerce_unsized)]
#![feature(unsize)]
#![feature(extern_types)]

pub mod entity;
pub mod math;
pub mod list;
pub mod dict;
pub mod boxed;
pub mod player;

pub use rust2genshin_lib_internal::event_listener;

use rust2genshin_lib_internal::*;

pub type String = &'static str;
pub trait ToString {
    fn to_string(&self) -> String;
}
impl ToString for String {
    #[inline(always)]
    fn to_string(&self) -> String {
        self
    }
}
impl ToString for str {
    #[inline(always)]
    fn to_string(&self) -> String {
        unsafe {
            &*(self as *const str)
        }
    }
}

#[inline(always)]
pub fn log(s: &(impl ToString + ?Sized)) {
    log_(s.to_string())
}

#[native_exec(1)]
fn log_(s: String);

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Guid(pub i64);
