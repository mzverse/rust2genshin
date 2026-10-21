#![no_std]

#[allow(unused_imports)]
use rust2genshin_lib::*;
#[allow(unused_imports)]
use rust2genshin_lib::math::*;


#[unsafe(no_mangle)]
pub fn times99() {
    let mut sum = 0;
    let mut i = 1;
    while i < 10 {
        let mut j = 1;
        while j < 10 {
            sum += i * j;
            j += 1;
        }
        i += 1;
    }
    log(sum);
}
