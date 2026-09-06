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

/// Compare two 2-tuples by field, returning a bool.
///
/// Expands `tuple_eq!(a, b)` to `a.0 == b.0 && a.1 == b.1`. The expansion uses
/// only scalar `==` (which MIR lowers to `Rvalue::BinaryOp(Eq, ...)`, not the
/// `<T as PartialEq>::eq` trait dispatch), so the backend's existing scalar
/// comparison paths handle every field. For nested tuples, recurse at the
/// source level:
///
/// ```ignore
/// tuple_eq!(p.0, q.0) && p.1 == q.1   // for ((A, B), C)
/// ```
#[macro_export]
macro_rules! tuple_eq {
    ($a:expr, $b:expr) => {
        ($a).0 == ($b).0 && ($a).1 == ($b).1
    };
}
