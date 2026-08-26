//! 触发域节点(Server,Trigger)
//!
//! 事件触发器:无 flow 输入,1 个 flow 输出(事件发生时触发),值输出为事件参数。
//! 人工设计,`trigger_node!` 宏消除样板;execute/get_value 仅模拟(todo!())。

use crate::asset::node_graph::{ControlOut, INode, NodeRef, Simulation, ValueIn};
use crate::asset::raw_node_graph::NodeType;
use crate::asset::value::{
    AnyValue, ValueBool, ValueConfig, ValueDefault, ValueDict, ValueEntity, ValueEnum,
    ValueFaction, ValueFloat, ValueGuid, ValueInt, ValueIntList, ValueString, ValueStringList,
    ValueVector,
};
use anyhow::Result;

macro_rules! trigger_node {
    ($name:ident, $id:expr, $nm:literal, [$($vout:expr),*]) => {
        impl INode for $name {
            fn get_controls_in(&self) -> i32 { 0 }
            fn get_controls_out(&self) -> Vec<ControlOut> { vec![self.next.clone()] }
            fn get_values_in(&self) -> Vec<&ValueIn> { vec![] }
            fn get_values_out(&self) -> Vec<AnyValue> { vec![$($vout),*] }
            fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
                todo!(concat!("ID ", $nm, " execute"))
            }
            fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
                todo!(concat!("ID ", $nm, " get_value"))
            }
            fn get_type(&self) -> NodeType { NodeType::simple($id) }
        }
    };
}

/// 变量变化(ID 36)
pub struct NodeOnVariableChange {
    next: ControlOut,
}
impl Default for NodeOnVariableChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnVariableChange, 36, "36 On_Variable_Change", [ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueInt::def(), ValueInt::def()]);

/// 状态变化(ID 67)
pub struct NodeOnStatusChange {
    next: ControlOut,
}
impl Default for NodeOnStatusChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnStatusChange, 67, "67 On_Status_Change", [ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def(), ValueInt::def()]);

/// 实体创建(ID 71)
pub struct NodeOnCreated {
    next: ControlOut,
}
impl Default for NodeOnCreated {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnCreated, 71, "71 On_Created", [ValueEntity::def(), ValueGuid::def()]);

/// 实体移除(ID 72)
pub struct NodeOnRemoved {
    next: ControlOut,
}
impl Default for NodeOnRemoved {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnRemoved, 72, "72 On_Removed", [ValueGuid::def()]);

/// 计时器触发(ID 83)
pub struct NodeOnTimerTrigger {
    next: ControlOut,
}
impl Default for NodeOnTimerTrigger {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnTimerTrigger, 83, "83 On_Timer_Trigger", [ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueInt::def(), ValueInt::def()]);

/// 运动停止(ID 89)
pub struct NodeOnMotionStop {
    next: ControlOut,
}
impl Default for NodeOnMotionStop {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnMotionStop, 89, "89 On_Motion_Stop", [ValueEntity::def(), ValueGuid::def(), ValueString::def()]);

/// 离开碰撞触发(ID 91)
pub struct NodeOnTriggerExit {
    next: ControlOut,
}
impl Default for NodeOnTriggerExit {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnTriggerExit, 91, "91 On_Exit", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueGuid::def(), ValueInt::def()]);

/// 进入碰撞触发(ID 92)
pub struct NodeOnTriggerEnter {
    next: ControlOut,
}
impl Default for NodeOnTriggerEnter {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnTriggerEnter, 92, "92 On_Enter", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueGuid::def(), ValueInt::def()]);

/// 到达路径点(ID 177)
pub struct NodeOnReachWaypoint {
    next: ControlOut,
}
impl Default for NodeOnReachWaypoint {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnReachWaypoint, 177, "177 On_Reach_Waypoint", [ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueInt::def()]);

/// 阵营变化(ID 251)
pub struct NodeOnFactionChange {
    next: ControlOut,
}
impl Default for NodeOnFactionChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnFactionChange, 251, "251 On_Faction_Change", [ValueEntity::def(), ValueGuid::def(), ValueFaction::def(), ValueFaction::def()]);

/// 命中检测(ID 253)
pub struct NodeOnHitDetected {
    next: ControlOut,
}
impl Default for NodeOnHitDetected {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnHitDetected, 253, "253 On_Hit_Detected", [ValueEntity::def(), ValueGuid::def(), ValueBool::def(), ValueEntity::def(), ValueVector::def()]);

/// 角色倒地(ID 280)
pub struct NodeOnCharacterDown {
    next: ControlOut,
}
impl Default for NodeOnCharacterDown {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnCharacterDown, 280, "280 On_Character_Down", [ValueEntity::def(), ValueEnum::def(), ValueEntity::def()]);

/// 角色复活(ID 281)
pub struct NodeOnCharacterRevive {
    next: ControlOut,
}
impl Default for NodeOnCharacterRevive {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnCharacterRevive, 281, "281 On_Character_Revive", [ValueEntity::def()]);

/// 全员倒地(ID 284)
pub struct NodeOnAllDown {
    next: ControlOut,
}
impl Default for NodeOnAllDown {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnAllDown, 284, "284 On_All_Down", [ValueEntity::def(), ValueEnum::def()]);

/// 异常复活(ID 285)
pub struct NodeOnAbnormalRevive {
    next: ControlOut,
}
impl Default for NodeOnAbnormalRevive {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnAbnormalRevive, 285, "285 On_Abnormal_Revive", [ValueEntity::def()]);

/// 全员复活(ID 286)
pub struct NodeOnAllRevived {
    next: ControlOut,
}
impl Default for NodeOnAllRevived {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnAllRevived, 286, "286 On_All_Revived", [ValueEntity::def()]);

/// 传送完成(ID 289)
pub struct NodeOnTeleportComplete {
    next: ControlOut,
}
impl Default for NodeOnTeleportComplete {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnTeleportComplete, 289, "289 On_Teleport_Complete", [ValueEntity::def(), ValueGuid::def()]);

/// 状态结束(ID 299)
pub struct NodeOnStatusEnd {
    next: ControlOut,
}
impl Default for NodeOnStatusEnd {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnStatusEnd, 299, "299 On_Status_End", [ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueEntity::def(), ValueBool::def(), ValueFloat::def(), ValueInt::def(), ValueEntity::def()]);

/// 状态变化(ID 300)
pub struct NodeOnUnitStatusChange {
    next: ControlOut,
}
impl Default for NodeOnUnitStatusChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnUnitStatusChange, 300, "300 On_Status_Change", [ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueEntity::def(), ValueBool::def(), ValueFloat::def(), ValueInt::def(), ValueInt::def()]);

/// 受到攻击(ID 304)
pub struct NodeOnBeAttacked {
    next: ControlOut,
}
impl Default for NodeOnBeAttacked {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnBeAttacked, 304, "304 On_Be_Attacked", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueFloat::def(), ValueStringList::def(), ValueEnum::def(), ValueFloat::def()]);

/// 命中目标(ID 305)
pub struct NodeOnHitTarget {
    next: ControlOut,
}
impl Default for NodeOnHitTarget {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnHitTarget, 305, "305 On_Hit_Target", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueFloat::def(), ValueStringList::def(), ValueEnum::def(), ValueFloat::def()]);

/// 页签选择(ID 307)
pub struct NodeOnTabSelect {
    next: ControlOut,
}
impl Default for NodeOnTabSelect {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnTabSelect, 307, "307 On_Tab_Select", [ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueEntity::def()]);

/// 全局计时器触发(ID 315)
pub struct NodeOnGlobalTimerTrigger {
    next: ControlOut,
}
impl Default for NodeOnGlobalTimerTrigger {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnGlobalTimerTrigger, 315, "315 On_Timer_Trigger", [ValueEntity::def(), ValueGuid::def(), ValueString::def()]);

/// 控件组触发(ID 316)
pub struct NodeOnGroupTrigger {
    next: ControlOut,
}
impl Default for NodeOnGroupTrigger {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnGroupTrigger, 316, "316 On_Group_Trigger", [ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def()]);

/// 图变量变化(ID 351)
pub struct NodeOnGraphVariableChange {
    next: ControlOut,
}
impl Default for NodeOnGraphVariableChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnGraphVariableChange, 351, "351 On_Graph_Variable_Change", [ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueInt::def(), ValueInt::def()]);

/// 实体销毁(ID 373)
pub struct NodeOnDestroyed {
    next: ControlOut,
}
impl Default for NodeOnDestroyed {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnDestroyed, 373, "373 On_Destroyed", [ValueEntity::def(), ValueGuid::def(), ValueVector::def(), ValueVector::def(), ValueEnum::def(), ValueFaction::def(), ValueEntity::def()]);

/// 进入战斗(造物)(ID 374)
pub struct NodeOnCreationEnterCombat {
    next: ControlOut,
}
impl Default for NodeOnCreationEnterCombat {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnCreationEnterCombat, 374, "374 On_Enter_Combat", [ValueEntity::def(), ValueGuid::def()]);

/// 离开战斗(造物)(ID 375)
pub struct NodeOnCreationLeaveCombat {
    next: ControlOut,
}
impl Default for NodeOnCreationLeaveCombat {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnCreationLeaveCombat, 375, "375 On_Leave_Combat", [ValueEntity::def(), ValueGuid::def()]);

/// 职业变化(ID 385)
pub struct NodeOnClassChange {
    next: ControlOut,
}
impl Default for NodeOnClassChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnClassChange, 385, "385 On_Class_Change", [ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueConfig::def()]);

/// 等级变化(ID 386)
pub struct NodeOnLevelChange {
    next: ControlOut,
}
impl Default for NodeOnLevelChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnLevelChange, 386, "386 On_Level_Change", [ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def()]);

/// 技能调用(ID 392)
pub struct NodeOnSkillCall {
    next: ControlOut,
}
impl Default for NodeOnSkillCall {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnSkillCall, 392, "392 On_Skill_Call", [ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueString::def(), ValueString::def()]);

/// 生命恢复(ID 584)
pub struct NodeOnHpRecover {
    next: ControlOut,
}
impl Default for NodeOnHpRecover {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnHpRecover, 584, "584 On_HP_Recover", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueFloat::def(), ValueStringList::def()]);

/// 生命恢复开始(ID 585)
pub struct NodeOnHpRecoveryStart {
    next: ControlOut,
}
impl Default for NodeOnHpRecoveryStart {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnHpRecoveryStart, 585, "585 On_HP_Recovery_Start", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueFloat::def(), ValueStringList::def()]);

/// 仇恨目标变化(ID 611)
pub struct NodeOnAggroTargetChange {
    next: ControlOut,
}
impl Default for NodeOnAggroTargetChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnAggroTargetChange, 611, "611 On_Target_Change", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueEntity::def()]);

/// 进入战斗(仇恨)(ID 612)
pub struct NodeOnAggroEnterCombat {
    next: ControlOut,
}
impl Default for NodeOnAggroEnterCombat {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnAggroEnterCombat, 612, "612 On_Enter_Combat", [ValueEntity::def(), ValueGuid::def()]);

/// 离开战斗(仇恨)(ID 613)
pub struct NodeOnAggroLeaveCombat {
    next: ControlOut,
}
impl Default for NodeOnAggroLeaveCombat {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnAggroLeaveCombat, 613, "613 On_Leave_Combat", [ValueEntity::def(), ValueGuid::def()]);

/// 巡逻到达路径点(ID 620)
pub struct NodeOnPatrolReachWaypoint {
    next: ControlOut,
}
impl Default for NodeOnPatrolReachWaypoint {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnPatrolReachWaypoint, 620, "620 On_Reach_Waypoint", [ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def(), ValueInt::def(), ValueInt::def()]);

/// 卡组选定(ID 633)
pub struct NodeOnDeckSelected {
    next: ControlOut,
}
impl Default for NodeOnDeckSelected {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnDeckSelected, 633, "633 On_Deck_Selected", [ValueEntity::def(), ValueIntList::def(), ValueEnum::def(), ValueInt::def()]);

/// 元素反应(ID 642)
pub struct NodeOnElementReaction {
    next: ControlOut,
}
impl Default for NodeOnElementReaction {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnElementReaction, 642, "642 On_Element_Reaction", [ValueEntity::def(), ValueGuid::def(), ValueEnum::def(), ValueEntity::def(), ValueGuid::def()]);

/// 护盾受击(ID 643)
pub struct NodeOnShieldHit {
    next: ControlOut,
}
impl Default for NodeOnShieldHit {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnShieldHit, 643, "643 On_Shield_Hit", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def(), ValueInt::def(), ValueFloat::def()]);

/// 气泡完成(ID 679)
pub struct NodeOnBubbleComplete {
    next: ControlOut,
}
impl Default for NodeOnBubbleComplete {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnBubbleComplete, 679, "679 On_Bubble_Complete", [ValueEntity::def(), ValueEntity::def(), ValueConfig::def(), ValueInt::def()]);

/// 词缀变化(ID 680)
pub struct NodeOnAffixChange {
    next: ControlOut,
}
impl Default for NodeOnAffixChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnAffixChange, 680, "680 On_Affix_Change", [ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def(), ValueFloat::def(), ValueFloat::def()]);

/// 物品添加(ID 681)
pub struct NodeOnItemAdd {
    next: ControlOut,
}
impl Default for NodeOnItemAdd {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnItemAdd, 681, "681 On_Item_Add", [ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def()]);

/// 物品丢失(ID 682)
pub struct NodeOnItemLose {
    next: ControlOut,
}
impl Default for NodeOnItemLose {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnItemLose, 682, "682 On_Item_Lose", [ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def()]);

/// 物品数量变化(ID 683)
pub struct NodeOnItemQuantityChange {
    next: ControlOut,
}
impl Default for NodeOnItemQuantityChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnItemQuantityChange, 683, "683 On_Item_Quantity_Change", [ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def(), ValueInt::def(), ValueEnum::def()]);

/// 货币变化(ID 684)
pub struct NodeOnCurrencyChange {
    next: ControlOut,
}
impl Default for NodeOnCurrencyChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnCurrencyChange, 684, "684 On_Currency_Change", [ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def()]);

/// 装备初始化(ID 694)
pub struct NodeOnEquipmentInit {
    next: ControlOut,
}
impl Default for NodeOnEquipmentInit {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnEquipmentInit, 694, "694 On_Init", [ValueEntity::def(), ValueGuid::def(), ValueInt::def()]);

/// 装备佩戴(ID 695)
pub struct NodeOnEquip {
    next: ControlOut,
}
impl Default for NodeOnEquip {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnEquip, 695, "695 On_Equip", [ValueEntity::def(), ValueGuid::def(), ValueInt::def()]);

/// 装备卸下(ID 696)
pub struct NodeOnUnequip {
    next: ControlOut,
}
impl Default for NodeOnUnequip {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnUnequip, 696, "696 On_Unequip", [ValueEntity::def(), ValueGuid::def(), ValueInt::def()]);

/// 自定义商品售出(ID 700)
pub struct NodeOnCustomItemSold {
    next: ControlOut,
}
impl Default for NodeOnCustomItemSold {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnCustomItemSold, 700, "700 On_Custom_Item_Sold", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueInt::def(), ValueInt::def(), ValueInt::def()]);

/// 库存商品售出(ID 701)
pub struct NodeOnInvItemSold {
    next: ControlOut,
}
impl Default for NodeOnInvItemSold {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnInvItemSold, 701, "701 On_Inv_Item_Sold", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueInt::def(), ValueConfig::def(), ValueInt::def()]);

/// 卖出物品(ID 705)
pub struct NodeOnSellItem {
    next: ControlOut,
}
impl Default for NodeOnSellItem {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnSellItem, 705, "705 On_Sell_Item", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def(), ValueInt::def(), ValueDict::new(ValueConfig::default(), ValueInt::default()).into()]);

/// 物品使用(ID 733)
pub struct NodeOnItemUse {
    next: ControlOut,
}
impl Default for NodeOnItemUse {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnItemUse, 733, "733 On_Item_Use", [ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueInt::def()]);

/// 职业移除(ID 764)
pub struct NodeOnClassRemove {
    next: ControlOut,
}
impl Default for NodeOnClassRemove {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnClassRemove, 764, "764 On_Class_Remove", [ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueConfig::def()]);

/// 可打断(ID 765)
pub struct NodeOnInterruptible {
    next: ControlOut,
}
impl Default for NodeOnInterruptible {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnInterruptible, 765, "765 On_Interruptible", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def()]);

/// 速度条件(ID 946)
pub struct NodeOnSpeedCondition {
    next: ControlOut,
}
impl Default for NodeOnSpeedCondition {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnSpeedCondition, 946, "946 On_Speed_Condition", [ValueEntity::def(), ValueGuid::def(), ValueConfig::def(), ValueEnum::def(), ValueFloat::def(), ValueFloat::def()]);

/// 信号触发(ID 300001)
pub struct NodeOnSignal {
    next: ControlOut,
}
impl Default for NodeOnSignal {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnSignal, 300001, "300001 On_Signal", [ValueEntity::def(), ValueGuid::def(), ValueEntity::def()]);
