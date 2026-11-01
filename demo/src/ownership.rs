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

/// 元素是结构体的 `Box`:堆变量类型是 `List<结构体>`。
///
/// 图变量节点走 `using_struct`(选择器 22、`type1` 175),不是变体表 ——
/// 和 `List<Int>` 那种走 kernel 表的路径不一样。
#[unsafe(no_mangle)]
pub fn ownership_struct_demo() {
    let b = Box::new(crate::Point { x: 1, y: 2 });
    let p = &*b;
    log(p.x);
}

/// 带析构的元素类型:析构时打一条日志,好看出它到底跑没跑。
struct DropSpy(&'static str);

impl Drop for DropSpy {
    #[inline(always)]
    fn drop(&mut self) {
        log("drop spy");
        log(self.0);
    }
}

/// `Box<T>` 的析构会**自动析构 `T`**。
///
/// `Box` 不是指针,是槽位号(`usize`)。析构要做两件事:
/// 把 `T` 从堆里**按值取出来**并析构(`drop(List::get_via_move(...))`),
/// 再把槽位号还给空闲表。所以 `T` 自己的 `Drop` 会跑 —— 和普通 Rust 里
/// `Box<T>` 的行为一致。
///
/// 堆是**黑板变量**,不是局部变量:黑板变量本身没有析构,`Box` 得自己去取。
#[unsafe(no_mangle)]
pub fn ownership_drop_element_demo() {
    {
        let b = Box::new(DropSpy("inside box"));
        log("still alive");
        log(b.0);
    }
    // 到这里 `b` 已经出作用域:`DropSpy` 应该已经被析构过了。
    log("box dropped");
}

/// 析构之后槽位会**复用**:重新分配拿到的还是同一个槽位号。
///
/// 空闲表(`box_<T>_free`)里存的是已释放的槽位号,分配时优先从那里拿。
#[unsafe(no_mangle)]
pub fn ownership_drop_then_reuse_demo() {
    {
        let b = Box::new(DropSpy("first"));
        log(b.0);
    }
    // 第一个 `Box` 已经析构,槽位还回空闲表了。
    let c = Box::new(DropSpy("second"));
    log(c.0);
}
