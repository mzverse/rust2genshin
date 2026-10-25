#![no_std]

#[allow(unused_imports)]
use rust2genshin_lib::*;
#[allow(unused_imports)]
use rust2genshin_lib::math::*;

#[unsafe(no_mangle)]
pub fn bubble_sort() {
    let mut arr = [5, 3, 8, 1, 9, 2, 7, 6, 0];
    let n = arr.len();
    // 外层循环：控制趟数
    for i in 0..n {
        // 内层循环：相邻比较交换
        for j in 0..n - 1 - i {
            if arr[j] > arr[j + 1] {
                let c= arr[j];
                arr[j] = arr[j + 1];
                arr[j + 1] = c;
            }
        }
    }
    for i in 0..n {
        log(arr[i]);
    }
}
