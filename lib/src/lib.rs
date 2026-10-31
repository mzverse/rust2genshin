#![no_std]

#![feature(legacy_receiver_trait)]
#![feature(coerce_unsized)]
#![feature(unsize)]
#![feature(extern_types)]
#![feature(rustc_attrs)]
#![feature(reborrow)]
#![feature(decl_macro)]
#![feature(coroutine_trait)]
#![feature(coroutines)]
#![feature(stmt_expr_attributes, proc_macro_hygiene)]
#![feature(core_intrinsics)]
#![feature(freeze)]
#![feature(freeze_impls)]
#![allow(internal_features)]

pub mod entity;
pub mod math;
pub mod list;
pub mod dict;
pub mod boxed;
pub mod player;
pub mod event;
pub mod asynchronous;
pub mod task;
pub mod sc;

use core::num::NonZero;
use core::ops::{Deref, DerefMut};
use core::ptr::drop_in_place;
pub use rust2genshin_lib_internal::event_listener;

use rust2genshin_lib_internal::*;

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unreachable!()
}

#[native("PartialMut")]
pub struct PartialMut<'a, T: ?Sized>(&'a mut T);
impl<T> Deref for PartialMut<'_, T> {
    type Target = T;

    #[native("partial_mut_deref")]
    fn deref(&self) -> &Self::Target;
}
impl<T> DerefMut for PartialMut<'_, T> {
    #[native("partial_mut_deref_mut")]
    fn deref_mut(&mut self) -> &mut Self::Target;
}
/// 从 `&mut T` 造视图、以及把视图换回引用。
///
/// 都是**关联函数**而不是方法:`&mut` 和视图是同一种东西的两种写法,
/// 调用点应当写明方向(`from_mut` / `into_mut`),而不是让 `.into()`
/// 悄悄替人挑一个。
///
/// 单独一块、`T` 不带 `?Sized`：节点图里的值类型都是定长的（列表 / 结构体 /
/// 标量），没有 `str` / `[T]` 这种不定长值。
impl<'a, T> PartialMut<'a, T> {
    /// 从 `&'a mut T` 造一个视图。
    ///
    /// 「拆开 `&mut` 取视图」—— 和 [`PartialMut::into_mut`] 互逆。
    ///
    /// 调用点要能被 MIR 内联（`#[inline(always)]`），否则这套拆装的中间形态
    /// 会固化在一层独立的子图里，降级时报 `Unsupported`。
    #[native("partial_mut_from")]
    pub fn from_mut(value: &'a mut T) -> Self;

    /// 把视图换回 `&'a mut T`。
    ///
    /// 和 [`PartialMut::from_mut`] 互逆。要调用 `&mut self` 方法时用它。
    ///
    /// # 假 `&mut`
    ///
    /// 节点图里没有写句柄 —— 写视图就是写原处。所以这个 `&mut` 的「写句柄」
    /// 是假的：能读、能借字段、能调 `&mut self` 的方法，但**不能整体赋值**
    /// (`*r = v`)。整体赋值请改用能写回原处的 API（比如 `List::set`）。
    ///
    /// 调用点要能被 MIR 内联（`#[inline(always)]`），否则假句柄会固化在
    /// 一层独立的子图里，降级时报 `Unsupported`。
    #[native("partial_mut_into_mut")]
    pub fn into_mut(self) -> &'a mut T;

    /// 把视图换成 `&'a T`。
    ///
    /// `T: Freeze` 时 `&T` 在节点图里就是值本身，要读一次；否则还是同一份
    /// 视图，直通。
    #[native("partial_mut_into_ref")]
    pub fn into_ref(self) -> &'a T;
}

/// # Safety
/// See drop_in_place
#[rustc_force_inline]
pub unsafe fn drop_glue<T>(value: &mut T) {
    unsafe {
        drop_in_place(value as *mut T);
    }
}

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

#[inline(always)]
pub fn log(s: impl ToString) {
    #[native_exec(1)]
    fn log_(s: String);
    log_(s.to_string())
}

#[native("Guid")]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Guid(NonZero<i64>);
impl Guid {
    //noinspection RsAssertEqual
    #[rustc_force_inline]
    #[rustc_comptime]
    pub fn new(value: i64) -> Self {
        Self(NonZero::new(value).unwrap())
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
pub struct Faction(NonZero<i64>);
impl Faction {
    //noinspection RsAssertEqual
    #[rustc_force_inline]
    #[rustc_comptime]
    pub fn new(value: i64) -> Self {
        Self(NonZero::new(value).unwrap())
    }
}

/// 配置ID
#[native("Config")]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Config(NonZero<i64>);
impl Config {
    //noinspection RsAssertEqual
    #[rustc_force_inline]
    #[rustc_comptime]
    pub fn new(value: i64) -> Self {
        Self(NonZero::new(value).unwrap())
    }
}

/// 元件ID
#[native("Prefab")]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Prefab(NonZero<i64>);
impl Prefab {
    //noinspection RsAssertEqual
    #[rustc_force_inline]
    #[rustc_comptime]
    pub fn new(value: i64) -> Self {
        Self(NonZero::new(value).unwrap())
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
