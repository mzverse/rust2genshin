#![feature(coroutines)]
#![feature(coroutine_trait)]
#![feature(stmt_expr_attributes, proc_macro_hygiene)]

#![no_std]

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
