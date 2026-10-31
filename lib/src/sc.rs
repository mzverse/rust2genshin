//! 引用计数指针 `Sc<T>`。
//!
//! 底层是 [`Box`]:`Box<ScInner<T>>` 提供槽位,计数字段和值存在同一格里。
//! 计数归零时 `Box` 的析构会把槽还给这条类型链的空闲表。
//!
//! 没有真指针 —— 读写都经由 `Box` 的 `Deref` / `DerefMut`(假引用),
//! 所以 `*sc = v` 这种整体赋值不支持,字段级修改可以。

use crate::boxed::{Box, BoxRaw};
use core::ops::Deref;

/// 引用计数的一格:计数 + 值。
///
/// 普通结构体(不能 `#[repr(transparent)]`)—— 它要作为整体存进
/// `Box<ScInner<T>>` 的堆里。
struct ScInner<T> {
    count: i32,
    value: T,
}

pub struct Sc<T: 'static>(BoxRaw<ScInner<T>>);

impl<T: 'static> Drop for Sc<T> {
    fn drop(&mut self) {
        // 计数减一;归零时 `Box` 自己那一侧的析构会把槽还掉。
        self.0.count -= 1;
    }
}

impl<T: 'static> Clone for Sc<T> {
    fn clone(&self) -> Self {
        let count = self.0.count;
        let mut copy = unsafe { Sc::from_raw(self.0) };
        copy.0.count = count + 1;
        copy
    }
}

impl<T: 'static> Deref for Sc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0.value
    }
}

impl<T: 'static> Sc<T> {
    pub fn new(value: T) -> Self {
        Self(Box::into_raw(Box::new(ScInner { count: 1, value })))
    }

    unsafe fn from_raw(raw: BoxRaw<ScInner<T>>) -> Self {
        Self(raw)
    }
}
