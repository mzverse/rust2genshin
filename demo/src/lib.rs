#![no_std]

#![allow(unused_imports)]

extern crate rust2genshin_lib;

use rust2genshin_lib::*;
use rust2genshin_lib::math::*;

use core::f32::consts::PI;
use rust2genshin_lib::entity::Entity;


struct MyStruct(i32);
impl MyStruct {
    #[inline(never)]
    fn new(i: i32) -> Self {
        Self(i)
    }
}

#[unsafe(no_mangle)]
pub fn test_struct(f: bool) {
    let v= if f {
        MyStruct::new(114)
    } else {
        MyStruct::new(514)
    };
    log(v.0);
}

#[unsafe(no_mangle)]
pub fn test_mut() {
    let mut i = 114i32;
    test_mut_1(&mut i);
    log(i);
}

#[unsafe(no_mangle)]
pub fn test_mut_1(i: &mut i32) {
    *i = 514;
}

#[unsafe(no_mangle)]
pub fn solve(a: f32, b: f32, c: f32) -> f32 {
    (-b + delta(a, b, c).sqrt()) / (2. * a)
}

pub fn delta(a: f32, b: f32, c: f32) -> f32 {
    b * b - 4. * a * c
}

#[unsafe(no_mangle)]
pub fn hello_world() {
    log("Hello");
    log("World");
}

#[unsafe(no_mangle)]
#[inline(never)]
pub fn solve2(a: f32, b: f32, c: f32) -> (f32, f32) {
    let delta_sqrt = delta(a, b, c).sqrt();
    (
        (-b - delta_sqrt) / (2. * a), // x1
        (-b + delta_sqrt) / (2. * a), // x2
    )
}

#[unsafe(no_mangle)]
pub fn test_solve2() {
    let (x1, x2) = solve2(1., -5., 6.);
    log(x1);
    log(x2);
}

#[unsafe(no_mangle)]
pub fn test_guid(id: Guid) -> Entity {
    Entity::get(id)
}

#[unsafe(no_mangle)]
pub fn to_celsius(f: f32) -> f32 {
    (f - 32.) * 5. / 9.
}


#[unsafe(no_mangle)]
pub fn make_tuple(a: i32, b: f32) -> (i32, f32) {
    (a, b)
}

#[unsafe(no_mangle)]
pub fn tuple_first(t: (i32, f32)) -> i32 {
    t.0
}

#[unsafe(no_mangle)]
pub fn tuple_second(t: (i32, f32)) -> f32 {
    t.1
}

#[unsafe(no_mangle)]
pub fn nested_tuple_first(t: ((i32, f32), bool)) -> i32 {
    t.0.0
}

#[unsafe(no_mangle)]
pub fn swap_pair(mut pair: (i32, f32)) -> (i32, f32) {
    let tmp = pair.0;
    pair.0 = pair.1 as i32;
    pair.1 = tmp as f32;
    pair
}

#[unsafe(no_mangle)]
pub fn copy_pair(p: (i32, f32), _q: (i32, f32)) -> (i32, f32) {
    p
}

#[unsafe(no_mangle)]
pub fn update_field(p: (i32, f32), v: i32) -> (i32, f32) {
    let mut pair = p;
    pair.0 = v;
    pair
}

#[unsafe(no_mangle)]
pub fn nested_update(p: ((i32, f32), bool), n: i32) -> ((i32, f32), bool) {
    let mut pair = p;
    pair.0.0 = n;
    pair
}
