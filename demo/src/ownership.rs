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
/// 分配 → 读 → 借用 → 取回内容 → 析构。
#[unsafe(no_mangle)]
pub fn ownership_demo() {
    // 分配:槽位号从空闲表里拿,没有就把堆尾部追加一格
    let b = Box::new(114);

    // 读:解引用就是读堆里那一格
    let v = *b;
    log(v);

    // 借用就是普通引用 —— 没有专门的句柄类型
    let r = &b;
    log(**r);

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

/// 借用 `Box<T>`:只读用 `&Box<T>`,可写用 `PartialMut<Box<T>>`(`&mut` 编成它)。
///
/// 两者**都可储存**(能进局部变量、能当函数参数),因为 `Box<T>` 在 IR 里
/// 落成槽位号 `Int`,借用它就是借用一个标量 —— 不是借用一个「有内部状态的
/// 类型」(`IrKind::is_adt` 为假)。
///
/// 对比:`&Point` 就不行 —— `Point` 是结构体,`PartialMut<Point>` 是 adt,
/// 只能当视图用,存不进局部变量。
#[unsafe(no_mangle)]
pub fn ownership_borrow_demo() {
    let mut b = Box::new(114);

    // 只读借用:`&Box<T>` 在 IR 里就是槽位号这个值,可储存
    let r = &b;
    log(**r);

    // 可写借用:走 `PartialMut<Box<T>>`
    let mut m = PartialMut::from_mut(&mut b);
    log(**m);

    // 借完还能读回原值 —— 借用没把所有权拿走
    log(*b);
}

/// 借用可以当函数参数传来传去。
fn read_slot(b: &Box<i32>) -> i32 {
    **b
}

fn bump_slot(b: PartialMut<'_, Box<i32>>) {
    // `PartialMut` 解引用后仍是 `Box<i32>`,再解一次才到 `i32`
    let v = **b;
    log(v);
}

#[unsafe(no_mangle)]
pub fn ownership_borrow_param_demo() {
    let mut b = Box::new(7);
    log(read_slot(&b));
    bump_slot(PartialMut::from_mut(&mut b));
    log(*b);
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
