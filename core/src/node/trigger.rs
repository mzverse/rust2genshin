//! 触发域节点(Server,Trigger)
//!
//! 事件触发器:无 flow 输入,1 个 flow 输出(事件发生时触发),值输出为事件参数。
//! 人工设计,统一用 `NodeType::trigger` 构建。

use crate::node::NodeKind;
use std::sync::LazyLock;
use crate::asset::value::NativeKind::*;

/// 变量变化(ID 36)
pub static NODE_ON_VARIABLE_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(36, vec![Entity, Guid, String, Int, Int])
});

/// 状态变化(ID 67)
pub static NODE_ON_STATUS_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(67, vec![Entity, Guid, Int, Int, Int])
});

/// 实体创建(ID 71)
pub static NODE_ON_CREATED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(71, vec![Entity, Guid])
});

/// 实体移除(ID 72)
pub static NODE_ON_REMOVED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(72, vec![Guid])
});

/// 计时器触发(ID 83)
pub static NODE_ON_TIMER_TRIGGER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(83, vec![Entity, Guid, String, Int, Int])
});

/// 运动停止(ID 89)
pub static NODE_ON_MOTION_STOP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(89, vec![Entity, Guid, String])
});

/// 触发器离开(ID 91)
pub static NODE_ON_TRIGGER_EXIT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(91, vec![Entity, Guid, Entity, Guid, Int])
});

/// 触发器进入(ID 92)
pub static NODE_ON_TRIGGER_ENTER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(92, vec![Entity, Guid, Entity, Guid, Int])
});

/// 到达路径点(ID 177)
pub static NODE_ON_REACH_WAYPOINT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(177, vec![Entity, Guid, String, Int])
});

/// 阵营变化(ID 251)
pub static NODE_ON_FACTION_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(251, vec![Entity, Guid, Faction, Faction])
});

/// 检测到命中(ID 253)
pub static NODE_ON_HIT_DETECTED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(253, vec![Entity, Guid, Bool, Entity, Vec3])
});

/// 角色倒下(ID 280)
pub static NODE_ON_CHARACTER_DOWN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(280, vec![Entity, Enum(todo!()), Entity])
});

/// 角色复活(ID 281)
pub static NODE_ON_CHARACTER_REVIVE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(281, vec![Entity])
});

/// 全部倒下(ID 284)
pub static NODE_ON_ALL_DOWN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(284, vec![Entity, Enum(todo!())])
});

/// 异常复活(ID 285)
pub static NODE_ON_ABNORMAL_REVIVE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(285, vec![Entity])
});

/// 全部复活(ID 286)
pub static NODE_ON_ALL_REVIVED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(286, vec![Entity])
});

/// 传送完成(ID 289)
pub static NODE_ON_TELEPORT_COMPLETE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(289, vec![Entity, Guid])
});

/// 状态结束(ID 299)
pub static NODE_ON_STATUS_END: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(299, vec![Entity, Guid, Config, Entity, Bool, Float, Int, Entity])
});

/// 单位状态变化(ID 300)
pub static NODE_ON_UNIT_STATUS_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(300, vec![Entity, Guid, Config, Entity, Bool, Float, Int, Int])
});

/// 被攻击(ID 304)
pub static NODE_ON_BE_ATTACKED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(304, vec![Entity, Guid, Entity, Float, List(String.into()), Enum(todo!()), Float])
});

/// 命中目标(ID 305)
pub static NODE_ON_HIT_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(305, vec![Entity, Guid, Entity, Float, List(String.into()), Enum(todo!()), Float])
});

/// 标签页选择(ID 307)
pub static NODE_ON_TAB_SELECT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(307, vec![Entity, Guid, Int, Entity])
});

/// 全局计时器触发(ID 315)
pub static NODE_ON_GLOBAL_TIMER_TRIGGER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(315, vec![Entity, Guid, String])
});

/// 布设组触发(ID 316)
pub static NODE_ON_GROUP_TRIGGER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(316, vec![Entity, Guid, Int, Int])
});

/// 图变量变化(ID 351)
pub static NODE_ON_GRAPH_VARIABLE_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(351, vec![Entity, Guid, String, Int, Int])
});

/// 销毁(ID 373)
pub static NODE_ON_DESTROYED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(373, vec![Entity, Guid, Vec3, Vec3, Enum(todo!()), Faction, Entity])
});

/// 造物进入战斗(ID 374)
pub static NODE_ON_CREATION_ENTER_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(374, vec![Entity, Guid])
});

/// 造物离开战斗(ID 375)
pub static NODE_ON_CREATION_LEAVE_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(375, vec![Entity, Guid])
});

/// 职业变化(ID 385)
pub static NODE_ON_CLASS_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(385, vec![Entity, Guid, Config, Config])
});

/// 等级变化(ID 386)
pub static NODE_ON_LEVEL_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(386, vec![Entity, Guid, Int, Int])
});

/// 技能调用(ID 392)
pub static NODE_ON_SKILL_CALL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(392, vec![Entity, Guid, String, String, String])
});

/// 血量恢复(ID 584)
pub static NODE_ON_HP_RECOVER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(584, vec![Entity, Guid, Entity, Float, List(String.into())])
});

/// 血量恢复开始(ID 585)
pub static NODE_ON_HP_RECOVERY_START: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(585, vec![Entity, Guid, Entity, Float, List(String.into())])
});

/// 仇恨目标变化(ID 611)
pub static NODE_ON_AGGRO_TARGET_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(611, vec![Entity, Guid, Entity, Entity])
});

/// 仇恨进入战斗(ID 612)
pub static NODE_ON_AGGRO_ENTER_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(612, vec![Entity, Guid])
});

/// 仇恨离开战斗(ID 613)
pub static NODE_ON_AGGRO_LEAVE_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(613, vec![Entity, Guid])
});

/// 巡逻到达路径点(ID 620)
pub static NODE_ON_PATROL_REACH_WAYPOINT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(620, vec![Entity, Guid, Int, Int, Int, Int])
});

/// 卡组选择(ID 633)
pub static NODE_ON_DECK_SELECTED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(633, vec![Entity, List(Int.into()), Enum(todo!()), Int])
});

/// 元素反应(ID 642)
pub static NODE_ON_ELEMENT_REACTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(642, vec![Entity, Guid, Enum(todo!()), Entity, Guid])
});

/// 护盾命中(ID 643)
pub static NODE_ON_SHIELD_HIT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(643, vec![Entity, Guid, Entity, Guid, Config, Int, Int, Float])
});

/// 气泡完成(ID 679)
pub static NODE_ON_BUBBLE_COMPLETE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(679, vec![Entity, Entity, Config, Int])
});

/// 词缀变化(ID 680)
pub static NODE_ON_AFFIX_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(680, vec![Entity, Guid, Int, Int, Float, Float])
});

/// 物品添加(ID 681)
pub static NODE_ON_ITEM_ADD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(681, vec![Entity, Guid, Config, Int])
});

/// 物品失去(ID 682)
pub static NODE_ON_ITEM_LOSE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(682, vec![Entity, Guid, Config, Int])
});

/// 物品数量变化(ID 683)
pub static NODE_ON_ITEM_QUANTITY_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(683, vec![Entity, Guid, Config, Int, Int, Enum(todo!())])
});

/// 货币变化(ID 684)
pub static NODE_ON_CURRENCY_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(684, vec![Entity, Guid, Config, Int])
});

/// 装备初始化(ID 694)
pub static NODE_ON_EQUIPMENT_INIT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(694, vec![Entity, Guid, Int])
});

/// 装备(ID 695)
pub static NODE_ON_EQUIP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(695, vec![Entity, Guid, Int])
});

/// 卸下装备(ID 696)
pub static NODE_ON_UNEQUIP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(696, vec![Entity, Guid, Int])
});

/// 自定义商品售出(ID 700)
pub static NODE_ON_CUSTOM_ITEM_SOLD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(700, vec![Entity, Guid, Entity, Int, Int, Int])
});

/// 库存商品售出(ID 701)
pub static NODE_ON_INV_ITEM_SOLD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(701, vec![Entity, Guid, Entity, Int, Config, Int])
});

/// 出售物品(ID 705)
pub static NODE_ON_SELL_ITEM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(705, vec![Entity, Guid, Entity, Int, Dict { key: Config.into(), value: Int.into() }])
});

/// 物品使用(ID 733)
pub static NODE_ON_ITEM_USE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(733, vec![Entity, Guid, Config, Int])
});

/// 职业移除(ID 764)
pub static NODE_ON_CLASS_REMOVE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(764, vec![Entity, Guid, Config, Config])
});

/// 可打断(ID 765)
pub static NODE_ON_INTERRUPTIBLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(765, vec![Entity, Guid, Entity])
});

/// 速度条件(ID 946)
pub static NODE_ON_SPEED_CONDITION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(946, vec![Entity, Guid, Config, Enum(todo!()), Float, Float])
});

/// 信号(ID 300001)
pub static NODE_ON_SIGNAL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(300001, vec![Entity, Guid, Entity])
});
