#![no_std]

#![feature(legacy_receiver_trait)]
#![feature(coerce_unsized)]
#![feature(unsize)]
#![feature(extern_types)]
#![feature(rustc_attrs)]
#![feature(reborrow)]

#![allow(internal_features)]

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
impl ToString for str {
    #[inline(always)]
    fn to_string(&self) -> String {
        unsafe {
            &*(self as *const str)
        }
    }
}
impl<T: ToString + ?Sized> ToString for &T {
    #[inline(always)]
    fn to_string(&self) -> String {
        T::to_string(*self)
    }
}

#[rustc_force_inline]
pub fn log(s: impl ToString) {
    #[native_exec(1)]
    fn log_(s: String);
    log_(s.to_string())
}

#[native("Guid")]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Guid(pub i64);

impl ToString for Guid {
    #[native("to_string")]
    fn to_string(&self) -> String;
}
