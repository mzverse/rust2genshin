#![no_std]

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