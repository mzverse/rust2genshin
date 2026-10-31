//! 所有权示例:`Box<T>` 的分配、读、借用、析构。
//!
//! 节点图没有内存,`Box<T>` 也不是指针 —— 它持一个**槽位号**(`usize`),
//! 槽位号是这一类 `T` 专属堆(`HEAP`)里的下标。每种 `T` 各有一组
//! 黑板变量:`box_<T>_heap` 存活着的元素,`box_<T>_free` 存已释放的
//! 槽号,分配时优先复用。
//!
//! 所以 `Box` 的借用是**假借用**:节点图里没有写句柄,写元素走的是
//! 「写视图」(`list_set`)。能读、能借字段、能调 `&mut self` 的方法,
//! **不能整体赋值**(`*b = v`)。

use rust2genshin_lib::boxed::Box;
use rust2genshin_lib::*;

/// 完整示例:`Box` 的生命周期走一遍。
///
/// 分配 → 读 → 借出只读句柄 → 借出可写句柄 → 取回内容 → 析构。
#[unsafe(no_mangle)]
pub fn ownership_demo() {
    // 分配:槽位号从空闲表里拿,没有就把堆尾部追加一格
    let b = Box::new(114);

    // 读:解引用就是读堆里那一格
    let v = *b;
    log(v);

    // 借出只读句柄:只持槽位号,不持所有权
    let r = b.borrow();
    log(*r);

    // 取回内容:槽位还给空闲表
    let y = Box::into_inner(b);
    log(y);
}

/// `Box` 用完即弃:析构把槽还给空闲表。
///
/// 之后再分配会**复用**同一个槽位号。
#[unsafe(no_mangle)]
pub fn ownership_drop_demo() {
    {
        let _b = Box::new(7);
    }
    let c = Box::new(8);
    log(*c);
}
