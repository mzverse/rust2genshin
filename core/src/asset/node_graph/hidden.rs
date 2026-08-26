//! 隐藏域节点(Server,Hidden)
//!
//! 人工设计:相机/震屏/名牌/GM 等隐藏功能节点。execute/get_value 仅模拟(todo!())。

use crate::asset::node_graph::{ControlOut, INode, NodeRef, Simulation, ValueIn};
use crate::asset::raw_node_graph::NodeType;
use crate::asset::value::{
    AnyValue, ValueBool, ValueConfig, ValueDefault, ValueEntity, ValueEntityList, ValueFloat,
    ValueGuid, ValueInt, ValueIntList, ValueString,
};
use anyhow::Result;

macro_rules! flow_node {
    ($name:ident, $id:expr, $nm:literal, [$($vin:ident),*], [$($vout:expr),*]) => {
        impl INode for $name {
            fn get_controls_in(&self) -> i32 { 1 }
            fn get_controls_out(&self) -> Vec<ControlOut> { vec![self.next.clone()] }
            fn get_values_in(&self) -> Vec<&ValueIn> { vec![$( &self.$vin ),*] }
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

/// 激活实体相机(ID 262)
pub struct NodeActivateEntityCamera {
    entities: ValueIn,
    target: ValueIn,
    next: ControlOut,
}
impl Default for NodeActivateEntityCamera {
    fn default() -> Self {
        Self {
            entities: ValueIn::new(ValueEntityList::def()),
            target: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeActivateEntityCamera, 262, "262 Activate_Entity_Camera", [entities, target], []);

/// 关闭实体相机(ID 263)
pub struct NodeDisableEntityCamera {
    entities: ValueIn,
    next: ControlOut,
}
impl Default for NodeDisableEntityCamera {
    fn default() -> Self {
        Self {
            entities: ValueIn::new(ValueEntityList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeDisableEntityCamera, 263, "263 Disable_Entity_Camera", [entities], []);

/// 激活聚焦相机(ID 264)
pub struct NodeActivateFocusCamera {
    entities: ValueIn,
    target: ValueIn,
    next: ControlOut,
}
impl Default for NodeActivateFocusCamera {
    fn default() -> Self {
        Self {
            entities: ValueIn::new(ValueEntityList::def()),
            target: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeActivateFocusCamera, 264, "264 Activate_Focus_Camera", [entities, target], []);

/// 关闭聚焦相机(ID 265)
pub struct NodeDisableFocusCamera {
    entities: ValueIn,
    next: ControlOut,
}
impl Default for NodeDisableFocusCamera {
    fn default() -> Self {
        Self {
            entities: ValueIn::new(ValueEntityList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeDisableFocusCamera, 265, "265 Disable_Focus_Camera", [entities], []);

/// 屏幕震动(ID 266)
pub struct NodePlayScreenShake {
    entities: ValueIn,
    duration: ValueIn,
    intensity: ValueIn,
    frequency: ValueIn,
    next: ControlOut,
}
impl Default for NodePlayScreenShake {
    fn default() -> Self {
        Self {
            entities: ValueIn::new(ValueEntityList::def()),
            duration: ValueIn::new(ValueFloat::def()),
            intensity: ValueIn::new(ValueFloat::def()),
            frequency: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodePlayScreenShake, 266, "266 Play_Screen_Shake", [entities, duration, intensity, frequency], []);

/// 设置干扰器状态(ID 366)
pub struct NodeSetDisruptorState {
    entity: ValueIn,
    disruptor: ValueIn,
    enabled: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetDisruptorState {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            disruptor: ValueIn::new(ValueEntity::def()),
            enabled: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetDisruptorState, 366, "366 Set_Disruptor_State", [entity, disruptor, enabled], []);

/// 设置原生值(ID 445)
pub struct NodeSetNativeValue {
    entity: ValueIn,
    name: ValueIn,
    value: ValueIn,
    is_global: ValueIn,
    notify: ValueIn,
    next: ControlOut,
}
impl Default for NodeSetNativeValue {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            value: ValueIn::new(ValueInt::def()),
            is_global: ValueIn::new(ValueBool::def()),
            notify: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeSetNativeValue, 445, "445 Set_Native_Value", [entity, name, value, is_global, notify], []);

/// 添加名牌(ID 615)
pub struct NodeAddNameplate {
    entity: ValueIn,
    nameplate: ValueIn,
    next: ControlOut,
}
impl Default for NodeAddNameplate {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            nameplate: ValueIn::new(ValueConfig::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeAddNameplate, 615, "615 Add_Nameplate", [entity, nameplate], []);

/// 移除名牌(ID 616)
pub struct NodeRemoveNameplate {
    entity: ValueIn,
    nameplate: ValueIn,
    next: ControlOut,
}
impl Default for NodeRemoveNameplate {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            nameplate: ValueIn::new(ValueConfig::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeRemoveNameplate, 616, "616 Remove_Nameplate", [entity, nameplate], []);

/// 更新排行榜(ID 678)
pub struct NodeUpdateLeaderboard {
    leaderboard: ValueIn,
    score: ValueIn,
    index: ValueIn,
    next: ControlOut,
}
impl Default for NodeUpdateLeaderboard {
    fn default() -> Self {
        Self {
            leaderboard: ValueIn::new(ValueIntList::def()),
            score: ValueIn::new(ValueInt::def()),
            index: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeUpdateLeaderboard, 678, "678 Update_Leaderboard", [leaderboard, score, index], []);

/// 读取原生值(ID 459):值查询,无 flow
pub struct NodeGetNativeValue {
    target: ValueIn,
    name: ValueIn,
    is_global: ValueIn,
}
impl INode for NodeGetNativeValue {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.target, &self.name, &self.is_global]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 459 Get_Native_Value execute")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 459 Get_Native_Value get_value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(459)
    }
}
impl Default for NodeGetNativeValue {
    fn default() -> Self {
        Self {
            target: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
            is_global: ValueIn::new(ValueBool::def()),
        }
    }
}

/// 原生值变化(ID 428)
pub struct NodeOnNativeValueChange {
    next: ControlOut,
}
impl Default for NodeOnNativeValueChange {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnNativeValueChange, 428, "428 On_Native_Value_Change", [ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueInt::def(), ValueInt::def(), ValueBool::def()]);

/// GM 调用(ID 100000)
pub struct NodeOnGmCall {
    next: ControlOut,
}
impl Default for NodeOnGmCall {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
trigger_node!(NodeOnGmCall, 100000, "100000 On_GM_Call", [ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def(), ValueString::def(), ValueString::def()]);
