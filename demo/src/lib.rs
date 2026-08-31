#![no_std]

extern crate rust2genshin_lib;

use math::*;
use rust2genshin_lib::*;

use core::f32::consts::PI;

#[unsafe(no_mangle)]
pub fn dis_square(a: f32, b: f32) -> f32 {
    a * a + b * b
}

#[unsafe(no_mangle)]
pub fn circumference(r: f32) -> f32 {
    2. * PI * r
}

#[unsafe(no_mangle)]
pub fn div(a: i32, b: i32) -> i32 {
    a.divide(b)
}

#[unsafe(no_mangle)]
pub fn hello_world() {
    log("Hello");
    log("World");
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
