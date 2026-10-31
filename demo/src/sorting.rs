//! 排序示例:数组的嵌套循环与分支。
//!
//! 这是节点图最能说明问题的一块 —— 节点图里**没有环**,普通 `for` 循环
//! 会被改写成「有限循环」节点(见 `core/src/compile/loops.rs`),循环体
//! 里的分支是另一组节点。

use rust2genshin_lib::*;

/// 完整示例:冒泡排序一个数组,把结果逐项打出来。
///
/// `arr` 是固定长度的数组 —— 节点图里数组就是列表,按下标读写都走
/// 「写视图」(`list_set`)。
#[unsafe(no_mangle)]
pub fn sorting_demo() {
    let mut arr = [5, 3, 8, 1, 9, 2, 7, 6, 0];
    let n = arr.len();
    // 外层循环:控制趟数
    for i in 0..n {
        // 内层循环:相邻比较交换
        for j in 0..n - 1 - i {
            if arr[j] > arr[j + 1] {
                let c = arr[j];
                arr[j] = arr[j + 1];
                arr[j + 1] = c;
            }
        }
    }
    for i in 0..n {
        log(arr[i]);
    }
}

/// 九九乘法表求和:两层嵌套循环 + 累加。
///
/// 和 [`sorting_demo`] 分开是因为它是「算」而不是「改」:没有写视图,
/// 只有循环和加法。
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
