#![no_std]

pub mod entity;
pub mod math;

extern crate rust2genshin_lib_internal;
extern crate alloc;

pub use alloc::string::{String, ToString};
pub use rust2genshin_lib_internal::event_listener;

use rust2genshin_lib_internal::*;

#[native_exec(1)]
pub fn log(s: impl ToString);
