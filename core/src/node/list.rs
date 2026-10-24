//! 列表相关节点(Server)
//!
//! 游戏里这些节点都是**泛型变体节点**(参考项目 `utils/node_data/data.json` 里
//! `Type = Variant`):shell ID 固定,`kernel` 与「类型选择器下标」随元素类型变。
//! 比如清除列表 shell = 107,元素是 Int 时 kernel = 107、是 Str 时 kernel = 108。
//!
//! 变体表照抄参考项目 data.json 的 `Variants`(约束 `C<T:X>` → `KernelID`),
//! 选择器下标就是变体在 `TypeSelectorIndex` 里的位置。**各个节点的顺序不一样**:
//! 多数是 Int→0…Fct→9,取最大 / 取最小 / 排序只有 Int→0、Flt→1,
//! 遍历是 Bol→0…Fct→9。
//!
//! 选择器在 wire 上叫 `PolymorphicValue.chosen_type_index`(参考项目
//! `utils/protobuf/gia.proto`),编辑器读回来叫 `poly_value`,它的值就是变体表里
//! 的 `TypeSelectorIndex` —— 参考项目 `utils/gia_gen/interface.ts` 正是拿
//! `TypeSelectorIndex === poly_value` 反查变体的。
//!
//! 元素类型是结构体时,节点要多带一份结构体依赖声明
//! (`NodeInstance.DependencyDeclaration`),并把吃泛型的引脚改成 `Struct` 实现
//! (引脚的类型标签相应变成 `S_STRUCT` / `S_STRUCT_LIST`)。它的两个未知字段按
//! 实测 `.gia` 填:
//!
//! - `type1`:参考项目**没有实现**(他们 proto 里那行是注释掉的 TODO),
//!   `data.json` 里也没有这个字段。实测列表节点一律是 10,见 [`STRUCT_TYPE1`]。
//! - 选择器下标:结构体接在最后一档后面 —— 事件节点(ID 428)的选择器占 0..20,
//!   结构体用 21(见 `node_on_native_custom_value_change`)。列表节点因此是 10,
//!   与实测的 `chosen_type_index` 一致,见 [`struct_selector`]。
//!
//! 命名统一 `node_list_*`:列表这一类节点散在 Execution / Query / Arithmetic
//! 三个域里,收在一处才好找。

use crate::node::{NodeGraph, NodeKind};
use crate::structure::node_struct;
use crate::value::NativeKind::{self, *};

/// 多数列表节点的泛型顺序:Int→0、Str→1、Ety→2、Gid→3、Flt→4、
/// Vec→5、Bol→6、Cfg→7、Pfb→8、Fct→9。
const COMMON: &[NativeKind] = &[Int, String, Entity, Guid, Float, Vec3, Bool, Config, Prefab, Faction];

/// 只支持数值的节点(取最大 / 取最小 / 排序)的泛型顺序:Int→0、Flt→1。
const NUMERIC: &[NativeKind] = &[Int, Float];

/// 列表遍历的泛型顺序:Bol→0、Ety→1、Flt→2、Gid→3、Int→4、Str→5、
/// Vec→6、Cfg→7、Pfb→8、Fct→9。
const ITERATE: &[NativeKind] = &[Bool, Entity, Float, Guid, Int, String, Vec3, Config, Prefab, Faction];

/// 一个列表节点的变体表:`order` 是泛型顺序(元素类型 → 选择器下标),
/// `kernels` 与 `order` 一一对应。
struct Variants {
    /// shell ID:同一节点的所有变体共用
    shell: i64,
    order: &'static [NativeKind],
    kernels: &'static [i64],
}

/// 各列表节点的变体表(kernel 抄自参考项目 data.json)。
const VARIANTS: &[Variants] = &[
    // Execution.List_Operation.Concatenate
    Variants { shell: 100, order: COMMON, kernels: &[100, 101, 102, 103, 104, 105, 106, 550, 551, 2654] },
    // Execution.List_Operation.Clear
    Variants { shell: 107, order: COMMON, kernels: &[107, 108, 109, 110, 111, 112, 113, 552, 553, 2653] },
    // Query.List_Related.Contains
    Variants { shell: 114, order: COMMON, kernels: &[114, 115, 116, 117, 118, 119, 120, 554, 555, 2655] },
    // Query.List_Related.Find_Index
    Variants { shell: 121, order: COMMON, kernels: &[121, 122, 123, 124, 125, 126, 127, 556, 557, 2652] },
    // Query.List_Related.Get_At_Index
    Variants { shell: 128, order: COMMON, kernels: &[128, 129, 130, 131, 132, 133, 134, 558, 559, 2651] },
    // Execution.List_Operation.Insert
    Variants { shell: 135, order: COMMON, kernels: &[135, 136, 137, 138, 139, 140, 141, 560, 561, 2650] },
    // Query.List_Related.Get_Length
    Variants { shell: 142, order: COMMON, kernels: &[142, 143, 144, 145, 146, 147, 148, 562, 563, 2645] },
    // Query.List_Related.Get_Max
    Variants { shell: 149, order: NUMERIC, kernels: &[2648, 2649] },
    // Query.List_Related.Get_Min
    Variants { shell: 151, order: NUMERIC, kernels: &[2646, 2647] },
    // Execution.List_Operation.Remove
    Variants { shell: 153, order: COMMON, kernels: &[153, 154, 155, 156, 157, 158, 159, 564, 565, 2644] },
    // Execution.List_Operation.Modify_Index
    Variants { shell: 160, order: COMMON, kernels: &[160, 161, 162, 163, 164, 165, 166, 566, 567, 2643] },
    // Execution.List_Operation.Sort
    Variants { shell: 167, order: NUMERIC, kernels: &[167, 168] },
    // Arithmetic.General.Assemble_List
    Variants { shell: 169, order: COMMON, kernels: &[169, 170, 171, 172, 173, 174, 175, 568, 569, 2640] },
    // Execution.List_Operation.For_Each
    Variants { shell: 509, order: ITERATE, kernels: &[509, 510, 511, 512, 513, 514, 515, 570, 571, 3280] },
];

/// 查某个 shell 在给定元素类型下的 (kernel, 选择器下标)。
fn variant(shell: i64, ty: &NativeKind) -> Option<(i64, i32)> {
    let variants = VARIANTS.iter().find(|v| v.shell == shell)?;
    let index = variants.order.iter().position(|t| t == ty)?;
    Some((variants.kernels[index], index as i32))
}

/// 控制流引脚数(入口 / 出口)。
struct Flow(usize, usize);

/// 一个值引脚:类型 + 是否吃泛型选择器。
///
/// 吃选择器的引脚就是泛型参数的位置(列表本身、元素、元素的下标…),
/// 其余引脚与元素类型无关(插入用的下标是 Int、排序的规则是枚举)。
struct ValuePin {
    ty: NativeKind,
    generic: bool,
}

impl ValuePin {
    /// 与元素类型无关的引脚。
    fn plain(ty: NativeKind) -> Self {
        Self { ty, generic: false }
    }

    /// 吃泛型选择器的引脚。
    fn poly(ty: NativeKind) -> Self {
        Self { ty, generic: true }
    }
}

/// 造一个列表泛型节点。
///
/// 元素类型是结构体时走 [`node_struct`]:要额外写结构体依赖声明,引脚的实现
/// 方式也变成 Struct。结构体类型在游戏里是按 schema 注册的,参考项目的变体表
/// 里没有这一档,`type1` 与选择器下标按实测 `.gia` 填(见 [`STRUCT_TYPE1`])。
fn generic<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    shell: i64,
    ty: &NativeKind,
    flow: Flow,
    inputs: Vec<ValuePin>,
    outputs: Vec<ValuePin>,
) -> NodeKind {
    let mut kind = NodeKind::simple(
        shell,
        flow.0,
        flow.1,
        inputs.iter().map(|p| p.ty.clone()).collect(),
        outputs.iter().map(|p| p.ty.clone()).collect(),
    );
    if let NativeKind::Struct(st) = ty {
        if !struct_supported(shell) {
            panic!("列表节点 {shell} 不支持元素类型 {ty:?}");
        }
        // kernel 由 node_struct 清零,这里只需要引脚的形状
        return node_struct(
            graph,
            kind,
            st,
            STRUCT_TYPE1,
            struct_selector(shell),
            generic_pins(&inputs),
            generic_pins(&outputs),
        );
    }
    let (kernel, selector) =
        variant(shell, ty).unwrap_or_else(|| panic!("列表节点 {shell} 不支持元素类型 {ty:?}"));
    kind.id.kernel = kernel;
    for i in generic_pins(&inputs) {
        kind.id.selectors_in[i] = Some(selector);
    }
    for i in generic_pins(&outputs) {
        kind.id.selectors_out[i] = Some(selector);
    }
    kind
}

/// 吃泛型选择器的引脚下标。
fn generic_pins(pins: &[ValuePin]) -> impl Iterator<Item = usize> + '_ {
    pins.iter().enumerate().filter(|(_, p)| p.generic).map(|(i, _)| i)
}

/// 结构体元素时依赖声明里的 `type1`。
///
/// 参考项目和 `data.json` 都没有这个字段的线索,这个值来自实测 `.gia`:
/// 游戏里九个列表节点(100 / 107 / 128 / 135 / 142 / 153 / 160 / 169 / 509)
/// 配同一个结构体时**都是 10**,与 shell 无关。
///
/// **它跟选择器下标没有关系** —— 列表节点上两者恰好都是 10,但事件节点
/// (ID 428)的 `type1` 是 174、选择器是 21,对不上。
const STRUCT_TYPE1: i32 = 10;

/// 这一档列表节点在游戏里有没有结构体变体。
///
/// 实测 `.gia` 里列表操作 / 查询 / 拼装 / 遍历都能配结构体,只有「取最大 /
/// 取最小 / 排序」这三个只支持数值的节点没有 —— 参考项目 `data.json` 的变体表
/// 里也确实只有 Int / Flt 两档。
fn struct_supported(shell: i64) -> bool {
    !matches!(shell, 149 | 151 | 167)
}

/// 结构体元素时的选择器下标。
///
/// 结构体接在最后一档后面:事件节点(ID 428)的选择器占 0..20,结构体用 21;
/// 列表节点普遍占 0..9,所以结构体是 10。实测 `.gia` 里
/// `chosen_type_index` 与列表引脚的类型选择器都是 10,与这里的算法一致。
fn struct_selector(shell: i64) -> i32 {
    variant_count(shell) as i32
}

/// 某个节点注册了几个变体(= 选择器下标的上界 + 1)。
fn variant_count(shell: i64) -> usize {
    VARIANTS.iter().find(|v| v.shell == shell)
        .unwrap_or_else(|| panic!("未知的列表节点 {shell}"))
        .order.len()
}

// ========================================================================
// Execution.List_Operation
// ========================================================================

/// 拼接列表(ID 100):把 incoming_list 接到 target_list 后面。
pub fn node_list_concatenate<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        100,
        &ty,
        Flow(1, 1),
        vec![ValuePin::poly(List(ty.clone().into())), ValuePin::poly(List(ty.clone().into()))],
        vec![],
    )
}

/// 清空列表(ID 107)。
pub fn node_list_clear<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(graph, 107, &ty, Flow(1, 1), vec![ValuePin::poly(List(ty.clone().into()))], vec![])
}

/// 插入元素(ID 135):在 index 处插入 value。
pub fn node_list_insert<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        135,
        &ty,
        Flow(1, 1),
        vec![ValuePin::poly(List(ty.clone().into())), ValuePin::plain(Int), ValuePin::poly(ty.clone())],
        vec![],
    )
}

/// 移除元素(ID 153):按下标移除。
pub fn node_list_remove<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        153,
        &ty,
        Flow(1, 1),
        vec![ValuePin::poly(List(ty.clone().into())), ValuePin::plain(Int)],
        vec![],
    )
}

/// 修改下标处的值(ID 160)。
pub fn node_list_modify_index<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        160,
        &ty,
        Flow(1, 1),
        vec![ValuePin::poly(List(ty.clone().into())), ValuePin::plain(Int), ValuePin::poly(ty.clone())],
        vec![],
    )
}

/// 排序列表(ID 167):只支持 Int / Float,规则是排序枚举。
pub fn node_list_sort<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        167,
        &ty,
        Flow(1, 1),
        vec![ValuePin::poly(List(ty.clone().into())), ValuePin::plain(Enum(SORTING_RULE))],
        vec![],
    )
}

/// 遍历列表(ID 509):每项执行一遍 body,结束后走 done;输出当前项。
///
/// 控制流是 Start / Break 两个入口、Iteration / End 两个出口。
pub fn node_list_for_each<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        509,
        &ty,
        Flow(2, 2),
        vec![ValuePin::poly(List(ty.clone().into()))],
        vec![ValuePin::poly(ty.clone())],
    )
}

// ========================================================================
// Query.List_Related
// ========================================================================

/// 列表是否包含该值(ID 114)。
pub fn node_list_contains<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        114,
        &ty,
        Flow(0, 0),
        vec![ValuePin::poly(List(ty.clone().into())), ValuePin::poly(ty.clone())],
        vec![ValuePin::plain(Bool)],
    )
}

/// 查找值并返回下标(ID 121):返回所有命中项的下标。
pub fn node_list_find_index<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        121,
        &ty,
        Flow(0, 0),
        vec![ValuePin::poly(List(ty.clone().into())), ValuePin::poly(ty.clone())],
        vec![ValuePin::plain(List(Int.into()))],
    )
}

/// 按下标取值(ID 128)。
pub fn node_list_get_at_index<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        128,
        &ty,
        Flow(0, 0),
        vec![ValuePin::poly(List(ty.clone().into())), ValuePin::plain(Int)],
        vec![ValuePin::poly(ty.clone())],
    )
}

/// 列表长度(ID 142)。
pub fn node_list_get_length<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(graph, 142, &ty, Flow(0, 0), vec![ValuePin::poly(List(ty.clone().into()))], vec![ValuePin::plain(Int)])
}

/// 列表最大值(ID 149):只支持 Int / Float。
pub fn node_list_get_max<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        149,
        &ty,
        Flow(0, 0),
        vec![ValuePin::poly(List(ty.clone().into()))],
        vec![ValuePin::poly(ty.clone())],
    )
}

/// 列表最小值(ID 151):只支持 Int / Float。
pub fn node_list_get_min<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        151,
        &ty,
        Flow(0, 0),
        vec![ValuePin::poly(List(ty.clone().into()))],
        vec![ValuePin::poly(ty.clone())],
    )
}

// ========================================================================
// Arithmetic.General
// ========================================================================

/// 拼装列表(ID 169):把若干元素拼成一个列表。
///
/// 游戏里的引脚是 `Input0: Int`(元素个数,参考项目没写它的含义)+ 最多 100 个
/// `R<T>` 元素引脚,输出 `L<R<T>>`。元素引脚是动态的,这里只按 1 个元素占位,
/// 实际编译期按需扩展 `values_in_types`。
pub fn node_list_assemble<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    ty: NativeKind,
) -> NodeKind {
    generic(
        graph,
        169,
        &ty,
        Flow(0, 0),
        vec![ValuePin::poly(ty.clone())],
        vec![ValuePin::poly(List(ty.clone().into()))],
    )
}

/// 排序规则枚举(`SortingRule`,TypeID 10007 → `Enum(7)`)。
const SORTING_RULE: i32 = 7;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeGraphKind;
    use crate::asset::generated::type_definition;

    /// 元素类型 → (kernel, 选择器下标)。
    fn of(shell: i64, ty: NativeKind) -> (i64, i32) {
        variant(shell, &ty).unwrap()
    }

    #[test]
    fn 清除列表的泛型变体() {
        assert_eq!(of(107, Int), (107, 0));
        assert_eq!(of(107, String), (108, 1));
        assert_eq!(of(107, Entity), (109, 2));
        assert_eq!(of(107, Guid), (110, 3));
        assert_eq!(of(107, Float), (111, 4));
        assert_eq!(of(107, Vec3), (112, 5));
        assert_eq!(of(107, Bool), (113, 6));
        assert_eq!(of(107, Config), (552, 7));
        assert_eq!(of(107, Prefab), (553, 8));
        assert_eq!(of(107, Faction), (2653, 9));
    }

    #[test]
    fn 只有数值支持的节点() {
        assert_eq!(of(149, Int), (2648, 0));
        assert_eq!(of(149, Float), (2649, 1));
        assert_eq!(of(151, Int), (2646, 0));
        assert_eq!(of(151, Float), (2647, 1));
        assert_eq!(of(167, Int), (167, 0));
        assert_eq!(of(167, Float), (168, 1));
    }

    #[test]
    fn 遍历的泛型顺序跟别的不一样() {
        assert_eq!(of(509, Bool), (509, 0));
        assert_eq!(of(509, Entity), (510, 1));
        assert_eq!(of(509, Float), (511, 2));
        assert_eq!(of(509, Guid), (512, 3));
        assert_eq!(of(509, Int), (513, 4));
        assert_eq!(of(509, String), (514, 5));
        assert_eq!(of(509, Vec3), (515, 6));
        assert_eq!(of(509, Config), (570, 7));
        assert_eq!(of(509, Prefab), (571, 8));
        assert_eq!(of(509, Faction), (3280, 9));
    }

    #[test]
    fn 几个节点的引脚形状() {
        let mut g: NodeGraph = NodeGraph::new(NodeGraphKind::ServerEntity, "test");
        // 插入元素:list / index / value,index 与元素类型无关
        let insert = node_list_insert(&mut g, Int);
        assert_eq!(insert.values_in_types, vec![Some(List(Int.into())), Some(Int), Some(Int)]);
        assert_eq!(insert.id.selectors_in, vec![Some(0), None, Some(0)]);
        // 按下标取值:输入 list / index,输出元素
        let get = node_list_get_at_index(&mut g, String);
        assert_eq!(get.values_in_types, vec![Some(List(String.into())), Some(Int)]);
        assert_eq!(get.values_out_types, vec![String]);
        assert_eq!(get.id.selectors_out, vec![Some(1)]);
        // 拼装列表:元素进,列表出
        let assemble = node_list_assemble(&mut g, Float);
        assert_eq!(assemble.values_out_types, vec![List(Float.into())]);
    }

    #[test]
    fn 结构体的选择器接在最后一档后面() {
        // 事件节点(ID 428)的选择器占 0..20,结构体用 21;列表节点占 0..9 → 10。
        // 实测 .gia 里的 chosen_type_index 也是 10。
        assert_eq!(struct_selector(107), 10);
        assert_eq!(struct_selector(509), 10);
    }

    #[test]
    fn 结构体元素的引脚实现是_struct() {
        let mut g: NodeGraph = NodeGraph::new(NodeGraphKind::ServerEntity, "test");
        let st = crate::structure::StructRef {
            id: crate::structure::id_struct(7),
            fields: vec![],
        };
        let kind = node_list_clear(&mut g, NativeKind::Struct(st));
        // kernel 清零,依赖声明挂上(type1 与选择器都是实测的 10),泛型引脚改成 Struct 实现
        assert_eq!(kind.id.kernel, 0);
        let decl = kind.id.using_struct.as_ref().expect("结构体依赖声明");
        assert_eq!(decl.type1, 10);
        assert_eq!(kind.id.selectors_in, vec![Some(10)]);
        assert_eq!(kind.id.imps_in[0], type_definition::server_type::Implementation::Struct);
        assert_eq!(g.embedded.len(), 1);
    }

    #[test]
    fn 结构体变体的适用范围() {
        // 游戏里列表操作 / 查询 / 拼装 / 遍历都有结构体变体,取最大 / 最小 / 排序没有
        assert!(struct_supported(100));
        assert!(struct_supported(107));
        assert!(struct_supported(509));
        assert!(!struct_supported(149));
        assert!(!struct_supported(151));
        assert!(!struct_supported(167));
    }

    #[test]
    #[should_panic(expected = "不支持元素类型")]
    fn 只支持数值的节点不接受结构体() {
        let mut g: NodeGraph = NodeGraph::new(NodeGraphKind::ServerEntity, "test");
        let st = crate::structure::StructRef {
            id: crate::structure::id_struct(7),
            fields: vec![],
        };
        node_list_get_max(&mut g, NativeKind::Struct(st));
    }

    #[test]
    #[should_panic(expected = "不支持元素类型")]
    fn 不支持的元素类型会报错() {
        let mut g: NodeGraph = NodeGraph::new(NodeGraphKind::ServerEntity, "test");
        node_list_sort(&mut g, String);
    }
}
