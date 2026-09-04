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

#[unsafe(no_mangle)]
pub fn solve(a: f32, b: f32, c: f32) -> f32 {
    (- b + delta(a, b, c).sqrt()) / (2. * a)
}

#[inline(never)]
#[unsafe(no_mangle)]
pub fn delta(a: f32, b: f32, c: f32) -> f32 {
    b * b - 4. * a * c
}

#[unsafe(no_mangle)]
pub fn cast_i32_to_f32(x: i32) -> f32 {
    x as f32
}

#[unsafe(no_mangle)]
pub fn cast_f32_to_i32(x: f32) -> i32 {
    x as i32
}

#[unsafe(no_mangle)]
pub fn cast_bool_to_i32(b: bool) -> i32 {
    b as i32
}

#[unsafe(no_mangle)]
pub fn cast_i32_to_bool(x: i32) -> bool {
    x as i32 != 0
}
