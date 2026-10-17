#![no_std]

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