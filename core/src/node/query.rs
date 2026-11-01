//! 查询域节点(Server,Query)
//!
//! 人工设计,替换自动生成版本:引脚语义命名、动态结构用 Vec、类型准确。

#![allow(unreachable_code)]

use crate::node::{NodeGraph, NodeKind};
use crate::structure::node_struct;
use crate::value::NativeKind::{self, *};
use std::sync::LazyLock;
// ========================================================================
// 随机 / 数学常量
// ========================================================================

/// 随机浮点(ID 7):[min, max) 内随机
pub static NODE_RANDOM_FLOAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(7, vec![Float, Float], Float)
});

/// 加权随机(ID 8):按权重列表抽取下标
pub static NODE_WEIGHTED_RANDOM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(8, vec![List(Int.into())], Int)
});

/// 随机整数(ID 257):[min, max] 内随机
pub static NODE_RANDOM_INT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(257, vec![Int, Int], Int)
});

/// 圆周率(ID 191)
pub static NODE_PI: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(191, vec![], Float)
});

// ========================================================================
// 向量常量
// ========================================================================

/// 零向量(ID 192)
pub static NODE_VECTOR_ZERO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(192, vec![], Vec3)
});

/// 上向量(ID 193)
pub static NODE_VECTOR_UP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(193, vec![], Vec3)
});

/// 下向量(ID 194)
pub static NODE_VECTOR_DOWN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(194, vec![], Vec3)
});

/// 左向量(ID 195)
pub static NODE_VECTOR_LEFT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(195, vec![], Vec3)
});

/// 右向量(ID 196)
pub static NODE_VECTOR_RIGHT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(196, vec![], Vec3)
});

/// 前向量(ID 197)
pub static NODE_VECTOR_FORWARD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(197, vec![], Vec3)
});

/// 后向量(ID 198)
pub static NODE_VECTOR_BACKWARD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(198, vec![], Vec3)
});

// ========================================================================
// 时间
// ========================================================================

/// 当前时间戳(ID 755)
pub static NODE_GET_TIMESTAMP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(755, vec![], Int)
});

/// 当前时区(ID 756)
pub static NODE_GET_TIMEZONE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(756, vec![], Int)
});

// ========================================================================
// 实体查询
// ========================================================================

/// 获取自身(ID 73)
pub static NODE_GET_SELF: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(73, vec![], Entity)
});

/// 按 GUID 获取实体(ID 75)
pub static NODE_GET_BY_GUID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(75, vec![Guid], Entity)
});

/// 获取实体 GUID(ID 76)
pub static NODE_GET_GUID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(76, vec![Entity], Guid)
});

/// 获取变换(ID 99):位置 + 旋转
pub static NODE_GET_TRANSFORM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(99, 0, 0, vec![Entity], vec![Vec3, Vec3])
});

/// 获取实体类型(ID 260)
pub static NODE_GET_ENTITY_TYPE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(260, vec![Entity], Enum(todo!()))
});

/// 获取所有实体(ID 318)
pub static NODE_GET_ALL_ENTITIES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(318, vec![], List(Entity.into()))
});

/// 按类型获取实体(ID 319)
pub static NODE_GET_ENTITY_BY_TYPE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(319, vec![Enum(todo!())], List(Entity.into()))
});

/// 获取带预制体的实体(ID 320)
pub static NODE_GET_WITH_PREFAB: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(320, vec![Prefab], List(Entity.into()))
});

/// 按类型筛选实体列表(ID 377)
pub static NODE_GET_BY_TYPE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(377, vec![List(Entity.into()), Enum(todo!())], List(Entity.into()))
});

/// 按预制体筛选实体列表(ID 378)
pub static NODE_GET_BY_PREFAB: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(378, vec![List(Entity.into()), Prefab], List(Entity.into()))
});

/// 按阵营筛选实体列表(ID 379)
pub static NODE_GET_BY_FACTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(379, vec![List(Entity.into()), Faction], List(Entity.into()))
});

/// 按范围筛选实体列表(ID 380)
pub static NODE_GET_BY_RANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(380, vec![List(Entity.into()), Vec3, Float], List(Entity.into()))
});

/// 是否存活(ID 507)
pub static NODE_IS_ACTIVE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(507, vec![Entity], Bool)
});

/// 获取前向向量(ID 516)
pub static NODE_GET_FORWARD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(516, vec![Entity], Vec3)
});

/// 获取右向向量(ID 517)
pub static NODE_GET_RIGHT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(517, vec![Entity], Vec3)
});

/// 获取上向向量(ID 518)
pub static NODE_GET_UP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(518, vec![Entity], Vec3)
});

// ========================================================================
// 属性查询
// ========================================================================

/// 获取对象属性(ID 580)
pub static NODE_GET_OBJ_ATTR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(580, 0, 0, vec![Entity], vec![
        Int,
        Float, Float, Float,
        Float, Float, Float,
    ])
});

/// 获取高级属性(ID 670)
pub static NODE_GET_ADV_ATTR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(670, 0, 0, vec![Entity], vec![
        Float, Float, Float, Float,
        Float, Float, Float,
    ])
});

/// 获取元素属性(ID 671)
pub static NODE_GET_ELEM_ATTR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(671, 0, 0, vec![Entity], vec![
        Float, Float, Float, Float,
        Float, Float, Float, Float,
        Float, Float,
    ])
});

/// 获取拥有者(ID 744)
pub static NODE_GET_OWNER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(744, vec![Entity], Entity)
});

/// 获取拥有的实体列表(ID 745)
pub static NODE_GET_OWNED_ENTITIES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(745, vec![Entity], List(Entity.into()))
});

/// 获取移动速度(ID 947):速度值 + 方向
pub static NODE_GET_MOVE_SPEED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(947, 0, 0, vec![Entity], vec![Float, Vec3])
});

// ========================================================================
// 玩家
// ========================================================================

/// 获取所有玩家(ID 248)
pub static NODE_GET_ALL_PLAYERS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(248, vec![], List(Entity.into()))
});

/// 获取玩家的角色(ID 258)
pub static NODE_GET_PLAYER_CHARACTERS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(258, vec![Entity], List(Entity.into()))
});

/// 获取玩家的主人(ID 259)
pub static NODE_GET_OWNER_PLAYER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(259, vec![Entity], Entity)
});

/// 获取复活次数(ID 275)
pub static NODE_GET_REVIVES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(275, vec![Entity], Int)
});

/// 获取复活时间(ID 277)
pub static NODE_GET_REVIVE_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(277, vec![Entity], Int)
});

/// 是否全部倒下(ID 287)
pub static NODE_IS_ALL_DOWN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(287, vec![Entity], Bool)
});

/// 按 ID 获取 GUID(ID 750)
pub static NODE_GET_GUID_BY_ID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(750, vec![Int], Guid)
});

/// 按 GUID 获取 ID(ID 751)
pub static NODE_GET_ID_BY_GUID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(751, vec![Guid], Int)
});

/// 获取昵称(ID 767)
pub static NODE_GET_NICKNAME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(767, vec![Entity], String)
});

/// 获取输入设备类型(ID 768)
pub static NODE_GET_INPUT_TYPE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(768, vec![Entity], Enum(todo!()))
});

// ========================================================================
// 变量 / 状态
// ========================================================================

/// 获取局部变量(ID 18,泛型 Variant):
/// initial_value(idx0,R<T>) 输入;local_variable(idx0,Loc) 与 value(idx1,R<T>) 输出。
/// 变体顺序(TSI 与 kernel 均按参考 data.json):Bol/Int/Str/Ety/Gid/Flt/Vec/
/// L<Int>/L<Str>/L<Ety>/L<Gid>/L<Flt>/L<Vec>/L<Bol>/Cfg/Pfb/L<Cfg>/L<Pfb>/Fct/L<Fct>
pub fn node_local(kind: &NativeKind) -> Option<NodeKind> {
    let mut result = NodeKind::simple(18, 0, 0, vec![kind.clone()], vec![LocalVarRef, kind.clone()]);
    let (selected, kernel) = match kind {
        Bool => (0, 18),
        Int => (1, 20),
        String => (2, 2656),
        Entity => (3, 2657),
        Guid => (4, 2658),
        Float => (5, 2659),
        Vec3 => (6, 2660),
        Config => (14, 2668),
        Prefab => (15, 2669),
        Faction => (18, 2672),
        List(ele) => match ele.as_ref() {
            Int => (7, 2661),
            String => (8, 2662),
            Entity => (9, 2663),
            Guid => (10, 2664),
            Float => (11, 2665),
            Vec3 => (12, 2666),
            Bool => (13, 2667),
            Config => (16, 2670),
            Prefab => (17, 2671),
            Faction => (19, 2673),
            _ => return None,
        },
        _ => return None,
    };
    result.id.kernel = kernel;
    result.id.selectors_in[0] = selected.into();
    result.id.selectors_out[1] = selected.into();
    result.into()
}

/// 图变量(ID 337,泛型 Variant):变量名 → 值。
///
/// 变体按值类型分:shell 固定 337,`kernel` 与类型选择器随 `T` 变。
/// 选择器顺序(参考项目 `data.json` 的 `Variants`)是
/// Ety→0、Gid→1、Int→2、Bol→3、Flt→4、Str→5、L<Gid>→6、L<Int>→7、
/// L<Bol>→8、L<Flt>→9、L<Str>→10、Vec→11、L<Ety>→12、L<Vec>→13、
/// Cfg→14、Pfb→15、L<Cfg>→16、L<Pfb>→17、Fct→18、L<Fct>→19。
///
/// 写节点的变体顺序和它**不一样**(ID 323 从 Int 开始),所以两边各自查表。
///
/// # 字典与结构体
///
/// 133 个 `D<K,V>` 变体的 `TypeSelectorIndex` **全是 20**(档位不区分键值对,
/// 真正的区分在 kernel);结构体不走变体表,接在字典后面用 21(单结构体)/
/// 22(结构体列表),靠 `using_struct` 声明 —— 见 [`node_struct`] 与
/// [`graph_variable_struct_type1`]。
///
/// 字典还没接:kernel 表是 133 条 `(键,值)` 组合,不是一维数组能查的。
pub fn node_get_graph_variable<NodeId, Kind>(graph: &mut NodeGraph<NodeId, Kind>, kind: &NativeKind) -> Option<NodeKind> {
    let mut result = NodeKind::expr(337, vec![String], kind.clone());
    if let Some((st, list)) = graph_variable_struct(kind) {
        // 结构体:kernel 由 `node_struct` 清零,选择器吃在出参 0 上。
        return Some(node_struct(
            graph,
            result,
            &st,
            graph_variable_struct_type1(list),
            graph_variable_struct_selector(list),
            [],
            [0],
        ));
    }
    let selector = graph_variable_selector_get(kind)?;
    result.id.kernel = graph_variable_kernel_for(337, selector)?;
    result.id.selectors_out[0] = selector.into();
    result.into()
}

/// 写图变量(ID 323,泛型 Variant):变量名 + 值 + 是否触发事件。
pub fn node_set_graph_variable<NodeId, Kind>(graph: &mut NodeGraph<NodeId, Kind>, kind: &NativeKind) -> Option<NodeKind> {
    let mut result = NodeKind::procedure(323, vec![String, kind.clone(), Bool]);
    if let Some((st, list)) = graph_variable_struct(kind) {
        // 选择器吃在值的那个入参(下标 1)上。
        return Some(node_struct(
            graph,
            result,
            &st,
            graph_variable_struct_type1(list),
            graph_variable_struct_selector(list),
            [1],
            [],
        ));
    }
    let selector = graph_variable_selector_set(kind)?;
    result.id.kernel = graph_variable_kernel_for(323, selector)?;
    result.id.selectors_in[1] = selector.into();
    result.into()
}

/// 图变量的类型是不是结构体;是的话返回结构体与「是不是列表」。
///
/// 两种都走 `using_struct`,只是引脚类型 / `type1` / 选择器不同。
pub(crate) fn graph_variable_struct(kind: &NativeKind) -> Option<(crate::structure::StructRef, bool)> {
    use NativeKind::*;
    match kind {
        Struct(st) => Some((st.clone(), false)),
        List(ele) => match ele.as_ref() {
            Struct(st) => Some((st.clone(), true)),
            _ => None,
        },
        _ => None,
    }
}

/// 结构体 / 结构体列表的选择器档位。
///
/// 实测 `.gia`(`target/export.gia` 单结构体、`target/export1.gia` 结构体列表):
/// 单结构体 **21**、结构体列表 **22** —— 也就是字典那一档(20)再往后排,
/// 和 [`crate::node::list`] 的「结构体接在最后一档后面」是同一套算法。
fn graph_variable_struct_selector(list: bool) -> i32 {
    if list { 22 } else { 21 }
}

/// 结构体 / 结构体列表的 `using_struct.type1`。
///
/// 实测同样是 174 / 175。这个数**不是**选择器下标,也不跟 shell 走
/// (列表节点上 `type1` 恒为 10,见 `node::list::STRUCT_TYPE1`)——
/// 只能按壳 + 是否列表查表。
fn graph_variable_struct_type1(list: bool) -> i32 {
    if list { 175 } else { 174 }
}

/// 图变量的选择器档位(`T` → `TypeSelectorIndex`)。
///
/// 取值节点(ID 337)和写节点(ID 323)共用同一套类型顺序，只是 kernel 表不同。
/// 触发节点(351)用的也是这一套。
pub(crate) fn graph_variable_selector(kind: &NativeKind) -> Option<i32> {
    use NativeKind::*;
    Some(match kind {
        Entity => 0,
        Guid => 1,
        Int => 2,
        Bool => 3,
        Float => 4,
        String => 5,
        Vec3 => 11,
        Config => 14,
        Prefab => 15,
        Faction => 18,
        List(ele) => match ele.as_ref() {
            Guid => 6,
            Int => 7,
            Bool => 8,
            Float => 9,
            String => 10,
            Entity => 12,
            Vec3 => 13,
            Config => 16,
            Prefab => 17,
            Faction => 19,
            _ => return None,
        },
        _ => return None,
    })
}

/// 取值节点的档位(与触发节点 351 顺序相同)。
fn graph_variable_selector_get(kind: &NativeKind) -> Option<i32> {
    graph_variable_selector(kind)
}

/// 写节点的档位。顺序与取值节点**不同**：Int→0、Flt→1、Bol→2、Str→3、
/// Gid→4、Ety→5，列表从 6 起。
fn graph_variable_selector_set(kind: &NativeKind) -> Option<i32> {
    use NativeKind::*;
    Some(match kind {
        Int => 0,
        Float => 1,
        Bool => 2,
        String => 3,
        Guid => 4,
        Entity => 5,
        Vec3 => 11,
        Config => 14,
        Prefab => 15,
        Faction => 18,
        List(ele) => match ele.as_ref() {
            Guid => 6,
            Int => 7,
            Bool => 8,
            Float => 9,
            String => 10,
            Entity => 12,
            Vec3 => 13,
            Config => 16,
            Prefab => 17,
            Faction => 19,
            _ => return None,
        },
        _ => return None,
    })
}

/// 图变量节点的 kernel：`shell` 是该节点 shell ID，`selector` 是档位。
///
/// 三个节点的 kernel 表各不相同(337 系列 / 323 系列 / 351 系列)，按 shell 分表。
pub(crate) fn graph_variable_kernel_for(shell: i64, selector: i32) -> Option<i64> {
    let table: &[i64] = match shell {
        337 => &[337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 538, 539, 540, 541, 626, 2998],
        323 => &[323, 324, 325, 326, 327, 328, 329, 330, 331, 332, 333, 334, 335, 336, 534, 535, 536, 537, 625, 2857],
        // 触发节点的顺序与取值节点相同,但 kernel 是另一组(档位 18 是 627 不是 626,
        // 档位 19 是 3139 不是 2998)。
        351 => &[351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 542, 543, 544, 545, 627, 3139],
        _ => return None,
    };
    table.get(selector as usize).copied()
}

/// 自定义变量(ID 50):entity + 变量名 → 值
pub static NODE_GET_VARIABLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(50, vec![Entity, String], Int)
});

/// 获取变量快照(ID 3360)
pub static NODE_GET_SNAPSHOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(3360, vec![VarSnapshotRef, String], Int)
});

// ========================================================================
// 状态
// ========================================================================

/// 获取状态值(ID 68)
pub static NODE_GET_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(68, vec![Entity, Int], Int)
});

/// 是否有状态(ID 508)
pub static NODE_HAS_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(508, vec![Entity, Config], Bool)
});

/// 获取状态层数(ID 746)
pub static NODE_GET_STATUS_STACKS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(746, vec![Entity, Config, Int], Int)
});

/// 获取状态施加者(ID 747)
pub static NODE_GET_STATUS_APPLIER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(747, vec![Entity, Config, Int], Entity)
});

/// 获取状态槽位(ID 748)
pub static NODE_GET_STATUS_SLOTS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(748, vec![Entity, Config], List(Int.into()))
});

// ========================================================================
// 环境 / 计时器
// ========================================================================

/// 获取活跃组(ID 179)
pub static NODE_GET_ACTIVE_GROUPS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(179, vec![], List(Int.into()))
});

/// 获取已流逝时间(ID 290)
pub static NODE_GET_ELAPSED_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(290, vec![], Int)
});

/// 获取环境时间(ID 664)
pub static NODE_GET_ENV_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(664, 0, 0, vec![], vec![Float, Int])
});

/// 获取游戏信息(ID 766)
pub static NODE_GET_GAME_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(766, 0, 0, vec![], vec![Int, Enum(todo!())])
});

/// 获取计时器时间(ID 310)
pub static NODE_GET_TIMER_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(310, vec![Entity, String], Float)
});

/// 获取当前布局(ID 317)
pub static NODE_GET_CURRENT_LAYOUT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(317, vec![Entity], Int)
});

// ========================================================================
// 阵营
// ========================================================================

/// 获取阵营(ID 249)
pub static NODE_GET_FACTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(249, vec![Entity], Faction)
});

/// 是否敌对(ID 614)
pub static NODE_IS_HOSTILE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(614, vec![Faction, Faction], Bool)
});

// ========================================================================
// 标签
// ========================================================================

/// 获取标签列表(ID 589)
pub static NODE_GET_TAGS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(589, vec![Entity], List(Int.into()))
});

/// 按标签获取实体(ID 590)
pub static NODE_GET_BY_TAG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(590, vec![Int], List(Entity.into()))
});

// ========================================================================
// 造物
// ========================================================================

/// 获取造物目标(ID 376)
pub static NODE_GET_CREATION_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(376, vec![Entity], Entity)
});

/// 获取造物属性(ID 381)
pub static NODE_GET_CREATION_ATTR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(381, 0, 0, vec![Entity], vec![
        Int,
        Float, Float, Float,
        Float, Float, Float,
        Enum(todo!()),
    ])
});

/// 获取造物仇恨列表(ID 758)
pub static NODE_GET_CREATION_AGGRO_LIST: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(758, vec![Entity], List(Entity.into()))
});

/// 获取跟随目标(ID 246)
pub static NODE_GET_FOLLOW_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(246, 0, 0, vec![Entity], vec![Entity, Guid])
});

// ========================================================================
// 预设点 / 巡逻
// ========================================================================

/// 获取预设点变换(ID 270)
pub static NODE_GET_PRESET_POINT_TRANSFORM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(270, 0, 0, vec![Int], vec![Vec3, Vec3])
});

/// 按标签获取预设点(ID 271)
pub static NODE_GET_PRESET_POINT_BY_TAG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(271, vec![Int], List(Int.into()))
});

/// 获取巡逻模板(ID 619)
pub static NODE_GET_PATROL_TEMPLATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(619, 0, 0, vec![Entity], vec![Int, Int, Int])
});

/// 获取路径点(ID 621)
pub static NODE_GET_WAYPOINT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(621, 0, 0, vec![Int, Int], vec![Vec3, Vec3])
});

// ========================================================================
// 职业 / 等级 / 技能
// ========================================================================

/// 获取职业(ID 387)
pub static NODE_GET_CLASS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(387, vec![Entity], Config)
});

/// 获取等级(ID 388)
pub static NODE_GET_LEVEL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(388, vec![Entity, Config], Int)
});

/// 获取技能信息(ID 398)
pub static NODE_GET_SKILL_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(398, vec![Entity, Enum(todo!())], Config)
});

// ========================================================================
// 仇恨
// ========================================================================

/// 获取仇恨值(ID 603)
pub static NODE_GET_AGGRO_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(603, vec![Entity, Entity], Int)
});

/// 获取仇恨倍率(ID 604)
pub static NODE_GET_AGGRO_MULTIPLIER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(604, vec![Entity], Float)
});

/// 获取全局仇恨倍率(ID 605)
pub static NODE_GET_GLOBAL_MULTIPLIER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(605, vec![], Float)
});

/// 获取仇恨目标(ID 606)
pub static NODE_GET_AGGRO_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(606, vec![Entity], Entity)
});

/// 获取仇恨所有者(ID 607)
pub static NODE_GET_AGGRO_OWNERS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(607, vec![Entity], List(Entity.into()))
});

/// 获取瞄准所有者(ID 608)
pub static NODE_GET_TARGETING_OWNERS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(608, vec![Entity], List(Entity.into()))
});

/// 获取仇恨列表(ID 609)
pub static NODE_GET_AGGRO_LIST: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(609, vec![Entity], List(Entity.into()))
});

/// 是否在战斗中(ID 610)
pub static NODE_IS_IN_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(610, vec![Entity], Bool)
});

// ========================================================================
// 标记 / 完成度
// ========================================================================

/// 获取标记信息(ID 638)
pub static NODE_GET_MARKER_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(638, 0, 0, vec![Entity, Int], vec![Bool, List(Entity.into()), List(Entity.into())])
});

/// 获取标记状态(ID 639)
pub static NODE_GET_MARKER_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(639, 0, 0, vec![Entity], vec![List(Int.into()), List(Int.into()), List(Int.into())])
});

/// 是否完成(ID 644)
pub static NODE_IS_COMPLETED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(644, vec![Entity, Int], Bool)
});

// ========================================================================
// 排行 / 分数
// ========================================================================

/// 获取玩家排行(ID 651)
pub static NODE_GET_PLAYER_RANK: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(651, vec![Entity], Int)
});

/// 获取玩家结果(ID 653)
pub static NODE_GET_PLAYER_RESULT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(653, vec![Entity], Enum(todo!()))
});

/// 获取阵营排行(ID 655)
pub static NODE_GET_FACTION_RANK: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(655, vec![Faction], Int)
});

/// 获取阵营结果(ID 657)
pub static NODE_GET_FACTION_RESULT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(657, vec![Faction], Enum(todo!()))
});

/// 获取排行信息(ID 658)
pub static NODE_GET_RANK_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(658, 0, 0, vec![Entity], vec![Int, Int, Int, Int])
});

/// 获取分数变化(ID 660)
pub static NODE_GET_SCORE_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(660, vec![Entity, Enum(todo!())], Int)
});

/// 获取逃生状态(ID 662)
pub static NODE_GET_ESCAPE_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(662, vec![Entity], Bool)
});

/// 获取重叠实体(ID 669)
pub static NODE_GET_OVERLAPPING_ENTITIES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(669, vec![Entity, Int], List(Entity.into()))
});

// ========================================================================
// 词缀 / 装备
// ========================================================================

/// 获取词缀列表(ID 675)
pub static NODE_GET_AFFIXES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(675, vec![Int], List(Int.into()))
});

/// 获取词缀配置(ID 676)
pub static NODE_GET_AFFIX_CONFIG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(676, vec![Int, Int], Config)
});

/// 获取词缀值(ID 677)
pub static NODE_GET_AFFIX_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(677, vec![Int, Int], Float)
});

/// 获取装备标签(ID 734)
pub static NODE_GET_EQUIP_TAGS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(734, vec![Int], List(Config.into()))
});

/// 获取配置 ID(ID 749)
pub static NODE_GET_CONFIG_ID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(749, vec![Int], Config)
});

// ========================================================================
// 物品 / 货币
// ========================================================================

/// 获取容量(ID 689)
pub static NODE_GET_CAPACITY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(689, vec![Entity], Int)
});

/// 获取物品数量(ID 690)
pub static NODE_GET_ITEM_AMOUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(690, vec![Entity, Config], Int)
});

/// 获取货币数量(ID 691)
pub static NODE_GET_CURRENCY_AMOUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(691, vec![Entity, Config], Int)
});

/// 获取基础物品(ID 721):配置 → 数量 字典
pub static NODE_GET_BASIC_ITEMS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(721, vec![Entity], Dict { key: Config.into(), value: Int.into() })
});

/// 获取全部货币(ID 722)
pub static NODE_GET_CURRENCY_ALL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(722, vec![Entity], Dict { key: Config.into(), value: Int.into() })
});

/// 获取全部装备(ID 723)
pub static NODE_GET_EQUIPMENT_ALL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(723, vec![Entity], List(Int.into()))
});

/// 获取掉落物品数量(ID 728)
pub static NODE_GET_LOOT_ITEM_AMOUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(728, vec![Entity, Config], Int)
});

/// 获取掉落货币数量(ID 729)
pub static NODE_GET_LOOT_CURRENCY_AMOUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(729, vec![Entity, Config], Int)
});

/// 获取掉落物品(ID 730)
pub static NODE_GET_LOOT_ITEMS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(730, vec![Entity], Dict { key: Config.into(), value: Int.into() })
});

/// 获取掉落货币(ID 731)
pub static NODE_GET_LOOT_CURRENCY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(731, vec![Entity], Dict { key: Config.into(), value: Int.into() })
});

/// 获取掉落装备(ID 732)
pub static NODE_GET_LOOT_EQUIPMENT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(732, vec![Entity], List(Int.into()))
});

// ========================================================================
// 商店
// ========================================================================

/// 获取自定义商品(ID 714)
pub static NODE_GET_CUSTOM_SALES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(714, vec![Entity, Int], List(Int.into()))
});

/// 获取库存商品(ID 715)
pub static NODE_GET_INV_SALES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(715, vec![Entity, Int], List(Config.into()))
});

/// 获取购物车商品(ID 716)
pub static NODE_GET_CART_ITEMS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(716, vec![Entity, Int], List(Config.into()))
});

/// 获取自定义商品信息(ID 717)
pub static NODE_GET_CUSTOM_ITEM_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(717, 0, 0, vec![Entity, Int, Int], vec![
        Config,
        Dict { key: Config.into(), value: Int.into() },
        Int,
        Bool,
        Int,
        Int,
        Bool,
    ])
});

/// 获取库存商品信息(ID 718)
pub static NODE_GET_INV_ITEM_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(718, 0, 0, vec![Entity, Int, Config], vec![
        Dict { key: Config.into(), value: Int.into() },
        Int,
        Bool,
    ])
});

/// 获取购买信息(ID 719)
pub static NODE_GET_PURCHASE_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(719, 0, 0, vec![Entity, Int, Config], vec![
        Dict { key: Config.into(), value: Int.into() },
        Bool,
    ])
});

// ========================================================================
// 标签 / 角色属性
// ========================================================================

/// 获取活跃标签(ID 737)
pub static NODE_GET_ACTIVE_TAG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(737, vec![Entity], Config)
});

/// 获取角色属性(ID 738)
pub static NODE_GET_CHARACTER_ATTR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(738, 0, 0, vec![Entity], vec![
        Int,
        Float, Float, Float,
        Float, Float, Float, Float,
        Float,
        Enum(todo!()),
    ])
});

// ========================================================================
// 箱子
// ========================================================================

/// 获取箱子数量(ID 773)
pub static NODE_GET_BOX_QUANTITY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(773, vec![Entity, Int], Int)
});

/// 获取箱子消耗(ID 774)
pub static NODE_GET_BOX_CONSUMPTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(774, vec![Entity, Int], Int)
});

// ========================================================================
// 字典操作
// ========================================================================

/// 字典取值(ID 1158)
pub static NODE_DICT_GET_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1158, vec![Dict { key: Int.into(), value: Int.into() }, Int], Int)
});

/// 是否有键(ID 1368)
pub static NODE_DICT_HAS_KEY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1368, vec![Dict { key: Int.into(), value: Int.into() }, Int], Bool)
});

/// 是否有值(ID 1438)
pub static NODE_DICT_HAS_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1438, vec![Dict { key: Int.into(), value: Int.into() }, Int], Bool)
});

/// 获取键列表(ID 1508)
pub static NODE_DICT_GET_KEYS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1508, vec![Dict { key: Int.into(), value: Int.into() }], List(Int.into()))
});

/// 获取值列表(ID 1578)
pub static NODE_DICT_GET_VALUES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1578, vec![Dict { key: Int.into(), value: Int.into() }], List(Int.into()))
});

/// 获取字典长度(ID 1648)
pub static NODE_DICT_GET_LENGTH: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1648, vec![Dict { key: Int.into(), value: Int.into() }], Int)
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeGraphKind;
    use crate::structure::{id_struct, StructRef};

    fn point() -> StructRef {
        StructRef {
            id: id_struct(7),
            fields: vec![("x".into(), NativeKind::Int)],
        }
    }

    fn graph() -> NodeGraph {
        NodeGraph::new(NodeGraphKind::ServerEntity, "test")
    }

    /// 结构体不走变体表:kernel 清零、挂依赖声明、类型选择器是 21。
    /// 实测 `target/export.gia`(单结构体)与 `target/export1.gia`(结构体列表)。
    #[test]
    fn 结构体图变量的选择器与_type1() {
        let st = NativeKind::Struct(point());
        let g = &mut graph();
        let get = node_get_graph_variable(g, &st).expect("结构体应当有图变量节点");
        assert_eq!(get.id.kernel, 0, "结构体变体没有 kernel");
        assert_eq!(get.id.selectors_out[0], Some(21));
        let decl = get.id.using_struct.as_ref().expect("结构体依赖声明");
        assert_eq!(decl.type1, 174);

        let g = &mut graph();
        let set = node_set_graph_variable(g, &st).expect("结构体应当有图变量节点");
        assert_eq!(set.id.selectors_in[1], Some(21));
        assert_eq!(set.id.using_struct.as_ref().unwrap().type1, 174);
    }

    #[test]
    fn 结构体列表图变量的选择器与_type1() {
        let st = NativeKind::List(NativeKind::Struct(point()).into());
        let g = &mut graph();
        let get = node_get_graph_variable(g, &st).expect("结构体列表应当有图变量节点");
        assert_eq!(get.id.selectors_out[0], Some(22));
        assert_eq!(get.id.using_struct.as_ref().unwrap().type1, 175);

        let g = &mut graph();
        let set = node_set_graph_variable(g, &st).expect("结构体列表应当有图变量节点");
        assert_eq!(set.id.selectors_in[1], Some(22));
        assert_eq!(set.id.using_struct.as_ref().unwrap().type1, 175);
    }

    /// 原生类型照旧走 kernel 表。
    #[test]
    fn 原生类型图变量走_kernel_表() {
        let g = &mut graph();
        let get = node_get_graph_variable(g, &NativeKind::Int).unwrap();
        assert_eq!(get.id.kernel, 339);
        assert_eq!(get.id.selectors_out[0], Some(2));
        assert!(get.id.using_struct.is_none());

        let g = &mut graph();
        let set = node_set_graph_variable(g, &NativeKind::Int).unwrap();
        assert_eq!(set.id.kernel, 323);
        assert_eq!(set.id.selectors_in[1], Some(0));
    }

    /// 触发节点(351)的 kernel 表与取值节点**不同**:档位 18 是 627 不是 626。
    #[test]
    fn 图变量变化节点的_kernel_表与取值节点不同() {
        let g = &mut graph();
        let trig = crate::node::trigger::node_on_graph_variable_change(g, &NativeKind::Faction).unwrap();
        assert_eq!(trig.id.kernel, 627);
        assert_eq!(trig.id.selectors_out[3], Some(18));
        assert_eq!(trig.id.selectors_out[4], Some(18));
    }
}
