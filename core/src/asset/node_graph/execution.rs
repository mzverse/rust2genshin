//! 执行域节点(Server,Execution)
//!
//! 人工设计:每个节点结构逐个手写(字段语义命名、类型准确),
//! 用 `flow_node!` 宏消除重复的 INode 样板;execute/get_value 仅模拟(todo!())。
//!
//! `flow_node!` 模板:1 个 flow 输入 + 值输入 + 1 个 flow 输出(next)。
//! 用法:flow_node!(NodeXxx, ID, [值输入引用...], [值输出默认值...]);

use crate::asset::generated::ServerTypeId;
use crate::asset::node_graph::{ControlOut, Node, LogType, NodeRef, Simulation, ValueIn};
use crate::asset::raw_node_graph::NodeType;
use crate::asset::value::{
    AnyValue, ValueBool, ValueConfig, ValueConfigList, ValueDefault, ValueDict, ValueEntity, ValueEntityList,
    ValueEnum, ValueFaction, ValueFloat, ValueFloatList, ValueGuid, ValueInt, ValueIntList,
    ValueLocalVarRef, ValuePrefab, ValueString, ValueStringList, ValueVector,
};
use anyhow::{Result, anyhow, bail};
use crate::asset::node_graph::query::NodeLocal;

macro_rules! flow_node {
    ($name:ident, $id:expr, $nm:literal, [$($vin:ident),*], [$($vout:expr),*]) => {
        impl Node for $name {
            fn get_controls_in(&self) -> i32 { 1 }
            fn get_controls_out(&self) -> Vec<ControlOut> { vec![self.next.clone()] }
            fn get_values_in(&self) -> Vec<ValueIn> { vec![$( self.$vin.clone() ),*] }
            fn get_values_out(&self) -> Vec<AnyValue> { vec![$($vout),*] }
            fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
                todo!(concat!("ID ", $nm, " execute"))
            }
            fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
                todo!(concat!("ID ", $nm, " get_value"))
            }
            fn get_type(&self) -> NodeType { NodeType::simple($id) }
            fn get_controls_out_mut(&mut self) -> Vec<&mut ControlOut> {
                vec![&mut self.next]
            }
        }
    };
}

pub struct NodeLog {
    value: ValueIn,
    next: ControlOut,
}
impl Node for NodeLog {
    fn get_controls_in(&self) -> i32 {
        1
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.next.clone()]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![]
    }

    fn execute(&mut self, context: &mut Simulation) -> anyhow::Result<Vec<NodeRef>> {
        let Ok(value) = self.value.get(context)?.downcast::<ValueString>() else {
            return Err(anyhow!("Log must be String"));
        };
        context.logs.push((LogType::Info, value.0));
        Ok(self.next.clone())
    }

    fn get_value(&self, _index: i32, _context: &Simulation) -> anyhow::Result<AnyValue> {
        bail!("No value")
    }

    fn get_type(&self) -> NodeType {
        NodeType::simple(1)
    }
}

// ========================================================================
// 变量 / 状态写入
// ========================================================================

/// 写局部变量(ID 19,泛型 Variant):variable(Loc) + value(R<T>) 输入。
/// 变体顺序(TSI 与 kernel 均按参考 data.json,与 Get_Local 同序):
/// Bol/Int/Str/Ety/Gid/Flt/Vec/L<Int>/L<Str>/L<Ety>/L<Gid>/L<Flt>/L<Vec>/L<Bol>/
/// Cfg/Pfb/L<Cfg>/L<Pfb>/Fct/L<Fct>
pub struct NodeSetLocal {
    pub local: ValueIn,
    pub value: ValueIn,
    pub next: ControlOut,
}
impl Default for NodeSetLocal {
    fn default() -> Self {
        Self {
            local: ValueIn::new(ValueLocalVarRef::def()),
            value: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
impl NodeSetLocal {
    fn kernel(ty: &AnyValue) -> i64 {
        match ty.get_server_type() {
            ServerTypeId::SBoolean => 19,
            ServerTypeId::SInt => 21,
            ServerTypeId::SString => 2674,
            ServerTypeId::SEntity => 2675,
            ServerTypeId::SGuid => 2676,
            ServerTypeId::SFloat => 2677,
            ServerTypeId::SVector => 2678,
            ServerTypeId::SIntList => 2679,
            ServerTypeId::SStringList => 2680,
            ServerTypeId::SEntityList => 2681,
            ServerTypeId::SGuidList => 2682,
            ServerTypeId::SFloatList => 2683,
            ServerTypeId::SVectorList => 2684,
            ServerTypeId::SBooleanList => 2685,
            ServerTypeId::SConfig => 2686,
            ServerTypeId::SPrefab => 2687,
            ServerTypeId::SConfigList => 2688,
            ServerTypeId::SPrefabList => 2689,
            ServerTypeId::SFaction => 2690,
            ServerTypeId::SFactionList => 2691,
            other => panic!("Unsupported type: {other:?}"),
        }
    }
}
impl Node for NodeSetLocal {
    fn get_controls_in(&self) -> i32 { 1 }
    fn get_controls_out(&self) -> Vec<ControlOut> { vec![self.next.clone()] }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.local.clone(),
            self.value.clone().into_selected(NodeLocal::select).unwrap(),
        ]
    }
    fn get_values_out(&self) -> Vec<AnyValue> { vec![] }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!(concat!( "ID ", "19 Set_Local", " execute" ))
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!(concat!( "ID ", "19 Set_Local", " get_value" ))
    }
    fn get_type(&self) -> NodeType { NodeType::variant(19, Self::kernel(&self.value.value)) }
    fn get_controls_out_mut(&mut self) -> Vec<&mut ControlOut> {
        vec![&mut self.next]
    }
}

/// 写自定义变量(ID 22):entity + 变量名 + 值(+ 是否全局)
pub struct NodeSetVariable {
    entity: ValueIn,
    name: ValueIn,
    value: ValueIn,
    global: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetVariable {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            value: ValueIn::new(ValueInt::def()),
            global: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetVariable, 22, "22 Set_Variable", [entity, name, value, global], []);

/// 写状态值(ID 66)
pub struct NodeSetStatus {
    entity: ValueIn,
    status: ValueIn,
    value: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetStatus {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            status: ValueIn::new(ValueInt::def()),
            value: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetStatus, 66, "66 Set_Status", [entity, status, value], []);

/// 写图变量(ID 323)
pub struct NodeSetGraphVariable {
    name: ValueIn,
    value: ValueIn,
    global: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetGraphVariable {
    fn default() -> Self {
        Self {
            name: ValueIn::new(ValueString::def()),
            value: ValueIn::new(ValueInt::def()),
            global: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetGraphVariable, 323, "323 Set_Graph_Variable", [name, value, global], []);

// ========================================================================
// 实体创建 / 销毁
// ========================================================================

/// 销毁实体(ID 69)
pub struct NodeDestroyEntity {
    entity: ValueIn,
    next: ControlOut,
}
impl Default for NodeDestroyEntity {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeDestroyEntity, 69, "69 Destroy_Entity", [entity], []);

/// 创建实体(ID 70):按 GUID + 实例 ID 列表
pub struct NodeCreateEntity {
    guid: ValueIn,
    instance_ids: ValueIn,
    next: ControlOut,
}
impl Default for NodeCreateEntity {
    fn default() -> Self {
        Self {
            guid: ValueIn::new(ValueGuid::def()),
            instance_ids: ValueIn::new(ValueIntList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeCreateEntity, 70, "70 Create_Entity", [guid, instance_ids], []);

/// 创建预制体实体(ID 252):输出实体
pub struct NodeCreatePrefab {
    prefab: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    owner: ValueIn,
    auto_active: ValueIn,
    level: ValueIn,
    tags: ValueIn,
    next: ControlOut,
}
impl Default for NodeCreatePrefab {
    fn default() -> Self {
        Self {
            prefab: ValueIn::new(ValuePrefab::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            owner: ValueIn::new(ValueEntity::def()),
            auto_active: ValueIn::new(ValueBool::def()),
            level: ValueIn::new(ValueInt::def()),
            tags: ValueIn::new(ValueIntList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeCreatePrefab, 252, "252 Create_Prefab", [prefab, position, rotation, owner, auto_active, level, tags], [ValueEntity::def()]);

/// 创建投射物(ID 256):输出实体
pub struct NodeCreateProjectile {
    prefab: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    shooter: ValueIn,
    target: ValueIn,
    auto_active: ValueIn,
    level: ValueIn,
    tags: ValueIn,
    next: ControlOut,
}
impl Default for NodeCreateProjectile {
    fn default() -> Self {
        Self {
            prefab: ValueIn::new(ValuePrefab::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            shooter: ValueIn::new(ValueEntity::def()),
            target: ValueIn::new(ValueEntity::def()),
            auto_active: ValueIn::new(ValueBool::def()),
            level: ValueIn::new(ValueInt::def()),
            tags: ValueIn::new(ValueIntList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeCreateProjectile, 256, "256 Create", [prefab, position, rotation, shooter, target, auto_active, level, tags], [ValueEntity::def()]);

/// 结算关卡(ID 77)
pub struct NodeSettle {
    next: ControlOut,
}
impl Default for NodeSettle {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
flow_node!(NodeSettle, 77, "77 Settle", [], []);

/// 转发事件(ID 190)
pub struct NodeForwardEvent {
    entity: ValueIn,
    next: ControlOut,
}
impl Default for NodeForwardEvent {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeForwardEvent, 190, "190 Forward_Event", [entity], []);

/// 设置模型可见(ID 308)
pub struct NodeSetModelVisible {
    entity: ValueIn,
    visible: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetModelVisible {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            visible: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetModelVisible, 308, "308 Set_Model_Visible", [entity, visible], []);

/// 实体布设组状态(ID 178)
pub struct NodeSetGroupState {
    group: ValueIn,
    active: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetGroupState {
    fn default() -> Self {
        Self {
            group: ValueIn::new(ValueInt::def()),
            active: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetGroupState, 178, "178 Set_Group_State", [group, active], []);

// ========================================================================
// 计时器 / 全局计时器
// ========================================================================

/// 启动计时器(ID 79)
pub struct NodeStart {
    entity: ValueIn,
    name: ValueIn,
    loop_mode: ValueIn,
    intervals: ValueIn,
    next: ControlOut,
}
impl Default for NodeStart {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            loop_mode: ValueIn::new(ValueBool::def()),
            intervals: ValueIn::new(ValueFloatList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeStart, 79, "79 Start", [entity, name, loop_mode, intervals], []);

/// 暂停计时器(ID 80)
pub struct NodePause {
    entity: ValueIn,
    name: ValueIn,
    next: ControlOut,
}
impl Default for NodePause {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodePause, 80, "80 Pause", [entity, name], []);

/// 恢复计时器(ID 81)
pub struct NodeResume {
    entity: ValueIn,
    name: ValueIn,
    next: ControlOut,
}
impl Default for NodeResume {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeResume, 81, "81 Resume", [entity, name], []);

/// 停止计时器(ID 82)
pub struct NodeStop {
    entity: ValueIn,
    name: ValueIn,
    next: ControlOut,
}
impl Default for NodeStop {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeStop, 82, "82 Stop", [entity, name], []);

/// 启动全局计时器(ID 311)
pub struct NodeGlobalTimerStart {
    entity: ValueIn,
    name: ValueIn,
    next: ControlOut,
}
impl Default for NodeGlobalTimerStart {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeGlobalTimerStart, 311, "311 Start", [entity, name], []);

/// 暂停全局计时器(ID 309)
pub struct NodeGlobalTimerPause {
    entity: ValueIn,
    name: ValueIn,
    next: ControlOut,
}
impl Default for NodeGlobalTimerPause {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeGlobalTimerPause, 309, "309 Pause", [entity, name], []);

/// 恢复全局计时器(ID 312)
pub struct NodeGlobalTimerResume {
    entity: ValueIn,
    name: ValueIn,
    next: ControlOut,
}
impl Default for NodeGlobalTimerResume {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeGlobalTimerResume, 312, "312 Resume", [entity, name], []);

/// 停止全局计时器(ID 313)
pub struct NodeGlobalTimerStop {
    entity: ValueIn,
    name: ValueIn,
    next: ControlOut,
}
impl Default for NodeGlobalTimerStop {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeGlobalTimerStop, 313, "313 Stop", [entity, name], []);

/// 修改全局计时器(ID 314)
pub struct NodeGlobalTimerModify {
    entity: ValueIn,
    name: ValueIn,
    time: ValueIn,
    next: ControlOut,
}
impl Default for NodeGlobalTimerModify {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            time: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeGlobalTimerModify, 314, "314 Modify", [entity, name, time], []);

// ========================================================================
// 运动设备
// ========================================================================

/// 添加线性运动(ID 84)
pub struct NodeAddLinearMotion {
    entity: ValueIn,
    name: ValueIn,
    speed: ValueIn,
    direction: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddLinearMotion {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            speed: ValueIn::new(ValueFloat::def()),
            direction: ValueIn::new(ValueVector::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddLinearMotion, 84, "84 Add_Linear_Motion", [entity, name, speed, direction], []);

/// 添加旋转运动(ID 85)
pub struct NodeAddRotationMotion {
    entity: ValueIn,
    name: ValueIn,
    speed: ValueIn,
    angle: ValueIn,
    axis: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddRotationMotion {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            speed: ValueIn::new(ValueFloat::def()),
            angle: ValueIn::new(ValueFloat::def()),
            axis: ValueIn::new(ValueVector::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddRotationMotion, 85, "85 Add_Rotation_Motion", [entity, name, speed, angle, axis], []);

/// 停止并删除运动设备(ID 86)
pub struct NodeStopDelete {
    entity: ValueIn,
    name: ValueIn,
    delete: ValueIn,
    next: ControlOut,
}
impl Default for NodeStopDelete {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            delete: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeStopDelete, 86, "86 Stop_Delete", [entity, name, delete], []);

/// 暂停运动设备(ID 87)
pub struct NodeMotionPause {
    entity: ValueIn,
    name: ValueIn,
    next: ControlOut,
}
impl Default for NodeMotionPause {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeMotionPause, 87, "87 Pause", [entity, name], []);

/// 恢复运动设备(ID 88)
pub struct NodeMotionResume {
    entity: ValueIn,
    name: ValueIn,
    next: ControlOut,
}
impl Default for NodeMotionResume {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeMotionResume, 88, "88 Resume", [entity, name], []);

/// 激活运动设备(ID 267)
pub struct NodeActivate {
    entity: ValueIn,
    name: ValueIn,
    next: ControlOut,
}
impl Default for NodeActivate {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeActivate, 267, "267 Activate", [entity, name], []);

// ========================================================================
// 特效
// ========================================================================

/// 播放限时特效(ID 93)
pub struct NodePlayTimed {
    effect: ValueIn,
    target: ValueIn,
    name: ValueIn,
    loop_mode: ValueIn,
    destroy_on_end: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    scale: ValueIn,
    relative: ValueIn,
    next: ControlOut,
}
impl Default for NodePlayTimed {
    fn default() -> Self {
        Self {
            effect: ValueIn::new(ValueConfig::def()),
            target: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            loop_mode: ValueIn::new(ValueBool::def()),
            destroy_on_end: ValueIn::new(ValueBool::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            scale: ValueIn::new(ValueFloat::def()),
            relative: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodePlayTimed, 93, "93 Play_Timed", [effect, target, name, loop_mode, destroy_on_end, position, rotation, scale, relative], []);

/// 播放循环特效(ID 94):输出句柄
pub struct NodePlayLoop {
    effect: ValueIn,
    target: ValueIn,
    name: ValueIn,
    loop_mode: ValueIn,
    destroy_on_end: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    scale: ValueIn,
    relative: ValueIn,
    next: ControlOut,
}
impl Default for NodePlayLoop {
    fn default() -> Self {
        Self {
            effect: ValueIn::new(ValueConfig::def()),
            target: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            loop_mode: ValueIn::new(ValueBool::def()),
            destroy_on_end: ValueIn::new(ValueBool::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            scale: ValueIn::new(ValueFloat::def()),
            relative: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodePlayLoop, 94, "94 Play_Loop", [effect, target, name, loop_mode, destroy_on_end, position, rotation, scale, relative], [ValueInt::def()]);

/// 停止循环特效(ID 95)
pub struct NodeStopLoop {
    handle: ValueIn,
    target: ValueIn,
    next: ControlOut,
}
impl Default for NodeStopLoop {
    fn default() -> Self {
        Self {
            handle: ValueIn::new(ValueInt::def()),
            target: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeStopLoop, 95, "95 Stop_Loop", [handle, target], []);

// ========================================================================
// 碰撞
// ========================================================================

/// 设置原生碰撞(ID 240)
pub struct NodeSetNativeCollision {
    entity: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetNativeCollision {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetNativeCollision, 240, "240 Set_Native_Collision", [entity, enabled], []);

/// 设置原生攀爬(ID 241)
pub struct NodeSetNativeClimb {
    entity: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetNativeClimb {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetNativeClimb, 241, "241 Set_Native_Climb", [entity, enabled], []);

/// 设置附加碰撞(ID 242)
pub struct NodeSetExtraCollision {
    entity: ValueIn,
    index: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetExtraCollision {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetExtraCollision, 242, "242 Set_Extra_Collision", [entity, index, enabled], []);

/// 设置附加攀爬(ID 243)
pub struct NodeSetExtraClimb {
    entity: ValueIn,
    index: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetExtraClimb {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetExtraClimb, 243, "243 Set_Extra_Climb", [entity, index, enabled], []);

/// 设置碰撞触发器状态(ID 90)
pub struct NodeSetTriggerState {
    entity: ValueIn,
    index: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetTriggerState {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetTriggerState, 90, "90 Set_Trigger_State", [entity, index, enabled], []);

// ========================================================================
// 角色 / 复活 / 传送
// ========================================================================

/// 激活复活点(ID 272)
pub struct NodeActivateRevivePoint {
    entity: ValueIn,
    index: ValueIn,
    next: ControlOut,
}
impl Default for NodeActivateRevivePoint {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeActivateRevivePoint, 272, "272 Activate_Revive_Point", [entity, index], []);

/// 停用复活点(ID 273)
pub struct NodeDeactivateRevivePoint {
    entity: ValueIn,
    index: ValueIn,
    next: ControlOut,
}
impl Default for NodeDeactivateRevivePoint {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeDeactivateRevivePoint, 273, "273 Deactivate_Revive_Point", [entity, index], []);

/// 设置允许复活(ID 274)
pub struct NodeSetReviveAllowed {
    entity: ValueIn,
    allowed: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetReviveAllowed {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            allowed: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetReviveAllowed, 274, "274 Set_Revive_Allowed", [entity, allowed], []);

/// 设置复活次数(ID 276)
pub struct NodeSetReviveCount {
    entity: ValueIn,
    count: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetReviveCount {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            count: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetReviveCount, 276, "276 Set_Revive_Count", [entity, count], []);

/// 设置复活时间(ID 278)
pub struct NodeSetReviveTime {
    entity: ValueIn,
    seconds: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetReviveTime {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            seconds: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetReviveTime, 278, "278 Set_Revive_Time", [entity, seconds], []);

/// 复活单个角色(ID 279)
pub struct NodeReviveSingle {
    entity: ValueIn,
    next: ControlOut,
}
impl Default for NodeReviveSingle {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeReviveSingle, 279, "279 Revive_Single", [entity], []);

/// 击败全部角色(ID 282)
pub struct NodeDefeatAll {
    entity: ValueIn,
    next: ControlOut,
}
impl Default for NodeDefeatAll {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeDefeatAll, 282, "282 Defeat_All", [entity], []);

/// 复活全部角色(ID 283)
pub struct NodeReviveAll {
    entity: ValueIn,
    full_restore: ValueIn,
    next: ControlOut,
}
impl Default for NodeReviveAll {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            full_restore: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeReviveAll, 283, "283 Revive_All", [entity, full_restore], []);

/// 传送(ID 288)
pub struct NodeTeleport {
    entity: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    next: ControlOut,
}
impl Default for NodeTeleport {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeTeleport, 288, "288 Teleport", [entity, position, rotation], []);

/// 修改设备(ID 302)
pub struct NodeModifyDevice {
    entity: ValueIn,
    index: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyDevice {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyDevice, 302, "302 Modify_Device", [entity, index], []);

/// 设置页签状态(ID 306)
pub struct NodeSetTabState {
    entity: ValueIn,
    tab: ValueIn,
    active: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetTabState {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            tab: ValueIn::new(ValueInt::def()),
            active: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetTabState, 306, "306 Set_State", [entity, tab, active], []);

/// 切换相机模板(ID 261)
pub struct NodeSwitchTemplate {
    entities: ValueIn,
    template: ValueIn,
    next: ControlOut,
}
impl Default for NodeSwitchTemplate {
    fn default() -> Self {
        Self {
            entities: ValueIn::new(ValueEntityList::def()),
            template: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSwitchTemplate, 261, "261 Switch_Template", [entities, template], []);

/// 设置阵营(ID 250)
pub struct NodeSetFaction {
    entity: ValueIn,
    faction: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetFaction {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            faction: ValueIn::new(ValueFaction::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetFaction, 250, "250 Set_Faction", [entity, faction], []);

/// 跟随目标 GUID(ID 245)
pub struct NodeSetTargetGuid {
    entity: ValueIn,
    guid: ValueIn,
    name: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    system: ValueIn,
    mode: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetTargetGuid {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            guid: ValueIn::new(ValueGuid::def()),
            name: ValueIn::new(ValueString::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            system: ValueIn::new(ValueEnum::def()),
            mode: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetTargetGuid, 245, "245 Set_Target_GUID", [entity, guid, name, position, rotation, system, mode], []);

/// 设置跟随设备状态(ID 365)
pub struct NodeSetDeviceState {
    entity: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetDeviceState {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetDeviceState, 365, "365 Set_Device_State", [entity, enabled], []);

/// 设置碰撞触发源状态(ID 367)
pub struct NodeSetSourceState {
    entity: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetSourceState {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetSourceState, 367, "367 Set_Source_State", [entity, enabled], []);

// ========================================================================
// 列表操作
// ========================================================================

/// 拼接列表(ID 100)
pub struct NodeConcatenate {
    list_a: ValueIn,
    list_b: ValueIn,
    next: ControlOut,
}
impl Default for NodeConcatenate {
    fn default() -> Self {
        Self {
            list_a: ValueIn::new(ValueIntList::def()),
            list_b: ValueIn::new(ValueIntList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeConcatenate, 100, "100 Concatenate", [list_a, list_b], []);

/// 清空列表(ID 107)
pub struct NodeClearList {
    list: ValueIn,
    next: ControlOut,
}
impl Default for NodeClearList {
    fn default() -> Self {
        Self {
            list: ValueIn::new(ValueIntList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClearList, 107, "107 Clear", [list], []);

/// 插入元素(ID 135)
pub struct NodeInsert {
    list: ValueIn,
    index: ValueIn,
    item: ValueIn,
    next: ControlOut,
}
impl Default for NodeInsert {
    fn default() -> Self {
        Self {
            list: ValueIn::new(ValueIntList::def()),
            index: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeInsert, 135, "135 Insert", [list, index, item], []);

/// 移除元素(ID 153)
pub struct NodeRemove {
    list: ValueIn,
    index: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemove {
    fn default() -> Self {
        Self {
            list: ValueIn::new(ValueIntList::def()),
            index: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemove, 153, "153 Remove", [list, index], []);

/// 修改下标元素(ID 160)
pub struct NodeModifyIndex {
    list: ValueIn,
    index: ValueIn,
    item: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyIndex {
    fn default() -> Self {
        Self {
            list: ValueIn::new(ValueIntList::def()),
            index: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyIndex, 160, "160 Modify_Index", [list, index, item], []);

/// 排序列表(ID 167)
pub struct NodeSortList {
    list: ValueIn,
    mode: ValueIn,
    next: ControlOut,
}
impl Default for NodeSortList {
    fn default() -> Self {
        Self {
            list: ValueIn::new(ValueIntList::def()),
            mode: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSortList, 167, "167 Sort", [list, mode], []);

// ========================================================================
// 状态增删 / 战斗
// ========================================================================

/// 添加状态(ID 297):输出状态 ID + 层数
pub struct NodeAddStatus {
    entity: ValueIn,
    applier: ValueIn,
    status: ValueIn,
    stacks: ValueIn,
    params: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddStatus {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            applier: ValueIn::new(ValueEntity::def()),
            status: ValueIn::new(ValueConfig::def()),
            stacks: ValueIn::new(ValueInt::def()),
            params: ValueIn::new(ValueDict::new(ValueString::default(), ValueFloat::default()).into()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddStatus, 297, "297 Add_Status", [entity, applier, status, stacks, params], [ValueEnum::def(), ValueInt::def()]);

/// 移除状态(ID 301)
pub struct NodeRemoveStatus {
    entity: ValueIn,
    status: ValueIn,
    mode: ValueIn,
    applier: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveStatus {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            status: ValueIn::new(ValueConfig::def()),
            mode: ValueIn::new(ValueEnum::def()),
            applier: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveStatus, 301, "301 Remove_Status", [entity, status, mode, applier], []);

/// 攻击(ID 303)
pub struct NodeAttack {
    attacker: ValueIn,
    damage: ValueIn,
    knockback: ValueIn,
    position: ValueIn,
    direction: ValueIn,
    hit_effect: ValueIn,
    is_critical: ValueIn,
    target: ValueIn,
    next: ControlOut,
}
impl Default for NodeAttack {
    fn default() -> Self {
        Self {
            attacker: ValueIn::new(ValueEntity::def()),
            damage: ValueIn::new(ValueFloat::def()),
            knockback: ValueIn::new(ValueFloat::def()),
            position: ValueIn::new(ValueVector::def()),
            direction: ValueIn::new(ValueVector::def()),
            hit_effect: ValueIn::new(ValueString::def()),
            is_critical: ValueIn::new(ValueBool::def()),
            target: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAttack, 303, "303 Attack", [attacker, damage, knockback, position, direction, hit_effect, is_critical, target], []);

// ========================================================================
// 实体 / UI / 职业
// ========================================================================

/// 移除实体(ID 372)
pub struct NodeRemoveEntity {
    entity: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveEntity {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveEntity, 372, "372 Remove_Entity", [entity], []);

/// 切换界面布局(ID 382)
pub struct NodeSwitchLayout {
    entity: ValueIn,
    layout: ValueIn,
    next: ControlOut,
}
impl Default for NodeSwitchLayout {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            layout: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSwitchLayout, 382, "382 Switch_Layout", [entity, layout], []);

/// 激活界面控件组(ID 383)
pub struct NodeActivateGroup {
    entity: ValueIn,
    group: ValueIn,
    next: ControlOut,
}
impl Default for NodeActivateGroup {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            group: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeActivateGroup, 383, "383 Activate_Group", [entity, group], []);

/// 修改界面控件状态(ID 384)
pub struct NodeModifyGroupStatus {
    entity: ValueIn,
    group: ValueIn,
    status: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyGroupStatus {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            group: ValueIn::new(ValueInt::def()),
            status: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyGroupStatus, 384, "384 Modify_Status", [entity, group, status], []);

/// 移除界面控件组(ID 521)
pub struct NodeRemoveGroup {
    entity: ValueIn,
    group: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveGroup {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            group: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveGroup, 521, "521 Remove_Group", [entity, group], []);

/// 转职(ID 389)
pub struct NodeChangeClass {
    entity: ValueIn,
    class: ValueIn,
    next: ControlOut,
}
impl Default for NodeChangeClass {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            class: ValueIn::new(ValueConfig::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeChangeClass, 389, "389 Change_Class", [entity, class], []);

/// 加经验(ID 390)
pub struct NodeAddExp {
    entity: ValueIn,
    exp: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddExp {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            exp: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddExp, 390, "390 Add_Exp", [entity, exp], []);

/// 设置等级(ID 391)
pub struct NodeSetLevel {
    entity: ValueIn,
    level: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetLevel {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            level: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetLevel, 391, "391 Set_Level", [entity, level], []);

// ========================================================================
// 技能 / 资源
// ========================================================================

/// 修改技能资源(ID 393)
pub struct NodeModifySkillResource {
    entity: ValueIn,
    resource: ValueIn,
    amount: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifySkillResource {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            resource: ValueIn::new(ValueConfig::def()),
            amount: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifySkillResource, 393, "393 Modify_Resource", [entity, resource, amount], []);

/// 设置技能资源(ID 394)
pub struct NodeSetSkillResource {
    entity: ValueIn,
    resource: ValueIn,
    value: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetSkillResource {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            resource: ValueIn::new(ValueConfig::def()),
            value: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetSkillResource, 394, "394 Set_Resource", [entity, resource, value], []);

/// 添加技能(ID 395)
pub struct NodeAddSkill {
    entity: ValueIn,
    skill: ValueIn,
    slot: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddSkill {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            skill: ValueIn::new(ValueConfig::def()),
            slot: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddSkill, 395, "395 Add_Skill", [entity, skill, slot], []);

/// 按 ID 移除技能(ID 396)
pub struct NodeRemoveSkillById {
    entity: ValueIn,
    skill: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveSkillById {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            skill: ValueIn::new(ValueConfig::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveSkillById, 396, "396 Remove_By_ID", [entity, skill], []);

/// 初始化技能(ID 397)
pub struct NodeInitSkill {
    entity: ValueIn,
    slot: ValueIn,
    next: ControlOut,
}
impl Default for NodeInitSkill {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            slot: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeInitSkill, 397, "397 Init_Skill", [entity, slot], []);

/// 按槽位移除技能(ID 399)
pub struct NodeRemoveSkillBySlot {
    entity: ValueIn,
    slot: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveSkillBySlot {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            slot: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveSkillBySlot, 399, "399 Remove_By_Slot", [entity, slot], []);

/// 设置技能冷却(ID 739)
pub struct NodeSetSkillCd {
    entity: ValueIn,
    slot: ValueIn,
    cooldown: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetSkillCd {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            slot: ValueIn::new(ValueEnum::def()),
            cooldown: ValueIn::new(ValueFloat::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetSkillCd, 739, "739 Set_CD", [entity, slot, cooldown, enabled], []);

/// 修改技能冷却(ID 740)
pub struct NodeModifySkillCd {
    entity: ValueIn,
    slot: ValueIn,
    delta: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifySkillCd {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            slot: ValueIn::new(ValueEnum::def()),
            delta: ValueIn::new(ValueFloat::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifySkillCd, 740, "740 Modify_CD", [entity, slot, delta, enabled], []);

/// 修改技能冷却比例(ID 741)
pub struct NodeModifySkillCdRatio {
    entity: ValueIn,
    slot: ValueIn,
    ratio: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifySkillCdRatio {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            slot: ValueIn::new(ValueEnum::def()),
            ratio: ValueIn::new(ValueFloat::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifySkillCdRatio, 741, "741 Modify_CD_Ratio", [entity, slot, ratio, enabled], []);

// ========================================================================
// 特效 / 运动
// ========================================================================

/// 按资源停止特效(ID 473)
pub struct NodeStopEffectByAsset {
    entity: ValueIn,
    effect: ValueIn,
    next: ControlOut,
}
impl Default for NodeStopEffectByAsset {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            effect: ValueIn::new(ValueConfig::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeStopEffectByAsset, 473, "473 Stop_By_Asset", [entity, effect], []);

/// 添加目标旋转运动(ID 520)
pub struct NodeAddTargetRotation {
    entity: ValueIn,
    name: ValueIn,
    speed: ValueIn,
    axis: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddTargetRotation {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            speed: ValueIn::new(ValueFloat::def()),
            axis: ValueIn::new(ValueVector::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddTargetRotation, 520, "520 Add_Target_Rotation", [entity, name, speed, axis], []);

/// 激活固定点运动(ID 775)
pub struct NodeActivateFixedPoint {
    entity: ValueIn,
    name: ValueIn,
    move_type: ValueIn,
    speed: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    relative: ValueIn,
    mode: ValueIn,
    next: ControlOut,
}
impl Default for NodeActivateFixedPoint {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            move_type: ValueIn::new(ValueEnum::def()),
            speed: ValueIn::new(ValueFloat::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            relative: ValueIn::new(ValueBool::def()),
            mode: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeActivateFixedPoint, 775, "775 Activate_Fixed_Point", [entity, name, move_type, speed, position, rotation, relative, mode], []);

/// 设置跟随目标实体(ID 668)
pub struct NodeSetTargetEntity {
    entity: ValueIn,
    target: ValueIn,
    name: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    system: ValueIn,
    mode: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetTargetEntity {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            target: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            system: ValueIn::new(ValueEnum::def()),
            mode: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetTargetEntity, 668, "668 Set_Target_Entity", [entity, target, name, position, rotation, system, mode], []);

// ========================================================================
// 标签 / 仇恨
// ========================================================================

/// 添加标签(ID 586)
pub struct NodeAddTag {
    entity: ValueIn,
    tag: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddTag {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            tag: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddTag, 586, "586 Add_Tag", [entity, tag], []);

/// 移除标签(ID 587)
pub struct NodeRemoveTag {
    entity: ValueIn,
    tag: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveTag {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            tag: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveTag, 587, "587 Remove_Tag", [entity, tag], []);

/// 清空标签(ID 588)
pub struct NodeClearTags {
    entity: ValueIn,
    next: ControlOut,
}
impl Default for NodeClearTags {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClearTags, 588, "588 Clear_Tags", [entity], []);

/// 设置仇恨(ID 599)
pub struct NodeSetAggro {
    entity: ValueIn,
    target: ValueIn,
    value: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetAggro {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            target: ValueIn::new(ValueEntity::def()),
            value: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetAggro, 599, "599 Set_Aggro", [entity, target, value], []);

/// 移除仇恨(ID 600)
pub struct NodeRemoveAggro {
    entity: ValueIn,
    target: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveAggro {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            target: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveAggro, 600, "600 Remove_Aggro", [entity, target], []);

/// 清空仇恨(ID 601)
pub struct NodeClearAggro {
    entity: ValueIn,
    next: ControlOut,
}
impl Default for NodeClearAggro {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClearAggro, 601, "601 Clear_Aggro", [entity], []);

/// 嘲讽(ID 602)
pub struct NodeTaunt {
    entity: ValueIn,
    target: ValueIn,
    next: ControlOut,
}
impl Default for NodeTaunt {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            target: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeTaunt, 602, "602 Taunt", [entity, target], []);

// ========================================================================
// 战斗
// ========================================================================

/// 恢复生命(ID 583)
pub struct NodeRecoverHp {
    entity: ValueIn,
    amount: ValueIn,
    effect: ValueIn,
    is_critical: ValueIn,
    source: ValueIn,
    next: ControlOut,
}
impl Default for NodeRecoverHp {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            amount: ValueIn::new(ValueFloat::def()),
            effect: ValueIn::new(ValueString::def()),
            is_critical: ValueIn::new(ValueBool::def()),
            source: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRecoverHp, 583, "583 Recover_HP", [entity, amount, effect, is_critical, source], []);

/// 立即恢复生命(ID 698)
pub struct NodeRecoverHpInstant {
    entity: ValueIn,
    source: ValueIn,
    amount: ValueIn,
    is_critical: ValueIn,
    ratio: ValueIn,
    max_ratio: ValueIn,
    tags: ValueIn,
    next: ControlOut,
}
impl Default for NodeRecoverHpInstant {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            source: ValueIn::new(ValueEntity::def()),
            amount: ValueIn::new(ValueFloat::def()),
            is_critical: ValueIn::new(ValueBool::def()),
            ratio: ValueIn::new(ValueFloat::def()),
            max_ratio: ValueIn::new(ValueFloat::def()),
            tags: ValueIn::new(ValueStringList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRecoverHpInstant, 698, "698 Recover_HP_Instant", [entity, source, amount, is_critical, ratio, max_ratio, tags], []);

/// 损失生命(ID 697)
pub struct NodeLossHp {
    entity: ValueIn,
    amount: ValueIn,
    is_critical: ValueIn,
    ignore_defense: ValueIn,
    ignore_shield: ValueIn,
    damage_type: ValueIn,
    next: ControlOut,
}
impl Default for NodeLossHp {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            amount: ValueIn::new(ValueFloat::def()),
            is_critical: ValueIn::new(ValueBool::def()),
            ignore_defense: ValueIn::new(ValueBool::def()),
            ignore_shield: ValueIn::new(ValueBool::def()),
            damage_type: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeLossHp, 697, "697 Loss_HP", [entity, amount, is_critical, ignore_defense, ignore_shield, damage_type], []);

// ========================================================================
// 名称 / 气泡 / 探索
// ========================================================================

/// 设置名牌(ID 617)
pub struct NodeSetNameplate {
    entity: ValueIn,
    nameplates: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetNameplate {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            nameplates: ValueIn::new(ValueConfigList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetNameplate, 617, "617 Set_Nameplate", [entity, nameplates], []);

/// 切换巡逻模板(ID 618)
pub struct NodeSwitchPatrolTemplate {
    entity: ValueIn,
    template: ValueIn,
    next: ControlOut,
}
impl Default for NodeSwitchPatrolTemplate {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            template: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSwitchPatrolTemplate, 618, "618 Switch_Template", [entity, template], []);

/// 设置气泡文本(ID 631)
pub struct NodeSetBubble {
    entity: ValueIn,
    bubble: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetBubble {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            bubble: ValueIn::new(ValueConfig::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetBubble, 631, "631 Set_Bubble", [entity, bubble], []);

// ========================================================================
// 卡组 / 小地图
// ========================================================================

/// 打开卡组选择器(ID 632)
pub struct NodeDeckOpen {
    entity: ValueIn,
    deck: ValueIn,
    time_limit: ValueIn,
    allowed: ValueIn,
    disallowed: ValueIn,
    min_count: ValueIn,
    max_count: ValueIn,
    next: ControlOut,
}
impl Default for NodeDeckOpen {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            deck: ValueIn::new(ValueInt::def()),
            time_limit: ValueIn::new(ValueFloat::def()),
            allowed: ValueIn::new(ValueIntList::def()),
            disallowed: ValueIn::new(ValueIntList::def()),
            min_count: ValueIn::new(ValueInt::def()),
            max_count: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeDeckOpen, 632, "632 Open", [entity, deck, time_limit, allowed, disallowed, min_count, max_count], []);

/// 关闭卡组选择器(ID 641)
pub struct NodeDeckClose {
    entity: ValueIn,
    deck: ValueIn,
    next: ControlOut,
}
impl Default for NodeDeckClose {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            deck: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeDeckClose, 641, "641 Close", [entity, deck], []);

/// 随机卡组列表(ID 743)
pub struct NodeGetRandomList {
    list: ValueIn,
    next: ControlOut,
}
impl Default for NodeGetRandomList {
    fn default() -> Self {
        Self {
            list: ValueIn::new(ValueIntList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeGetRandomList, 743, "743 Get_Random_List", [list], [ValueIntList::def()]);

/// 设置小地图缩放(ID 634)
pub struct NodeSetMapZoom {
    entity: ValueIn,
    zoom: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetMapZoom {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            zoom: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetMapZoom, 634, "634 Set_Zoom", [entity, zoom], []);

/// 设置标记状态(ID 635)
pub struct NodeSetMarkerState {
    entity: ValueIn,
    markers: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetMarkerState {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            markers: ValueIn::new(ValueIntList::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetMarkerState, 635, "635 Set_Marker_State", [entity, markers, enabled], []);

/// 设置可见标记列表(ID 636)
pub struct NodeSetVisibleList {
    entity: ValueIn,
    index: ValueIn,
    entities: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetVisibleList {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
            entities: ValueIn::new(ValueEntityList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetVisibleList, 636, "636 Set_Visible_List", [entity, index, entities], []);

/// 设置追踪标记列表(ID 637)
pub struct NodeSetTrackList {
    entity: ValueIn,
    index: ValueIn,
    entities: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetTrackList {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
            entities: ValueIn::new(ValueEntityList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetTrackList, 637, "637 Set_Track_List", [entity, index, entities], []);

/// 更新标记(ID 640)
pub struct NodeUpdateMarkers {
    entity: ValueIn,
    index: ValueIn,
    target: ValueIn,
    next: ControlOut,
}
impl Default for NodeUpdateMarkers {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
            target: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeUpdateMarkers, 640, "640 Update_Markers", [entity, index, target], []);

// ========================================================================
// 成就 / 结算 / 排名
// ========================================================================

/// 设置成就进度(ID 645)
pub struct NodeSetAchievementProgress {
    entity: ValueIn,
    achievement: ValueIn,
    progress: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetAchievementProgress {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            achievement: ValueIn::new(ValueInt::def()),
            progress: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetAchievementProgress, 645, "645 Set_Progress", [entity, achievement, progress], []);

/// 增加成就进度(ID 646)
pub struct NodeAddAchievementProgress {
    entity: ValueIn,
    achievement: ValueIn,
    progress: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddAchievementProgress {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            achievement: ValueIn::new(ValueInt::def()),
            progress: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddAchievementProgress, 646, "646 Add_Progress", [entity, achievement, progress], []);

/// 设置记分板(ID 647)
pub struct NodeSetScoreboard {
    entity: ValueIn,
    index: ValueIn,
    name: ValueIn,
    value: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetScoreboard {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
            name: ValueIn::new(ValueString::def()),
            value: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetScoreboard, 647, "647 Set_Scoreboard", [entity, index, name, value], []);

/// 设置玩家排名(ID 650)
pub struct NodeSetPlayerRank {
    entity: ValueIn,
    rank: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetPlayerRank {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            rank: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetPlayerRank, 650, "650 Set_Player_Rank", [entity, rank], []);

/// 设置玩家结算结果(ID 652)
pub struct NodeSetPlayerResult {
    entity: ValueIn,
    result: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetPlayerResult {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            result: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetPlayerResult, 652, "652 Set_Player_Result", [entity, result], []);

/// 设置阵营排名(ID 654)
pub struct NodeSetFactionRank {
    faction: ValueIn,
    rank: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetFactionRank {
    fn default() -> Self {
        Self {
            faction: ValueIn::new(ValueFaction::def()),
            rank: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetFactionRank, 654, "654 Set_Faction_Rank", [faction, rank], []);

/// 设置阵营结算结果(ID 656)
pub struct NodeSetFactionResult {
    faction: ValueIn,
    result: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetFactionResult {
    fn default() -> Self {
        Self {
            faction: ValueIn::new(ValueFaction::def()),
            result: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetFactionResult, 656, "656 Set_Faction_Result", [faction, result], []);

/// 修改分数(ID 659)
pub struct NodeModifyScore {
    entity: ValueIn,
    result: ValueIn,
    score: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyScore {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            result: ValueIn::new(ValueEnum::def()),
            score: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyScore, 659, "659 Modify_Score", [entity, result, score], []);

/// 设置逃脱有效(ID 661)
pub struct NodeSetEscapeValid {
    entity: ValueIn,
    valid: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetEscapeValid {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            valid: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetEscapeValid, 661, "661 Set_Escape_Valid", [entity, valid], []);

/// 切换分数组(ID 663)
pub struct NodeSwitchScoreGroup {
    entity: ValueIn,
    group: ValueIn,
    next: ControlOut,
}
impl Default for NodeSwitchScoreGroup {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            group: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSwitchScoreGroup, 663, "663 Switch_Score_Group", [entity, group], []);

/// 设置关卡时间(ID 665)
pub struct NodeSetTime {
    time: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetTime {
    fn default() -> Self {
        Self {
            time: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetTime, 665, "665 Set_Time", [time], []);

/// 设置时间流速(ID 666)
pub struct NodeSetTimeSpeed {
    speed: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetTimeSpeed {
    fn default() -> Self {
        Self {
            speed: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetTimeSpeed, 666, "666 Set_Time_Speed", [speed], []);

/// 开关灯光(ID 667)
pub struct NodeToggleLight {
    entity: ValueIn,
    index: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeToggleLight {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeToggleLight, 667, "667 Toggle_Light", [entity, index, enabled], []);

// ========================================================================
// 音效
// ========================================================================

/// 关闭音效播放器(ID 591)
pub struct NodeCloseSoundPlayer {
    entity: ValueIn,
    channel: ValueIn,
    next: ControlOut,
}
impl Default for NodeCloseSoundPlayer {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            channel: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeCloseSoundPlayer, 591, "591 Close_Player", [entity, channel], []);

/// 切换音效播放器(ID 592)
pub struct NodeToggleSoundPlayer {
    entity: ValueIn,
    channel: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeToggleSoundPlayer {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            channel: ValueIn::new(ValueInt::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeToggleSoundPlayer, 592, "592 Toggle_Player", [entity, channel, enabled], []);

/// 调整音效播放器(ID 593)
pub struct NodeAdjustSoundPlayer {
    entity: ValueIn,
    channel: ValueIn,
    index: ValueIn,
    volume: ValueIn,
    next: ControlOut,
}
impl Default for NodeAdjustSoundPlayer {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            channel: ValueIn::new(ValueInt::def()),
            index: ValueIn::new(ValueInt::def()),
            volume: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAdjustSoundPlayer, 593, "593 Adjust_Player", [entity, channel, index, volume], []);

/// 添加音效播放器(ID 594):输出句柄
pub struct NodeAddSoundPlayer {
    entity: ValueIn,
    channel: ValueIn,
    index: ValueIn,
    volume: ValueIn,
    loop_mode: ValueIn,
    fade_in: ValueIn,
    fade_out: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddSoundPlayer {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            channel: ValueIn::new(ValueInt::def()),
            index: ValueIn::new(ValueInt::def()),
            volume: ValueIn::new(ValueFloat::def()),
            loop_mode: ValueIn::new(ValueBool::def()),
            fade_in: ValueIn::new(ValueFloat::def()),
            fade_out: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddSoundPlayer, 594, "594 Add_Player", [entity, channel, index, volume, loop_mode, fade_in, fade_out], [ValueInt::def()]);

/// 切换 BGM(ID 595)
pub struct NodeToggleBgm {
    entity: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeToggleBgm {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeToggleBgm, 595, "595 Toggle_BGM", [entity, enabled], []);

/// 设置 BGM 音量(ID 596)
pub struct NodeSetBgmVolume {
    entity: ValueIn,
    volume: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetBgmVolume {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            volume: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetBgmVolume, 596, "596 Set_BGM_Volume", [entity, volume], []);

/// 设置 BGM(ID 597)
pub struct NodeSetBgm {
    entity: ValueIn,
    sound: ValueIn,
    volume: ValueIn,
    fade_in: ValueIn,
    loop_count: ValueIn,
    loop_mode: ValueIn,
    fade_out: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetBgm {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            sound: ValueIn::new(ValueInt::def()),
            volume: ValueIn::new(ValueFloat::def()),
            fade_in: ValueIn::new(ValueFloat::def()),
            loop_count: ValueIn::new(ValueInt::def()),
            loop_mode: ValueIn::new(ValueBool::def()),
            fade_out: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetBgm, 597, "597 Set_BGM", [entity, sound, volume, fade_in, loop_count, loop_mode, fade_out], []);

/// 播放 2D 单次音效(ID 598)
pub struct NodePlay2dOneShot {
    entity: ValueIn,
    channel: ValueIn,
    sound: ValueIn,
    volume: ValueIn,
    next: ControlOut,
}
impl Default for NodePlay2dOneShot {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            channel: ValueIn::new(ValueInt::def()),
            sound: ValueIn::new(ValueInt::def()),
            volume: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodePlay2dOneShot, 598, "598 Play_2D_One_Shot", [entity, channel, sound, volume], []);

// ========================================================================
// 装备 / 词缀
// ========================================================================

/// 添加词缀(ID 672)
pub struct NodeAddAffix {
    equipment: ValueIn,
    affix: ValueIn,
    enabled: ValueIn,
    value: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddAffix {
    fn default() -> Self {
        Self {
            equipment: ValueIn::new(ValueInt::def()),
            affix: ValueIn::new(ValueConfig::def()),
            enabled: ValueIn::new(ValueBool::def()),
            value: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddAffix, 672, "672 Add_Affix", [equipment, affix, enabled, value], []);

/// 移除词缀(ID 673)
pub struct NodeRemoveAffix {
    equipment: ValueIn,
    affix: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveAffix {
    fn default() -> Self {
        Self {
            equipment: ValueIn::new(ValueInt::def()),
            affix: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveAffix, 673, "673 Remove_Affix", [equipment, affix], []);

/// 修改词缀(ID 674)
pub struct NodeModifyAffix {
    equipment: ValueIn,
    affix: ValueIn,
    value: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyAffix {
    fn default() -> Self {
        Self {
            equipment: ValueIn::new(ValueInt::def()),
            affix: ValueIn::new(ValueInt::def()),
            value: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyAffix, 674, "674 Modify_Affix", [equipment, affix, value], []);

/// 按 ID 添加词缀(ID 742)
pub struct NodeAddAffixById {
    equipment: ValueIn,
    affix: ValueIn,
    index: ValueIn,
    enabled: ValueIn,
    value: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddAffixById {
    fn default() -> Self {
        Self {
            equipment: ValueIn::new(ValueInt::def()),
            affix: ValueIn::new(ValueConfig::def()),
            index: ValueIn::new(ValueInt::def()),
            enabled: ValueIn::new(ValueBool::def()),
            value: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddAffixById, 742, "742 Add_Affix_By_ID", [equipment, affix, index, enabled, value], []);

// ========================================================================
// 背包 / 物品
// ========================================================================

/// 扩容背包(ID 685)
pub struct NodeExpandCapacity {
    entity: ValueIn,
    amount: ValueIn,
    next: ControlOut,
}
impl Default for NodeExpandCapacity {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            amount: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeExpandCapacity, 685, "685 Expand_Capacity", [entity, amount], []);

/// 修改物品数量(ID 686)
pub struct NodeModifyItem {
    entity: ValueIn,
    item: ValueIn,
    amount: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyItem {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            item: ValueIn::new(ValueConfig::def()),
            amount: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyItem, 686, "686 Modify_Item", [entity, item, amount], []);

/// 设置掉落数量(ID 687)
pub struct NodeSetDropAmount {
    entity: ValueIn,
    item: ValueIn,
    amount: ValueIn,
    mode: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetDropAmount {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            item: ValueIn::new(ValueConfig::def()),
            amount: ValueIn::new(ValueInt::def()),
            mode: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetDropAmount, 687, "687 Set_Drop_Amount", [entity, item, amount, mode], []);

/// 修改货币数量(ID 688)
pub struct NodeModifyCurrency {
    entity: ValueIn,
    currency: ValueIn,
    amount: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyCurrency {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            currency: ValueIn::new(ValueConfig::def()),
            amount: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyCurrency, 688, "688 Modify_Currency", [entity, currency, amount], []);

/// 设置掉落物品列表(ID 720)
pub struct NodeSetDropItems {
    entity: ValueIn,
    items: ValueIn,
    mode: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetDropItems {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            items: ValueIn::new(ValueDict::new(ValueConfig::default(), ValueInt::default()).into()),
            mode: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetDropItems, 720, "720 Set_Drop_Items", [entity, items, mode], []);

/// 触发掉落(ID 724)
pub struct NodeTriggerDrop {
    entity: ValueIn,
    mode: ValueIn,
    next: ControlOut,
}
impl Default for NodeTriggerDrop {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            mode: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeTriggerDrop, 724, "724 Trigger_Drop", [entity, mode], []);

/// 设置战利品内容(ID 725)
pub struct NodeSetLootContent {
    entity: ValueIn,
    items: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetLootContent {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            items: ValueIn::new(ValueDict::new(ValueConfig::default(), ValueInt::default()).into()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetLootContent, 725, "725 Set_Loot_Content", [entity, items], []);

/// 修改战利品物品(ID 726)
pub struct NodeModifyLootItem {
    entity: ValueIn,
    item: ValueIn,
    amount: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyLootItem {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            item: ValueIn::new(ValueConfig::def()),
            amount: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyLootItem, 726, "726 Modify_Loot_Item", [entity, item, amount], []);

/// 修改战利品货币(ID 727)
pub struct NodeModifyLootCurrency {
    entity: ValueIn,
    currency: ValueIn,
    amount: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyLootCurrency {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            currency: ValueIn::new(ValueConfig::def()),
            amount: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyLootCurrency, 727, "727 Modify_Loot_Currency", [entity, currency, amount], []);

// ========================================================================
// 商店
// ========================================================================

/// 打开商店(ID 702)
pub struct NodeShopOpen {
    entity: ValueIn,
    shop: ValueIn,
    mode: ValueIn,
    next: ControlOut,
}
impl Default for NodeShopOpen {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueEntity::def()),
            mode: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeShopOpen, 702, "702 Open", [entity, shop, mode], []);

/// 关闭商店(ID 703)
pub struct NodeShopClose {
    entity: ValueIn,
    next: ControlOut,
}
impl Default for NodeShopClose {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeShopClose, 703, "703 Close", [entity], []);

/// 修改自定义商品(ID 704)
pub struct NodeModifyCustomSale {
    entity: ValueIn,
    shop: ValueIn,
    index: ValueIn,
    item: ValueIn,
    price: ValueIn,
    amount: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyCustomSale {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            index: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueConfig::def()),
            price: ValueIn::new(ValueDict::new(ValueConfig::default(), ValueInt::default()).into()),
            amount: ValueIn::new(ValueInt::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyCustomSale, 704, "704 Modify_Custom_Sale", [entity, shop, index, item, price, amount, enabled], []);

/// 修改库存商品(ID 706)
pub struct NodeModifyInventorySale {
    entity: ValueIn,
    shop: ValueIn,
    item: ValueIn,
    price: ValueIn,
    amount: ValueIn,
    index: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyInventorySale {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueConfig::def()),
            price: ValueIn::new(ValueDict::new(ValueConfig::default(), ValueInt::default()).into()),
            amount: ValueIn::new(ValueInt::def()),
            index: ValueIn::new(ValueInt::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyInventorySale, 706, "706 Modify_Inventory_Sale", [entity, shop, item, price, amount, index, enabled], []);

/// 修改购物车物品(ID 707)
pub struct NodeModifyCartItem {
    entity: ValueIn,
    shop: ValueIn,
    item: ValueIn,
    price: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyCartItem {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueConfig::def()),
            price: ValueIn::new(ValueDict::new(ValueConfig::default(), ValueInt::default()).into()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyCartItem, 707, "707 Modify_Cart_Item", [entity, shop, item, price, enabled], []);

/// 添加自定义商品(ID 708):输出商品索引
pub struct NodeAddCustomSale {
    entity: ValueIn,
    shop: ValueIn,
    item: ValueIn,
    price: ValueIn,
    amount: ValueIn,
    enabled: ValueIn,
    index: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddCustomSale {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueConfig::def()),
            price: ValueIn::new(ValueDict::new(ValueConfig::default(), ValueInt::default()).into()),
            amount: ValueIn::new(ValueInt::def()),
            enabled: ValueIn::new(ValueBool::def()),
            index: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddCustomSale, 708, "708 Add_Custom_Sale", [entity, shop, item, price, amount, enabled, index], [ValueInt::def()]);

/// 添加库存商品(ID 709)
pub struct NodeAddInventorySale {
    entity: ValueIn,
    shop: ValueIn,
    item: ValueIn,
    price: ValueIn,
    amount: ValueIn,
    index: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddInventorySale {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueConfig::def()),
            price: ValueIn::new(ValueDict::new(ValueConfig::default(), ValueInt::default()).into()),
            amount: ValueIn::new(ValueInt::def()),
            index: ValueIn::new(ValueInt::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddInventorySale, 709, "709 Add_Inventory_Sale", [entity, shop, item, price, amount, index, enabled], []);

/// 加入购物车(ID 710)
pub struct NodeAddToCart {
    entity: ValueIn,
    shop: ValueIn,
    item: ValueIn,
    price: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddToCart {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueConfig::def()),
            price: ValueIn::new(ValueDict::new(ValueConfig::default(), ValueInt::default()).into()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddToCart, 710, "710 Add_To_Cart", [entity, shop, item, price, enabled], []);

/// 移除自定义商品(ID 711)
pub struct NodeRemoveCustomSale {
    entity: ValueIn,
    shop: ValueIn,
    index: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveCustomSale {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            index: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveCustomSale, 711, "711 Remove_Custom_Sale", [entity, shop, index], []);

/// 移除库存商品(ID 712)
pub struct NodeRemoveInventorySale {
    entity: ValueIn,
    shop: ValueIn,
    item: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveInventorySale {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueConfig::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveInventorySale, 712, "712 Remove_Inventory_Sale", [entity, shop, item], []);

/// 移出购物车(ID 713)
pub struct NodeRemoveFromCart {
    entity: ValueIn,
    shop: ValueIn,
    item: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveFromCart {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueConfig::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveFromCart, 713, "713 Remove_From_Cart", [entity, shop, item], []);

// ========================================================================
// 扫描标签 / 排行榜 / 聊天 / 其他
// ========================================================================

/// 设置扫描规则(ID 735)
pub struct NodeSetScanRules {
    entity: ValueIn,
    rules: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetScanRules {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            rules: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetScanRules, 735, "735 Set_Rules", [entity, rules], []);

/// 设置激活扫描标签(ID 736)
pub struct NodeSetActiveScanTag {
    entity: ValueIn,
    tag: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetActiveScanTag {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            tag: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetActiveScanTag, 736, "736 Set_Active_Tag", [entity, tag], []);

/// 创建预制体组(ID 757):输出实体列表
pub struct NodeCreatePrefabGroup {
    group: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    owner: ValueIn,
    count: ValueIn,
    tags: ValueIn,
    active: ValueIn,
    next: ControlOut,
}
impl Default for NodeCreatePrefabGroup {
    fn default() -> Self {
        Self {
            group: ValueIn::new(ValueInt::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            owner: ValueIn::new(ValueEntity::def()),
            count: ValueIn::new(ValueInt::def()),
            tags: ValueIn::new(ValueIntList::def()),
            active: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeCreatePrefabGroup, 757, "757 Create_Prefab_Group", [group, position, rotation, owner, count, tags, active], [ValueEntityList::def()]);

/// 设置整数分数(ID 761)
pub struct NodeSetScoreInt {
    leaderboard: ValueIn,
    score: ValueIn,
    index: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetScoreInt {
    fn default() -> Self {
        Self {
            leaderboard: ValueIn::new(ValueIntList::def()),
            score: ValueIn::new(ValueInt::def()),
            index: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetScoreInt, 761, "761 Set_Score_Int", [leaderboard, score, index], []);

/// 设置浮点分数(ID 762)
pub struct NodeSetScoreFloat {
    leaderboard: ValueIn,
    score: ValueIn,
    index: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetScoreFloat {
    fn default() -> Self {
        Self {
            leaderboard: ValueIn::new(ValueIntList::def()),
            score: ValueIn::new(ValueFloat::def()),
            index: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetScoreFloat, 762, "762 Set_Score_Float", [leaderboard, score, index], []);

/// 修改环境(ID 763)
pub struct NodeModifyEnvironment {
    index: ValueIn,
    entities: ValueIn,
    enabled: ValueIn,
    mode: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyEnvironment {
    fn default() -> Self {
        Self {
            index: ValueIn::new(ValueInt::def()),
            entities: ValueIn::new(ValueEntityList::def()),
            enabled: ValueIn::new(ValueBool::def()),
            mode: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyEnvironment, 763, "763 Modify_Environment", [index, entities, enabled, mode], []);

/// 设置聊天频道开关(ID 769)
pub struct NodeSetChatSwitch {
    channel: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetChatSwitch {
    fn default() -> Self {
        Self {
            channel: ValueIn::new(ValueInt::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetChatSwitch, 769, "769 Set_Switch", [channel, enabled], []);

/// 修改聊天权限(ID 770)
pub struct NodeModifyChatPermission {
    guid: ValueIn,
    channel: ValueIn,
    allowed: ValueIn,
    next: ControlOut,
}
impl Default for NodeModifyChatPermission {
    fn default() -> Self {
        Self {
            guid: ValueIn::new(ValueGuid::def()),
            channel: ValueIn::new(ValueInt::def()),
            allowed: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeModifyChatPermission, 770, "770 Modify_Permission", [guid, channel, allowed], []);

/// 设置当前聊天频道(ID 771)
pub struct NodeSetCurrentChannel {
    guid: ValueIn,
    channels: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetCurrentChannel {
    fn default() -> Self {
        Self {
            guid: ValueIn::new(ValueGuid::def()),
            channels: ValueIn::new(ValueIntList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetCurrentChannel, 771, "771 Set_Current_Channel", [guid, channels], []);

/// 消耗奇趣盒(ID 772)
pub struct NodeConsumeBox {
    entity: ValueIn,
    box_id: ValueIn,
    count: ValueIn,
    next: ControlOut,
}
impl Default for NodeConsumeBox {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            box_id: ValueIn::new(ValueInt::def()),
            count: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeConsumeBox, 772, "772 Consume_Box", [entity, box_id, count], []);

// ========================================================================
// 字典操作
// ========================================================================

/// 字典写入(ID 948)
pub struct NodeDictSetValue {
    dict: ValueIn,
    key: ValueIn,
    value: ValueIn,
    next: ControlOut,
}
impl Default for NodeDictSetValue {
    fn default() -> Self {
        Self {
            dict: ValueIn::new(ValueDict::new(ValueInt::default(), ValueInt::default()).into()),
            key: ValueIn::new(ValueInt::def()),
            value: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeDictSetValue, 948, "948 Set_Value", [dict, key, value], []);

/// 按键移除字典项(ID 1298)
pub struct NodeDictRemoveByKey {
    dict: ValueIn,
    key: ValueIn,
    next: ControlOut,
}
impl Default for NodeDictRemoveByKey {
    fn default() -> Self {
        Self {
            dict: ValueIn::new(ValueDict::new(ValueInt::default(), ValueInt::default()).into()),
            key: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeDictRemoveByKey, 1298, "1298 Remove_By_Key", [dict, key], []);

/// 清空字典(ID 1718)
pub struct NodeDictClear {
    dict: ValueIn,
    next: ControlOut,
}
impl Default for NodeDictClear {
    fn default() -> Self {
        Self {
            dict: ValueIn::new(ValueDict::new(ValueInt::default(), ValueInt::default()).into()),
            next: vec![],
        }
    }
}
flow_node!(NodeDictClear, 1718, "1718 Clear", [dict], []);

/// 按键排序字典(ID 1928):输出键列表 + 值列表
pub struct NodeDictSortByKey {
    dict: ValueIn,
    mode: ValueIn,
    next: ControlOut,
}
impl Default for NodeDictSortByKey {
    fn default() -> Self {
        Self {
            dict: ValueIn::new(ValueDict::new(ValueInt::default(), ValueInt::default()).into()),
            mode: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeDictSortByKey, 1928, "1928 Sort_By_Key", [dict, mode], [ValueIntList::def(), ValueIntList::def()]);

/// 按值排序字典(ID 1938):输出键列表 + 值列表
pub struct NodeDictSortByValue {
    dict: ValueIn,
    mode: ValueIn,
    next: ControlOut,
}
impl Default for NodeDictSortByValue {
    fn default() -> Self {
        Self {
            dict: ValueIn::new(ValueDict::new(ValueInt::default(), ValueInt::default()).into()),
            mode: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeDictSortByValue, 1938, "1938 Sort_By_Value", [dict, mode], [ValueIntList::def(), ValueIntList::def()]);

// ========================================================================
// 特殊(多 flow 输出)
// ========================================================================

/// 遍历列表(ID 509):2 flow 入(进入/继续),2 flow 出(元素/完成),输出当前元素
pub struct NodeForEach {
    list: ValueIn,
    body: ControlOut,
    done: ControlOut,
}
impl Default for NodeForEach {
    fn default() -> Self {
        Self {
            list: ValueIn::new(ValueIntList::def()),
            body: vec![],
            done: vec![],
        }
    }
}
impl Node for NodeForEach {
    fn get_controls_in(&self) -> i32 {
        2
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.body.clone(), self.done.clone()]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.list.clone()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 509 For_Each execute")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 509 For_Each get_value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(509)
    }
}

/// 发送信号(ID 300000)
pub struct NodeSendSignal {
    next: ControlOut,
}
impl Default for NodeSendSignal {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
flow_node!(NodeSendSignal, 300000, "300000 Send", [], []);

/// 修改结构体(ID 300004)
pub struct NodeStructureModify {
    next: ControlOut,
}
impl Default for NodeStructureModify {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
flow_node!(NodeStructureModify, 300004, "300004 Modify", [], []);
