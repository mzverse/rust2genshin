//! 容器示例:`List`、数组、嵌套列表、结构体列表。
//!
//! 节点图里所有「一串东西」都是列表。`List<T>` 是运行时长度可变的,
//! 数组 `[T; N]` 是定长的 —— 但在 IR 层两者是同一个东西。

use rust2genshin_lib::list::{list, List};
use rust2genshin_lib::*;

use crate::Point;

/// 完整示例:把 `List` 的全套操作走一遍。
///
/// 顺序刻意照着「建 → 读 → 写 → 插 → 删 → 查」的来,读日志就能对上
/// 每一步的效果。
#[unsafe(no_mangle)]
pub fn container_demo() {
    // 建:字面量
    let mut list = list![114, 10, 1919810];
    log(list.len());

    // 读:按下标
    let first = list[0];
    log(first);

    // 写:按下标改值
    list.set(0, 514);
    log(list[0]);

    // 插:中间插入一个,后面的元素后移
    list.insert(1, 7);
    log(list.len());
    log(list[1]);

    // 删:移除下标处的元素,后面的前移
    list.remove(1);
    log(list.len());
    log(list[1]);

    // 追加
    list.push(999);
    log(list.len());
}

/// 数组示例:字面量、重复初始化、按下标读写。
///
/// 数组在节点图里就是列表,但它是**值**不是句柄 —— 赋值是整体拷贝。
#[unsafe(no_mangle)]
pub fn array_demo() {
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
fn array_sum(v: [i32; 2]) -> i32 {
    v[0] + v[1]
}

#[unsafe(no_mangle)]
pub fn array_from_expr_demo() {
    let mut a = [1, 2];
    a[0] = 3;
    log(array_sum([a[0], a[1]]));
}

/// 嵌套列表:列表的元素还是列表。
///
/// 节点图里列表元素只认原生类型和结构体,所以嵌套列表在降级时会被
/// 包成单字段结构体(见 `IrKind::lower_list_elem`)。
#[unsafe(no_mangle)]
pub fn nested_list_demo() -> List<List<i32>> {
    list![list![114, 514], list![1919810]]
}

/// 结构体列表:元素是结构体。
///
/// 结构体元素在列表节点里要带一份结构体依赖声明,引脚的实现方式
/// 也变成 Struct(见 `core/src/node/list.rs`)。
#[unsafe(no_mangle)]
pub fn struct_list_demo() -> List<Point> {
    list![
        Point { x: 114, y: 514 },
        Point { x: 10, y: 5 },
    ]
}

/// 结构体字段是结构体列表:字段的 `kind.sub` 与默认值里的 `structList`
/// 都要带上元素结构体的 id(对照官方导出)。
pub struct PointList {
    pub points: List<Point>,
}

#[unsafe(no_mangle)]
pub fn struct_list_field_demo(v: PointList) -> i32 {
    v.points.len() as i32
}

/// 列表的析构:元素逐个析构。
///
/// `List` 的 `Drop` 会走一遍每个元素的 drop glue —— 节点图里这件事
/// 体现在元素的析构节点上。
struct TestDrop(&'static str);

impl Drop for TestDrop {
    #[inline(always)]
    fn drop(&mut self) {
        log("hello drop");
        log(self.0);
    }
}

#[unsafe(no_mangle)]
pub fn list_drop_demo() {
    _ = core::hint::black_box(list![TestDrop("test"), TestDrop("drop")]);
}
