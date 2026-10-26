#![feature(coroutines)]
#![feature(coroutine_trait)]
#![feature(stmt_expr_attributes, proc_macro_hygiene)]

#![no_std]

use core::hint::black_box;
#[allow(unused_imports)]
use rust2genshin_lib::*;
#[allow(unused_imports)]
use rust2genshin_lib::math::*;

use rust2genshin_lib::asynchronous::{Resume, async_loop, asynchronous};
use rust2genshin_lib::list::{list, List};
use rust2genshin_lib::PartialMut;
use rust2genshin_lib::event::entity::EntityCreatedEvent;

use core::ops::CoroutineState;

#[asynchronous]
pub fn co0() -> f32 {
    let mut i = 10;
    #[async_loop]
    while i < 15 {
        log(i);
        i += 1;
    }
    114.
}

#[asynchronous]
pub fn co() {
    // co0().await;
    let mut i = 0;
    #[async_loop]
    while i < 5 {
        log(i);
        i += 1;
    }
}

/// 普通函数里的循环。IR 里这是控制流上的环，native 节点图里没有环 ——
/// 改写成「有限循环」节点（见 core/src/compile/loops.rs）。
#[unsafe(no_mangle)]
pub fn test_loop() {
    for i in 0..10 {
        if i == 4 {
            continue;
        }
        log(i);
    }
}

#[unsafe(no_mangle)]
pub fn test_co() {
    let mut co = co();
    loop {
        match co.resume() {
            CoroutineState::Yielded(_) => {}
            CoroutineState::Complete(_) => break,
        }
    }
}

/// 计算九九乘法表的总和
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

// use rust2genshin_lib::entity::Entity;
// use rust2genshin_lib::event::entity::TabSelectedEvent;
//
// #[unsafe(no_mangle)]
// pub fn test_closure(i: i32) {
//     #[inline(never)]
//     fn f<T: FnOnce(&mut i32)>(t: T) {
//         let mut i = 0;
//         t(&mut i);
//         log(i);
//     }
//     f(|j| {
//         *j = 114;
//         log(i);
//     });
// }
//
// #[event_listener]
// pub fn on_tab_selected(event: TabSelectedEvent) {
//     log(event.guid);
//     event.entity.delete();
// }
//
// struct MyStruct(i32);
// impl MyStruct {
//     #[inline(never)]
//     fn new(i: i32) -> Self {
//         Self(i)
//     }
// }
//
// #[unsafe(no_mangle)]
// pub fn test_struct(f: bool) {
//     let v= if f {
//         MyStruct::new(114)
//     } else {
//         MyStruct::new(514)
//     };
//     log(v.0);
// }
//
// #[unsafe(no_mangle)]
// pub fn test_mut() {
//     let mut i = 114i32;
//     test_mut_1(&mut i);
//     log(i);
// }
//
// #[unsafe(no_mangle)]
// pub fn test_mut_1(i: &mut i32) {
//     *i = 514;
// }
//
// #[unsafe(no_mangle)]
// pub fn solve(a: f32, b: f32, c: f32) -> f32 {
//     (-b + delta(a, b, c).sqrt()) / (2. * a)
// }
//
// pub fn delta(a: f32, b: f32, c: f32) -> f32 {
//     b * b - 4. * a * c
// }
//
// #[unsafe(no_mangle)]
// pub fn hello_world() {
//     log("Hello");
//     log("World");
// }
//
// #[unsafe(no_mangle)]
// #[inline(never)]
// pub fn solve2(a: f32, b: f32, c: f32) -> (f32, f32) {
//     let delta_sqrt = delta(a, b, c).sqrt();
//     (
//         (-b - delta_sqrt) / (2. * a), // x1
//         (-b + delta_sqrt) / (2. * a), // x2
//     )
// }
//
// #[unsafe(no_mangle)]
// pub fn test_solve2() {
//     let (x1, x2) = solve2(1., -5., 6.);
//     log(x1);
//     log(x2);
// }
//
// #[unsafe(no_mangle)]
// pub fn test_guid(id: Guid) -> Option<Entity> {
//     Entity::get(id)
// }
//
// #[unsafe(no_mangle)]
// pub fn to_celsius(f: f32) -> f32 {
//     (f - 32.) * 5. / 9.
// }
//
//
// #[unsafe(no_mangle)]
// pub fn make_tuple(a: i32, b: f32) -> (i32, f32) {
//     (a, b)
// }
//
// #[unsafe(no_mangle)]
// pub fn tuple_first(t: (i32, f32)) -> i32 {
//     t.0
// }
//
// #[unsafe(no_mangle)]
// pub fn tuple_second(t: (i32, f32)) -> f32 {
//     t.1
// }
//
// #[unsafe(no_mangle)]
// pub fn nested_tuple_first(t: ((i32, f32), bool)) -> i32 {
//     t.0.0
// }
//
// #[unsafe(no_mangle)]
// pub fn swap_pair(mut pair: (i32, f32)) -> (i32, f32) {
//     let tmp = pair.0;
//     pair.0 = pair.1 as i32;
//     pair.1 = tmp as f32;
//     pair
// }
//
// #[unsafe(no_mangle)]
// pub fn copy_pair(p: (i32, f32), _q: (i32, f32)) -> (i32, f32) {
//     p
// }
//
// #[unsafe(no_mangle)]
// pub fn update_field(p: (i32, f32), v: i32) -> (i32, f32) {
//     let mut pair = p;
//     pair.0 = v;
//     pair
// }
//
// #[unsafe(no_mangle)]
// pub fn nested_update(p: ((i32, f32), bool), n: i32) -> ((i32, f32), bool) {
//     let mut pair = p;
//     pair.0.0 = n;
//     pair
// }

#[unsafe(no_mangle)]
pub fn test_list() -> List<i32> {
    log(list![114, 10].len());
    list![114, 10]
}

/// 列表插入:元素进列表,列表本身不动。
#[unsafe(no_mangle)]
pub fn test_list_insert() {
    let mut list = list![114, 10];
    list.insert(1, 5);
}

/// 列表读 / 写 / 长度 / push。
#[unsafe(no_mangle)]
pub fn test_list_ops() {
    let mut list = list![114, 10];
    let a = list[0];
    list.set(0, a + 1);
    list.push(5);
    log(list.len());
}

/// 数组 → 列表:字面量、重复初始化、按下标读、按下标写。
#[unsafe(no_mangle)]
pub fn test_array() {
    let mut a = [1, 2, 3];
    a[0] = 7;
    let mut b = [0; 3];
    b[1] = 10;
    let i = 2;
    b[i] = 20;
    log(a[0] + b[1] + b[i]);
}

/// 元素不是常量时的数组字面量:拼装节点前面会多出「元素个数」引脚,
/// 已有连线得整体后移(见 `NodeGraph::insert_value_in_front`)。
/// `#[inline(never)]` 是为了让聚合点真的保留下来,别被 MIR 优化拆掉。
#[inline(never)]
fn array_sum(v: [i32; 2]) -> i32 { v[0] + v[1] }

#[unsafe(no_mangle)]
pub fn test_array2() {
    let mut a = [1, 2];
    a[0] = 3;
    log(array_sum([a[0], a[1]]));
}

/// 常量求值:数组、元组、`&str` 都走 valtree(见 `compile/func.rs` 里的 `const_to_value`)。
const ARR: [i32; 3] = [4, 5, 6];
const PAIR: (i32, f32) = (7, 8.5);

#[inline(never)]
fn arr_sum3(a: [i32; 3]) -> i32 { a[0] + a[1] + a[2] }

#[inline(never)]
fn pair_sum(p: (i32, f32)) -> f32 { p.0 as f32 + p.1 }

#[unsafe(no_mangle)]
pub fn test_const() {
    log(arr_sum3(ARR));
    log(pair_sum(PAIR));
    log("hello");
}

/// `Option<句柄>` 的常量:枚举用 niche 编码,内存里只有一个标量,
/// variant 靠 `const_variant` 从 tag 反推。
#[inline(never)]
fn opt_is_none(g: Option<Guid>) -> i32 { if g.is_none() { 1 } else { 0 } }

#[unsafe(no_mangle)]
pub fn test_option() {
    log(opt_is_none(None));
    log(opt_is_none(Some(guid!(514))));
}

/// 原生句柄常量:`Guid` 是 `#[repr(transparent)]` 包着 `NonZero<i64>`,
/// 值要一路剥到 `i64` 叶子再交给 `native_const`。
#[unsafe(no_mangle)]
pub fn test_guid_const() {
    log(guid!(114));
}

/// `#[native_enum]` 常量(`#[repr(i32)]` 枚举)在节点图里就是整数:valtree 里只有
/// variant 下标,判别式是 `compile_const` 回 ADT 定义查出来的。
#[inline(never)]
fn element() -> event::character::ElementType {
    event::character::ElementType::Pyro
}

/// 单字段结构体的常量:内存布局就是一个标量,`ConstValue` 也是标量而不是整块内存,
/// 拆字段时要把值原样往下传(见 `const_value_to_valtree` 的 `field_value`)。
struct Wrapper(i32);

#[inline(never)]
fn wrapper_sum(w: Wrapper) -> i32 { w.0 }

#[unsafe(no_mangle)]
pub fn test_wrapper() {
    let w = Wrapper(42);
    log(wrapper_sum(w));
}

/// 全 ZST 的聚合:`ConstValue::ZeroSized`,每个字段也还是 `ZeroSized`。
struct Empty;
struct Nested(Empty);

#[inline(never)]
fn nested_id(_: Nested) -> i32 { 7 }

#[unsafe(no_mangle)]
pub fn test_zst() {
    log(nested_id(Nested(Empty)));
}

/// `usize` 常量:8 字节的叶子,`NativeKind::Int` 要按 32 位截断取。
const N: usize = 3;

#[inline(never)]
fn add_n(a: i32, b: usize) -> i32 { a + b as i32 }

#[unsafe(no_mangle)]
pub fn test_usize_const() {
    log(add_n(1, N));
}

#[unsafe(no_mangle)]
pub fn test_native_enum() {
    if element() == event::character::ElementType::Pyro {
        log(1);
    }
}

struct Point {
    x: i32,
    y: i32,
}

#[unsafe(no_mangle)]
pub fn test_struct_list() -> List<Point> {
    list![Point { x: 114, y: 514 }, Point { x: 10, y: 5 }]
}

#[unsafe(no_mangle)]
pub fn test_list_list() -> List<List<i32>> {
    list![list![114, 514], list![1919810]]
}

/// `&mut` 参数 + 字段赋值:字段写是 `Modify`,节点图里没有字段句柄。
#[inline(never)]
fn set_x(p: &mut Point, v: i32) {
    p.x = v;
}

#[unsafe(no_mangle)]
pub fn test_mut_field() {
    let mut p = Point { x: 1, y: 2 };
    set_x(&mut p, 3);
    log(p.x + p.y);
}

/// 可变视图:把 `&mut` 变成一个能读能写的句柄。
#[unsafe(no_mangle)]
pub fn test_partial_mut() {
    let mut list = list![114, 10];
    let pm = PartialMut::from(&mut list);
    log(pm.len());
}

#[unsafe(no_mangle)]
pub fn test_swap(a: &mut i32, b: &mut i32) {
    let c = *a;
    *a = *b;
    *b = c;
}

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

#[unsafe(no_mangle)]
pub fn test_set(p: &mut i32) {
    *p = 114514;
}

#[inline(never)]
fn borrow_read(x: i32) -> i32 {
    let r = &x;
    *r + 1
}

#[inline(never)]
fn borrow_tuple_read(p: (i32, i32)) -> i32 {
    let q = &p;
    q.0 + q.1
}

/// 借用:`Freeze` 的 `&T` 等同 T,取借用等于读值。
#[unsafe(no_mangle)]
pub fn test_borrow() {
    log(borrow_read(3));
}

/// `Freeze` 的 `&(i32, i32)` 不扁平化,整体是一个 ADT 局部变量。
#[unsafe(no_mangle)]
pub fn test_borrow_tuple() {
    log(borrow_tuple_read((1, 2)));
}

/// 浮点转整数:Rust 的 `as` 是向零取整(`3.9 → 3`),编译成取整节点 + 截尾取整。
#[unsafe(no_mangle)]
pub fn test_float_to_int(f: f32) -> i32 {
    f as i32
}

/// 取整成整数:`itrunc` 截尾取整(向零),与 Rust 的 `as` 语义一致。
#[unsafe(no_mangle)]
pub fn test_itrunc(f: f32) -> i32 {
    f.itrunc()
}

/// 事件监听器:入口点,会生成主图(实体节点图)。
#[event_listener]
pub fn on_entity_created(event: EntityCreatedEvent) {
    let count = pair_sum((1, 2.5));
    log(count);
}

struct TestDrop(String);

impl Drop for TestDrop {
    #[inline(always)]
    fn drop(&mut self) {
        log("hello drop");
        log(self.0);
    }
}

#[unsafe(no_mangle)]
pub fn test_list_drop() {
    _ = black_box(list![TestDrop("test"), TestDrop("drop")]);
}

/// 结构体字段是结构体列表:字段的 `kind.sub` 与默认值里的 `structList`
/// 都要带上元素结构体的 id(对照官方导出)。
struct PointList {
    points: List<Point>,
}

#[unsafe(no_mangle)]
pub fn test_struct_list_field(v: PointList) -> i32 {
    v.points.len() as i32
}
