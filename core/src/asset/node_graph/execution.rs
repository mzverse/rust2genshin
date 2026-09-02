//! 执行域节点(Server,Execution)
//!
//! 人工设计:每个节点用 `NodeType::procedure`(无返回值)/ `NodeType::func`(单返回值)
//! 或 `NodeType::new`(多输出)构建。1 个 flow 输入 + 值输入 + 1 个 flow 输出。

use std::sync::LazyLock;
use crate::asset::generated::ServerTypeId;
use crate::asset::node_graph::NodeKind;
use crate::asset::value::{
    AnyValue, ValueBool, ValueConfig, ValueConfigList, ValueDefault, ValueDict, ValueEntity,
    ValueEntityList, ValueEnum, ValueFaction, ValueFloat, ValueFloatList, ValueGuid, ValueInt,
    ValueIntList, ValueLocalVarRef, ValuePrefab, ValueString, ValueStringList, ValueVector,
};

pub static NODE_LOG: LazyLock<NodeKind> = LazyLock::new(|| NodeKind::procedure(1, vec![ValueString::def()]));

// ========================================================================
// 变量 / 状态写入
// ========================================================================

/// 写局部变量(ID 19,泛型 Variant):variable(Loc) + value(R<T>) 输入。
/// 变体顺序(TSI 与 kernel 均按参考 data.json,与 Get_Local 同序):
/// Bol/Int/Str/Ety/Gid/Flt/Vec/L<Int>/L<Str>/L<Ety>/L<Gid>/L<Flt>/L<Vec>/L<Bol>/
/// Cfg/Pfb/L<Cfg>/L<Pfb>/Fct/L<Fct>
pub fn node_set_local(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::new(19, 1, 1, vec![ValueLocalVarRef::def(), ty.clone()], vec![]);
    let (selected, kernel) = match ty.get_server_type() {
        ServerTypeId::SBoolean => (0, 19),
        ServerTypeId::SInt => (1, 21),
        ServerTypeId::SString => (2, 2674),
        ServerTypeId::SEntity => (3, 2675),
        ServerTypeId::SGuid => (4, 2676),
        ServerTypeId::SFloat => (5, 2677),
        ServerTypeId::SVector => (6, 2678),
        ServerTypeId::SIntList => (7, 2679),
        ServerTypeId::SStringList => (8, 2680),
        ServerTypeId::SEntityList => (9, 2681),
        ServerTypeId::SGuidList => (10, 2682),
        ServerTypeId::SFloatList => (11, 2683),
        ServerTypeId::SVectorList => (12, 2684),
        ServerTypeId::SBooleanList => (13, 2685),
        ServerTypeId::SConfig => (14, 2686),
        ServerTypeId::SPrefab => (15, 2687),
        ServerTypeId::SConfigList => (16, 2688),
        ServerTypeId::SPrefabList => (17, 2689),
        ServerTypeId::SFaction => (18, 2690),
        ServerTypeId::SFactionList => (19, 2691),
        other => panic!("Unsupported type: {other:?}"),
    };
    result.kernel_id = kernel;
    result.selectors_in[1] = selected.into();
    result
}

/// 写自定义变量(ID 22):entity + 变量名 + 值(+ 是否全局)
pub static NODE_SET_VARIABLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(22, vec![ValueEntity::def(), ValueString::def(), ValueInt::def(), ValueBool::def()])
});

/// 写状态值(ID 66)
pub static NODE_SET_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(66, vec![ValueEntity::def(), ValueInt::def(), ValueInt::def()])
});

/// 写图变量(ID 323)
pub static NODE_SET_GRAPH_VARIABLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(323, vec![ValueString::def(), ValueInt::def(), ValueBool::def()])
});

// ========================================================================
// 实体创建 / 销毁
// ========================================================================

/// 销毁实体(ID 69)
pub static NODE_DESTROY_ENTITY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(69, vec![ValueEntity::def()])
});

/// 创建实体(ID 70):按 GUID + 实例 ID 列表
pub static NODE_CREATE_ENTITY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(70, vec![ValueGuid::def(), ValueIntList::def()])
});

/// 创建预制体实体(ID 252):输出实体
pub static NODE_CREATE_PREFAB: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::func(252, vec![ValuePrefab::def(), ValueVector::def(), ValueVector::def(), ValueEntity::def(), ValueBool::def(), ValueInt::def(), ValueIntList::def()], ValueEntity::def())
});

/// 创建投射物(ID 256):输出实体
pub static NODE_CREATE_PROJECTILE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::func(256, vec![ValuePrefab::def(), ValueVector::def(), ValueVector::def(), ValueEntity::def(), ValueEntity::def(), ValueBool::def(), ValueInt::def(), ValueIntList::def()], ValueEntity::def())
});

/// 结算关卡(ID 77)
pub static NODE_SETTLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(77, vec![])
});

/// 转发事件(ID 190)
pub static NODE_FORWARD_EVENT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(190, vec![ValueEntity::def()])
});

/// 设置模型可见(ID 308)
pub static NODE_SET_MODEL_VISIBLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(308, vec![ValueEntity::def(), ValueBool::def()])
});

/// 实体布设组状态(ID 178)
pub static NODE_SET_GROUP_STATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(178, vec![ValueInt::def(), ValueBool::def()])
});

// ========================================================================
// 计时器 / 全局计时器
// ========================================================================

/// 启动计时器(ID 79)
pub static NODE_START: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(79, vec![ValueEntity::def(), ValueString::def(), ValueBool::def(), ValueFloatList::def()])
});

/// 暂停计时器(ID 80)
pub static NODE_PAUSE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(80, vec![ValueEntity::def(), ValueString::def()])
});

/// 恢复计时器(ID 81)
pub static NODE_RESUME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(81, vec![ValueEntity::def(), ValueString::def()])
});

/// 停止计时器(ID 82)
pub static NODE_STOP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(82, vec![ValueEntity::def(), ValueString::def()])
});

/// 启动全局计时器(ID 311)
pub static NODE_GLOBAL_TIMER_START: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(311, vec![ValueEntity::def(), ValueString::def()])
});

/// 暂停全局计时器(ID 309)
pub static NODE_GLOBAL_TIMER_PAUSE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(309, vec![ValueEntity::def(), ValueString::def()])
});

/// 恢复全局计时器(ID 312)
pub static NODE_GLOBAL_TIMER_RESUME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(312, vec![ValueEntity::def(), ValueString::def()])
});

/// 停止全局计时器(ID 313)
pub static NODE_GLOBAL_TIMER_STOP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(313, vec![ValueEntity::def(), ValueString::def()])
});

/// 修改全局计时器(ID 314)
pub static NODE_GLOBAL_TIMER_MODIFY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(314, vec![ValueEntity::def(), ValueString::def(), ValueFloat::def()])
});

// ========================================================================
// 运动设备
// ========================================================================

/// 添加线性运动(ID 84)
pub static NODE_ADD_LINEAR_MOTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(84, vec![ValueEntity::def(), ValueString::def(), ValueFloat::def(), ValueVector::def()])
});

/// 添加旋转运动(ID 85)
pub static NODE_ADD_ROTATION_MOTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(85, vec![ValueEntity::def(), ValueString::def(), ValueFloat::def(), ValueFloat::def(), ValueVector::def()])
});

/// 停止并删除运动设备(ID 86)
pub static NODE_STOP_DELETE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(86, vec![ValueEntity::def(), ValueString::def(), ValueBool::def()])
});

/// 暂停运动设备(ID 87)
pub static NODE_MOTION_PAUSE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(87, vec![ValueEntity::def(), ValueString::def()])
});

/// 恢复运动设备(ID 88)
pub static NODE_MOTION_RESUME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(88, vec![ValueEntity::def(), ValueString::def()])
});

/// 激活(ID 267)
pub static NODE_ACTIVATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(267, vec![ValueEntity::def(), ValueString::def()])
});

// ========================================================================
// 动画
// ========================================================================

/// 播放单次动画(ID 93)
pub static NODE_PLAY_TIMED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(93, vec![ValueConfig::def(), ValueEntity::def(), ValueString::def(), ValueBool::def(), ValueBool::def(), ValueVector::def(), ValueVector::def(), ValueFloat::def(), ValueBool::def()])
});

/// 播放循环动画(ID 94):输出播放 ID
pub static NODE_PLAY_LOOP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::func(94, vec![ValueConfig::def(), ValueEntity::def(), ValueString::def(), ValueBool::def(), ValueBool::def(), ValueVector::def(), ValueVector::def(), ValueFloat::def(), ValueBool::def()], ValueInt::def())
});

/// 停止循环动画(ID 95)
pub static NODE_STOP_LOOP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(95, vec![ValueInt::def(), ValueEntity::def()])
});

// ========================================================================
// 碰撞 / 触发器
// ========================================================================

/// 设置原生碰撞(ID 240)
pub static NODE_SET_NATIVE_COLLISION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(240, vec![ValueEntity::def(), ValueBool::def()])
});

/// 设置原生攀爬(ID 241)
pub static NODE_SET_NATIVE_CLIMB: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(241, vec![ValueEntity::def(), ValueBool::def()])
});

/// 设置附加碰撞(ID 242)
pub static NODE_SET_EXTRA_COLLISION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(242, vec![ValueEntity::def(), ValueInt::def(), ValueBool::def()])
});

/// 设置附加攀爬(ID 243)
pub static NODE_SET_EXTRA_CLIMB: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(243, vec![ValueEntity::def(), ValueInt::def(), ValueBool::def()])
});

/// 设置触发器状态(ID 90)
pub static NODE_SET_TRIGGER_STATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(90, vec![ValueEntity::def(), ValueInt::def(), ValueBool::def()])
});

// ========================================================================
// 复活
// ========================================================================

/// 激活复活点(ID 272)
pub static NODE_ACTIVATE_REVIVE_POINT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(272, vec![ValueEntity::def(), ValueInt::def()])
});

/// 停用复活点(ID 273)
pub static NODE_DEACTIVATE_REVIVE_POINT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(273, vec![ValueEntity::def(), ValueInt::def()])
});

/// 设置允许复活(ID 274)
pub static NODE_SET_REVIVE_ALLOWED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(274, vec![ValueEntity::def(), ValueBool::def()])
});

/// 设置复活次数(ID 276)
pub static NODE_SET_REVIVE_COUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(276, vec![ValueEntity::def(), ValueInt::def()])
});

/// 设置复活时间(ID 278)
pub static NODE_SET_REVIVE_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(278, vec![ValueEntity::def(), ValueInt::def()])
});

/// 单体复活(ID 279)
pub static NODE_REVIVE_SINGLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(279, vec![ValueEntity::def()])
});

/// 全部倒下(ID 282)
pub static NODE_DEFEAT_ALL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(282, vec![ValueEntity::def()])
});

/// 全部复活(ID 283)
pub static NODE_REVIVE_ALL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(283, vec![ValueEntity::def(), ValueBool::def()])
});

// ========================================================================
// 传送 / 设备 / 阵营
// ========================================================================

/// 传送(ID 288)
pub static NODE_TELEPORT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(288, vec![ValueEntity::def(), ValueVector::def(), ValueVector::def()])
});

/// 修改设备(ID 302)
pub static NODE_MODIFY_DEVICE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(302, vec![ValueEntity::def(), ValueInt::def()])
});

/// 设置标签页状态(ID 306)
pub static NODE_SET_TAB_STATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(306, vec![ValueEntity::def(), ValueInt::def(), ValueBool::def()])
});

/// 切换模板(ID 261)
pub static NODE_SWITCH_TEMPLATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(261, vec![ValueEntityList::def(), ValueString::def()])
});

/// 设置阵营(ID 250)
pub static NODE_SET_FACTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(250, vec![ValueEntity::def(), ValueFaction::def()])
});

/// 设置目标 GUID(ID 245)
pub static NODE_SET_TARGET_GUID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(245, vec![ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueVector::def(), ValueVector::def(), ValueEnum::def(), ValueEnum::def()])
});

/// 设置设备状态(ID 365)
pub static NODE_SET_DEVICE_STATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(365, vec![ValueEntity::def(), ValueBool::def()])
});

/// 设置源状态(ID 367)
pub static NODE_SET_SOURCE_STATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(367, vec![ValueEntity::def(), ValueBool::def()])
});

// ========================================================================
// 列表操作
// ========================================================================

/// 拼接列表(ID 100)
pub static NODE_CONCATENATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(100, vec![ValueIntList::def(), ValueIntList::def()])
});

/// 清空列表(ID 107)
pub static NODE_CLEAR_LIST: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(107, vec![ValueIntList::def()])
});

/// 插入元素(ID 135)
pub static NODE_INSERT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(135, vec![ValueIntList::def(), ValueInt::def(), ValueInt::def()])
});

/// 移除元素(ID 153)
pub static NODE_REMOVE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(153, vec![ValueIntList::def(), ValueInt::def()])
});

/// 修改下标(ID 160)
pub static NODE_MODIFY_INDEX: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(160, vec![ValueIntList::def(), ValueInt::def(), ValueInt::def()])
});

/// 排序列表(ID 167)
pub static NODE_SORT_LIST: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(167, vec![ValueIntList::def(), ValueEnum::def()])
});

// ========================================================================
// 状态 / 攻击
// ========================================================================

/// 添加状态(ID 297):输出状态枚举与层数
pub static NODE_ADD_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(297, 1, 1, vec![ValueEntity::def(), ValueEntity::def(), ValueConfig::def(), ValueInt::def(), ValueDict::new(ValueString::default(), ValueFloat::default()).into()], vec![ValueEnum::def(), ValueInt::def()])
});

/// 移除状态(ID 301)
pub static NODE_REMOVE_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(301, vec![ValueEntity::def(), ValueConfig::def(), ValueEnum::def(), ValueEntity::def()])
});

/// 攻击(ID 303)
pub static NODE_ATTACK: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(303, vec![ValueEntity::def(), ValueFloat::def(), ValueFloat::def(), ValueVector::def(), ValueVector::def(), ValueString::def(), ValueBool::def(), ValueEntity::def()])
});

/// 移除实体(ID 372)
pub static NODE_REMOVE_ENTITY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(372, vec![ValueEntity::def()])
});

// ========================================================================
// 布设组
// ========================================================================

/// 切换布局(ID 382)
pub static NODE_SWITCH_LAYOUT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(382, vec![ValueEntity::def(), ValueInt::def()])
});

/// 激活布设组(ID 383)
pub static NODE_ACTIVATE_GROUP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(383, vec![ValueEntity::def(), ValueInt::def()])
});

/// 修改布设组状态(ID 384)
pub static NODE_MODIFY_GROUP_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(384, vec![ValueEntity::def(), ValueInt::def(), ValueEnum::def()])
});

/// 移除布设组(ID 521)
pub static NODE_REMOVE_GROUP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(521, vec![ValueEntity::def(), ValueInt::def()])
});

// ========================================================================
// 职业 / 等级 / 技能
// ========================================================================

/// 切换职业(ID 389)
pub static NODE_CHANGE_CLASS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(389, vec![ValueEntity::def(), ValueConfig::def()])
});

/// 添加经验(ID 390)
pub static NODE_ADD_EXP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(390, vec![ValueEntity::def(), ValueInt::def()])
});

/// 设置等级(ID 391)
pub static NODE_SET_LEVEL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(391, vec![ValueEntity::def(), ValueInt::def()])
});

/// 修改技能资源(ID 393)
pub static NODE_MODIFY_SKILL_RESOURCE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(393, vec![ValueEntity::def(), ValueConfig::def(), ValueFloat::def()])
});

/// 设置技能资源(ID 394)
pub static NODE_SET_SKILL_RESOURCE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(394, vec![ValueEntity::def(), ValueConfig::def(), ValueFloat::def()])
});

/// 添加技能(ID 395)
pub static NODE_ADD_SKILL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(395, vec![ValueEntity::def(), ValueConfig::def(), ValueEnum::def()])
});

/// 按 ID 移除技能(ID 396)
pub static NODE_REMOVE_SKILL_BY_ID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(396, vec![ValueEntity::def(), ValueConfig::def()])
});

/// 初始化技能(ID 397)
pub static NODE_INIT_SKILL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(397, vec![ValueEntity::def(), ValueEnum::def()])
});

/// 按槽位移除技能(ID 399)
pub static NODE_REMOVE_SKILL_BY_SLOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(399, vec![ValueEntity::def(), ValueEnum::def()])
});

/// 设置技能冷却(ID 739)
pub static NODE_SET_SKILL_CD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(739, vec![ValueEntity::def(), ValueEnum::def(), ValueFloat::def(), ValueBool::def()])
});

/// 修改技能冷却(ID 740)
pub static NODE_MODIFY_SKILL_CD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(740, vec![ValueEntity::def(), ValueEnum::def(), ValueFloat::def(), ValueBool::def()])
});

/// 修改技能冷却比例(ID 741)
pub static NODE_MODIFY_SKILL_CD_RATIO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(741, vec![ValueEntity::def(), ValueEnum::def(), ValueFloat::def(), ValueBool::def()])
});

// ========================================================================
// 特效 / 运动
// ========================================================================

/// 按资产停止特效(ID 473)
pub static NODE_STOP_EFFECT_BY_ASSET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(473, vec![ValueEntity::def(), ValueConfig::def()])
});

/// 添加目标旋转(ID 520)
pub static NODE_ADD_TARGET_ROTATION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(520, vec![ValueEntity::def(), ValueString::def(), ValueFloat::def(), ValueVector::def()])
});

/// 激活固定点(ID 775)
pub static NODE_ACTIVATE_FIXED_POINT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(775, vec![ValueEntity::def(), ValueString::def(), ValueEnum::def(), ValueFloat::def(), ValueVector::def(), ValueVector::def(), ValueBool::def(), ValueEnum::def()])
});

/// 设置目标实体(ID 668)
pub static NODE_SET_TARGET_ENTITY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(668, vec![ValueEntity::def(), ValueEntity::def(), ValueString::def(), ValueVector::def(), ValueVector::def(), ValueEnum::def(), ValueEnum::def()])
});

// ========================================================================
// 标签
// ========================================================================

/// 添加标签(ID 586)
pub static NODE_ADD_TAG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(586, vec![ValueEntity::def(), ValueInt::def()])
});

/// 移除标签(ID 587)
pub static NODE_REMOVE_TAG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(587, vec![ValueEntity::def(), ValueInt::def()])
});

/// 清空标签(ID 588)
pub static NODE_CLEAR_TAGS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(588, vec![ValueEntity::def()])
});

// ========================================================================
// 仇恨
// ========================================================================

/// 设置仇恨(ID 599)
pub static NODE_SET_AGGRO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(599, vec![ValueEntity::def(), ValueEntity::def(), ValueInt::def()])
});

/// 移除仇恨(ID 600)
pub static NODE_REMOVE_AGGRO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(600, vec![ValueEntity::def(), ValueEntity::def()])
});

/// 清空仇恨(ID 601)
pub static NODE_CLEAR_AGGRO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(601, vec![ValueEntity::def()])
});

/// 嘲讽(ID 602)
pub static NODE_TAUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(602, vec![ValueEntity::def(), ValueEntity::def()])
});

// ========================================================================
// 血量
// ========================================================================

/// 恢复血量(ID 583)
pub static NODE_RECOVER_HP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(583, vec![ValueEntity::def(), ValueFloat::def(), ValueString::def(), ValueBool::def(), ValueEntity::def()])
});

/// 立即恢复血量(ID 698)
pub static NODE_RECOVER_HP_INSTANT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(698, vec![ValueEntity::def(), ValueEntity::def(), ValueFloat::def(), ValueBool::def(), ValueFloat::def(), ValueFloat::def(), ValueStringList::def()])
});

/// 损失血量(ID 697)
pub static NODE_LOSS_HP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(697, vec![ValueEntity::def(), ValueFloat::def(), ValueBool::def(), ValueBool::def(), ValueBool::def(), ValueEnum::def()])
});

// ========================================================================
// 名牌 / 巡逻 / 气泡
// ========================================================================

/// 设置名牌(ID 617)
pub static NODE_SET_NAMEPLATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(617, vec![ValueEntity::def(), ValueConfigList::def()])
});

/// 切换巡逻模板(ID 618)
pub static NODE_SWITCH_PATROL_TEMPLATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(618, vec![ValueEntity::def(), ValueInt::def()])
});

/// 设置气泡(ID 631)
pub static NODE_SET_BUBBLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(631, vec![ValueEntity::def(), ValueConfig::def()])
});

// ========================================================================
// 卡组
// ========================================================================

/// 打开卡组(ID 632)
pub static NODE_DECK_OPEN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(632, vec![ValueEntity::def(), ValueInt::def(), ValueFloat::def(), ValueIntList::def(), ValueIntList::def(), ValueInt::def(), ValueInt::def()])
});

/// 关闭卡组(ID 641)
pub static NODE_DECK_CLOSE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(641, vec![ValueEntity::def(), ValueInt::def()])
});

/// 获取随机列表(ID 743):输出打乱后的列表
pub static NODE_GET_RANDOM_LIST: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::func(743, vec![ValueIntList::def()], ValueIntList::def())
});

// ========================================================================
// 地图 / 标记
// ========================================================================

/// 设置地图缩放(ID 634)
pub static NODE_SET_MAP_ZOOM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(634, vec![ValueEntity::def(), ValueFloat::def()])
});

/// 设置标记状态(ID 635)
pub static NODE_SET_MARKER_STATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(635, vec![ValueEntity::def(), ValueIntList::def(), ValueBool::def()])
});

/// 设置可见列表(ID 636)
pub static NODE_SET_VISIBLE_LIST: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(636, vec![ValueEntity::def(), ValueInt::def(), ValueEntityList::def()])
});

/// 设置追踪列表(ID 637)
pub static NODE_SET_TRACK_LIST: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(637, vec![ValueEntity::def(), ValueInt::def(), ValueEntityList::def()])
});

/// 更新标记(ID 640)
pub static NODE_UPDATE_MARKERS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(640, vec![ValueEntity::def(), ValueInt::def(), ValueEntity::def()])
});

// ========================================================================
// 成就 / 计分
// ========================================================================

/// 设置成就进度(ID 645)
pub static NODE_SET_ACHIEVEMENT_PROGRESS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(645, vec![ValueEntity::def(), ValueInt::def(), ValueInt::def()])
});

/// 增加成就进度(ID 646)
pub static NODE_ADD_ACHIEVEMENT_PROGRESS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(646, vec![ValueEntity::def(), ValueInt::def(), ValueInt::def()])
});

/// 设置计分板(ID 647)
pub static NODE_SET_SCOREBOARD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(647, vec![ValueEntity::def(), ValueInt::def(), ValueString::def(), ValueInt::def()])
});

/// 设置玩家排行(ID 650)
pub static NODE_SET_PLAYER_RANK: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(650, vec![ValueEntity::def(), ValueInt::def()])
});

/// 设置玩家结果(ID 652)
pub static NODE_SET_PLAYER_RESULT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(652, vec![ValueEntity::def(), ValueEnum::def()])
});

/// 设置阵营排行(ID 654)
pub static NODE_SET_FACTION_RANK: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(654, vec![ValueFaction::def(), ValueInt::def()])
});

/// 设置阵营结果(ID 656)
pub static NODE_SET_FACTION_RESULT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(656, vec![ValueFaction::def(), ValueEnum::def()])
});

/// 修改分数(ID 659)
pub static NODE_MODIFY_SCORE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(659, vec![ValueEntity::def(), ValueEnum::def(), ValueInt::def()])
});

/// 设置逃生有效(ID 661)
pub static NODE_SET_ESCAPE_VALID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(661, vec![ValueEntity::def(), ValueBool::def()])
});

/// 切换计分组(ID 663)
pub static NODE_SWITCH_SCORE_GROUP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(663, vec![ValueEntity::def(), ValueInt::def()])
});

// ========================================================================
// 时间 / 灯光
// ========================================================================

/// 设置时间(ID 665)
pub static NODE_SET_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(665, vec![ValueFloat::def()])
});

/// 设置时间流速(ID 666)
pub static NODE_SET_TIME_SPEED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(666, vec![ValueFloat::def()])
});

/// 开关灯光(ID 667)
pub static NODE_TOGGLE_LIGHT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(667, vec![ValueEntity::def(), ValueInt::def(), ValueBool::def()])
});

// ========================================================================
// 音效
// ========================================================================

/// 关闭音效播放器(ID 591)
pub static NODE_CLOSE_SOUND_PLAYER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(591, vec![ValueEntity::def(), ValueInt::def()])
});

/// 切换音效播放器(ID 592)
pub static NODE_TOGGLE_SOUND_PLAYER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(592, vec![ValueEntity::def(), ValueInt::def(), ValueBool::def()])
});

/// 调整音效播放器(ID 593)
pub static NODE_ADJUST_SOUND_PLAYER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(593, vec![ValueEntity::def(), ValueInt::def(), ValueInt::def(), ValueFloat::def()])
});

/// 添加音效播放器(ID 594):输出播放 ID
pub static NODE_ADD_SOUND_PLAYER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::func(594, vec![ValueEntity::def(), ValueInt::def(), ValueInt::def(), ValueFloat::def(), ValueBool::def(), ValueFloat::def(), ValueFloat::def()], ValueInt::def())
});

/// 切换 BGM(ID 595)
pub static NODE_TOGGLE_BGM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(595, vec![ValueEntity::def(), ValueBool::def()])
});

/// 设置 BGM 音量(ID 596)
pub static NODE_SET_BGM_VOLUME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(596, vec![ValueEntity::def(), ValueInt::def()])
});

/// 设置 BGM(ID 597)
pub static NODE_SET_BGM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(597, vec![ValueEntity::def(), ValueInt::def(), ValueFloat::def(), ValueFloat::def(), ValueInt::def(), ValueBool::def(), ValueFloat::def()])
});

/// 播放 2D 单次音效(ID 598)
pub static NODE_PLAY_2D_ONE_SHOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(598, vec![ValueEntity::def(), ValueInt::def(), ValueInt::def(), ValueFloat::def()])
});

// ========================================================================
// 词缀
// ========================================================================

/// 添加词缀(ID 672)
pub static NODE_ADD_AFFIX: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(672, vec![ValueInt::def(), ValueConfig::def(), ValueBool::def(), ValueFloat::def()])
});

/// 移除词缀(ID 673)
pub static NODE_REMOVE_AFFIX: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(673, vec![ValueInt::def(), ValueInt::def()])
});

/// 修改词缀(ID 674)
pub static NODE_MODIFY_AFFIX: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(674, vec![ValueInt::def(), ValueInt::def(), ValueFloat::def()])
});

/// 按 ID 添加词缀(ID 742)
pub static NODE_ADD_AFFIX_BY_ID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(742, vec![ValueInt::def(), ValueConfig::def(), ValueInt::def(), ValueBool::def(), ValueFloat::def()])
});

// ========================================================================
// 物品 / 货币
// ========================================================================

/// 扩展容量(ID 685)
pub static NODE_EXPAND_CAPACITY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(685, vec![ValueEntity::def(), ValueInt::def()])
});

/// 修改物品(ID 686)
pub static NODE_MODIFY_ITEM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(686, vec![ValueEntity::def(), ValueConfig::def(), ValueInt::def()])
});

/// 设置掉落数量(ID 687)
pub static NODE_SET_DROP_AMOUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(687, vec![ValueEntity::def(), ValueConfig::def(), ValueInt::def(), ValueEnum::def()])
});

/// 修改货币(ID 688)
pub static NODE_MODIFY_CURRENCY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(688, vec![ValueEntity::def(), ValueConfig::def(), ValueInt::def()])
});

// ========================================================================
// 掉落
// ========================================================================

/// 设置掉落物品(ID 720)
pub static NODE_SET_DROP_ITEMS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(720, vec![ValueEntity::def(), ValueDict::new(ValueConfig::default(), ValueInt::default()).into(), ValueEnum::def()])
});

/// 触发掉落(ID 724)
pub static NODE_TRIGGER_DROP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(724, vec![ValueEntity::def(), ValueEnum::def()])
});

/// 设置战利品内容(ID 725)
pub static NODE_SET_LOOT_CONTENT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(725, vec![ValueEntity::def(), ValueDict::new(ValueConfig::default(), ValueInt::default()).into()])
});

/// 修改战利品物品(ID 726)
pub static NODE_MODIFY_LOOT_ITEM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(726, vec![ValueEntity::def(), ValueConfig::def(), ValueInt::def()])
});

/// 修改战利品货币(ID 727)
pub static NODE_MODIFY_LOOT_CURRENCY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(727, vec![ValueEntity::def(), ValueConfig::def(), ValueInt::def()])
});

// ========================================================================
// 商店
// ========================================================================

/// 打开商店(ID 702)
pub static NODE_SHOP_OPEN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(702, vec![ValueEntity::def(), ValueEntity::def(), ValueInt::def()])
});

/// 关闭商店(ID 703)
pub static NODE_SHOP_CLOSE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(703, vec![ValueEntity::def()])
});

/// 修改自定义商品(ID 704)
pub static NODE_MODIFY_CUSTOM_SALE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(704, vec![ValueEntity::def(), ValueInt::def(), ValueInt::def(), ValueConfig::def(), ValueDict::new(ValueConfig::default(), ValueInt::default()).into(), ValueInt::def(), ValueBool::def()])
});

/// 修改库存商品(ID 706)
pub static NODE_MODIFY_INVENTORY_SALE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(706, vec![ValueEntity::def(), ValueInt::def(), ValueConfig::def(), ValueDict::new(ValueConfig::default(), ValueInt::default()).into(), ValueInt::def(), ValueInt::def(), ValueBool::def()])
});

/// 修改购物车商品(ID 707)
pub static NODE_MODIFY_CART_ITEM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(707, vec![ValueEntity::def(), ValueInt::def(), ValueConfig::def(), ValueDict::new(ValueConfig::default(), ValueInt::default()).into(), ValueBool::def()])
});

/// 添加自定义商品(ID 708):输出价格
pub static NODE_ADD_CUSTOM_SALE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::func(708, vec![ValueEntity::def(), ValueInt::def(), ValueConfig::def(), ValueDict::new(ValueConfig::default(), ValueInt::default()).into(), ValueInt::def(), ValueBool::def(), ValueInt::def()], ValueInt::def())
});

/// 添加库存商品(ID 709)
pub static NODE_ADD_INVENTORY_SALE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(709, vec![ValueEntity::def(), ValueInt::def(), ValueConfig::def(), ValueDict::new(ValueConfig::default(), ValueInt::default()).into(), ValueInt::def(), ValueInt::def(), ValueBool::def()])
});

/// 加入购物车(ID 710)
pub static NODE_ADD_TO_CART: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(710, vec![ValueEntity::def(), ValueInt::def(), ValueConfig::def(), ValueDict::new(ValueConfig::default(), ValueInt::default()).into(), ValueBool::def()])
});

/// 移除自定义商品(ID 711)
pub static NODE_REMOVE_CUSTOM_SALE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(711, vec![ValueEntity::def(), ValueInt::def(), ValueInt::def()])
});

/// 移除库存商品(ID 712)
pub static NODE_REMOVE_INVENTORY_SALE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(712, vec![ValueEntity::def(), ValueInt::def(), ValueConfig::def()])
});

/// 移出购物车(ID 713)
pub static NODE_REMOVE_FROM_CART: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(713, vec![ValueEntity::def(), ValueInt::def(), ValueConfig::def()])
});

// ========================================================================
// 扫描
// ========================================================================

/// 设置扫描规则(ID 735)
pub static NODE_SET_SCAN_RULES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(735, vec![ValueEntity::def(), ValueEnum::def()])
});

/// 设置活跃扫描标签(ID 736)
pub static NODE_SET_ACTIVE_SCAN_TAG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(736, vec![ValueEntity::def(), ValueInt::def()])
});

// ========================================================================
// 布设组创建
// ========================================================================

/// 创建预制体布设组(ID 757):输出实体列表
pub static NODE_CREATE_PREFAB_GROUP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::func(757, vec![ValueInt::def(), ValueVector::def(), ValueVector::def(), ValueEntity::def(), ValueInt::def(), ValueIntList::def(), ValueBool::def()], ValueEntityList::def())
});

// ========================================================================
// 计分 / 环境 / 聊天
// ========================================================================

/// 设置整数分数(ID 761)
pub static NODE_SET_SCORE_INT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(761, vec![ValueIntList::def(), ValueInt::def(), ValueInt::def()])
});

/// 设置浮点分数(ID 762)
pub static NODE_SET_SCORE_FLOAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(762, vec![ValueIntList::def(), ValueFloat::def(), ValueInt::def()])
});

/// 修改环境(ID 763)
pub static NODE_MODIFY_ENVIRONMENT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(763, vec![ValueInt::def(), ValueEntityList::def(), ValueBool::def(), ValueInt::def()])
});

/// 设置聊天开关(ID 769)
pub static NODE_SET_CHAT_SWITCH: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(769, vec![ValueInt::def(), ValueBool::def()])
});

/// 修改聊天权限(ID 770)
pub static NODE_MODIFY_CHAT_PERMISSION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(770, vec![ValueGuid::def(), ValueInt::def(), ValueBool::def()])
});

/// 设置当前频道(ID 771)
pub static NODE_SET_CURRENT_CHANNEL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(771, vec![ValueGuid::def(), ValueIntList::def()])
});

// ========================================================================
// 箱子
// ========================================================================

/// 消耗箱子(ID 772)
pub static NODE_CONSUME_BOX: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(772, vec![ValueEntity::def(), ValueInt::def(), ValueInt::def()])
});

// ========================================================================
// 字典操作
// ========================================================================

/// 字典设值(ID 948)
pub static NODE_DICT_SET_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(948, vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into(), ValueInt::def(), ValueInt::def()])
});

/// 按键移除(ID 1298)
pub static NODE_DICT_REMOVE_BY_KEY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(1298, vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into(), ValueInt::def()])
});

/// 清空字典(ID 1718)
pub static NODE_DICT_CLEAR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(1718, vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into()])
});

/// 按键排序(ID 1928):输出键列表与值列表
pub static NODE_DICT_SORT_BY_KEY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(1928, 1, 1, vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into(), ValueEnum::def()], vec![ValueIntList::def(), ValueIntList::def()])
});

/// 按值排序(ID 1938)
pub static NODE_DICT_SORT_BY_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(1938, 1, 1, vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into(), ValueEnum::def()], vec![ValueIntList::def(), ValueIntList::def()])
});

// ========================================================================
// 遍历
// ========================================================================

/// 遍历列表(ID 509):list → 每项执行 body,结束后走 done;输出当前项(Int)
pub static NODE_FOR_EACH: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(509, 2, 2, vec![ValueIntList::def()], vec![ValueInt::def()])
});

// ========================================================================
// 信号 / 结构体
// ========================================================================

/// 发送信号(ID 300000)
pub static NODE_SEND_SIGNAL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(300000, vec![])
});

/// 修改结构体(ID 300004)
pub static NODE_STRUCTURE_MODIFY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(300004, vec![])
});
