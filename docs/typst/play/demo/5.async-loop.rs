#![feature(coroutines)]
#![feature(coroutine_trait)]
#![feature(stmt_expr_attributes, proc_macro_hygiene)]

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

use core::ops::CoroutineState;
use rust2genshin_lib::asynchronous::{Resume, async_loop, asynchronous};

#[asynchronous]
pub fn count_to_five() {
    let mut i = 0;
    #[async_loop]
    while i < 5 {
        log(i);
        i += 1;
    }
}

#[unsafe(no_mangle)]
pub fn async_loop_demo() {
    let mut co = count_to_five();
    loop {
        match co.resume() {
            CoroutineState::Yielded(_) => {}
            CoroutineState::Complete(_) => break,
        }
    }
}
