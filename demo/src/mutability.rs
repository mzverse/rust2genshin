//! 可变性与借用示例:`&mut`、借用、假 `&mut` 视图。
//!
//! 节点图里没有「写句柄」这种东西 —— 写元素走的是**写视图**
//! (`list_set` / `Modify`)。所以 `&mut T` 在 IR 里是两槽元组
//! `(LocalRef(T), PartialMut(T))`:第 0 槽是写句柄,第 1 槽是视图。
//! 视图能读能写,但**没有句柄** —— 由视图造出来的 `&mut` 是**假的**:
//! 能读、能借字段、能调 `&mut self` 的方法,不能整体赋值(`*r = v`)。

use rust2genshin_lib::list::list;
use rust2genshin_lib::*;

use crate::Point;

/// `&mut` 参数 + 字段赋值:字段写是 `Modify`,节点图里没有字段句柄。
#[inline(never)]
fn set_x(p: &mut Point, v: i32) {
    p.x = v;
}

/// 完整示例:可变性的几种用法。
#[unsafe(no_mangle)]
pub fn mutability_demo() {
    // `&mut` 参数:写字段
    let mut p = Point { x: 1, y: 2 };
    set_x(&mut p, 3);
    log(p.x + p.y);

    // 借用:`Freeze` 的 `&T` 等同 T,取借用等于读值
    log(borrow_read(3));

    // `Freeze` 的 `&(i32, i32)` 不扁平化,整体是一个 ADT 局部变量
    log(borrow_tuple_read((1, 2)));

    // 交换两个 `&mut`
    let mut a = 1;
    let mut b = 2;
    swap(&mut a, &mut b);
    log(a);
    log(b);
}

#[inline(never)]
fn borrow_read(x: i32) -> i32 {
    let r = &x;
    *r + 1
}

#[inline(never)]
fn borrow_tuple_read(p: (i32, i32)) -> i32 {
    let q = &p;
    q.0 + q.1
}

#[unsafe(no_mangle)]
fn swap(a: &mut i32, b: &mut i32) {
    let c = *a;
    *a = *b;
    *b = c;
}

/// 可变视图:`PartialMut::from_mut` 把 `&mut` 变成一个能读能写的句柄。
///
/// [`PartialMut::into_mut`] 是它的逆;[`PartialMut::into_ref`] 把视图
/// 换成只读引用。
#[unsafe(no_mangle)]
pub fn partial_mut_demo() {
    let mut list = list![114, 10];

    // 从 `&mut` 造视图,读长度
    let pm = PartialMut::from_mut(&mut list);
    log(pm.len());

    // 视图 → `&mut`:读一个元素
    let view = list.get_mut(0);
    let r = PartialMut::into_mut(view);
    let v = *r;
    log(v);

    // 视图 → `&T`
    let view = list.get_mut(1);
    let r = PartialMut::into_ref(view);
    log(*r);
}

/// `List::index_mut`:经它读元素。
///
/// 必须 `#[inline(always)]`(见 `lib/src/list.rs` 的 `IndexMut`):节点图里
/// 写元素是「写视图」,这个 `&mut` 的写句柄是假的。不内联的话它就是个
/// 独立函数,假句柄会留在函数体里降级时报 `Unsupported`。
#[unsafe(no_mangle)]
pub fn index_mut_demo() {
    let mut list = list![114, 10];
    let r = &mut list[1];
    let v = *r;
    log(v);
}

/// 整体赋值 `*p = v`:**目前不支持**。
///
/// 假 `&mut` 的写句柄造不出来,所以整体赋值这条路在节点图里走不通。
/// 要改一个值请用能写回原处的 API(`List::set`、字段赋值)。
#[cfg(feature = "unsupported")]
#[unsafe(no_mangle)]
pub fn whole_value_assign_demo(p: &mut i32) {
    *p = 114514;
}
