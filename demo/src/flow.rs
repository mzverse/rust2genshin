//! 控制流示例:循环、分支、协程。
//!
//! 普通 `for` / `while` 在节点图里没有环,会被改写成「有限循环」节点
//! (见 `core/src/compile/loops.rs`)。协程(`#[asynchronous]`)则是另一
//! 套:它有 `yield`,编译成任务节点。

use rust2genshin_lib::asynchronous::{Resume, async_loop, asynchronous};
use rust2genshin_lib::*;

use core::ops::CoroutineState;

/// 带 `continue` 的循环。
///
/// `continue` 在节点图里是循环节点的一个出口,不是 goto。
#[unsafe(no_mangle)]
pub fn flow_demo() {
    for i in 0..10 {
        if i == 4 {
            continue;
        }
        log(i);
    }
}

/// 协程:每轮 `yield` 一次,外面手动 `resume`。
///
/// `#[asynchronous]` 把函数体包成 `Coroutine<Yield = f32>`;`#[async_loop]`
/// 在循环体末尾插一次 `yield`。
#[asynchronous]
pub fn counter() -> f32 {
    let mut i = 0;
    #[async_loop]
    while i < 5 {
        log(i);
        i += 1;
    }
    114.
}

/// 把协程跑完。
#[unsafe(no_mangle)]
pub fn coroutine_demo() {
    let mut co = counter();
    loop {
        match co.resume() {
            CoroutineState::Yielded(_) => {}
            CoroutineState::Complete(_) => break,
        }
    }
}
