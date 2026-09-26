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
pub mod event;

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
pub struct Guid(i64);
impl Guid {
    //noinspection RsAssertEqual
    #[rustc_force_inline]
    #[rustc_comptime]
    pub fn new(value: i64) -> Guid {
        assert!(value != 0);
        Guid(value)
    }
}

impl ToString for Guid {
    #[native("to_string")]
    fn to_string(&self) -> String;
}

/// 阵营
#[native("Faction")]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Faction(i64);
impl Faction {
    //noinspection RsAssertEqual
    #[rustc_force_inline]
    #[rustc_comptime]
    pub fn new(value: i64) -> Faction {
        assert!(value != 0);
        Faction(value)
    }
}

/// 配置ID
#[native("Config")]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Config(i64);
impl Config {
    //noinspection RsAssertEqual
    #[rustc_force_inline]
    #[rustc_comptime]
    pub fn new(value: i64) -> Config {
        assert!(value != 0);
        Config(value)
    }
}

/// 元件ID
#[native("Prefab")]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Prefab(i64);
impl Prefab {
    //noinspection RsAssertEqual
    #[rustc_force_inline]
    #[rustc_comptime]
    pub fn new(value: i64) -> Prefab {
        assert!(value != 0);
        Prefab(value)
    }
}

#[native("enum_eq")]
pub unsafe fn native_enum_eq<T>(a: T, b: T) -> bool;

/// 编译期构造 `Guid`。`#[rustc_comptime]` 保证参数必须是 const 表达式。
#[macro_export]
macro_rules! guid {
    ($id:expr) => {
        const { $crate::Guid::new($id) }
    };
}

/// 编译期构造 `Faction`,语义同 `guid!`。
#[macro_export]
macro_rules! faction {
    ($id:expr) => {
        const { $crate::Faction::new($id) }
    };
}

/// 编译期构造 `Config`,语义同 `guid!`。
#[macro_export]
macro_rules! config {
    ($id:expr) => {
        const { $crate::Config::new($id) }
    };
}

/// 编译期构造 `Prefab`,语义同 `guid!`。
#[macro_export]
macro_rules! prefab {
    ($id:expr) => {
        const { $crate::Prefab::new($id) }
    };
}
