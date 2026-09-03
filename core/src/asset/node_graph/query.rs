//! 查询域节点(Server,Query)
//!
//! 人工设计,替换自动生成版本:引脚语义命名、动态结构用 Vec、类型准确。

use crate::asset::generated::ServerTypeId;
use crate::asset::node_graph::NodeKind;
use crate::asset::value::{
    AnyValue, ValueBool, ValueConfig, ValueConfigList, ValueDefault, ValueDict, ValueEntity,
    ValueEntityList, ValueEnum, ValueFaction, ValueFloat, ValueGuid, ValueInt, ValueIntList,
    ValueLocalVarRef, ValuePrefab, ValueString, ValueVarSnapshotRef, ValueVector,
};
use std::sync::LazyLock;

// ========================================================================
// 随机 / 数学常量
// ========================================================================

/// 随机浮点(ID 7):[min, max) 内随机
pub static NODE_RANDOM_FLOAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(7, vec![ValueFloat::def(), ValueFloat::def()], ValueFloat::def())
});

/// 加权随机(ID 8):按权重列表抽取下标
pub static NODE_WEIGHTED_RANDOM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(8, vec![ValueIntList::def()], ValueInt::def())
});

/// 随机整数(ID 257):[min, max] 内随机
pub static NODE_RANDOM_INT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(257, vec![ValueInt::def(), ValueInt::def()], ValueInt::def())
});

/// 圆周率(ID 191)
pub static NODE_PI: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(191, vec![], ValueFloat::def())
});

// ========================================================================
// 向量常量
// ========================================================================

/// 零向量(ID 192)
pub static NODE_VECTOR_ZERO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(192, vec![], ValueVector::def())
});

/// 上向量(ID 193)
pub static NODE_VECTOR_UP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(193, vec![], ValueVector::def())
});

/// 下向量(ID 194)
pub static NODE_VECTOR_DOWN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(194, vec![], ValueVector::def())
});

/// 左向量(ID 195)
pub static NODE_VECTOR_LEFT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(195, vec![], ValueVector::def())
});

/// 右向量(ID 196)
pub static NODE_VECTOR_RIGHT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(196, vec![], ValueVector::def())
});

/// 前向量(ID 197)
pub static NODE_VECTOR_FORWARD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(197, vec![], ValueVector::def())
});

/// 后向量(ID 198)
pub static NODE_VECTOR_BACKWARD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(198, vec![], ValueVector::def())
});

// ========================================================================
// 时间
// ========================================================================

/// 当前时间戳(ID 755)
pub static NODE_GET_TIMESTAMP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(755, vec![], ValueInt::def())
});

/// 当前时区(ID 756)
pub static NODE_GET_TIMEZONE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(756, vec![], ValueInt::def())
});

// ========================================================================
// 实体查询
// ========================================================================

/// 获取自身(ID 73)
pub static NODE_GET_SELF: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(73, vec![], ValueEntity::def())
});

/// 按 GUID 获取实体(ID 75)
pub static NODE_GET_BY_GUID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(75, vec![ValueGuid::def()], ValueEntity::def())
});

/// 获取实体 GUID(ID 76)
pub static NODE_GET_GUID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(76, vec![ValueEntity::def()], ValueGuid::def())
});

/// 获取变换(ID 99):位置 + 旋转
pub static NODE_GET_TRANSFORM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(99, 0, 0, vec![ValueEntity::def()], vec![ValueVector::def(), ValueVector::def()])
});

/// 获取实体类型(ID 260)
pub static NODE_GET_ENTITY_TYPE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(260, vec![ValueEntity::def()], ValueEnum::def())
});

/// 获取所有实体(ID 318)
pub static NODE_GET_ALL_ENTITIES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(318, vec![], ValueEntityList::def())
});

/// 按类型获取实体(ID 319)
pub static NODE_GET_ENTITY_BY_TYPE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(319, vec![ValueEnum::def()], ValueEntityList::def())
});

/// 获取带预制体的实体(ID 320)
pub static NODE_GET_WITH_PREFAB: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(320, vec![ValuePrefab::def()], ValueEntityList::def())
});

/// 按类型筛选实体列表(ID 377)
pub static NODE_GET_BY_TYPE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(377, vec![ValueEntityList::def(), ValueEnum::def()], ValueEntityList::def())
});

/// 按预制体筛选实体列表(ID 378)
pub static NODE_GET_BY_PREFAB: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(378, vec![ValueEntityList::def(), ValuePrefab::def()], ValueEntityList::def())
});

/// 按阵营筛选实体列表(ID 379)
pub static NODE_GET_BY_FACTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(379, vec![ValueEntityList::def(), ValueFaction::def()], ValueEntityList::def())
});

/// 按范围筛选实体列表(ID 380)
pub static NODE_GET_BY_RANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(380, vec![ValueEntityList::def(), ValueVector::def(), ValueFloat::def()], ValueEntityList::def())
});

/// 是否存活(ID 507)
pub static NODE_IS_ACTIVE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(507, vec![ValueEntity::def()], ValueBool::def())
});

/// 获取前向向量(ID 516)
pub static NODE_GET_FORWARD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(516, vec![ValueEntity::def()], ValueVector::def())
});

/// 获取右向向量(ID 517)
pub static NODE_GET_RIGHT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(517, vec![ValueEntity::def()], ValueVector::def())
});

/// 获取上向向量(ID 518)
pub static NODE_GET_UP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(518, vec![ValueEntity::def()], ValueVector::def())
});

// ========================================================================
// 属性查询
// ========================================================================

/// 获取对象属性(ID 580)
pub static NODE_GET_OBJ_ATTR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(580, 0, 0, vec![ValueEntity::def()], vec![
        ValueInt::def(),
        ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
        ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
    ])
});

/// 获取高级属性(ID 670)
pub static NODE_GET_ADV_ATTR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(670, 0, 0, vec![ValueEntity::def()], vec![
        ValueFloat::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
        ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
    ])
});

/// 获取元素属性(ID 671)
pub static NODE_GET_ELEM_ATTR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(671, 0, 0, vec![ValueEntity::def()], vec![
        ValueFloat::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
        ValueFloat::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
        ValueFloat::def(), ValueFloat::def(),
    ])
});

/// 获取拥有者(ID 744)
pub static NODE_GET_OWNER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(744, vec![ValueEntity::def()], ValueEntity::def())
});

/// 获取拥有的实体列表(ID 745)
pub static NODE_GET_OWNED_ENTITIES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(745, vec![ValueEntity::def()], ValueEntityList::def())
});

/// 获取移动速度(ID 947):速度值 + 方向
pub static NODE_GET_MOVE_SPEED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(947, 0, 0, vec![ValueEntity::def()], vec![ValueFloat::def(), ValueVector::def()])
});

// ========================================================================
// 玩家
// ========================================================================

/// 获取所有玩家(ID 248)
pub static NODE_GET_ALL_PLAYERS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(248, vec![], ValueEntityList::def())
});

/// 获取玩家的角色(ID 258)
pub static NODE_GET_PLAYER_CHARACTERS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(258, vec![ValueEntity::def()], ValueEntityList::def())
});

/// 获取玩家的主人(ID 259)
pub static NODE_GET_OWNER_PLAYER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(259, vec![ValueEntity::def()], ValueEntity::def())
});

/// 获取复活次数(ID 275)
pub static NODE_GET_REVIVES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(275, vec![ValueEntity::def()], ValueInt::def())
});

/// 获取复活时间(ID 277)
pub static NODE_GET_REVIVE_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(277, vec![ValueEntity::def()], ValueInt::def())
});

/// 是否全部倒下(ID 287)
pub static NODE_IS_ALL_DOWN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(287, vec![ValueEntity::def()], ValueBool::def())
});

/// 按 ID 获取 GUID(ID 750)
pub static NODE_GET_GUID_BY_ID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(750, vec![ValueInt::def()], ValueGuid::def())
});

/// 按 GUID 获取 ID(ID 751)
pub static NODE_GET_ID_BY_GUID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(751, vec![ValueGuid::def()], ValueInt::def())
});

/// 获取昵称(ID 767)
pub static NODE_GET_NICKNAME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(767, vec![ValueEntity::def()], ValueString::def())
});

/// 获取输入设备类型(ID 768)
pub static NODE_GET_INPUT_TYPE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(768, vec![ValueEntity::def()], ValueEnum::def())
});

// ========================================================================
// 变量 / 状态
// ========================================================================

/// 获取局部变量(ID 18,泛型 Variant):
/// initial_value(idx0,R<T>) 输入;local_variable(idx0,Loc) 与 value(idx1,R<T>) 输出。
/// 变体顺序(TSI 与 kernel 均按参考 data.json):Bol/Int/Str/Ety/Gid/Flt/Vec/
/// L<Int>/L<Str>/L<Ety>/L<Gid>/L<Flt>/L<Vec>/L<Bol>/Cfg/Pfb/L<Cfg>/L<Pfb>/Fct/L<Fct>
pub fn node_local(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::new(18, 0, 0, vec![ty.clone()], vec![ValueLocalVarRef::def(), ty.clone()]);
    let (selected, kernel) = match ty.get_server_type() {
        ServerTypeId::SBoolean => (0, 18),
        ServerTypeId::SInt => (1, 20),
        ServerTypeId::SString => (2, 2656),
        ServerTypeId::SEntity => (3, 2657),
        ServerTypeId::SGuid => (4, 2658),
        ServerTypeId::SFloat => (5, 2659),
        ServerTypeId::SVector => (6, 2660),
        ServerTypeId::SIntList => (7, 2661),
        ServerTypeId::SStringList => (8, 2662),
        ServerTypeId::SEntityList => (9, 2663),
        ServerTypeId::SGuidList => (10, 2664),
        ServerTypeId::SFloatList => (11, 2665),
        ServerTypeId::SVectorList => (12, 2666),
        ServerTypeId::SBooleanList => (13, 2667),
        ServerTypeId::SConfig => (14, 2668),
        ServerTypeId::SPrefab => (15, 2669),
        ServerTypeId::SConfigList => (16, 2670),
        ServerTypeId::SPrefabList => (17, 2671),
        ServerTypeId::SFaction => (18, 2672),
        ServerTypeId::SFactionList => (19, 2673),
        other => panic!("Unsupported type: {other:?}"),
    };
    result.kernel_id = kernel;
    result.selectors_in[0] = selected.into();
    result.selectors_out[1] = selected.into();
    result
}

/// 自定义变量(ID 50):entity + 变量名 → 值
pub static NODE_GET_VARIABLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(50, vec![ValueEntity::def(), ValueString::def()], ValueInt::def())
});

/// 图变量(ID 337)
pub static NODE_GET_GRAPH_VARIABLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(337, vec![ValueString::def()], ValueInt::def())
});

/// 获取变量快照(ID 3360)
pub static NODE_GET_SNAPSHOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(3360, vec![ValueVarSnapshotRef::def(), ValueString::def()], ValueInt::def())
});

// ========================================================================
// 状态
// ========================================================================

/// 获取状态值(ID 68)
pub static NODE_GET_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(68, vec![ValueEntity::def(), ValueInt::def()], ValueInt::def())
});

/// 是否有状态(ID 508)
pub static NODE_HAS_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(508, vec![ValueEntity::def(), ValueConfig::def()], ValueBool::def())
});

/// 获取状态层数(ID 746)
pub static NODE_GET_STATUS_STACKS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(746, vec![ValueEntity::def(), ValueConfig::def(), ValueInt::def()], ValueInt::def())
});

/// 获取状态施加者(ID 747)
pub static NODE_GET_STATUS_APPLIER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(747, vec![ValueEntity::def(), ValueConfig::def(), ValueInt::def()], ValueEntity::def())
});

/// 获取状态槽位(ID 748)
pub static NODE_GET_STATUS_SLOTS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(748, vec![ValueEntity::def(), ValueConfig::def()], ValueIntList::def())
});

// ========================================================================
// 列表操作
// ========================================================================

/// 是否包含(ID 114)
pub static NODE_CONTAINS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(114, vec![ValueIntList::def(), ValueInt::def()], ValueBool::def())
});

/// 查找下标(ID 121)
pub static NODE_FIND_INDEX: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(121, vec![ValueIntList::def(), ValueInt::def()], ValueIntList::def())
});

/// 按下标取值(ID 128)
pub static NODE_GET_AT_INDEX: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(128, vec![ValueIntList::def(), ValueInt::def()], ValueInt::def())
});

/// 列表长度(ID 142)
pub static NODE_GET_LIST_LENGTH: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(142, vec![ValueIntList::def()], ValueInt::def())
});

/// 列表最大值(ID 149)
pub static NODE_GET_MAX: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(149, vec![ValueIntList::def()], ValueInt::def())
});

/// 列表最小值(ID 151)
pub static NODE_GET_MIN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(151, vec![ValueIntList::def()], ValueInt::def())
});

// ========================================================================
// 环境 / 计时器
// ========================================================================

/// 获取活跃组(ID 179)
pub static NODE_GET_ACTIVE_GROUPS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(179, vec![], ValueIntList::def())
});

/// 获取已流逝时间(ID 290)
pub static NODE_GET_ELAPSED_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(290, vec![], ValueInt::def())
});

/// 获取环境时间(ID 664)
pub static NODE_GET_ENV_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(664, 0, 0, vec![], vec![ValueFloat::def(), ValueInt::def()])
});

/// 获取游戏信息(ID 766)
pub static NODE_GET_GAME_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(766, 0, 0, vec![], vec![ValueInt::def(), ValueEnum::def()])
});

/// 获取计时器时间(ID 310)
pub static NODE_GET_TIMER_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(310, vec![ValueEntity::def(), ValueString::def()], ValueFloat::def())
});

/// 获取当前布局(ID 317)
pub static NODE_GET_CURRENT_LAYOUT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(317, vec![ValueEntity::def()], ValueInt::def())
});

// ========================================================================
// 阵营
// ========================================================================

/// 获取阵营(ID 249)
pub static NODE_GET_FACTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(249, vec![ValueEntity::def()], ValueFaction::def())
});

/// 是否敌对(ID 614)
pub static NODE_IS_HOSTILE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(614, vec![ValueFaction::def(), ValueFaction::def()], ValueBool::def())
});

// ========================================================================
// 标签
// ========================================================================

/// 获取标签列表(ID 589)
pub static NODE_GET_TAGS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(589, vec![ValueEntity::def()], ValueIntList::def())
});

/// 按标签获取实体(ID 590)
pub static NODE_GET_BY_TAG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(590, vec![ValueInt::def()], ValueEntityList::def())
});

// ========================================================================
// 造物
// ========================================================================

/// 获取造物目标(ID 376)
pub static NODE_GET_CREATION_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(376, vec![ValueEntity::def()], ValueEntity::def())
});

/// 获取造物属性(ID 381)
pub static NODE_GET_CREATION_ATTR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(381, 0, 0, vec![ValueEntity::def()], vec![
        ValueInt::def(),
        ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
        ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
        ValueEnum::def(),
    ])
});

/// 获取造物仇恨列表(ID 758)
pub static NODE_GET_CREATION_AGGRO_LIST: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(758, vec![ValueEntity::def()], ValueEntityList::def())
});

/// 获取跟随目标(ID 246)
pub static NODE_GET_FOLLOW_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(246, 0, 0, vec![ValueEntity::def()], vec![ValueEntity::def(), ValueGuid::def()])
});

// ========================================================================
// 预设点 / 巡逻
// ========================================================================

/// 获取预设点变换(ID 270)
pub static NODE_GET_PRESET_POINT_TRANSFORM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(270, 0, 0, vec![ValueInt::def()], vec![ValueVector::def(), ValueVector::def()])
});

/// 按标签获取预设点(ID 271)
pub static NODE_GET_PRESET_POINT_BY_TAG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(271, vec![ValueInt::def()], ValueIntList::def())
});

/// 获取巡逻模板(ID 619)
pub static NODE_GET_PATROL_TEMPLATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(619, 0, 0, vec![ValueEntity::def()], vec![ValueInt::def(), ValueInt::def(), ValueInt::def()])
});

/// 获取路径点(ID 621)
pub static NODE_GET_WAYPOINT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(621, 0, 0, vec![ValueInt::def(), ValueInt::def()], vec![ValueVector::def(), ValueVector::def()])
});

// ========================================================================
// 职业 / 等级 / 技能
// ========================================================================

/// 获取职业(ID 387)
pub static NODE_GET_CLASS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(387, vec![ValueEntity::def()], ValueConfig::def())
});

/// 获取等级(ID 388)
pub static NODE_GET_LEVEL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(388, vec![ValueEntity::def(), ValueConfig::def()], ValueInt::def())
});

/// 获取技能信息(ID 398)
pub static NODE_GET_SKILL_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(398, vec![ValueEntity::def(), ValueEnum::def()], ValueConfig::def())
});

// ========================================================================
// 仇恨
// ========================================================================

/// 获取仇恨值(ID 603)
pub static NODE_GET_AGGRO_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(603, vec![ValueEntity::def(), ValueEntity::def()], ValueInt::def())
});

/// 获取仇恨倍率(ID 604)
pub static NODE_GET_AGGRO_MULTIPLIER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(604, vec![ValueEntity::def()], ValueFloat::def())
});

/// 获取全局仇恨倍率(ID 605)
pub static NODE_GET_GLOBAL_MULTIPLIER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(605, vec![], ValueFloat::def())
});

/// 获取仇恨目标(ID 606)
pub static NODE_GET_AGGRO_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(606, vec![ValueEntity::def()], ValueEntity::def())
});

/// 获取仇恨所有者(ID 607)
pub static NODE_GET_AGGRO_OWNERS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(607, vec![ValueEntity::def()], ValueEntityList::def())
});

/// 获取瞄准所有者(ID 608)
pub static NODE_GET_TARGETING_OWNERS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(608, vec![ValueEntity::def()], ValueEntityList::def())
});

/// 获取仇恨列表(ID 609)
pub static NODE_GET_AGGRO_LIST: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(609, vec![ValueEntity::def()], ValueEntityList::def())
});

/// 是否在战斗中(ID 610)
pub static NODE_IS_IN_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(610, vec![ValueEntity::def()], ValueBool::def())
});

// ========================================================================
// 标记 / 完成度
// ========================================================================

/// 获取标记信息(ID 638)
pub static NODE_GET_MARKER_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(638, 0, 0, vec![ValueEntity::def(), ValueInt::def()], vec![ValueBool::def(), ValueEntityList::def(), ValueEntityList::def()])
});

/// 获取标记状态(ID 639)
pub static NODE_GET_MARKER_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(639, 0, 0, vec![ValueEntity::def()], vec![ValueIntList::def(), ValueIntList::def(), ValueIntList::def()])
});

/// 是否完成(ID 644)
pub static NODE_IS_COMPLETED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(644, vec![ValueEntity::def(), ValueInt::def()], ValueBool::def())
});

// ========================================================================
// 排行 / 分数
// ========================================================================

/// 获取玩家排行(ID 651)
pub static NODE_GET_PLAYER_RANK: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(651, vec![ValueEntity::def()], ValueInt::def())
});

/// 获取玩家结果(ID 653)
pub static NODE_GET_PLAYER_RESULT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(653, vec![ValueEntity::def()], ValueEnum::def())
});

/// 获取阵营排行(ID 655)
pub static NODE_GET_FACTION_RANK: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(655, vec![ValueFaction::def()], ValueInt::def())
});

/// 获取阵营结果(ID 657)
pub static NODE_GET_FACTION_RESULT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(657, vec![ValueFaction::def()], ValueEnum::def())
});

/// 获取排行信息(ID 658)
pub static NODE_GET_RANK_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(658, 0, 0, vec![ValueEntity::def()], vec![ValueInt::def(), ValueInt::def(), ValueInt::def(), ValueInt::def()])
});

/// 获取分数变化(ID 660)
pub static NODE_GET_SCORE_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(660, vec![ValueEntity::def(), ValueEnum::def()], ValueInt::def())
});

/// 获取逃生状态(ID 662)
pub static NODE_GET_ESCAPE_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(662, vec![ValueEntity::def()], ValueBool::def())
});

/// 获取重叠实体(ID 669)
pub static NODE_GET_OVERLAPPING_ENTITIES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(669, vec![ValueEntity::def(), ValueInt::def()], ValueEntityList::def())
});

// ========================================================================
// 词缀 / 装备
// ========================================================================

/// 获取词缀列表(ID 675)
pub static NODE_GET_AFFIXES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(675, vec![ValueInt::def()], ValueIntList::def())
});

/// 获取词缀配置(ID 676)
pub static NODE_GET_AFFIX_CONFIG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(676, vec![ValueInt::def(), ValueInt::def()], ValueConfig::def())
});

/// 获取词缀值(ID 677)
pub static NODE_GET_AFFIX_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(677, vec![ValueInt::def(), ValueInt::def()], ValueFloat::def())
});

/// 获取装备标签(ID 734)
pub static NODE_GET_EQUIP_TAGS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(734, vec![ValueInt::def()], ValueConfigList::def())
});

/// 获取配置 ID(ID 749)
pub static NODE_GET_CONFIG_ID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(749, vec![ValueInt::def()], ValueConfig::def())
});

// ========================================================================
// 物品 / 货币
// ========================================================================

/// 获取容量(ID 689)
pub static NODE_GET_CAPACITY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(689, vec![ValueEntity::def()], ValueInt::def())
});

/// 获取物品数量(ID 690)
pub static NODE_GET_ITEM_AMOUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(690, vec![ValueEntity::def(), ValueConfig::def()], ValueInt::def())
});

/// 获取货币数量(ID 691)
pub static NODE_GET_CURRENCY_AMOUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(691, vec![ValueEntity::def(), ValueConfig::def()], ValueInt::def())
});

/// 获取基础物品(ID 721):配置 → 数量 字典
pub static NODE_GET_BASIC_ITEMS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(721, vec![ValueEntity::def()], ValueDict::new(ValueConfig::default(), ValueInt::default()).into())
});

/// 获取全部货币(ID 722)
pub static NODE_GET_CURRENCY_ALL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(722, vec![ValueEntity::def()], ValueDict::new(ValueConfig::default(), ValueInt::default()).into())
});

/// 获取全部装备(ID 723)
pub static NODE_GET_EQUIPMENT_ALL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(723, vec![ValueEntity::def()], ValueIntList::def())
});

/// 获取掉落物品数量(ID 728)
pub static NODE_GET_LOOT_ITEM_AMOUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(728, vec![ValueEntity::def(), ValueConfig::def()], ValueInt::def())
});

/// 获取掉落货币数量(ID 729)
pub static NODE_GET_LOOT_CURRENCY_AMOUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(729, vec![ValueEntity::def(), ValueConfig::def()], ValueInt::def())
});

/// 获取掉落物品(ID 730)
pub static NODE_GET_LOOT_ITEMS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(730, vec![ValueEntity::def()], ValueDict::new(ValueConfig::default(), ValueInt::default()).into())
});

/// 获取掉落货币(ID 731)
pub static NODE_GET_LOOT_CURRENCY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(731, vec![ValueEntity::def()], ValueDict::new(ValueConfig::default(), ValueInt::default()).into())
});

/// 获取掉落装备(ID 732)
pub static NODE_GET_LOOT_EQUIPMENT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(732, vec![ValueEntity::def()], ValueIntList::def())
});

// ========================================================================
// 商店
// ========================================================================

/// 获取自定义商品(ID 714)
pub static NODE_GET_CUSTOM_SALES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(714, vec![ValueEntity::def(), ValueInt::def()], ValueIntList::def())
});

/// 获取库存商品(ID 715)
pub static NODE_GET_INV_SALES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(715, vec![ValueEntity::def(), ValueInt::def()], ValueConfigList::def())
});

/// 获取购物车商品(ID 716)
pub static NODE_GET_CART_ITEMS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(716, vec![ValueEntity::def(), ValueInt::def()], ValueConfigList::def())
});

/// 获取自定义商品信息(ID 717)
pub static NODE_GET_CUSTOM_ITEM_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(717, 0, 0, vec![ValueEntity::def(), ValueInt::def(), ValueInt::def()], vec![
        ValueConfig::def(),
        ValueDict::new(ValueConfig::default(), ValueInt::default()).into(),
        ValueInt::def(),
        ValueBool::def(),
        ValueInt::def(),
        ValueInt::def(),
        ValueBool::def(),
    ])
});

/// 获取库存商品信息(ID 718)
pub static NODE_GET_INV_ITEM_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(718, 0, 0, vec![ValueEntity::def(), ValueInt::def(), ValueConfig::def()], vec![
        ValueDict::new(ValueConfig::default(), ValueInt::default()).into(),
        ValueInt::def(),
        ValueBool::def(),
    ])
});

/// 获取购买信息(ID 719)
pub static NODE_GET_PURCHASE_INFO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(719, 0, 0, vec![ValueEntity::def(), ValueInt::def(), ValueConfig::def()], vec![
        ValueDict::new(ValueConfig::default(), ValueInt::default()).into(),
        ValueBool::def(),
    ])
});

// ========================================================================
// 标签 / 角色属性
// ========================================================================

/// 获取活跃标签(ID 737)
pub static NODE_GET_ACTIVE_TAG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(737, vec![ValueEntity::def()], ValueConfig::def())
});

/// 获取角色属性(ID 738)
pub static NODE_GET_CHARACTER_ATTR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(738, 0, 0, vec![ValueEntity::def()], vec![
        ValueInt::def(),
        ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
        ValueFloat::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
        ValueFloat::def(),
        ValueEnum::def(),
    ])
});

// ========================================================================
// 箱子
// ========================================================================

/// 获取箱子数量(ID 773)
pub static NODE_GET_BOX_QUANTITY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(773, vec![ValueEntity::def(), ValueInt::def()], ValueInt::def())
});

/// 获取箱子消耗(ID 774)
pub static NODE_GET_BOX_CONSUMPTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(774, vec![ValueEntity::def(), ValueInt::def()], ValueInt::def())
});

// ========================================================================
// 字典操作
// ========================================================================

/// 字典取值(ID 1158)
pub static NODE_DICT_GET_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1158, vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into(), ValueInt::def()], ValueInt::def())
});

/// 是否有键(ID 1368)
pub static NODE_DICT_HAS_KEY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1368, vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into(), ValueInt::def()], ValueBool::def())
});

/// 是否有值(ID 1438)
pub static NODE_DICT_HAS_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1438, vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into(), ValueInt::def()], ValueBool::def())
});

/// 获取键列表(ID 1508)
pub static NODE_DICT_GET_KEYS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1508, vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into()], ValueIntList::def())
});

/// 获取值列表(ID 1578)
pub static NODE_DICT_GET_VALUES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1578, vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into()], ValueIntList::def())
});

/// 获取字典长度(ID 1648)
pub static NODE_DICT_GET_LENGTH: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1648, vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into()], ValueInt::def())
});
