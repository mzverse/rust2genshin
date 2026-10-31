//! 常量示例:各种能被编译期求值的字面量。
//!
//! 节点图里的常量不是「内存里的一块字节」,而是**一个标量**加上
//! 「怎么把它拆开」的说明。常量的求值在 `core/src/compile/func.rs` 的
//! `const_to_value` / `const_value_to_valtree` 里做。

use rust2genshin_lib::event::character::ElementType;
use rust2genshin_lib::math::F32;
use rust2genshin_lib::*;

/// 数组常量:三个 `i32`,求和。
const ARR: [i32; 3] = [4, 5, 6];

/// 元组常量:两个不同类型的字段。
const PAIR: (i32, f32) = (7, 8.5);

#[inline(never)]
fn arr_sum3(a: [i32; 3]) -> i32 {
    a[0] + a[1] + a[2]
}

#[inline(never)]
fn pair_sum(p: (i32, f32)) -> f32 {
    p.0 as f32 + p.1
}

/// 完整示例:各类常量各打一条日志。
#[unsafe(no_mangle)]
pub fn value_demo() {
    // 数组常量
    log(arr_sum3(ARR));
    // 元组常量
    log(pair_sum(PAIR));
    // 字符串常量
    log("hello");
    // 句柄常量:`Guid` 是 `#[repr(transparent)]` 包着 `NonZero<i64>`,
    // 值要一路剥到 `i64` 叶子
    log(guid!(114));
}

/// `Option<句柄>` 的常量:枚举用 niche 编码,内存里只有一个标量,
/// variant 靠 `const_variant` 从 tag 反推。
#[inline(never)]
fn opt_is_none(g: Option<Guid>) -> i32 {
    if g.is_none() { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub fn option_const_demo() {
    log(opt_is_none(None));
    log(opt_is_none(Some(guid!(514))));
}

/// `#[native_enum]` 常量(`#[repr(i32)]` 枚举)在节点图里就是整数:valtree
/// 里只有 variant 下标,判别式是 `compile_const` 回 ADT 定义查出来的。
#[inline(never)]
fn element() -> ElementType {
    ElementType::Pyro
}

#[unsafe(no_mangle)]
pub fn native_enum_demo() {
    if element() == ElementType::Pyro {
        log(1);
    }
}

/// 单字段结构体的常量:内存布局就是一个标量,`ConstValue` 也是标量而不是
/// 整块内存,拆字段时要把值原样往下传(见 `const_value_to_valtree` 的
/// `field_value`)。
struct Wrapper(i32);

#[inline(never)]
fn wrapper_sum(w: Wrapper) -> i32 {
    w.0
}

#[unsafe(no_mangle)]
pub fn single_field_struct_demo() {
    let w = Wrapper(42);
    log(wrapper_sum(w));
}

/// 全 ZST 的聚合:`ConstValue::ZeroSized`,每个字段也还是 `ZeroSized`。
struct Empty;
struct Nested(Empty);

#[inline(never)]
fn nested_id(_: Nested) -> i32 {
    7
}

#[unsafe(no_mangle)]
pub fn zero_sized_demo() {
    log(nested_id(Nested(Empty)));
}

/// `usize` 常量:8 字节的叶子,`NativeKind::Int` 要按 32 位截断取。
const N: usize = 3;

#[inline(never)]
fn add_n(a: i32, b: usize) -> i32 {
    a + b as i32
}

#[unsafe(no_mangle)]
pub fn usize_const_demo() {
    log(add_n(1, N));
}

/// 浮点转整数:Rust 的 `as` 是向零取整(`3.9 → 3`),编译成取整节点 +
/// 截尾取整。取整成整数用 `itrunc`(向零),与 `as` 语义一致。
#[unsafe(no_mangle)]
pub fn float_to_int_demo(f: f32) -> i32 {
    f.itrunc()
}
