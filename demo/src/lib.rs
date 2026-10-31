//! 示例集合 —— 每块是一个能独立跑的「示例功能」。
//!
//! 早先这里是三十多个 `test_*` 小函数,一个函数测一个后端特性,产物
//! (`.gia`) 里也就三十多个互不相干的复合节点,看不出「节点图能干什么」。
//!
//! 现在按主题分成若干模块,每个模块导出一个**完整的示例程序** ——
//! 像 [`sorting::sorting_demo`] 那样:有输入、有过程、有输出。示例内部
//! 用到的那些「只为了测一个特性」的小函数都收进模块里,不再对外暴露。
//!
//! 模块一览:
//!
//! | 模块 | 示例 | 覆盖的东西 |
//! |---|---|---|
//! | [`sorting`] | `sorting_demo` | 嵌套循环、分支、数组按下标读写 |
//! | [`containers`] | `container_demo` | `List` 全套操作、数组、嵌套列表、结构体列表 |
//! | [`ownership`] | `ownership_demo` | `Box` 的分配 / 读 / 借用 / 析构 |
//! | [`values`] | `value_demo` | 各类常量:数组、元组、`&str`、`Option`、枚举、结构体、ZST |
//! | [`mutability`] | `mutability_demo` | `&mut` 字段写、借用、`PartialMut`、`IndexMut` |
//! | [`flow`] | `flow_demo` | 循环与 `continue`、嵌套循环、协程 |
//!
//! 已知不支持的用法放在 [`unsupported`] 里,默认**不编译** —— 打开
//! `unsupported` feature 才会编进来,用来复现那些还没落地的后端缺口。
//!
//! 哪些特性归哪个模块、哪些还没覆盖,见 `src/NOTES.md`。

#![feature(coroutines)]
#![feature(coroutine_trait)]
#![feature(stmt_expr_attributes, proc_macro_hygiene)]

#![no_std]

#[allow(unused_imports)]
use rust2genshin_lib::*;
#[allow(unused_imports)]
use rust2genshin_lib::math::*;

use rust2genshin_lib::event::entity::EntityCreatedEvent;

pub mod containers;
pub mod flow;
pub mod mutability;
pub mod ownership;
pub mod sorting;
pub mod unsupported;
pub mod values;

/// 事件监听器:入口点,会生成主图(实体节点图)。
///
/// 这里把六个示例都调一遍 —— 复合节点的调用点在主图里排成一片,
/// 正好能看出「一个主图挂多个子图」长什么样。
#[event_listener]
pub fn on_entity_created(event: EntityCreatedEvent) {
    log(event.guid);
    sorting::sorting_demo();
    containers::container_demo();
    ownership::ownership_demo();
    values::value_demo();
    mutability::mutability_demo();
    flow::flow_demo();
}

/// 坐标点。字段写的例子(`&mut` 参数、`Box` 借用)都要用它。
pub struct Point {
    pub x: i32,
    pub y: i32,
}
