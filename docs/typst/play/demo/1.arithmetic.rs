#![no_std]

// `rust2genshin_lib` 的宏(`#[native]` / `#[event_listener]` …)会发一条
// `register_tool` 注册的属性,后端从那里读 —— 所以用到这些宏的 crate 要
// 先注册这个 tool。
#![feature(register_tool)]
#![register_tool(rust2genshin)]

#[allow(unused_imports)]
use rust2genshin_lib::*;
#[allow(unused_imports)]
use rust2genshin_lib::math::*;

#[unsafe(no_mangle)]
pub fn arithmetic() {
    let a = 17;
    let b = 25;
    log(a + b);
    log(a * b);
    log(a < b);
    // 位运算也走同一批算术节点;`&`/`>>` 是 i32 上的,结果还是 i32。
    log((a & b) >> 1);
}
