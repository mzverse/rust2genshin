#![no_std]

#[allow(unused_imports)]
use rust2genshin_lib::*;
#[allow(unused_imports)]
use rust2genshin_lib::math::*;


/// 计算九九乘法表的总和
#[unsafe(no_mangle)]
pub fn times99() {
    let mut sum = 0;
    for i in 1..10 {
        for j in 1..10 {
            sum += i * j;
        }
    }
    log(sum);
}
