//! 节点图里的「堆」:`Box<T>`。
//!
//! 目标平台没有内存,所以 `Box` 不持指针 —— 它持一个**槽位号**(`usize`),
//! 槽位号是该类型专属堆 `HEAP` 里的下标。每种 `T` 各有一组黑板变量:
//!
//! ```text
//! box_i32_heap : List<i32>    // 存活的槽
//! box_i32_free : List<i32>    // 已释放的槽号,分配时优先复用
//! ```
//!
//! 变量名由后端按 `T` 拼出来(见 `core/src/compile/native.rs` 的
//! `box_heap_get` 分支),lib 侧不需要知道名字。
//!
//! `#[repr(transparent)]` 让 `Box<T>` 在 IR 里直接落成 `Int`
//! (`core/src/compile/ty.rs:243` 对 transparent ADT 取 field 0),不需要
//! 任何类型特判。
//!
//! # 假借用
//!
//! [`Deref`] / [`DerefMut`] 交出来的是**假**引用:节点图里写元素走的是
//! 「写视图」(`list_set`),没有写句柄。于是:
//!
//! - 能读:`*b`、`b.field`、`b.method()`
//! - 能调 `&mut self` 的方法:`b.field = v`、`b.set_xxx(v)`
//! - **不能整体赋值**:`*b = v` 要真写句柄,降级时会报 `Unsupported`
//!
//! [`List`] 的 `IndexMut` 同理。
//!
//! # 借用
//!
//! 借用就是普通引用:`&Box<T>` 拿只读,`&mut Box<T>` 拿可写(后者在 IR 里
//! 编成 `PartialMut<Box<T>>`)。**没有**专门的句柄类型 —— `Box<T>` 在 IR 里
//! 落成槽位号(`Int`),借用它就是借用一个标量,`&Int` 与 `PartialMut<Int>`
//! 都可储存(见 `IrKind::is_storable`)。

use crate::PartialMut;
use crate::list::List;
use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut, LegacyReceiver};
use rust2genshin_lib_internal::native;


#[repr(transparent)]
pub(crate) struct BoxRaw<T: 'static> {
    slot: usize,
    _marker: PhantomData<T>,
}
impl<T: 'static> Copy for BoxRaw<T> {
}
impl<T: 'static> Clone for BoxRaw<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[repr(transparent)]
pub struct Box<T: 'static>(BoxRaw<T>);
impl<T: 'static> LegacyReceiver for Box<T> {
}
impl<T: 'static> Drop for Box<T> {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe {
            drop(List::get_via_move(Self::get_heap(), self.0.slot));
            Self::get_free_mut().insert(0, self.0.slot);
        }
    }
}
impl<T: 'static> From<T> for Box<T> {
    #[inline(always)]
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

#[rustc_force_inline]
pub fn unbox<T: 'static>(boxed: Box<T>) -> T {
    Box::into_inner(boxed)
}

impl<T: 'static> Box<T> {
    #![allow(clippy::wrong_self_convention)]

    /// 这一类 `Box` 的共享堆,以**可写视图**交出。
    ///
    /// 每种 `T` 一组黑板变量,所以它是泛型的:同一个 `Box<i32>` 的所有实例
    /// 共用一张 `HEAP`。变量名由后端按 `T` 拼。
    #[native("box_heap_get")]
    pub unsafe fn get_heap_mut() -> PartialMut<'static, List<T>>;

    #[rustc_force_inline]
    pub unsafe fn get_heap() -> &'static List<T> {
        unsafe {
            PartialMut::into_ref(Self::get_heap_mut())
        }
    }

    /// 把堆写回黑板变量。
    ///
    /// **预留** —— 现在还没有调用点:图变量里的列表是句柄,`HEAP` 的改动
    /// 就地生效,不需要写回。如果以后实测发现是值语义,再改成调用它。
    #[allow(dead_code)]
    #[native("box_heap_set")]
    pub unsafe fn set_heap(heap: List<T>);

    /// 这一类 `Box` 的空闲槽表,以可写视图交出。
    #[native("box_free_get")]
    pub unsafe fn get_free_mut() -> PartialMut<'static, List<usize>>;

    /// 把空闲槽表写回黑板变量。同样是预留。
    #[allow(dead_code)]
    #[native("box_free_set")]
    pub unsafe fn set_free(free: List<usize>);

    /// 分配一格并把 `value` 放进去。
    ///
    /// 空闲表非空就复用末尾那个槽,否则让 `HEAP` 尾部追加一格。
    pub fn new(value: T) -> Self {
        unsafe {
            let mut heap = Self::get_heap_mut();
            let mut free = Self::get_free_mut();
            let slot;
            if free.len() > 0 {
                slot = free[0];
                free.remove(0);
                List::set_via_move(heap, slot, value);
            } else {
                slot = heap.len();
                heap.insert(slot, value);
            }
            Self::from_slot(slot)
        }
    }

    /// 取出里面的值,并把槽还给空闲表。
    pub fn into_inner(boxed: Self) -> T {
        unsafe {
            let value = List::get_via_move(Self::get_heap(), boxed.0.slot);
            Self::get_free_mut().insert(0, boxed.0.slot);
            mem::forget(boxed);
            value
        }
    }

    #[rustc_force_inline]
    pub fn into_raw(boxed: Self) -> BoxRaw<T> {
        let raw = boxed.0;
        mem::forget(boxed);
        raw
    }

    #[rustc_force_inline]
    pub unsafe fn from_raw(raw: BoxRaw<T>) -> Self {
        Self(raw)
    }

    #[rustc_force_inline]
    pub fn into_slot(boxed: Self) -> usize {
        Self::into_raw(boxed).slot
    }

    #[rustc_force_inline]
    pub unsafe fn from_slot(slot: usize) -> Self {
        unsafe {
            Self::from_raw(BoxRaw {
                slot,
                _marker: PhantomData,
            })
        }
    }
}

impl<T: 'static> Deref for BoxRaw<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe {
            let heap = PartialMut::into_mut(Box::get_heap_mut());
            &heap[self.slot]
        }
    }
}
impl<T: 'static> DerefMut for BoxRaw<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            let heap = PartialMut::into_mut(Box::get_heap_mut());
            &mut heap[self.slot]
        }
    }
}

impl<T: 'static> Deref for Box<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}
impl<T: 'static> DerefMut for Box<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}
impl<T: 'static> AsRef<T> for Box<T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        self.deref()
    }
}
impl<T: 'static> AsMut<T> for Box<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut T {
        self.deref_mut()
    }
}

