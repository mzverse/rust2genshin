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

/// `Box<Point>` 的**字段写**。
///
/// 堆变量类型 `List<Point>` 本身是支持的(读路径见
/// [`crate::ownership::ownership_struct_demo`],图变量节点走 `using_struct`,
/// 选择器 22、`type1` 175)。卡住的是**写**:`m.x = 114` 要 `BoxRefMut` 的
/// `DerefMut`,那条路要真写句柄 —— 见下面 `box_whole_assign_demo` 的说明。
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
