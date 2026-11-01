#![no_std]

// `#[event_listener]` 这类宏会发一条 `register_tool` 注册的属性,后端从
// 那里读它 —— 所以用到这些宏的 crate 要先注册这个 tool。
#![feature(register_tool)]
#![register_tool(rust2genshin)]

#[allow(unused_imports)]
use rust2genshin_lib::*;

use rust2genshin_lib::event::entity::TabSelectedEvent;

// 带 `#[event_listener]` 的函数是**入口**:后端在主图里给它插一个 trigger
// 节点,再挂上事件复合节点。它得是裸的 `pub fn`,事件类型就是第一个参数。
#[event_listener]
pub fn on_tab_selected(event: TabSelectedEvent) {
    log(event.guid);
    event.entity.delete();
}
