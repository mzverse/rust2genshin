//! 查询域节点(Server,Query)
//!
//! 人工设计,替换自动生成版本:引脚语义命名、动态结构用 Vec、类型准确。
//! execute/get_value 仅模拟(todo!())。

use crate::asset::node_graph::{ControlOut, INode, NodeRef, Simulation, ValueIn};
use crate::asset::raw_node_graph::NodeType;
use crate::asset::value::{
    AnyValue, ValueBool, ValueConfig, ValueConfigList, ValueDefault, ValueDict, ValueEntity,
    ValueEntityList, ValueEnum, ValueFaction, ValueFloat, ValueGuid, ValueInt, ValueIntList,
    ValueLocalVarRef, ValuePrefab, ValueString, ValueVarSnapshotRef, ValueVector,
};
use anyhow::Result;

// ========================================================================
// 随机 / 数学常量
// ========================================================================

/// 随机浮点(ID 7):[min, max) 内随机
pub struct NodeRandomFloat {
    min: ValueIn,
    max: ValueIn,
}
impl INode for NodeRandomFloat {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.min, &self.max]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 7 Random_Float")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 7 Random_Float")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(7)
    }
}
impl Default for NodeRandomFloat {
    fn default() -> Self {
        Self {
            min: ValueIn::new(ValueFloat::def()),
            max: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 加权随机(ID 8):按权重列表抽取下标
pub struct NodeWeightedRandom {
    weights: ValueIn,
}
impl INode for NodeWeightedRandom {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.weights]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 8 Weighted_Random")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 8 Weighted_Random")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(8)
    }
}
impl Default for NodeWeightedRandom {
    fn default() -> Self {
        Self { weights: ValueIn::new(ValueIntList::def()) }
    }
}

/// 随机整数(ID 257):[min, max] 内随机
pub struct NodeRandomInt {
    min: ValueIn,
    max: ValueIn,
}
impl INode for NodeRandomInt {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.min, &self.max]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 257 Random_Int")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 257 Random_Int")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(257)
    }
}
impl Default for NodeRandomInt {
    fn default() -> Self {
        Self {
            min: ValueIn::new(ValueInt::def()),
            max: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 圆周率(ID 191)
pub struct NodePi {
    _unused: (),
}
impl INode for NodePi {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 191 Pi")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 191 Pi")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(191)
    }
}
impl Default for NodePi {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 零向量(ID 192)
pub struct NodeVectorZero {
    _unused: (),
}
impl INode for NodeVectorZero {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 192 Vector_Zero")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 192 Vector_Zero")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(192)
    }
}
impl Default for NodeVectorZero {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 上向量(ID 193)
pub struct NodeVectorUp {
    _unused: (),
}
impl INode for NodeVectorUp {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 193 Vector_Up")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 193 Vector_Up")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(193)
    }
}
impl Default for NodeVectorUp {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 下向量(ID 194)
pub struct NodeVectorDown {
    _unused: (),
}
impl INode for NodeVectorDown {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 194 Vector_Down")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 194 Vector_Down")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(194)
    }
}
impl Default for NodeVectorDown {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 左向量(ID 195)
pub struct NodeVectorLeft {
    _unused: (),
}
impl INode for NodeVectorLeft {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 195 Vector_Left")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 195 Vector_Left")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(195)
    }
}
impl Default for NodeVectorLeft {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 右向量(ID 196)
pub struct NodeVectorRight {
    _unused: (),
}
impl INode for NodeVectorRight {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 196 Vector_Right")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 196 Vector_Right")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(196)
    }
}
impl Default for NodeVectorRight {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 前向量(ID 197)
pub struct NodeVectorForward {
    _unused: (),
}
impl INode for NodeVectorForward {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 197 Vector_Forward")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 197 Vector_Forward")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(197)
    }
}
impl Default for NodeVectorForward {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 后向量(ID 198)
pub struct NodeVectorBackward {
    _unused: (),
}
impl INode for NodeVectorBackward {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 198 Vector_Backward")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 198 Vector_Backward")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(198)
    }
}
impl Default for NodeVectorBackward {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 当前时间戳(ID 755)
pub struct NodeGetTimestamp {
    _unused: (),
}
impl INode for NodeGetTimestamp {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 755 Get_Timestamp")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 755 Get_Timestamp")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(755)
    }
}
impl Default for NodeGetTimestamp {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 时区偏移(ID 756)
pub struct NodeGetTimezone {
    _unused: (),
}
impl INode for NodeGetTimezone {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 756 Get_Timezone")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 756 Get_Timezone")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(756)
    }
}
impl Default for NodeGetTimezone {
    fn default() -> Self {
        Self { _unused: () }
    }
}

// ========================================================================
// 实体查询
// ========================================================================

/// 获取自身实体(ID 73)
pub struct NodeGetSelf {
    _unused: (),
}
impl INode for NodeGetSelf {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntity::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 73 Get_Self")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 73 Get_Self")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(73)
    }
}
impl Default for NodeGetSelf {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 按 GUID 获取实体(ID 75)
pub struct NodeGetByGuid {
    guid: ValueIn,
}
impl INode for NodeGetByGuid {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.guid]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntity::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 75 Get_By_GUID")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 75 Get_By_GUID")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(75)
    }
}
impl Default for NodeGetByGuid {
    fn default() -> Self {
        Self { guid: ValueIn::new(ValueGuid::def()) }
    }
}

/// 获取实体 GUID(ID 76)
pub struct NodeGetGuid {
    entity: ValueIn,
}
impl INode for NodeGetGuid {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueGuid::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 76 Get_GUID")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 76 Get_GUID")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(76)
    }
}
impl Default for NodeGetGuid {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 获取实体变换(ID 99):位置 + 旋转
pub struct NodeGetTransform {
    entity: ValueIn,
}
impl INode for NodeGetTransform {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def(), ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 99 Get_Transform")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 99 Get_Transform")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(99)
    }
}
impl Default for NodeGetTransform {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 获取实体类型(ID 260)
pub struct NodeGetEntityType {
    entity: ValueIn,
}
impl INode for NodeGetEntityType {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEnum::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 260 Get_Type")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 260 Get_Type")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(260)
    }
}
impl Default for NodeGetEntityType {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 全部实体(ID 318)
pub struct NodeGetAllEntities {
    _unused: (),
}
impl INode for NodeGetAllEntities {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 318 Get_All_Entities")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 318 Get_All_Entities")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(318)
    }
}
impl Default for NodeGetAllEntities {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 按类型获取实体(ID 319)
pub struct NodeGetEntityByType {
    entity_type: ValueIn,
}
impl INode for NodeGetEntityByType {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity_type]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 319 Get_Entity_By_Type")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 319 Get_Entity_By_Type")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(319)
    }
}
impl Default for NodeGetEntityByType {
    fn default() -> Self {
        Self { entity_type: ValueIn::new(ValueEnum::def()) }
    }
}

/// 按预制体获取实体(ID 320)
pub struct NodeGetWithPrefab {
    prefab: ValueIn,
}
impl INode for NodeGetWithPrefab {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.prefab]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 320 Get_With_Prefab")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 320 Get_With_Prefab")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(320)
    }
}
impl Default for NodeGetWithPrefab {
    fn default() -> Self {
        Self { prefab: ValueIn::new(ValuePrefab::def()) }
    }
}

/// 列表按类型过滤(ID 377)
pub struct NodeGetByType {
    entities: ValueIn,
    entity_type: ValueIn,
}
impl INode for NodeGetByType {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entities, &self.entity_type]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 377 Get_By_Type")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 377 Get_By_Type")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(377)
    }
}
impl Default for NodeGetByType {
    fn default() -> Self {
        Self {
            entities: ValueIn::new(ValueEntityList::def()),
            entity_type: ValueIn::new(ValueEnum::def()),
        }
    }
}

/// 列表按预制体过滤(ID 378)
pub struct NodeGetByPrefab {
    entities: ValueIn,
    prefab: ValueIn,
}
impl INode for NodeGetByPrefab {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entities, &self.prefab]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 378 Get_By_Prefab")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 378 Get_By_Prefab")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(378)
    }
}
impl Default for NodeGetByPrefab {
    fn default() -> Self {
        Self {
            entities: ValueIn::new(ValueEntityList::def()),
            prefab: ValueIn::new(ValuePrefab::def()),
        }
    }
}

/// 列表按阵营过滤(ID 379)
pub struct NodeGetByFaction {
    entities: ValueIn,
    faction: ValueIn,
}
impl INode for NodeGetByFaction {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entities, &self.faction]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 379 Get_By_Faction")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 379 Get_By_Faction")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(379)
    }
}
impl Default for NodeGetByFaction {
    fn default() -> Self {
        Self {
            entities: ValueIn::new(ValueEntityList::def()),
            faction: ValueIn::new(ValueFaction::def()),
        }
    }
}

/// 列表按范围过滤(ID 380):中心 + 半径
pub struct NodeGetByRange {
    entities: ValueIn,
    center: ValueIn,
    radius: ValueIn,
}
impl INode for NodeGetByRange {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entities, &self.center, &self.radius]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 380 Get_By_Range")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 380 Get_By_Range")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(380)
    }
}
impl Default for NodeGetByRange {
    fn default() -> Self {
        Self {
            entities: ValueIn::new(ValueEntityList::def()),
            center: ValueIn::new(ValueVector::def()),
            radius: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 是否激活(ID 507)
pub struct NodeIsActive {
    entity: ValueIn,
}
impl INode for NodeIsActive {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 507 Is_Active")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 507 Is_Active")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(507)
    }
}
impl Default for NodeIsActive {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 前向向量(ID 516)
pub struct NodeGetForward {
    entity: ValueIn,
}
impl INode for NodeGetForward {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 516 Get_Forward")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 516 Get_Forward")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(516)
    }
}
impl Default for NodeGetForward {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 右向向量(ID 517)
pub struct NodeGetRight {
    entity: ValueIn,
}
impl INode for NodeGetRight {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 517 Get_Right")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 517 Get_Right")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(517)
    }
}
impl Default for NodeGetRight {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 上向向量(ID 518)
pub struct NodeGetUp {
    entity: ValueIn,
}
impl INode for NodeGetUp {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 518 Get_Up")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 518 Get_Up")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(518)
    }
}
impl Default for NodeGetUp {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 对象属性(ID 580):7 项数值属性
pub struct NodeGetObjAttr {
    entity: ValueIn,
}
impl INode for NodeGetObjAttr {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![
            ValueInt::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
            ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
        ]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 580 Get_Obj_Attr")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 580 Get_Obj_Attr")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(580)
    }
}
impl Default for NodeGetObjAttr {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 高级属性(ID 670):7 项浮点属性
pub struct NodeGetAdvAttr {
    entity: ValueIn,
}
impl INode for NodeGetAdvAttr {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![
            ValueFloat::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
            ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
        ]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 670 Get_Adv_Attr")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 670 Get_Adv_Attr")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(670)
    }
}
impl Default for NodeGetAdvAttr {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 元素属性(ID 671):10 项浮点属性
pub struct NodeGetElemAttr {
    entity: ValueIn,
}
impl INode for NodeGetElemAttr {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![
            ValueFloat::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
            ValueFloat::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
            ValueFloat::def(), ValueFloat::def(),
        ]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 671 Get_Elem_Attr")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 671 Get_Elem_Attr")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(671)
    }
}
impl Default for NodeGetElemAttr {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 所有者(ID 744)
pub struct NodeGetOwner {
    entity: ValueIn,
}
impl INode for NodeGetOwner {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntity::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 744 Get_Owner")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 744 Get_Owner")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(744)
    }
}
impl Default for NodeGetOwner {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 被拥有的实体列表(ID 745)
pub struct NodeGetOwnedEntities {
    entity: ValueIn,
}
impl INode for NodeGetOwnedEntities {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 745 Get_Owned_Entities")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 745 Get_Owned_Entities")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(745)
    }
}
impl Default for NodeGetOwnedEntities {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 移速(ID 947):速度 + 方向
pub struct NodeGetMoveSpeed {
    entity: ValueIn,
}
impl INode for NodeGetMoveSpeed {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def(), ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 947 Get_Move_Speed")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 947 Get_Move_Speed")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(947)
    }
}
impl Default for NodeGetMoveSpeed {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

// ========================================================================
// 角色 / 玩家
// ========================================================================

/// 全部玩家(ID 248)
pub struct NodeGetAllPlayers {
    _unused: (),
}
impl INode for NodeGetAllPlayers {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 248 Get_All_Players")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 248 Get_All_Players")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(248)
    }
}
impl Default for NodeGetAllPlayers {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 玩家角色列表(ID 258)
pub struct NodeGetPlayerCharacters {
    player: ValueIn,
}
impl INode for NodeGetPlayerCharacters {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.player]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 258 Get_Player_Characters")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 258 Get_Player_Characters")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(258)
    }
}
impl Default for NodeGetPlayerCharacters {
    fn default() -> Self {
        Self { player: ValueIn::new(ValueEntity::def()) }
    }
}

/// 所属玩家(ID 259)
pub struct NodeGetOwnerPlayer {
    entity: ValueIn,
}
impl INode for NodeGetOwnerPlayer {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntity::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 259 Get_Owner_Player")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 259 Get_Owner_Player")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(259)
    }
}
impl Default for NodeGetOwnerPlayer {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 复活次数(ID 275)
pub struct NodeGetRevives {
    entity: ValueIn,
}
impl INode for NodeGetRevives {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 275 Get_Revives")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 275 Get_Revives")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(275)
    }
}
impl Default for NodeGetRevives {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 复活时间(ID 277)
pub struct NodeGetReviveTime {
    entity: ValueIn,
}
impl INode for NodeGetReviveTime {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 277 Get_Revive_Time")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 277 Get_Revive_Time")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(277)
    }
}
impl Default for NodeGetReviveTime {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 是否全部倒地(ID 287)
pub struct NodeIsAllDown {
    entity: ValueIn,
}
impl INode for NodeIsAllDown {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 287 Is_All_Down")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 287 Is_All_Down")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(287)
    }
}
impl Default for NodeIsAllDown {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 按 ID 获取 GUID(ID 750)
pub struct NodeGetGuidById {
    id: ValueIn,
}
impl INode for NodeGetGuidById {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.id]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueGuid::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 750 Get_GUID_By_ID")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 750 Get_GUID_By_ID")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(750)
    }
}
impl Default for NodeGetGuidById {
    fn default() -> Self {
        Self { id: ValueIn::new(ValueInt::def()) }
    }
}

/// 按 GUID 获取 ID(ID 751)
pub struct NodeGetIdByGuid {
    guid: ValueIn,
}
impl INode for NodeGetIdByGuid {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.guid]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 751 Get_ID_By_GUID")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 751 Get_ID_By_GUID")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(751)
    }
}
impl Default for NodeGetIdByGuid {
    fn default() -> Self {
        Self { guid: ValueIn::new(ValueGuid::def()) }
    }
}

/// 昵称(ID 767)
pub struct NodeGetNickname {
    entity: ValueIn,
}
impl INode for NodeGetNickname {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueString::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 767 Get_Nickname")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 767 Get_Nickname")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(767)
    }
}
impl Default for NodeGetNickname {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 输入方式(ID 768)
pub struct NodeGetInputType {
    entity: ValueIn,
}
impl INode for NodeGetInputType {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEnum::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 768 Get_Input_Type")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 768 Get_Input_Type")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(768)
    }
}
impl Default for NodeGetInputType {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

// ========================================================================
// 变量 / 状态
// ========================================================================

/// 获取局部变量(ID 18)
pub struct NodeGetLocal {
    name: ValueIn,
}
impl INode for NodeGetLocal {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.name]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueLocalVarRef::def(), ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 18 Get_Local")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 18 Get_Local")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(18)
    }
}
impl Default for NodeGetLocal {
    fn default() -> Self {
        Self { name: ValueIn::new(ValueInt::def()) }
    }
}

/// 自定义变量(ID 50):entity + 变量名 → 值
pub struct NodeGetVariable {
    entity: ValueIn,
    name: ValueIn,
}
impl INode for NodeGetVariable {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.name]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 50 Get_Variable")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 50 Get_Variable")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(50)
    }
}
impl Default for NodeGetVariable {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
        }
    }
}

/// 图变量(ID 337)
pub struct NodeGetGraphVariable {
    name: ValueIn,
}
impl INode for NodeGetGraphVariable {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.name]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 337 Get_Graph_Variable")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 337 Get_Graph_Variable")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(337)
    }
}
impl Default for NodeGetGraphVariable {
    fn default() -> Self {
        Self { name: ValueIn::new(ValueString::def()) }
    }
}

/// 变量快照(ID 3360):snapshot + 变量名 → 值
pub struct NodeGetSnapshot {
    snapshot: ValueIn,
    name: ValueIn,
}
impl INode for NodeGetSnapshot {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.snapshot, &self.name]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 3360 Get_Snapshot")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 3360 Get_Snapshot")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(3360)
    }
}
impl Default for NodeGetSnapshot {
    fn default() -> Self {
        Self {
            snapshot: ValueIn::new(ValueVarSnapshotRef::def()),
            name: ValueIn::new(ValueString::def()),
        }
    }
}

/// 状态值(ID 68):entity + 状态 ID → 数值
pub struct NodeGetStatus {
    entity: ValueIn,
    status: ValueIn,
}
impl INode for NodeGetStatus {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.status]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 68 Get_Status")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 68 Get_Status")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(68)
    }
}
impl Default for NodeGetStatus {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            status: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 是否有状态(ID 508)
pub struct NodeHasStatus {
    entity: ValueIn,
    status: ValueIn,
}
impl INode for NodeHasStatus {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.status]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 508 Has_Status")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 508 Has_Status")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(508)
    }
}
impl Default for NodeHasStatus {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            status: ValueIn::new(ValueConfig::def()),
        }
    }
}

/// 状态层数(ID 746)
pub struct NodeGetStatusStacks {
    entity: ValueIn,
    status: ValueIn,
    slot: ValueIn,
}
impl INode for NodeGetStatusStacks {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.status, &self.slot]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 746 Get_Status_Stacks")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 746 Get_Status_Stacks")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(746)
    }
}
impl Default for NodeGetStatusStacks {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            status: ValueIn::new(ValueConfig::def()),
            slot: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 状态施加者(ID 747)
pub struct NodeGetStatusApplier {
    entity: ValueIn,
    status: ValueIn,
    slot: ValueIn,
}
impl INode for NodeGetStatusApplier {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.status, &self.slot]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntity::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 747 Get_Status_Applier")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 747 Get_Status_Applier")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(747)
    }
}
impl Default for NodeGetStatusApplier {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            status: ValueIn::new(ValueConfig::def()),
            slot: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 状态槽位列表(ID 748)
pub struct NodeGetStatusSlots {
    entity: ValueIn,
    status: ValueIn,
}
impl INode for NodeGetStatusSlots {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.status]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 748 Get_Status_Slots")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 748 Get_Status_Slots")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(748)
    }
}
impl Default for NodeGetStatusSlots {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            status: ValueIn::new(ValueConfig::def()),
        }
    }
}

// ========================================================================
// 列表操作
// ========================================================================

/// 包含(ID 114)
pub struct NodeContains {
    list: ValueIn,
    item: ValueIn,
}
impl INode for NodeContains {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.list, &self.item]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 114 Contains")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 114 Contains")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(114)
    }
}
impl Default for NodeContains {
    fn default() -> Self {
        Self {
            list: ValueIn::new(ValueIntList::def()),
            item: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 查找下标(ID 121):返回所有匹配下标
pub struct NodeFindIndex {
    list: ValueIn,
    item: ValueIn,
}
impl INode for NodeFindIndex {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.list, &self.item]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 121 Find_Index")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 121 Find_Index")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(121)
    }
}
impl Default for NodeFindIndex {
    fn default() -> Self {
        Self {
            list: ValueIn::new(ValueIntList::def()),
            item: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 按下标取元素(ID 128)
pub struct NodeGetAtIndex {
    list: ValueIn,
    index: ValueIn,
}
impl INode for NodeGetAtIndex {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.list, &self.index]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 128 Get_At_Index")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 128 Get_At_Index")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(128)
    }
}
impl Default for NodeGetAtIndex {
    fn default() -> Self {
        Self {
            list: ValueIn::new(ValueIntList::def()),
            index: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 列表长度(ID 142)
pub struct NodeGetListLength {
    list: ValueIn,
}
impl INode for NodeGetListLength {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.list]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 142 Get_Length")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 142 Get_Length")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(142)
    }
}
impl Default for NodeGetListLength {
    fn default() -> Self {
        Self { list: ValueIn::new(ValueIntList::def()) }
    }
}

/// 列表最大值(ID 149)
pub struct NodeGetMax {
    list: ValueIn,
}
impl INode for NodeGetMax {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.list]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 149 Get_Max")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 149 Get_Max")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(149)
    }
}
impl Default for NodeGetMax {
    fn default() -> Self {
        Self { list: ValueIn::new(ValueIntList::def()) }
    }
}

/// 列表最小值(ID 151)
pub struct NodeGetMin {
    list: ValueIn,
}
impl INode for NodeGetMin {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.list]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 151 Get_Min")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 151 Get_Min")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(151)
    }
}
impl Default for NodeGetMin {
    fn default() -> Self {
        Self { list: ValueIn::new(ValueIntList::def()) }
    }
}

// ========================================================================
// 全局 / 关卡
// ========================================================================

/// 激活的实体布局组(ID 179)
pub struct NodeGetActiveGroups {
    _unused: (),
}
impl INode for NodeGetActiveGroups {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 179 Get_Active_Groups")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 179 Get_Active_Groups")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(179)
    }
}
impl Default for NodeGetActiveGroups {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 已用时间(ID 290)
pub struct NodeGetElapsedTime {
    _unused: (),
}
impl INode for NodeGetElapsedTime {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 290 Get_Elapsed_Time")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 290 Get_Elapsed_Time")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(290)
    }
}
impl Default for NodeGetElapsedTime {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 环境时间(ID 664):时间 + 天
pub struct NodeGetEnvTime {
    _unused: (),
}
impl INode for NodeGetEnvTime {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def(), ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 664 Get_Env_Time")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 664 Get_Env_Time")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(664)
    }
}
impl Default for NodeGetEnvTime {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 游戏信息(ID 766):模式等
pub struct NodeGetGameInfo {
    _unused: (),
}
impl INode for NodeGetGameInfo {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def(), ValueEnum::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 766 Get_Game_Info")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 766 Get_Game_Info")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(766)
    }
}
impl Default for NodeGetGameInfo {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 全局计时器时间(ID 310)
pub struct NodeGetTimerTime {
    entity: ValueIn,
    name: ValueIn,
}
impl INode for NodeGetTimerTime {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.name]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 310 Get_Time")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 310 Get_Time")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(310)
    }
}
impl Default for NodeGetTimerTime {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
        }
    }
}

/// 当前界面布局(ID 317)
pub struct NodeGetCurrentLayout {
    entity: ValueIn,
}
impl INode for NodeGetCurrentLayout {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 317 Get_Current_Layout")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 317 Get_Current_Layout")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(317)
    }
}
impl Default for NodeGetCurrentLayout {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

// ========================================================================
// 阵营 / 标签 / 造物
// ========================================================================

/// 阵营(ID 249)
pub struct NodeGetFaction {
    entity: ValueIn,
}
impl INode for NodeGetFaction {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFaction::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 249 Get_Faction")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 249 Get_Faction")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(249)
    }
}
impl Default for NodeGetFaction {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 敌对判断(ID 614)
pub struct NodeIsHostile {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeIsHostile {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.a, &self.b]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 614 Is_Hostile")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 614 Is_Hostile")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(614)
    }
}
impl Default for NodeIsHostile {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFaction::def()),
            b: ValueIn::new(ValueFaction::def()),
        }
    }
}

/// 标签列表(ID 589)
pub struct NodeGetTags {
    entity: ValueIn,
}
impl INode for NodeGetTags {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 589 Get_Tags")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 589 Get_Tags")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(589)
    }
}
impl Default for NodeGetTags {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 按标签获取实体(ID 590)
pub struct NodeGetByTag {
    tag: ValueIn,
}
impl INode for NodeGetByTag {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.tag]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 590 Get_By_Tag")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 590 Get_By_Tag")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(590)
    }
}
impl Default for NodeGetByTag {
    fn default() -> Self {
        Self { tag: ValueIn::new(ValueInt::def()) }
    }
}

/// 造物目标(ID 376)
pub struct NodeGetCreationTarget {
    entity: ValueIn,
}
impl INode for NodeGetCreationTarget {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntity::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 376 Get_Target")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 376 Get_Target")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(376)
    }
}
impl Default for NodeGetCreationTarget {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 造物属性(ID 381)
pub struct NodeGetCreationAttr {
    entity: ValueIn,
}
impl INode for NodeGetCreationAttr {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![
            ValueInt::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
            ValueFloat::def(), ValueFloat::def(), ValueFloat::def(), ValueEnum::def(),
        ]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 381 Get_Attribute")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 381 Get_Attribute")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(381)
    }
}
impl Default for NodeGetCreationAttr {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 造物仇恨列表(ID 758)
pub struct NodeGetCreationAggroList {
    entity: ValueIn,
}
impl INode for NodeGetCreationAggroList {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 758 Get_Aggro_List")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 758 Get_Aggro_List")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(758)
    }
}
impl Default for NodeGetCreationAggroList {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 跟随目标(ID 246):目标实体 + GUID
pub struct NodeGetFollowTarget {
    entity: ValueIn,
}
impl INode for NodeGetFollowTarget {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntity::def(), ValueGuid::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 246 Get_Target")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 246 Get_Target")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(246)
    }
}
impl Default for NodeGetFollowTarget {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 预设点变换(ID 270)
pub struct NodeGetPresetPointTransform {
    index: ValueIn,
}
impl INode for NodeGetPresetPointTransform {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.index]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def(), ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 270 Get_Transform")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 270 Get_Transform")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(270)
    }
}
impl Default for NodeGetPresetPointTransform {
    fn default() -> Self {
        Self { index: ValueIn::new(ValueInt::def()) }
    }
}

/// 按标签取预设点(ID 271)
pub struct NodeGetPresetPointByTag {
    tag: ValueIn,
}
impl INode for NodeGetPresetPointByTag {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.tag]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 271 Get_By_Tag")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 271 Get_By_Tag")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(271)
    }
}
impl Default for NodeGetPresetPointByTag {
    fn default() -> Self {
        Self { tag: ValueIn::new(ValueInt::def()) }
    }
}

/// 巡逻模板(ID 619):三个整数
pub struct NodeGetPatrolTemplate {
    entity: ValueIn,
}
impl INode for NodeGetPatrolTemplate {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def(), ValueInt::def(), ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 619 Get_Patrol_Template")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 619 Get_Patrol_Template")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(619)
    }
}
impl Default for NodeGetPatrolTemplate {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 路径点(ID 621):位置 + 方向
pub struct NodeGetWaypoint {
    path: ValueIn,
    index: ValueIn,
}
impl INode for NodeGetWaypoint {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.path, &self.index]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def(), ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 621 Get_Waypoint")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 621 Get_Waypoint")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(621)
    }
}
impl Default for NodeGetWaypoint {
    fn default() -> Self {
        Self {
            path: ValueIn::new(ValueInt::def()),
            index: ValueIn::new(ValueInt::def()),
        }
    }
}

// ========================================================================
// 职业 / 技能 / 仇恨
// ========================================================================

/// 职业配置(ID 387)
pub struct NodeGetClass {
    entity: ValueIn,
}
impl INode for NodeGetClass {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueConfig::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 387 Get_Class")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 387 Get_Class")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(387)
    }
}
impl Default for NodeGetClass {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 职业等级(ID 388)
pub struct NodeGetLevel {
    entity: ValueIn,
    class: ValueIn,
}
impl INode for NodeGetLevel {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.class]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 388 Get_Level")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 388 Get_Level")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(388)
    }
}
impl Default for NodeGetLevel {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            class: ValueIn::new(ValueConfig::def()),
        }
    }
}

/// 技能信息(ID 398)
pub struct NodeGetSkillInfo {
    entity: ValueIn,
    slot: ValueIn,
}
impl INode for NodeGetSkillInfo {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.slot]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueConfig::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 398 Get_Skill_Info")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 398 Get_Skill_Info")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(398)
    }
}
impl Default for NodeGetSkillInfo {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            slot: ValueIn::new(ValueEnum::def()),
        }
    }
}

/// 仇恨值(ID 603)
pub struct NodeGetAggroValue {
    entity: ValueIn,
    target: ValueIn,
}
impl INode for NodeGetAggroValue {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.target]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 603 Get_Aggro_Value")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 603 Get_Aggro_Value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(603)
    }
}
impl Default for NodeGetAggroValue {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            target: ValueIn::new(ValueEntity::def()),
        }
    }
}

/// 仇恨倍率(ID 604)
pub struct NodeGetAggroMultiplier {
    entity: ValueIn,
}
impl INode for NodeGetAggroMultiplier {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 604 Get_Multiplier")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 604 Get_Multiplier")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(604)
    }
}
impl Default for NodeGetAggroMultiplier {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 全局仇恨倍率(ID 605)
pub struct NodeGetGlobalMultiplier {
    _unused: (),
}
impl INode for NodeGetGlobalMultiplier {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 605 Get_Global_Multiplier")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 605 Get_Global_Multiplier")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(605)
    }
}
impl Default for NodeGetGlobalMultiplier {
    fn default() -> Self {
        Self { _unused: () }
    }
}

/// 仇恨目标(ID 606)
pub struct NodeGetAggroTarget {
    entity: ValueIn,
}
impl INode for NodeGetAggroTarget {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntity::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 606 Get_Aggro_Target")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 606 Get_Aggro_Target")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(606)
    }
}
impl Default for NodeGetAggroTarget {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 仇恨拥有者列表(ID 607)
pub struct NodeGetAggroOwners {
    entity: ValueIn,
}
impl INode for NodeGetAggroOwners {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 607 Get_Aggro_Owners")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 607 Get_Aggro_Owners")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(607)
    }
}
impl Default for NodeGetAggroOwners {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 瞄准此实体的拥有者列表(ID 608)
pub struct NodeGetTargetingOwners {
    entity: ValueIn,
}
impl INode for NodeGetTargetingOwners {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 608 Get_Targeting_Owners")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 608 Get_Targeting_Owners")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(608)
    }
}
impl Default for NodeGetTargetingOwners {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 仇恨列表(ID 609)
pub struct NodeGetAggroList {
    entity: ValueIn,
}
impl INode for NodeGetAggroList {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 609 Get_Aggro_List")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 609 Get_Aggro_List")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(609)
    }
}
impl Default for NodeGetAggroList {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 是否在战斗中(ID 610)
pub struct NodeIsInCombat {
    entity: ValueIn,
}
impl INode for NodeIsInCombat {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 610 Is_In_Combat")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 610 Is_In_Combat")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(610)
    }
}
impl Default for NodeIsInCombat {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

// ========================================================================
// 小地图 / 成就 / 结算 / 排名
// ========================================================================

/// 标记信息(ID 638)
pub struct NodeGetMarkerInfo {
    entity: ValueIn,
    index: ValueIn,
}
impl INode for NodeGetMarkerInfo {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.index]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def(), ValueEntityList::def(), ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 638 Get_Marker_Info")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 638 Get_Marker_Info")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(638)
    }
}
impl Default for NodeGetMarkerInfo {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            index: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 标记状态(ID 639):三组整数列表
pub struct NodeGetMarkerStatus {
    entity: ValueIn,
}
impl INode for NodeGetMarkerStatus {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def(), ValueIntList::def(), ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 639 Get_Marker_Status")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 639 Get_Marker_Status")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(639)
    }
}
impl Default for NodeGetMarkerStatus {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 成就是否完成(ID 644)
pub struct NodeIsCompleted {
    entity: ValueIn,
    achievement: ValueIn,
}
impl INode for NodeIsCompleted {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.achievement]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 644 Is_Completed")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 644 Is_Completed")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(644)
    }
}
impl Default for NodeIsCompleted {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            achievement: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 玩家排名(ID 651)
pub struct NodeGetPlayerRank {
    entity: ValueIn,
}
impl INode for NodeGetPlayerRank {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 651 Get_Player_Rank")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 651 Get_Player_Rank")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(651)
    }
}
impl Default for NodeGetPlayerRank {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 玩家结算结果(ID 653)
pub struct NodeGetPlayerResult {
    entity: ValueIn,
}
impl INode for NodeGetPlayerResult {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEnum::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 653 Get_Player_Result")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 653 Get_Player_Result")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(653)
    }
}
impl Default for NodeGetPlayerResult {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 阵营排名(ID 655)
pub struct NodeGetFactionRank {
    faction: ValueIn,
}
impl INode for NodeGetFactionRank {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.faction]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 655 Get_Faction_Rank")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 655 Get_Faction_Rank")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(655)
    }
}
impl Default for NodeGetFactionRank {
    fn default() -> Self {
        Self { faction: ValueIn::new(ValueFaction::def()) }
    }
}

/// 阵营结算结果(ID 657)
pub struct NodeGetFactionResult {
    faction: ValueIn,
}
impl INode for NodeGetFactionResult {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.faction]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEnum::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 657 Get_Faction_Result")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 657 Get_Faction_Result")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(657)
    }
}
impl Default for NodeGetFactionResult {
    fn default() -> Self {
        Self { faction: ValueIn::new(ValueFaction::def()) }
    }
}

/// 排名信息(ID 658):四项整数
pub struct NodeGetRankInfo {
    entity: ValueIn,
}
impl INode for NodeGetRankInfo {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def(), ValueInt::def(), ValueInt::def(), ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 658 Get_Rank_Info")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 658 Get_Rank_Info")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(658)
    }
}
impl Default for NodeGetRankInfo {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 分数变化(ID 660)
pub struct NodeGetScoreChange {
    entity: ValueIn,
    result: ValueIn,
}
impl INode for NodeGetScoreChange {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.result]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 660 Get_Score_Change")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 660 Get_Score_Change")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(660)
    }
}
impl Default for NodeGetScoreChange {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            result: ValueIn::new(ValueEnum::def()),
        }
    }
}

/// 逃脱状态(ID 662)
pub struct NodeGetEscapeStatus {
    entity: ValueIn,
}
impl INode for NodeGetEscapeStatus {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 662 Get_Escape_Status")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 662 Get_Escape_Status")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(662)
    }
}
impl Default for NodeGetEscapeStatus {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 重叠实体列表(ID 669)
pub struct NodeGetOverlappingEntities {
    entity: ValueIn,
    radius: ValueIn,
}
impl INode for NodeGetOverlappingEntities {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.radius]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntityList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 669 Get_Overlapping_Entities")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 669 Get_Overlapping_Entities")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(669)
    }
}
impl Default for NodeGetOverlappingEntities {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            radius: ValueIn::new(ValueInt::def()),
        }
    }
}

// ========================================================================
// 装备 / 物品 / 商店 / 扫描标签
// ========================================================================

/// 词缀列表(ID 675)
pub struct NodeGetAffixes {
    equipment: ValueIn,
}
impl INode for NodeGetAffixes {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.equipment]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 675 Get_Affixes")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 675 Get_Affixes")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(675)
    }
}
impl Default for NodeGetAffixes {
    fn default() -> Self {
        Self { equipment: ValueIn::new(ValueInt::def()) }
    }
}

/// 词缀配置(ID 676)
pub struct NodeGetAffixConfig {
    equipment: ValueIn,
    affix: ValueIn,
}
impl INode for NodeGetAffixConfig {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.equipment, &self.affix]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueConfig::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 676 Get_Affix_Config")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 676 Get_Affix_Config")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(676)
    }
}
impl Default for NodeGetAffixConfig {
    fn default() -> Self {
        Self {
            equipment: ValueIn::new(ValueInt::def()),
            affix: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 词缀数值(ID 677)
pub struct NodeGetAffixValue {
    equipment: ValueIn,
    affix: ValueIn,
}
impl INode for NodeGetAffixValue {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.equipment, &self.affix]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 677 Get_Affix_Value")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 677 Get_Affix_Value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(677)
    }
}
impl Default for NodeGetAffixValue {
    fn default() -> Self {
        Self {
            equipment: ValueIn::new(ValueInt::def()),
            affix: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 装备标签列表(ID 734)
pub struct NodeGetEquipTags {
    equipment: ValueIn,
}
impl INode for NodeGetEquipTags {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.equipment]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueConfigList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 734 Get_Tags")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 734 Get_Tags")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(734)
    }
}
impl Default for NodeGetEquipTags {
    fn default() -> Self {
        Self { equipment: ValueIn::new(ValueInt::def()) }
    }
}

/// 配置 ID(ID 749)
pub struct NodeGetConfigId {
    equipment: ValueIn,
}
impl INode for NodeGetConfigId {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.equipment]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueConfig::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 749 Get_Config_ID")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 749 Get_Config_ID")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(749)
    }
}
impl Default for NodeGetConfigId {
    fn default() -> Self {
        Self { equipment: ValueIn::new(ValueInt::def()) }
    }
}

/// 物品容量(ID 689)
pub struct NodeGetCapacity {
    entity: ValueIn,
}
impl INode for NodeGetCapacity {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 689 Get_Capacity")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 689 Get_Capacity")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(689)
    }
}
impl Default for NodeGetCapacity {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 物品数量(ID 690)
pub struct NodeGetItemAmount {
    entity: ValueIn,
    item: ValueIn,
}
impl INode for NodeGetItemAmount {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.item]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 690 Get_Item_Amount")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 690 Get_Item_Amount")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(690)
    }
}
impl Default for NodeGetItemAmount {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            item: ValueIn::new(ValueConfig::def()),
        }
    }
}

/// 货币数量(ID 691)
pub struct NodeGetCurrencyAmount {
    entity: ValueIn,
    currency: ValueIn,
}
impl INode for NodeGetCurrencyAmount {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.currency]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 691 Get_Currency_Amount")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 691 Get_Currency_Amount")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(691)
    }
}
impl Default for NodeGetCurrencyAmount {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            currency: ValueIn::new(ValueConfig::def()),
        }
    }
}

/// 基础物品列表(ID 721):物品 → 数量映射
pub struct NodeGetBasicItems {
    entity: ValueIn,
}
impl INode for NodeGetBasicItems {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueDict::new(ValueConfig::default(), ValueInt::default()).into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 721 Get_Basic_Items")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 721 Get_Basic_Items")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(721)
    }
}
impl Default for NodeGetBasicItems {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 全部货币(ID 722)
pub struct NodeGetCurrencyAll {
    entity: ValueIn,
}
impl INode for NodeGetCurrencyAll {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueDict::new(ValueConfig::default(), ValueInt::default()).into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 722 Get_Currency_All")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 722 Get_Currency_All")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(722)
    }
}
impl Default for NodeGetCurrencyAll {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 全部装备(ID 723)
pub struct NodeGetEquipmentAll {
    entity: ValueIn,
}
impl INode for NodeGetEquipmentAll {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 723 Get_Equipment_All")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 723 Get_Equipment_All")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(723)
    }
}
impl Default for NodeGetEquipmentAll {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 掉落物品数量(ID 728)
pub struct NodeGetLootItemAmount {
    entity: ValueIn,
    item: ValueIn,
}
impl INode for NodeGetLootItemAmount {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.item]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 728 Get_Loot_Item_Amount")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 728 Get_Loot_Item_Amount")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(728)
    }
}
impl Default for NodeGetLootItemAmount {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            item: ValueIn::new(ValueConfig::def()),
        }
    }
}

/// 掉落货币数量(ID 729)
pub struct NodeGetLootCurrencyAmount {
    entity: ValueIn,
    currency: ValueIn,
}
impl INode for NodeGetLootCurrencyAmount {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.currency]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 729 Get_Loot_Currency_Amount")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 729 Get_Loot_Currency_Amount")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(729)
    }
}
impl Default for NodeGetLootCurrencyAmount {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            currency: ValueIn::new(ValueConfig::def()),
        }
    }
}

/// 掉落物品(ID 730)
pub struct NodeGetLootItems {
    entity: ValueIn,
}
impl INode for NodeGetLootItems {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueDict::new(ValueConfig::default(), ValueInt::default()).into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 730 Get_Loot_Items")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 730 Get_Loot_Items")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(730)
    }
}
impl Default for NodeGetLootItems {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 掉落货币(ID 731)
pub struct NodeGetLootCurrency {
    entity: ValueIn,
}
impl INode for NodeGetLootCurrency {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueDict::new(ValueConfig::default(), ValueInt::default()).into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 731 Get_Loot_Currency")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 731 Get_Loot_Currency")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(731)
    }
}
impl Default for NodeGetLootCurrency {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 掉落装备(ID 732)
pub struct NodeGetLootEquipment {
    entity: ValueIn,
}
impl INode for NodeGetLootEquipment {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 732 Get_Loot_Equipment")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 732 Get_Loot_Equipment")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(732)
    }
}
impl Default for NodeGetLootEquipment {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 自定义商品列表(ID 714)
pub struct NodeGetCustomSales {
    entity: ValueIn,
    shop: ValueIn,
}
impl INode for NodeGetCustomSales {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.shop]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 714 Get_Custom_Sales")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 714 Get_Custom_Sales")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(714)
    }
}
impl Default for NodeGetCustomSales {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 库存商品列表(ID 715)
pub struct NodeGetInvSales {
    entity: ValueIn,
    shop: ValueIn,
}
impl INode for NodeGetInvSales {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.shop]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueConfigList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 715 Get_Inv_Sales")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 715 Get_Inv_Sales")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(715)
    }
}
impl Default for NodeGetInvSales {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 购物车物品(ID 716)
pub struct NodeGetCartItems {
    entity: ValueIn,
    shop: ValueIn,
}
impl INode for NodeGetCartItems {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.shop]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueConfigList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 716 Get_Cart_Items")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 716 Get_Cart_Items")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(716)
    }
}
impl Default for NodeGetCartItems {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 自定义商品信息(ID 717)
pub struct NodeGetCustomItemInfo {
    entity: ValueIn,
    shop: ValueIn,
    item: ValueIn,
}
impl INode for NodeGetCustomItemInfo {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.shop, &self.item]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![
            ValueConfig::def(),
            ValueDict::new(ValueConfig::default(), ValueInt::default()).into(),
            ValueInt::def(),
            ValueBool::def(),
            ValueInt::def(),
            ValueInt::def(),
            ValueBool::def(),
        ]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 717 Get_Custom_Item_Info")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 717 Get_Custom_Item_Info")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(717)
    }
}
impl Default for NodeGetCustomItemInfo {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 库存商品信息(ID 718)
pub struct NodeGetInvItemInfo {
    entity: ValueIn,
    shop: ValueIn,
    item: ValueIn,
}
impl INode for NodeGetInvItemInfo {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.shop, &self.item]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![
            ValueDict::new(ValueConfig::default(), ValueInt::default()).into(),
            ValueInt::def(),
            ValueBool::def(),
        ]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 718 Get_Inv_Item_Info")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 718 Get_Inv_Item_Info")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(718)
    }
}
impl Default for NodeGetInvItemInfo {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueConfig::def()),
        }
    }
}

/// 购买信息(ID 719)
pub struct NodeGetPurchaseInfo {
    entity: ValueIn,
    shop: ValueIn,
    item: ValueIn,
}
impl INode for NodeGetPurchaseInfo {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.shop, &self.item]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![
            ValueDict::new(ValueConfig::default(), ValueInt::default()).into(),
            ValueBool::def(),
        ]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 719 Get_Purchase_Info")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 719 Get_Purchase_Info")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(719)
    }
}
impl Default for NodeGetPurchaseInfo {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            shop: ValueIn::new(ValueInt::def()),
            item: ValueIn::new(ValueConfig::def()),
        }
    }
}

/// 激活的扫描标签(ID 737)
pub struct NodeGetActiveTag {
    entity: ValueIn,
}
impl INode for NodeGetActiveTag {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueConfig::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 737 Get_Active_Tag")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 737 Get_Active_Tag")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(737)
    }
}
impl Default for NodeGetActiveTag {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 角色属性(ID 738):10 项
pub struct NodeGetCharacterAttr {
    entity: ValueIn,
}
impl INode for NodeGetCharacterAttr {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![
            ValueInt::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
            ValueFloat::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def(),
            ValueFloat::def(), ValueEnum::def(),
        ]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 738 Get_Character_Attr")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 738 Get_Character_Attr")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(738)
    }
}
impl Default for NodeGetCharacterAttr {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}

/// 奇趣盒数量(ID 773)
pub struct NodeGetBoxQuantity {
    entity: ValueIn,
    box_id: ValueIn,
}
impl INode for NodeGetBoxQuantity {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.box_id]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 773 Get_Box_Quantity")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 773 Get_Box_Quantity")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(773)
    }
}
impl Default for NodeGetBoxQuantity {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            box_id: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 奇趣盒消耗(ID 774)
pub struct NodeGetBoxConsumption {
    entity: ValueIn,
    box_id: ValueIn,
}
impl INode for NodeGetBoxConsumption {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.entity, &self.box_id]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 774 Get_Box_Consumption")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 774 Get_Box_Consumption")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(774)
    }
}
impl Default for NodeGetBoxConsumption {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            box_id: ValueIn::new(ValueInt::def()),
        }
    }
}

// ========================================================================
// 字典操作
// ========================================================================

/// 字典取值(ID 1158)
pub struct NodeDictGetValue {
    dict: ValueIn,
    key: ValueIn,
}
impl INode for NodeDictGetValue {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.dict, &self.key]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 1158 Get_Value")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 1158 Get_Value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(1158)
    }
}
impl Default for NodeDictGetValue {
    fn default() -> Self {
        Self {
            dict: ValueIn::new(ValueDict::new(ValueInt::default(), ValueInt::default()).into()),
            key: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 是否有键(ID 1368)
pub struct NodeDictHasKey {
    dict: ValueIn,
    key: ValueIn,
}
impl INode for NodeDictHasKey {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.dict, &self.key]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 1368 Has_Key")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 1368 Has_Key")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(1368)
    }
}
impl Default for NodeDictHasKey {
    fn default() -> Self {
        Self {
            dict: ValueIn::new(ValueDict::new(ValueInt::default(), ValueInt::default()).into()),
            key: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 是否有值(ID 1438)
pub struct NodeDictHasValue {
    dict: ValueIn,
    value: ValueIn,
}
impl INode for NodeDictHasValue {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.dict, &self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 1438 Has_Value")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 1438 Has_Value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(1438)
    }
}
impl Default for NodeDictHasValue {
    fn default() -> Self {
        Self {
            dict: ValueIn::new(ValueDict::new(ValueInt::default(), ValueInt::default()).into()),
            value: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 字典键列表(ID 1508)
pub struct NodeDictGetKeys {
    dict: ValueIn,
}
impl INode for NodeDictGetKeys {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.dict]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 1508 Get_Keys")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 1508 Get_Keys")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(1508)
    }
}
impl Default for NodeDictGetKeys {
    fn default() -> Self {
        Self { dict: ValueIn::new(ValueDict::new(ValueInt::default(), ValueInt::default()).into()) }
    }
}

/// 字典值列表(ID 1578)
pub struct NodeDictGetValues {
    dict: ValueIn,
}
impl INode for NodeDictGetValues {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.dict]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 1578 Get_Values")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 1578 Get_Values")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(1578)
    }
}
impl Default for NodeDictGetValues {
    fn default() -> Self {
        Self { dict: ValueIn::new(ValueDict::new(ValueInt::default(), ValueInt::default()).into()) }
    }
}

/// 字典长度(ID 1648)
pub struct NodeDictGetLength {
    dict: ValueIn,
}
impl INode for NodeDictGetLength {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.dict]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 1648 Get_Length")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 1648 Get_Length")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(1648)
    }
}
impl Default for NodeDictGetLength {
    fn default() -> Self {
        Self { dict: ValueIn::new(ValueDict::new(ValueInt::default(), ValueInt::default()).into()) }
    }
}
