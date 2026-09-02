//! 触发域节点(Server,Trigger)
//!
//! 事件触发器:无 flow 输入,1 个 flow 输出(事件发生时触发),值输出为事件参数。
//! 人工设计,统一用 `NodeType::trigger` 构建。

use std::sync::LazyLock;
use crate::asset::node_graph::NodeKind;
use crate::asset::value::{
    ValueBool, ValueConfig, ValueDefault, ValueDict, ValueEntity, ValueEnum,
    ValueFaction, ValueFloat, ValueGuid, ValueInt, ValueIntList, ValueString, ValueStringList,
    ValueVector,
};

/// 变量变化(ID 36)
pub static NODE_ON_VARIABLE_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(36, vec![ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueInt::def(), ValueInt::def()])
});

/// 状态变化(ID 67)
pub static NODE_ON_STATUS_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(67, vec![ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def(), ValueInt::def()])
});

/// 实体创建(ID 71)
pub static NODE_ON_CREATED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(71, vec![ValueEntity::def(), ValueGuid::def()])
});

/// 实体移除(ID 72)
pub static NODE_ON_REMOVED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(72, vec![ValueGuid::def()])
});

/// 计时器触发(ID 83)
pub static NODE_ON_TIMER_TRIGGER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(83, vec![ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueInt::def(), ValueInt::def()])
});

/// 运动停止(ID 89)
pub static NODE_ON_MOTION_STOP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(89, vec![ValueEntity::def(), ValueGuid::def(), ValueString::def()])
});

/// 触发器离开(ID 91)
pub static NODE_ON_TRIGGER_EXIT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(91, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueGuid::def(), ValueInt::def()])
});

/// 触发器进入(ID 92)
pub static NODE_ON_TRIGGER_ENTER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(92, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueGuid::def(), ValueInt::def()])
});

/// 到达路径点(ID 177)
pub static NODE_ON_REACH_WAYPOINT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(177, vec![ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueInt::def()])
});

/// 阵营变化(ID 251)
pub static NODE_ON_FACTION_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(251, vec![ValueEntity::def(), ValueGuid::def(), ValueFaction::def(), ValueFaction::def()])
});

/// 检测到命中(ID 253)
pub static NODE_ON_HIT_DETECTED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(253, vec![ValueEntity::def(), ValueGuid::def(), ValueBool::def(), ValueEntity::def(), ValueVector::def()])
});

/// 角色倒下(ID 280)
pub static NODE_ON_CHARACTER_DOWN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(280, vec![ValueEntity::def(), ValueEnum::def(), ValueEntity::def()])
});

/// 角色复活(ID 281)
pub static NODE_ON_CHARACTER_REVIVE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(281, vec![ValueEntity::def()])
});

/// 全部倒下(ID 284)
pub static NODE_ON_ALL_DOWN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(284, vec![ValueEntity::def(), ValueEnum::def()])
});

/// 异常复活(ID 285)
pub static NODE_ON_ABNORMAL_REVIVE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(285, vec![ValueEntity::def()])
});

/// 全部复活(ID 286)
pub static NODE_ON_ALL_REVIVED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(286, vec![ValueEntity::def()])
});

/// 传送完成(ID 289)
pub static NODE_ON_TELEPORT_COMPLETE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(289, vec![ValueEntity::def(), ValueGuid::def()])
});

/// 状态结束(ID 299)
pub static NODE_ON_STATUS_END: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(299, vec![ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueEntity::def(), ValueBool::def(), ValueFloat::def(), ValueInt::def(), ValueEntity::def()])
});

/// 单位状态变化(ID 300)
pub static NODE_ON_UNIT_STATUS_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(300, vec![ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueEntity::def(), ValueBool::def(), ValueFloat::def(), ValueInt::def(), ValueInt::def()])
});

/// 被攻击(ID 304)
pub static NODE_ON_BE_ATTACKED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(304, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueFloat::def(), ValueStringList::def(), ValueEnum::def(), ValueFloat::def()])
});

/// 命中目标(ID 305)
pub static NODE_ON_HIT_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(305, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueFloat::def(), ValueStringList::def(), ValueEnum::def(), ValueFloat::def()])
});

/// 标签页选择(ID 307)
pub static NODE_ON_TAB_SELECT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(307, vec![ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueEntity::def()])
});

/// 全局计时器触发(ID 315)
pub static NODE_ON_GLOBAL_TIMER_TRIGGER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(315, vec![ValueEntity::def(), ValueGuid::def(), ValueString::def()])
});

/// 布设组触发(ID 316)
pub static NODE_ON_GROUP_TRIGGER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(316, vec![ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def()])
});

/// 图变量变化(ID 351)
pub static NODE_ON_GRAPH_VARIABLE_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(351, vec![ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueInt::def(), ValueInt::def()])
});

/// 销毁(ID 373)
pub static NODE_ON_DESTROYED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(373, vec![ValueEntity::def(), ValueGuid::def(), ValueVector::def(), ValueVector::def(), ValueEnum::def(), ValueFaction::def(), ValueEntity::def()])
});

/// 造物进入战斗(ID 374)
pub static NODE_ON_CREATION_ENTER_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(374, vec![ValueEntity::def(), ValueGuid::def()])
});

/// 造物离开战斗(ID 375)
pub static NODE_ON_CREATION_LEAVE_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(375, vec![ValueEntity::def(), ValueGuid::def()])
});

/// 职业变化(ID 385)
pub static NODE_ON_CLASS_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(385, vec![ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueConfig::def()])
});

/// 等级变化(ID 386)
pub static NODE_ON_LEVEL_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(386, vec![ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def()])
});

/// 技能调用(ID 392)
pub static NODE_ON_SKILL_CALL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(392, vec![ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueString::def(), ValueString::def()])
});

/// 血量恢复(ID 584)
pub static NODE_ON_HP_RECOVER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(584, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueFloat::def(), ValueStringList::def()])
});

/// 血量恢复开始(ID 585)
pub static NODE_ON_HP_RECOVERY_START: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(585, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueFloat::def(), ValueStringList::def()])
});

/// 仇恨目标变化(ID 611)
pub static NODE_ON_AGGRO_TARGET_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(611, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueEntity::def()])
});

/// 仇恨进入战斗(ID 612)
pub static NODE_ON_AGGRO_ENTER_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(612, vec![ValueEntity::def(), ValueGuid::def()])
});

/// 仇恨离开战斗(ID 613)
pub static NODE_ON_AGGRO_LEAVE_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(613, vec![ValueEntity::def(), ValueGuid::def()])
});

/// 巡逻到达路径点(ID 620)
pub static NODE_ON_PATROL_REACH_WAYPOINT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(620, vec![ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def(), ValueInt::def(), ValueInt::def()])
});

/// 卡组选择(ID 633)
pub static NODE_ON_DECK_SELECTED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(633, vec![ValueEntity::def(), ValueIntList::def(), ValueEnum::def(), ValueInt::def()])
});

/// 元素反应(ID 642)
pub static NODE_ON_ELEMENT_REACTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(642, vec![ValueEntity::def(), ValueGuid::def(), ValueEnum::def(), ValueEntity::def(), ValueGuid::def()])
});

/// 护盾命中(ID 643)
pub static NODE_ON_SHIELD_HIT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(643, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def(), ValueInt::def(), ValueFloat::def()])
});

/// 气泡完成(ID 679)
pub static NODE_ON_BUBBLE_COMPLETE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(679, vec![ValueEntity::def(), ValueEntity::def(), ValueConfig::def(), ValueInt::def()])
});

/// 词缀变化(ID 680)
pub static NODE_ON_AFFIX_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(680, vec![ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def(), ValueFloat::def(), ValueFloat::def()])
});

/// 物品添加(ID 681)
pub static NODE_ON_ITEM_ADD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(681, vec![ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def()])
});

/// 物品失去(ID 682)
pub static NODE_ON_ITEM_LOSE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(682, vec![ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def()])
});

/// 物品数量变化(ID 683)
pub static NODE_ON_ITEM_QUANTITY_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(683, vec![ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def(), ValueInt::def(), ValueEnum::def()])
});

/// 货币变化(ID 684)
pub static NODE_ON_CURRENCY_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(684, vec![ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def()])
});

/// 装备初始化(ID 694)
pub static NODE_ON_EQUIPMENT_INIT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(694, vec![ValueEntity::def(), ValueGuid::def(), ValueInt::def()])
});

/// 装备(ID 695)
pub static NODE_ON_EQUIP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(695, vec![ValueEntity::def(), ValueGuid::def(), ValueInt::def()])
});

/// 卸下装备(ID 696)
pub static NODE_ON_UNEQUIP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(696, vec![ValueEntity::def(), ValueGuid::def(), ValueInt::def()])
});

/// 自定义商品售出(ID 700)
pub static NODE_ON_CUSTOM_ITEM_SOLD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(700, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueInt::def(), ValueInt::def(), ValueInt::def()])
});

/// 库存商品售出(ID 701)
pub static NODE_ON_INV_ITEM_SOLD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(701, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueInt::def(), ValueConfig::def(), ValueInt::def()])
});

/// 出售物品(ID 705)
pub static NODE_ON_SELL_ITEM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(705, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueInt::def(), ValueDict::new(ValueConfig::default(), ValueInt::default()).into()])
});

/// 物品使用(ID 733)
pub static NODE_ON_ITEM_USE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(733, vec![ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def()])
});

/// 职业移除(ID 764)
pub static NODE_ON_CLASS_REMOVE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(764, vec![ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueConfig::def()])
});

/// 可打断(ID 765)
pub static NODE_ON_INTERRUPTIBLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(765, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def()])
});

/// 速度条件(ID 946)
pub static NODE_ON_SPEED_CONDITION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(946, vec![ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueEnum::def(), ValueFloat::def(), ValueFloat::def()])
});

/// 信号(ID 300001)
pub static NODE_ON_SIGNAL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(300001, vec![ValueEntity::def(), ValueGuid::def(), ValueEntity::def()])
});
