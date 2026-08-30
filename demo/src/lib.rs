#![no_std]

extern crate rust2genshin_lib;

use rust2genshin_lib::*;

// #[event_listener]
// pub fn script_init() -> i32 {
//     0
// }
//
// #[event_listener]
// pub fn script_tick(dt: f32, frame: i32) -> f32 {
//     dt + frame as f32
// }
//
// #[event_listener]
// pub fn script_heal(amount: i32) -> i32 {
//     amount
// }
//
// // 普通辅助函数(被入口点调用 → 也会变成复合节点)
// fn helper_add(a: i32, b: i32) -> i32 {
//     a + b
// }
//
// #[doc(hidden)]
// #[event_listener]
// pub fn script_use_helper() -> i32 {
//     helper_add(1, 2)
// }

// pub extern "Rust" fn my_fn(a: i32, b: i32, C: i32) -> i32 {
//     (0 - b + (b * b - 4 * a * C)) * 2
// }

pub fn con() -> i32 {
    114
}

const C: fn() -> i32 = con;

#[unsafe(no_mangle)]
pub fn test(a: i32, b: i32) -> i32 {
    C() + bi(a, b) + b
}

pub fn bi(a: i32, b: i32) -> i32 {
    a + b * b
}

#[cfg(false)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = helper_add(2, 2);
        assert_eq!(result, 4);
    }
}
