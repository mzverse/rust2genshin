//! 已知不支持的用法 —— 默认**不编译**。
//!
//! 这些是后端还没落地的缺口,留在这里当「待办清单」:想复现某个报错时,
//! 打开 `unsupported` feature 即可:
//!
//! ```shell
//! cargo build -p rust2genshin-demo --features unsupported
//! ```
//!
//! 每条都注明了**为什么**不行。

#![cfg(feature = "unsupported")]

use rust2genshin_lib::boxed::Box;
use rust2genshin_lib::*;

use crate::Point;

/// `Box<Point>` —— 堆变量类型是 `List<Struct>`。
///
/// 图变量节点(`Get_Graph_Variable` ID 337 / `Set_Graph_Variable` ID 323)
/// 各有 153 个变体,里面**没有结构体档** —— 最靠后的是
/// `C<T:D<Pfb,L<Cfg>>>`。所以 `List<Struct>` 类型的黑板变量在游戏里
/// 没有对应的节点。
///
/// (列表节点是有结构体变体的,见 `core/src/node/list.rs`;但图变量节点没有。)
#[unsafe(no_mangle)]
pub fn box_of_struct_demo() {
    let b = Box::new(Point { x: 1, y: 2 });
    let p = &*b;
    log(p.x);
}

/// `Box<Point>` 的字段写 —— 同样卡在上面的堆变量类型上。
#[unsafe(no_mangle)]
pub fn box_field_write_demo(m: &mut Box<Point>) {
    m.x = 114;
}

/// `*b = v` 整体赋值 —— 假 `&mut` 造不出写句柄。
///
/// 节点图里没有写句柄,写元素走的是「写视图」(`list_set` / `Modify`)。
/// 由视图造出来的 `&mut` 读得了、借得了字段、调得了 `&mut self` 的方法,
/// 但整体赋值要真句柄,降级时报 `Unsupported`。
#[unsafe(no_mangle)]
pub fn box_whole_assign_demo(m: &mut Box<i32>) {
    **m = 114;
}
