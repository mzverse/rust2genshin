#![no_std]

#[allow(unused_imports)]
use rust2genshin_lib::*;
#[allow(unused_imports)]
use rust2genshin_lib::math::*;


#[unsafe(no_mangle)]
pub fn hello_world() {
    log("Hello");
    log("World");
}
