#![no_std]

#![allow(unused_imports)]

extern crate rust2genshin_lib;

use rust2genshin_lib::*;
use rust2genshin_lib::math::*;

use core::f32::consts::PI;
use rust2genshin_lib::entity::Entity;

#[unsafe(no_mangle)]
pub fn div(a: i32, b: i32) -> i32 {
    a / b
}

#[unsafe(no_mangle)]
pub fn hello_world() {
    log("Hello");
    log("World");
}

#[unsafe(no_mangle)]
pub fn test() -> i32 {
    player::get_player_id_by_guid(Guid(1145))
}

#[unsafe(no_mangle)]
pub fn test1(id: Guid) -> Entity {
    Entity::get(id)
}

#[unsafe(no_mangle)]
pub fn solve(a: f32, b: f32, c: f32) -> f32 {
    (- b + delta(a, b, c).sqrt()) / (2. * a)
}

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
    x != 0
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
pub fn swap_pair(p: (i32, f32)) -> (i32, f32) {
    let mut pair = p;
    let tmp = pair.0;
    pair.0 = pair.1 as i32;
    pair.1 = tmp as f32;
    pair
}

#[unsafe(no_mangle)]
pub fn copy_pair(p: (i32, f32), q: (i32, f32)) -> (i32, f32) {
    let copy = p;
    copy
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

#[unsafe(no_mangle)]
pub fn tuple_eq(p: (i32, f32), q: (i32, f32)) -> bool {
    rust2genshin_lib::tuple_eq!(p, q)
}

#[unsafe(no_mangle)]
pub fn nested_tuple_eq(p: ((i32, f32), bool), q: ((i32, f32), bool)) -> bool {
    rust2genshin_lib::tuple_eq!(p.0, q.0) && p.1 == q.1
}
