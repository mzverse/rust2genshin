#![no_std]

extern crate rust2genshin_lib;

use rust2genshin_lib::*;
use rust2genshin_lib::math::*;

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

// #[unsafe(no_mangle)]
// pub fn test(a: i32, b: i32) -> i32 {
//     a >> b
// }

#[unsafe(export_name = "awawa")]
pub fn test1(a: i32, b: i32) -> i32 {
    a.shr(b)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
