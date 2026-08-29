//! 客户端域节点(Client,ID 200000+)
//!
//! 人工设计:纯值节点用 `value_node!`、操作节点用 `flow_node!` 宏消除样板;
//! 命名统一 `NodeClient` + 尾段,避免与 Server 节点冲突。
//! execute/get_value 仅模拟(todo!())。

use crate::asset::node_graph::{ControlOut, Node, NodeRef, Simulation, ValueIn};
use crate::asset::raw_node_graph::NodeType;
use crate::asset::value::{
    AnyValue, ValueBool, ValueConfig, ValueDefault, ValueEntity, ValueEntityList,
    ValueEnum, ValueEnumList, ValueFaction, ValueFloat, ValueGuid, ValueInt, ValueIntList,
    ValuePrefab, ValueString, ValueVector,
};
use anyhow::Result;

macro_rules! value_node {
    ($name:ident, $id:expr, $nm:literal, [$($vin:ident),*], [$($vout:expr),*]) => {
        impl Node for $name {
            fn get_controls_in(&self) -> i32 { 0 }
            fn get_controls_out(&self) -> Vec<ControlOut> { vec![] }
            fn get_values_in(&self) -> Vec<ValueIn> { vec![$( self.$vin.clone() ),*] }
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
        }
    };
}

// ========================================================================
// 端口 / 图入口
// ========================================================================

/// 图结束(布尔)(ID 200000)
pub struct NodeClientGraphEndBool {
    _unused: (),
}
impl Default for NodeClientGraphEndBool {
    fn default() -> Self {
        Self { _unused: () }
    }
}
value_node!(NodeClientGraphEndBool, 200000, "200000 Graph_End_Bool", [], [ValueBool::def()]);

// ========================================================================
// 客户端算术(布尔 / 数值)
// ========================================================================

/// 与(ID 200001)
pub struct NodeClientAnd {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientAnd {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueBool::def()),
            b: ValueIn::new(ValueBool::def()),
        }
    }
}
value_node!(NodeClientAnd, 200001, "200001 And", [a, b], [ValueBool::def()]);

/// 或(ID 200002)
pub struct NodeClientOr {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientOr {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueBool::def()),
            b: ValueIn::new(ValueBool::def()),
        }
    }
}
value_node!(NodeClientOr, 200002, "200002 Or", [a, b], [ValueBool::def()]);

/// 非(ID 200003)
pub struct NodeClientNot {
    value: ValueIn,
}
impl Default for NodeClientNot {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueBool::def()) }
    }
}
value_node!(NodeClientNot, 200003, "200003 Not", [value], [ValueBool::def()]);

/// 异或(ID 200004)
pub struct NodeClientXor {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientXor {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueBool::def()),
            b: ValueIn::new(ValueBool::def()),
        }
    }
}
value_node!(NodeClientXor, 200004, "200004 Xor", [a, b], [ValueBool::def()]);

/// 枚举匹配(ID 200005)
pub struct NodeClientEnumMatch {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientEnumMatch {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueEnum::def()),
            b: ValueIn::new(ValueEnum::def()),
        }
    }
}
value_node!(NodeClientEnumMatch, 200005, "200005 Enum_Match", [a, b], [ValueBool::def()]);

/// 相等(ID 200006)
pub struct NodeClientEqual {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientEqual {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}
value_node!(NodeClientEqual, 200006, "200006 Equal", [a, b], [ValueBool::def()]);

/// 大于(ID 200007)
pub struct NodeClientGreaterThan {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientGreaterThan {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}
value_node!(NodeClientGreaterThan, 200007, "200007 Greater_Than", [a, b], [ValueBool::def()]);

/// 小于(ID 200008)
pub struct NodeClientLessThan {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientLessThan {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}
value_node!(NodeClientLessThan, 200008, "200008 Less_Than", [a, b], [ValueBool::def()]);

/// 小于等于(ID 200009)
pub struct NodeClientLessEqual {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientLessEqual {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}
value_node!(NodeClientLessEqual, 200009, "200009 Less_Equal", [a, b], [ValueBool::def()]);

/// 大于等于(ID 200010)
pub struct NodeClientGreaterEqual {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientGreaterEqual {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}
value_node!(NodeClientGreaterEqual, 200010, "200010 Greater_Equal", [a, b], [ValueBool::def()]);

/// 加法(ID 200011)
pub struct NodeClientAdd {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientAdd {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}
value_node!(NodeClientAdd, 200011, "200011 Add", [a, b], [ValueFloat::def()]);

/// 减法(ID 200012)
pub struct NodeClientSubtract {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientSubtract {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}
value_node!(NodeClientSubtract, 200012, "200012 Subtract", [a, b], [ValueFloat::def()]);

/// 乘法(ID 200013)
pub struct NodeClientMultiply {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientMultiply {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}
value_node!(NodeClientMultiply, 200013, "200013 Multiply", [a, b], [ValueFloat::def()]);

/// 除法(ID 200014)
pub struct NodeClientDivide {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientDivide {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}
value_node!(NodeClientDivide, 200014, "200014 Divide", [a, b], [ValueFloat::def()]);

/// 绝对值(ID 200015)
pub struct NodeClientAbs {
    value: ValueIn,
}
impl Default for NodeClientAbs {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueFloat::def()) }
    }
}
value_node!(NodeClientAbs, 200015, "200015 Abs", [value], [ValueFloat::def()]);

/// 随机(ID 200032)
pub struct NodeClientRandom {
    min: ValueIn,
    max: ValueIn,
}
impl Default for NodeClientRandom {
    fn default() -> Self {
        Self {
            min: ValueIn::new(ValueFloat::def()),
            max: ValueIn::new(ValueFloat::def()),
        }
    }
}
value_node!(NodeClientRandom, 200032, "200032 Random", [min, max], [ValueFloat::def()]);

/// 类型转换(ID 200022)
pub struct NodeClientConvertType {
    value: ValueIn,
}
impl Default for NodeClientConvertType {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueInt::def()) }
    }
}
value_node!(NodeClientConvertType, 200022, "200022 Convert_Type", [value], [ValueInt::def()]);

// ========================================================================
// 客户端向量运算
// ========================================================================

/// 向量点积(ID 200063)
pub struct NodeClientVectorDot {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientVectorDot {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueVector::def()),
            b: ValueIn::new(ValueVector::def()),
        }
    }
}
value_node!(NodeClientVectorDot, 200063, "200063 Vector_Dot", [a, b], [ValueFloat::def()]);

/// 向量叉积(ID 200064)
pub struct NodeClientVectorCross {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientVectorCross {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueVector::def()),
            b: ValueIn::new(ValueVector::def()),
        }
    }
}
value_node!(NodeClientVectorCross, 200064, "200064 Vector_Cross", [a, b], [ValueVector::def()]);

/// 拆分向量(ID 200065)
pub struct NodeClientSplitVector {
    vector: ValueIn,
}
impl Default for NodeClientSplitVector {
    fn default() -> Self {
        Self { vector: ValueIn::new(ValueVector::def()) }
    }
}
value_node!(NodeClientSplitVector, 200065, "200065 Split_Vector", [vector], [ValueFloat::def(), ValueFloat::def(), ValueFloat::def()]);

/// 向量缩放(ID 200066)
pub struct NodeClientVectorScale {
    scale: ValueIn,
    vector: ValueIn,
}
impl Default for NodeClientVectorScale {
    fn default() -> Self {
        Self {
            scale: ValueIn::new(ValueFloat::def()),
            vector: ValueIn::new(ValueVector::def()),
        }
    }
}
value_node!(NodeClientVectorScale, 200066, "200066 Vector_Scale", [scale, vector], [ValueVector::def()]);

/// 向量夹角(ID 200067)
pub struct NodeClientVectorAngle {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientVectorAngle {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueVector::def()),
            b: ValueIn::new(ValueVector::def()),
        }
    }
}
value_node!(NodeClientVectorAngle, 200067, "200067 Vector_Angle", [a, b], [ValueFloat::def()]);

/// 向量旋转(ID 200068)
pub struct NodeClientVectorRotate {
    vector: ValueIn,
    rotation: ValueIn,
}
impl Default for NodeClientVectorRotate {
    fn default() -> Self {
        Self {
            vector: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
        }
    }
}
value_node!(NodeClientVectorRotate, 200068, "200068 Vector_Rotate", [vector, rotation], [ValueVector::def()]);

/// 向量长度(ID 200069)
pub struct NodeClientVectorLength {
    vector: ValueIn,
}
impl Default for NodeClientVectorLength {
    fn default() -> Self {
        Self { vector: ValueIn::new(ValueVector::def()) }
    }
}
value_node!(NodeClientVectorLength, 200069, "200069 Vector_Length", [vector], [ValueFloat::def()]);

/// 创建向量(ID 200070)
pub struct NodeClientCreateVector {
    x: ValueIn,
    y: ValueIn,
    z: ValueIn,
}
impl Default for NodeClientCreateVector {
    fn default() -> Self {
        Self {
            x: ValueIn::new(ValueFloat::def()),
            y: ValueIn::new(ValueFloat::def()),
            z: ValueIn::new(ValueFloat::def()),
        }
    }
}
value_node!(NodeClientCreateVector, 200070, "200070 Create_Vector", [x, y, z], [ValueVector::def()]);

/// 向量加法(ID 200071)
pub struct NodeClientVectorAdd {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientVectorAdd {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueVector::def()),
            b: ValueIn::new(ValueVector::def()),
        }
    }
}
value_node!(NodeClientVectorAdd, 200071, "200071 Vector_Add", [a, b], [ValueVector::def()]);

/// 向量减法(ID 200072)
pub struct NodeClientVectorSubtract {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientVectorSubtract {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueVector::def()),
            b: ValueIn::new(ValueVector::def()),
        }
    }
}
value_node!(NodeClientVectorSubtract, 200072, "200072 Vector_Subtract", [a, b], [ValueVector::def()]);

/// 向量转旋转(ID 200073)
pub struct NodeClientVectorToRotation {
    forward: ValueIn,
    up: ValueIn,
}
impl Default for NodeClientVectorToRotation {
    fn default() -> Self {
        Self {
            forward: ValueIn::new(ValueVector::def()),
            up: ValueIn::new(ValueVector::def()),
        }
    }
}
value_node!(NodeClientVectorToRotation, 200073, "200073 Vector_To_Rotation", [forward, up], [ValueVector::def()]);

/// 朝向转旋转(ID 200074)
pub struct NodeClientOrientationToRotation {
    orientation: ValueIn,
}
impl Default for NodeClientOrientationToRotation {
    fn default() -> Self {
        Self { orientation: ValueIn::new(ValueVector::def()) }
    }
}
value_node!(NodeClientOrientationToRotation, 200074, "200074 Orientation_To_Rotation", [orientation], [ValueVector::def()]);

/// 向量归一化(ID 200100)
pub struct NodeClientVectorNormalize {
    vector: ValueIn,
}
impl Default for NodeClientVectorNormalize {
    fn default() -> Self {
        Self { vector: ValueIn::new(ValueVector::def()) }
    }
}
value_node!(NodeClientVectorNormalize, 200100, "200100 Vector_Normalize", [vector], [ValueVector::def()]);

/// 正弦(ID 200094)
pub struct NodeClientSin {
    angle: ValueIn,
}
impl Default for NodeClientSin {
    fn default() -> Self {
        Self { angle: ValueIn::new(ValueFloat::def()) }
    }
}
value_node!(NodeClientSin, 200094, "200094 Sin", [angle], [ValueFloat::def()]);

/// 余弦(ID 200095)
pub struct NodeClientCos {
    angle: ValueIn,
}
impl Default for NodeClientCos {
    fn default() -> Self {
        Self { angle: ValueIn::new(ValueFloat::def()) }
    }
}
value_node!(NodeClientCos, 200095, "200095 Cos", [angle], [ValueFloat::def()]);

/// 正切(ID 200096)
pub struct NodeClientTan {
    angle: ValueIn,
}
impl Default for NodeClientTan {
    fn default() -> Self {
        Self { angle: ValueIn::new(ValueFloat::def()) }
    }
}
value_node!(NodeClientTan, 200096, "200096 Tan", [angle], [ValueFloat::def()]);

/// 反正弦(ID 200097)
pub struct NodeClientAsin {
    value: ValueIn,
}
impl Default for NodeClientAsin {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueFloat::def()) }
    }
}
value_node!(NodeClientAsin, 200097, "200097 Asin", [value], [ValueFloat::def()]);

/// 反余弦(ID 200098)
pub struct NodeClientAcos {
    value: ValueIn,
}
impl Default for NodeClientAcos {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueFloat::def()) }
    }
}
value_node!(NodeClientAcos, 200098, "200098 Acos", [value], [ValueFloat::def()]);

/// 反正切(ID 200099)
pub struct NodeClientAtan {
    value: ValueIn,
}
impl Default for NodeClientAtan {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueFloat::def()) }
    }
}
value_node!(NodeClientAtan, 200099, "200099 Atan", [value], [ValueFloat::def()]);

/// 弧度转角度(ID 200101)
pub struct NodeClientRadToDeg {
    radians: ValueIn,
}
impl Default for NodeClientRadToDeg {
    fn default() -> Self {
        Self { radians: ValueIn::new(ValueFloat::def()) }
    }
}
value_node!(NodeClientRadToDeg, 200101, "200101 Rad_To_Deg", [radians], [ValueFloat::def()]);

/// 角度转弧度(ID 200102)
pub struct NodeClientDegToRad {
    degrees: ValueIn,
}
impl Default for NodeClientDegToRad {
    fn default() -> Self {
        Self { degrees: ValueIn::new(ValueFloat::def()) }
    }
}
value_node!(NodeClientDegToRad, 200102, "200102 Deg_To_Rad", [degrees], [ValueFloat::def()]);

// ========================================================================
// 客户端查询
// ========================================================================

/// 变量查询(ID 200016)
pub struct NodeClientGetVariable {
    entity: ValueIn,
    name: ValueIn,
}
impl Default for NodeClientGetVariable {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            name: ValueIn::new(ValueString::def()),
        }
    }
}
value_node!(NodeClientGetVariable, 200016, "200016 Get_Variable", [entity, name], [ValueInt::def()]);

/// 按下标取元素(ID 200017)
pub struct NodeClientGetAtIndex {
    index: ValueIn,
    list: ValueIn,
}
impl Default for NodeClientGetAtIndex {
    fn default() -> Self {
        Self {
            index: ValueIn::new(ValueInt::def()),
            list: ValueIn::new(ValueIntList::def()),
        }
    }
}
value_node!(NodeClientGetAtIndex, 200017, "200017 Get_At_Index", [index, list], [ValueInt::def()]);

/// 列表长度(ID 200018)
pub struct NodeClientGetLength {
    list: ValueIn,
}
impl Default for NodeClientGetLength {
    fn default() -> Self {
        Self { list: ValueIn::new(ValueIntList::def()) }
    }
}
value_node!(NodeClientGetLength, 200018, "200018 Get_Length", [list], [ValueInt::def()]);

/// 包含(ID 200019)
pub struct NodeClientContains {
    item: ValueIn,
    list: ValueIn,
}
impl Default for NodeClientContains {
    fn default() -> Self {
        Self {
            item: ValueIn::new(ValueInt::def()),
            list: ValueIn::new(ValueIntList::def()),
        }
    }
}
value_node!(NodeClientContains, 200019, "200019 Contains", [item, list], [ValueBool::def()]);

/// 列表最大值(ID 200020)
pub struct NodeClientGetMax {
    list: ValueIn,
}
impl Default for NodeClientGetMax {
    fn default() -> Self {
        Self { list: ValueIn::new(ValueIntList::def()) }
    }
}
value_node!(NodeClientGetMax, 200020, "200020 Get_Max", [list], [ValueInt::def()]);

/// 列表最小值(ID 200021)
pub struct NodeClientGetMin {
    list: ValueIn,
}
impl Default for NodeClientGetMin {
    fn default() -> Self {
        Self { list: ValueIn::new(ValueIntList::def()) }
    }
}
value_node!(NodeClientGetMin, 200021, "200021 Get_Min", [list], [ValueInt::def()]);

/// 按 GUID 获取实体(ID 200023)
pub struct NodeClientGetByGuid {
    guid: ValueIn,
}
impl Default for NodeClientGetByGuid {
    fn default() -> Self {
        Self { guid: ValueIn::new(ValueGuid::def()) }
    }
}
value_node!(NodeClientGetByGuid, 200023, "200023 Get_By_GUID", [guid], [ValueEntity::def()]);

/// 玩家角色(ID 200024)
pub struct NodeClientGetPlayerCharacter {
    entity: ValueIn,
}
impl Default for NodeClientGetPlayerCharacter {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetPlayerCharacter, 200024, "200024 Get_Player_Character", [entity], [ValueEntity::def()]);

/// 所属玩家(ID 200025)
pub struct NodeClientGetOwnerPlayer {
    entity: ValueIn,
}
impl Default for NodeClientGetOwnerPlayer {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetOwnerPlayer, 200025, "200025 Get_Owner_Player", [entity], [ValueEntity::def()]);

/// 全部玩家(ID 200026)
pub struct NodeClientGetAllPlayers {
    _unused: (),
}
impl Default for NodeClientGetAllPlayers {
    fn default() -> Self {
        Self { _unused: () }
    }
}
value_node!(NodeClientGetAllPlayers, 200026, "200026 Get_All_Players", [], [ValueEntityList::def()]);

/// 实体 GUID(ID 200027)
pub struct NodeClientGetGuid {
    entity: ValueIn,
}
impl Default for NodeClientGetGuid {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetGuid, 200027, "200027 Get_GUID", [entity], [ValueGuid::def()]);

/// 状态值(ID 200028)
pub struct NodeClientGetStatus {
    entity: ValueIn,
    status: ValueIn,
}
impl Default for NodeClientGetStatus {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            status: ValueIn::new(ValueInt::def()),
        }
    }
}
value_node!(NodeClientGetStatus, 200028, "200028 Get_Status", [entity, status], [ValueInt::def()]);

/// 阵营(ID 200029)
pub struct NodeClientGetFaction {
    entity: ValueIn,
}
impl Default for NodeClientGetFaction {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetFaction, 200029, "200029 Get_Faction", [entity], [ValueFaction::def()]);

/// 位置(ID 200030)
pub struct NodeClientGetLocation {
    entity: ValueIn,
}
impl Default for NodeClientGetLocation {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetLocation, 200030, "200030 Get_Location", [entity], [ValueVector::def()]);

/// 旋转(ID 200031)
pub struct NodeClientGetRotation {
    entity: ValueIn,
}
impl Default for NodeClientGetRotation {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetRotation, 200031, "200031 Get_Rotation", [entity], [ValueVector::def()]);

/// 自身实体(ID 200033)
pub struct NodeClientGetSelf {
    _unused: (),
}
impl Default for NodeClientGetSelf {
    fn default() -> Self {
        Self { _unused: () }
    }
}
value_node!(NodeClientGetSelf, 200033, "200033 Get_Self", [], [ValueEntity::def()]);

/// 目标实体(ID 200034)
pub struct NodeClientGetTarget {
    _unused: (),
}
impl Default for NodeClientGetTarget {
    fn default() -> Self {
        Self { _unused: () }
    }
}
value_node!(NodeClientGetTarget, 200034, "200034 Get_Target", [], [ValueEntity::def()]);

/// 攻击目标(ID 200035)
pub struct NodeClientGetAttackTarget {
    entity: ValueIn,
}
impl Default for NodeClientGetAttackTarget {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetAttackTarget, 200035, "200035 Get_Attack_Target", [entity], [ValueEntity::def()]);

/// 相机模板(ID 200036)
pub struct NodeClientGetCameraTemplate {
    _unused: (),
}
impl Default for NodeClientGetCameraTemplate {
    fn default() -> Self {
        Self { _unused: () }
    }
}
value_node!(NodeClientGetCameraTemplate, 200036, "200036 Get_Camera_Template", [], [ValueInt::def()]);

/// 是否在战斗(ID 200037)
pub struct NodeClientIsInCombat {
    _unused: (),
}
impl Default for NodeClientIsInCombat {
    fn default() -> Self {
        Self { _unused: () }
    }
}
value_node!(NodeClientIsInCombat, 200037, "200037 Is_In_Combat", [], [ValueBool::def()]);

/// 球体过滤(ID 200043)
pub struct NodeClientFilterSphere {
    radius: ValueIn,
    center: ValueIn,
    max_count: ValueIn,
    filter: ValueIn,
}
impl Default for NodeClientFilterSphere {
    fn default() -> Self {
        Self {
            radius: ValueIn::new(ValueFloat::def()),
            center: ValueIn::new(ValueVector::def()),
            max_count: ValueIn::new(ValueInt::def()),
            filter: ValueIn::new(ValueEnum::def()),
        }
    }
}
value_node!(NodeClientFilterSphere, 200043, "200043 Filter_Sphere", [radius, center, max_count, filter], [ValueEntityList::def()]);

/// 方形过滤(ID 200044)
pub struct NodeClientFilterSquare {
    length: ValueIn,
    width: ValueIn,
    height: ValueIn,
    center: ValueIn,
    max_count: ValueIn,
    filter: ValueIn,
}
impl Default for NodeClientFilterSquare {
    fn default() -> Self {
        Self {
            length: ValueIn::new(ValueFloat::def()),
            width: ValueIn::new(ValueFloat::def()),
            height: ValueIn::new(ValueFloat::def()),
            center: ValueIn::new(ValueVector::def()),
            max_count: ValueIn::new(ValueInt::def()),
            filter: ValueIn::new(ValueEnum::def()),
        }
    }
}
value_node!(NodeClientFilterSquare, 200044, "200044 Filter_Square", [length, width, height, center, max_count, filter], [ValueEntityList::def()]);

/// 实体类型(ID 200045)
pub struct NodeClientGetEntityType {
    entity: ValueIn,
}
impl Default for NodeClientGetEntityType {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetEntityType, 200045, "200045 Get_Type", [entity], [ValueEnum::def()]);

/// 相机旋转(ID 200046)
pub struct NodeClientGetCameraRotation {
    _unused: (),
}
impl Default for NodeClientGetCameraRotation {
    fn default() -> Self {
        Self { _unused: () }
    }
}
value_node!(NodeClientGetCameraRotation, 200046, "200046 Get_Camera_Rotation", [], [ValueVector::def()]);

/// 挂点位置(ID 200047)
pub struct NodeClientGetSocketLoc {
    entity: ValueIn,
    socket: ValueIn,
}
impl Default for NodeClientGetSocketLoc {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            socket: ValueIn::new(ValueString::def()),
        }
    }
}
value_node!(NodeClientGetSocketLoc, 200047, "200047 Get_Socket_Loc", [entity, socket], [ValueVector::def()]);

/// 挂点旋转(ID 200048)
pub struct NodeClientGetSocketRot {
    entity: ValueIn,
    socket: ValueIn,
}
impl Default for NodeClientGetSocketRot {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            socket: ValueIn::new(ValueString::def()),
        }
    }
}
value_node!(NodeClientGetSocketRot, 200048, "200048 Get_Socket_Rot", [entity, socket], [ValueVector::def()]);

/// 当前角色(ID 200076)
pub struct NodeClientGetCurrentCharacter {
    _unused: (),
}
impl Default for NodeClientGetCurrentCharacter {
    fn default() -> Self {
        Self { _unused: () }
    }
}
value_node!(NodeClientGetCurrentCharacter, 200076, "200076 Get_Current_Character", [], [ValueEntity::def()]);

/// 标签列表(ID 200077)
pub struct NodeClientGetTags {
    entity: ValueIn,
}
impl Default for NodeClientGetTags {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetTags, 200077, "200077 Get_Tags", [entity], [ValueIntList::def()]);

/// 按标签获取实体(ID 200078)
pub struct NodeClientGetByTag {
    tag: ValueIn,
}
impl Default for NodeClientGetByTag {
    fn default() -> Self {
        Self { tag: ValueIn::new(ValueInt::def()) }
    }
}
value_node!(NodeClientGetByTag, 200078, "200078 Get_By_Tag", [tag], [ValueEntityList::def()]);

/// 局部变量(ID 200082)
pub struct NodeClientGetLocal {
    name: ValueIn,
}
impl Default for NodeClientGetLocal {
    fn default() -> Self {
        Self { name: ValueIn::new(ValueString::def()) }
    }
}
value_node!(NodeClientGetLocal, 200082, "200082 Get_Local", [name], [ValueInt::def()]);

/// 仇恨目标(ID 200090)
pub struct NodeClientGetAggroTarget {
    entity: ValueIn,
}
impl Default for NodeClientGetAggroTarget {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetAggroTarget, 200090, "200090 Get_Aggro_Target", [entity], [ValueEntity::def()]);

/// 仇恨列表(ID 200091)
pub struct NodeClientGetAggroList {
    entity: ValueIn,
}
impl Default for NodeClientGetAggroList {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetAggroList, 200091, "200091 Get_Aggro_List", [entity], [ValueEntityList::def()]);

/// 是否在战斗(仇恨)(ID 200092)
pub struct NodeClientAggroIsInCombat {
    entity: ValueIn,
}
impl Default for NodeClientAggroIsInCombat {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientAggroIsInCombat, 200092, "200092 Is_In_Combat", [entity], [ValueBool::def()]);

/// 敌对判断(ID 200093)
pub struct NodeClientIsHostile {
    a: ValueIn,
    b: ValueIn,
}
impl Default for NodeClientIsHostile {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFaction::def()),
            b: ValueIn::new(ValueFaction::def()),
        }
    }
}
value_node!(NodeClientIsHostile, 200093, "200093 Is_Hostile", [a, b], [ValueBool::def()]);

/// 是否激活(ID 200103)
pub struct NodeClientIsActive {
    entity: ValueIn,
}
impl Default for NodeClientIsActive {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientIsActive, 200103, "200103 Is_Active", [entity], [ValueBool::def()]);

/// 重叠实体(ID 200107)
pub struct NodeClientGetOverlappingEntities {
    entity: ValueIn,
    radius: ValueIn,
}
impl Default for NodeClientGetOverlappingEntities {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            radius: ValueIn::new(ValueInt::def()),
        }
    }
}
value_node!(NodeClientGetOverlappingEntities, 200107, "200107 Get_Overlapping_Entities", [entity, radius], [ValueEntityList::def()]);

/// 射线结果(ID 200109)
pub struct NodeClientGetRayResult {
    entity: ValueIn,
    origin: ValueIn,
    direction: ValueIn,
    distance: ValueIn,
    filter: ValueIn,
    targets: ValueIn,
}
impl Default for NodeClientGetRayResult {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            origin: ValueIn::new(ValueVector::def()),
            direction: ValueIn::new(ValueVector::def()),
            distance: ValueIn::new(ValueFloat::def()),
            filter: ValueIn::new(ValueEnum::def()),
            targets: ValueIn::new(ValueEnumList::def()),
        }
    }
}
value_node!(NodeClientGetRayResult, 200109, "200109 Get_Ray_Result", [entity, origin, direction, distance, filter, targets], [ValueVector::def(), ValueEntity::def()]);

/// 射线过滤列表(ID 200110)
pub struct NodeClientGetRayFilters {
    filter_a: ValueIn,
    filter_b: ValueIn,
    filter_c: ValueIn,
    filter_d: ValueIn,
}
impl Default for NodeClientGetRayFilters {
    fn default() -> Self {
        Self {
            filter_a: ValueIn::new(ValueEnum::def()),
            filter_b: ValueIn::new(ValueEnum::def()),
            filter_c: ValueIn::new(ValueEnum::def()),
            filter_d: ValueIn::new(ValueEnum::def()),
        }
    }
}
value_node!(NodeClientGetRayFilters, 200110, "200110 Get_Ray_Filters", [filter_a, filter_b, filter_c, filter_d], [ValueEnumList::def()]);

/// 扫描实体(ID 200118)
pub struct NodeClientGetScannedEntity {
    _unused: (),
}
impl Default for NodeClientGetScannedEntity {
    fn default() -> Self {
        Self { _unused: () }
    }
}
value_node!(NodeClientGetScannedEntity, 200118, "200118 Get_Scanned_Entity", [], [ValueEntity::def(), ValueConfig::def()]);

/// 可扫描实体列表(ID 200119)
pub struct NodeClientGetScannableEntities {
    _unused: (),
}
impl Default for NodeClientGetScannableEntities {
    fn default() -> Self {
        Self { _unused: () }
    }
}
value_node!(NodeClientGetScannableEntities, 200119, "200119 Get_Scannable_Entities", [], [ValueEntityList::def()]);

/// 扫描状态(ID 200120)
pub struct NodeClientGetScanStatus {
    entity: ValueIn,
}
impl Default for NodeClientGetScanStatus {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetScanStatus, 200120, "200120 Get_Scan_Status", [entity], [ValueEnum::def()]);

/// 激活扫描标签(ID 200121)
pub struct NodeClientGetActiveScanTags {
    entity: ValueIn,
}
impl Default for NodeClientGetActiveScanTags {
    fn default() -> Self {
        Self { entity: ValueIn::new(ValueEntity::def()) }
    }
}
value_node!(NodeClientGetActiveScanTags, 200121, "200121 Get_Active_Tags", [entity], [ValueConfig::def()]);

/// 输入方式(ID 200123)
pub struct NodeClientGetInputType {
    _unused: (),
}
impl Default for NodeClientGetInputType {
    fn default() -> Self {
        Self { _unused: () }
    }
}
value_node!(NodeClientGetInputType, 200123, "200123 Get_Input_Type", [], [ValueEnum::def()]);

// ========================================================================
// 客户端技能执行
// ========================================================================

/// 播放限时特效(ID 200038)
pub struct NodeClientPlayTimedFx {
    effect: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    duration: ValueIn,
    attach: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientPlayTimedFx {
    fn default() -> Self {
        Self {
            effect: ValueIn::new(ValueConfig::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            duration: ValueIn::new(ValueFloat::def()),
            attach: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientPlayTimedFx, 200038, "200038 Play_Timed_FX", [effect, position, rotation, duration, attach], []);

/// 通知服务器(ID 200039)
pub struct NodeClientNotifyServer {
    event: ValueIn,
    param_a: ValueIn,
    param_b: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientNotifyServer {
    fn default() -> Self {
        Self {
            event: ValueIn::new(ValueString::def()),
            param_a: ValueIn::new(ValueString::def()),
            param_b: ValueIn::new(ValueString::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientNotifyServer, 200039, "200039 Notify_Server", [event, param_a, param_b], []);

/// 角色转身(ID 200040)
pub struct NodeClientTurnPlayer {
    turn_type: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTurnPlayer {
    fn default() -> Self {
        Self {
            turn_type: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTurnPlayer, 200040, "200040 Turn_Player", [turn_type], []);

/// 设置目标(ID 200041)
pub struct NodeClientSetTarget {
    target: ValueIn,
    lock: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientSetTarget {
    fn default() -> Self {
        Self {
            target: ValueIn::new(ValueEntity::def()),
            lock: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientSetTarget, 200041, "200041 Set_Target", [target, lock], []);

/// 触发判定盒(挂点)(ID 200051)
pub struct NodeClientTriggerHitboxLoc {
    hitbox: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    length: ValueIn,
    width: ValueIn,
    targets: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTriggerHitboxLoc {
    fn default() -> Self {
        Self {
            hitbox: ValueIn::new(ValueEnum::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            length: ValueIn::new(ValueFloat::def()),
            width: ValueIn::new(ValueFloat::def()),
            targets: ValueIn::new(ValueEnumList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTriggerHitboxLoc, 200051, "200051 Trigger_Hitbox_Loc", [hitbox, position, rotation, length, width, targets], []);

/// 发射投射物(ID 200052)
pub struct NodeClientLaunchProjectile {
    prefab: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    owner: ValueIn,
    faction: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientLaunchProjectile {
    fn default() -> Self {
        Self {
            prefab: ValueIn::new(ValuePrefab::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            owner: ValueIn::new(ValueEntity::def()),
            faction: ValueIn::new(ValueFaction::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientLaunchProjectile, 200052, "200052 Launch_Projectile", [prefab, position, rotation, owner, faction], []);

/// 移动到点(ID 200053)
pub struct NodeClientMoveToPoint {
    speed: ValueIn,
    stop_distance: ValueIn,
    turn_speed: ValueIn,
    position: ValueIn,
    face_target: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientMoveToPoint {
    fn default() -> Self {
        Self {
            speed: ValueIn::new(ValueFloat::def()),
            stop_distance: ValueIn::new(ValueFloat::def()),
            turn_speed: ValueIn::new(ValueFloat::def()),
            position: ValueIn::new(ValueVector::def()),
            face_target: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientMoveToPoint, 200053, "200053 Move_To_Point", [speed, stop_distance, turn_speed, position, face_target], []);

/// 添加状态(ID 200057)
pub struct NodeClientAddStatus {
    entity: ValueIn,
    stacks: ValueIn,
    status: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientAddStatus {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            stacks: ValueIn::new(ValueInt::def()),
            status: ValueIn::new(ValueConfig::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientAddStatus, 200057, "200057 Add_Status", [entity, stacks, status], []);

/// 移除状态(ID 200058)
pub struct NodeClientRemoveStatus {
    entity: ValueIn,
    status: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientRemoveStatus {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            status: ValueIn::new(ValueConfig::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientRemoveStatus, 200058, "200058 Remove_Status", [entity, status], []);

/// 触发判定盒(挂点槽)(ID 200059)
pub struct NodeClientTriggerHitboxSocket {
    hitbox: ValueIn,
    socket: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    length: ValueIn,
    width: ValueIn,
    targets: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTriggerHitboxSocket {
    fn default() -> Self {
        Self {
            hitbox: ValueIn::new(ValueEnum::def()),
            socket: ValueIn::new(ValueString::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            length: ValueIn::new(ValueFloat::def()),
            width: ValueIn::new(ValueFloat::def()),
            targets: ValueIn::new(ValueEnumList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTriggerHitboxSocket, 200059, "200059 Trigger_Hitbox_Socket", [hitbox, socket, position, rotation, length, width, targets], []);

/// 移除设备(ID 200060)
pub struct NodeClientRemoveDevice {
    device: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientRemoveDevice {
    fn default() -> Self {
        Self {
            device: ValueIn::new(ValueEnum::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientRemoveDevice, 200060, "200060 Remove_Device", [device], []);

/// 修改权重(ID 200061)
pub struct NodeClientModifyWeight {
    weight: ValueIn,
    lock: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientModifyWeight {
    fn default() -> Self {
        Self {
            weight: ValueIn::new(ValueFloat::def()),
            lock: ValueIn::new(ValueBool::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientModifyWeight, 200061, "200061 Modify_Weight", [weight, lock], []);

/// 相机数据(ID 200062):输出位置 + 旋转
pub struct NodeClientGetCameraData {
    camera: ValueIn,
    position: ValueIn,
    fov: ValueIn,
    aspect: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientGetCameraData {
    fn default() -> Self {
        Self {
            camera: ValueIn::new(ValueEnum::def()),
            position: ValueIn::new(ValueVector::def()),
            fov: ValueIn::new(ValueFloat::def()),
            aspect: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientGetCameraData, 200062, "200062 Get_Camera_Data", [camera, position, fov, aspect], [ValueVector::def(), ValueVector::def()]);

/// 恢复生命(ID 200075)
pub struct NodeClientRecoverHp {
    entity: ValueIn,
    amount: ValueIn,
    is_critical: ValueIn,
    ratio: ValueIn,
    stacks: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientRecoverHp {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            amount: ValueIn::new(ValueFloat::def()),
            is_critical: ValueIn::new(ValueBool::def()),
            ratio: ValueIn::new(ValueFloat::def()),
            stacks: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientRecoverHp, 200075, "200075 Recover_HP", [entity, amount, is_critical, ratio, stacks], []);

/// 面向目标(ID 200105)
pub struct NodeClientTurnToFace {
    position: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTurnToFace {
    fn default() -> Self {
        Self {
            position: ValueIn::new(ValueVector::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTurnToFace, 200105, "200105 Turn_To_Face", [position], []);

/// 重置目标(ID 200106)
pub struct NodeClientResetTarget {
    next: ControlOut,
}
impl Default for NodeClientResetTarget {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
flow_node!(NodeClientResetTarget, 200106, "200106 Reset_Target", [], []);

/// 退出瞄准(ID 200108)
pub struct NodeClientExitAiming {
    next: ControlOut,
}
impl Default for NodeClientExitAiming {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
flow_node!(NodeClientExitAiming, 200108, "200108 Exit_Aiming", [], []);

/// 触发球形判定盒(ID 200111)
pub struct NodeClientTriggerSphereHitboxLoc {
    hitbox: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    radius: ValueIn,
    height: ValueIn,
    targets: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTriggerSphereHitboxLoc {
    fn default() -> Self {
        Self {
            hitbox: ValueIn::new(ValueEnum::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            radius: ValueIn::new(ValueFloat::def()),
            height: ValueIn::new(ValueFloat::def()),
            targets: ValueIn::new(ValueEnumList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTriggerSphereHitboxLoc, 200111, "200111 Trigger_Sphere_Hitbox_Loc", [hitbox, position, rotation, radius, height, targets], []);

/// 触发矩形判定盒(ID 200112)
pub struct NodeClientTriggerRectHitboxLoc {
    hitbox: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    length: ValueIn,
    width: ValueIn,
    targets: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTriggerRectHitboxLoc {
    fn default() -> Self {
        Self {
            hitbox: ValueIn::new(ValueEnum::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            length: ValueIn::new(ValueFloat::def()),
            width: ValueIn::new(ValueFloat::def()),
            targets: ValueIn::new(ValueEnumList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTriggerRectHitboxLoc, 200112, "200112 Trigger_Rect_Hitbox_Loc", [hitbox, position, rotation, length, width, targets], []);

/// 触发扇形判定盒(ID 200113)
pub struct NodeClientTriggerSectorHitboxLoc {
    hitbox: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    angle: ValueIn,
    radius: ValueIn,
    targets: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTriggerSectorHitboxLoc {
    fn default() -> Self {
        Self {
            hitbox: ValueIn::new(ValueEnum::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            angle: ValueIn::new(ValueFloat::def()),
            radius: ValueIn::new(ValueFloat::def()),
            targets: ValueIn::new(ValueEnumList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTriggerSectorHitboxLoc, 200113, "200113 Trigger_Sector_Hitbox_Loc", [hitbox, position, rotation, angle, radius, targets], []);

/// 触发球形判定盒(挂点槽)(ID 200114)
pub struct NodeClientTriggerSphereHitboxSocket {
    hitbox: ValueIn,
    socket: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    radius: ValueIn,
    height: ValueIn,
    targets: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTriggerSphereHitboxSocket {
    fn default() -> Self {
        Self {
            hitbox: ValueIn::new(ValueEnum::def()),
            socket: ValueIn::new(ValueString::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            radius: ValueIn::new(ValueFloat::def()),
            height: ValueIn::new(ValueFloat::def()),
            targets: ValueIn::new(ValueEnumList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTriggerSphereHitboxSocket, 200114, "200114 Trigger_Sphere_Hitbox_Socket", [hitbox, socket, position, rotation, radius, height, targets], []);

/// 触发矩形判定盒(挂点槽)(ID 200115)
pub struct NodeClientTriggerRectHitboxSocket {
    hitbox: ValueIn,
    socket: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    length: ValueIn,
    width: ValueIn,
    targets: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTriggerRectHitboxSocket {
    fn default() -> Self {
        Self {
            hitbox: ValueIn::new(ValueEnum::def()),
            socket: ValueIn::new(ValueString::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            length: ValueIn::new(ValueFloat::def()),
            width: ValueIn::new(ValueFloat::def()),
            targets: ValueIn::new(ValueEnumList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTriggerRectHitboxSocket, 200115, "200115 Trigger_Rect_Hitbox_Socket", [hitbox, socket, position, rotation, length, width, targets], []);

/// 触发扇形判定盒(挂点槽)(ID 200116)
pub struct NodeClientTriggerSectorHitboxSocket {
    hitbox: ValueIn,
    socket: ValueIn,
    position: ValueIn,
    rotation: ValueIn,
    angle: ValueIn,
    radius: ValueIn,
    targets: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTriggerSectorHitboxSocket {
    fn default() -> Self {
        Self {
            hitbox: ValueIn::new(ValueEnum::def()),
            socket: ValueIn::new(ValueString::def()),
            position: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
            angle: ValueIn::new(ValueFloat::def()),
            radius: ValueIn::new(ValueFloat::def()),
            targets: ValueIn::new(ValueEnumList::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTriggerSectorHitboxSocket, 200116, "200116 Trigger_Sector_Hitbox_Socket", [hitbox, socket, position, rotation, angle, radius, targets], []);

/// 向服务器发送信号(ID 200124)
pub struct NodeClientSendToServer {
    next: ControlOut,
}
impl Default for NodeClientSendToServer {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
flow_node!(NodeClientSendToServer, 200124, "200124 Send_To_Server", [], []);

// ========================================================================
// 客户端控制流 / 变量 / 仇恨
// ========================================================================

/// 写局部变量(ID 200081)
pub struct NodeClientSetLocal {
    name: ValueIn,
    value: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientSetLocal {
    fn default() -> Self {
        Self {
            name: ValueIn::new(ValueString::def()),
            value: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientSetLocal, 200081, "200081 Set_Local", [name, value], []);

/// 设置仇恨(ID 200083)
pub struct NodeClientSetAggro {
    entity: ValueIn,
    target: ValueIn,
    value: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientSetAggro {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            target: ValueIn::new(ValueEntity::def()),
            value: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientSetAggro, 200083, "200083 Set_Aggro", [entity, target, value], []);

/// 修改仇恨(ID 200084)
pub struct NodeClientModifyAggro {
    entity: ValueIn,
    target: ValueIn,
    value: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientModifyAggro {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            target: ValueIn::new(ValueEntity::def()),
            value: ValueIn::new(ValueInt::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientModifyAggro, 200084, "200084 Modify_Aggro", [entity, target, value], []);

/// 修改仇恨比例(ID 200085)
pub struct NodeClientModifyAggroRatio {
    entity: ValueIn,
    target: ValueIn,
    ratio: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientModifyAggroRatio {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            target: ValueIn::new(ValueEntity::def()),
            ratio: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientModifyAggroRatio, 200085, "200085 Modify_Aggro_Ratio", [entity, target, ratio], []);

/// 转移仇恨(ID 200086)
pub struct NodeClientTransferAggro {
    entity: ValueIn,
    from: ValueIn,
    to: ValueIn,
    ratio: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTransferAggro {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            from: ValueIn::new(ValueEntity::def()),
            to: ValueIn::new(ValueEntity::def()),
            ratio: ValueIn::new(ValueFloat::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTransferAggro, 200086, "200086 Transfer_Aggro", [entity, from, to, ratio], []);

/// 清空仇恨(ID 200087)
pub struct NodeClientClearAggro {
    entity: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientClearAggro {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientClearAggro, 200087, "200087 Clear_Aggro", [entity], []);

/// 移除仇恨(ID 200088)
pub struct NodeClientRemoveAggro {
    entity: ValueIn,
    target: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientRemoveAggro {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            target: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientRemoveAggro, 200088, "200088 Remove_Aggro", [entity, target], []);

/// 嘲讽(ID 200089)
pub struct NodeClientTaunt {
    entity: ValueIn,
    target: ValueIn,
    next: ControlOut,
}
impl Default for NodeClientTaunt {
    fn default() -> Self {
        Self {
            entity: ValueIn::new(ValueEntity::def()),
            target: ValueIn::new(ValueEntity::def()),
            next: vec![],
        }
    }
}
flow_node!(NodeClientTaunt, 200089, "200089 Taunt", [entity, target], []);

// ========================================================================
// 客户端特殊节点
// ========================================================================

/// 图开始(ID 200042):入口节点
pub struct NodeClientGraphStart {
    next: ControlOut,
}
impl Default for NodeClientGraphStart {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
impl Node for NodeClientGraphStart {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.next.clone()]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 200042 Graph_Start execute")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 200042 Graph_Start get_value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(200042)
    }
}

/// 图结束(ID 200104):输出整数 + 过滤
pub struct NodeClientGraphEnd {
    value: ValueIn,
    filter: ValueIn,
}
impl Default for NodeClientGraphEnd {
    fn default() -> Self {
        Self {
            value: ValueIn::new(ValueInt::def()),
            filter: ValueIn::new(ValueEnum::def()),
        }
    }
}
impl Node for NodeClientGraphEnd {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone(), self.filter.clone()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 200104 Graph_End execute")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 200104 Graph_End get_value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(200104)
    }
}

/// 测试发送信号(ID 200117)
pub struct NodeClientTestSendSignal {
    event: ValueIn,
    entity: ValueIn,
    entities: ValueIn,
    count: ValueIn,
    ints: ValueIn,
    flag: ValueIn,
}
impl Default for NodeClientTestSendSignal {
    fn default() -> Self {
        Self {
            event: ValueIn::new(ValueString::def()),
            entity: ValueIn::new(ValueEntity::def()),
            entities: ValueIn::new(ValueEntityList::def()),
            count: ValueIn::new(ValueInt::def()),
            ints: ValueIn::new(ValueIntList::def()),
            flag: ValueIn::new(ValueBool::def()),
        }
    }
}
impl Node for NodeClientTestSendSignal {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.event.clone(), self.entity.clone(), self.entities.clone(), self.count.clone(), self.ints.clone(), self.flag.clone()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 200117 Test_Send_Signal execute")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 200117 Test_Send_Signal get_value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(200117)
    }
}

/// 实体类型列表(ID 200050)
pub struct NodeClientGetEntityTypes {
    type_a: ValueIn,
    type_b: ValueIn,
    type_c: ValueIn,
    type_d: ValueIn,
}
impl Default for NodeClientGetEntityTypes {
    fn default() -> Self {
        Self {
            type_a: ValueIn::new(ValueEnum::def()),
            type_b: ValueIn::new(ValueEnum::def()),
            type_c: ValueIn::new(ValueEnum::def()),
            type_d: ValueIn::new(ValueEnum::def()),
        }
    }
}
value_node!(NodeClientGetEntityTypes, 200050, "200050 Get_Entity_Types", [type_a, type_b, type_c, type_d], [ValueEnumList::def()]);

/// 组装列表(客户端)(ID 200049):元素动态
pub struct NodeClientAssembleList {
    items: Vec<ValueIn>,
}
impl Default for NodeClientAssembleList {
    fn default() -> Self {
        Self { items: vec![] }
    }
}
impl NodeClientAssembleList {
    pub fn add_item(&mut self, item: ValueIn) {
        self.items.push(item);
    }
}
impl Node for NodeClientAssembleList {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        self.items.clone()
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 200049 Assemble_List execute")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 200049 Assemble_List get_value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(200049)
    }
}

/// 遍历实体(ID 200055):1 flow 入,2 flow 出(元素/完成),输出当前实体
pub struct NodeClientForEachEntity {
    entities: ValueIn,
    body: ControlOut,
    done: ControlOut,
}
impl Default for NodeClientForEachEntity {
    fn default() -> Self {
        Self {
            entities: ValueIn::new(ValueEntityList::def()),
            body: vec![],
            done: vec![],
        }
    }
}
impl Node for NodeClientForEachEntity {
    fn get_controls_in(&self) -> i32 {
        1
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.body.clone(), self.done.clone()]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.entities.clone()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueEntity::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 200055 For_Each_Entity execute")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 200055 For_Each_Entity get_value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(200055)
    }
}

/// 分支(ID 200056):1 flow 入,2 flow 出(真/假)
pub struct NodeClientBranch {
    condition: ValueIn,
    branch_true: ControlOut,
    branch_false: ControlOut,
}
impl Default for NodeClientBranch {
    fn default() -> Self {
        Self {
            condition: ValueIn::new(ValueBool::def()),
            branch_true: vec![],
            branch_false: vec![],
        }
    }
}
impl Node for NodeClientBranch {
    fn get_controls_in(&self) -> i32 {
        1
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.branch_true.clone(), self.branch_false.clone()]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.condition.clone()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 200056 Branch execute")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 200056 Branch get_value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(200056)
    }
}

/// 有限循环(客户端)(ID 200079):2 flow 入,2 flow 出,输出当前索引
pub struct NodeClientForLoop {
    begin: ValueIn,
    end: ValueIn,
    body: ControlOut,
    done: ControlOut,
}
impl Default for NodeClientForLoop {
    fn default() -> Self {
        Self {
            begin: ValueIn::new(ValueInt::def()),
            end: ValueIn::new(ValueInt::def()),
            body: vec![],
            done: vec![],
        }
    }
}
impl Node for NodeClientForLoop {
    fn get_controls_in(&self) -> i32 {
        2
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.body.clone(), self.done.clone()]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.begin.clone(), self.end.clone()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 200079 For_Loop execute")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 200079 For_Loop get_value")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(200079)
    }
}

/// 中断循环(ID 200080)
pub struct NodeClientBreak {
    next: ControlOut,
}
impl Default for NodeClientBreak {
    fn default() -> Self {
        Self { next: vec![] }
    }
}
flow_node!(NodeClientBreak, 200080, "200080 Break", [], []);

/// 图结束(整数)(ID 200122)
pub struct NodeClientGraphEndInt {
    value: ValueIn,
}
impl Default for NodeClientGraphEndInt {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueInt::def()) }
    }
}
impl Node for NodeClientGraphEndInt {
    fn get_controls_in(&self) -> i32 { 0 }
    fn get_controls_out(&self) -> Vec<ControlOut> { vec![] }
    fn get_values_in(&self) -> Vec<ValueIn> { vec![self.value.clone()] }
    fn get_values_out(&self) -> Vec<AnyValue> { vec![] }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 200122 Graph_End_Int execute")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 200122 Graph_End_Int get_value")
    }
    fn get_type(&self) -> NodeType { NodeType::simple(200122) }
}

