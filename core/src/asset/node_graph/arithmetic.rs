//! 算术域节点(Server,Arithmetic)
//!
//! 人工设计,替换自动生成版本:
//! - 引脚按语义命名(a/b/vector/scale/x/y/z 等),不穷举 out_0/out_1
//! - 动态数量结构用 `Vec`(Assemble_List / Assemble_Dictionary 的参数不穷举字段)
//! - 泛型 `R<T>` 数值按 Float 语义;execute/get_value 仅模拟(todo!())

use crate::asset::node_graph::{ControlOut, INode, NodeRef, Simulation, ValueIn};
use crate::asset::raw_node_graph::NodeType;
use crate::asset::value::{
    AnyValue, ValueBool, ValueDefault, ValueDict, ValueEnum, ValueFloat, ValueInt, ValueIntList,
    ValueStruct, ValueVector,
};
use anyhow::Result;

// ========================================================================
// 向量运算
// ========================================================================

/// 拆分向量为分量(Arithmetic.Math.Split_Vector,ID 9):Vec → x/y/z
pub struct NodeSplitVector {
    vector: ValueIn,
}
impl INode for NodeSplitVector {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.vector]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def(), ValueFloat::def(), ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 9 Split_Vector")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 9 Split_Vector")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(9)
    }
}
impl Default for NodeSplitVector {
    fn default() -> Self {
        Self { vector: ValueIn::new(ValueVector::def()) }
    }
}

/// 向量加法(ID 10):a + b
pub struct NodeVectorAdd {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeVectorAdd {
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
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 10 Vector_Add")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 10 Vector_Add")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(10)
    }
}
impl Default for NodeVectorAdd {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueVector::def()),
            b: ValueIn::new(ValueVector::def()),
        }
    }
}

/// 向量减法(ID 11):a - b
pub struct NodeVectorSubtract {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeVectorSubtract {
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
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 11 Vector_Subtract")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 11 Vector_Subtract")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(11)
    }
}
impl Default for NodeVectorSubtract {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueVector::def()),
            b: ValueIn::new(ValueVector::def()),
        }
    }
}

/// 向量缩放(ID 12):vector * scale
pub struct NodeVectorScale {
    vector: ValueIn,
    scale: ValueIn,
}
impl INode for NodeVectorScale {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.vector, &self.scale]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 12 Vector_Scale")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 12 Vector_Scale")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(12)
    }
}
impl Default for NodeVectorScale {
    fn default() -> Self {
        Self {
            vector: ValueIn::new(ValueVector::def()),
            scale: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 向量夹角(ID 13):a 与 b 的夹角(度)
pub struct NodeVectorAngle {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeVectorAngle {
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
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 13 Vector_Angle")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 13 Vector_Angle")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(13)
    }
}
impl Default for NodeVectorAngle {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueVector::def()),
            b: ValueIn::new(ValueVector::def()),
        }
    }
}

/// 向量归一化(ID 74):长度归一为 1
pub struct NodeVectorNormalize {
    vector: ValueIn,
}
impl INode for NodeVectorNormalize {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.vector]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 74 Vector_Normalize")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 74 Vector_Normalize")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(74)
    }
}
impl Default for NodeVectorNormalize {
    fn default() -> Self {
        Self { vector: ValueIn::new(ValueVector::def()) }
    }
}

/// 向量长度(ID 220):模长
pub struct NodeVectorLength {
    vector: ValueIn,
}
impl INode for NodeVectorLength {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.vector]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 220 Vector_Length")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 220 Vector_Length")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(220)
    }
}
impl Default for NodeVectorLength {
    fn default() -> Self {
        Self { vector: ValueIn::new(ValueVector::def()) }
    }
}

/// 两点距离(ID 244):a 与 b 的距离
pub struct NodeDistance {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeDistance {
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
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 244 Distance")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 244 Distance")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(244)
    }
}
impl Default for NodeDistance {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueVector::def()),
            b: ValueIn::new(ValueVector::def()),
        }
    }
}

/// 向量旋转(ID 474):按旋转量旋转
pub struct NodeVectorRotate {
    vector: ValueIn,
    rotation: ValueIn,
}
impl INode for NodeVectorRotate {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.vector, &self.rotation]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 474 Vector_Rotate")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 474 Vector_Rotate")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(474)
    }
}
impl Default for NodeVectorRotate {
    fn default() -> Self {
        Self {
            vector: ValueIn::new(ValueVector::def()),
            rotation: ValueIn::new(ValueVector::def()),
        }
    }
}

/// 向量点积(ID 505)
pub struct NodeVectorDot {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeVectorDot {
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
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 505 Vector_Dot")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 505 Vector_Dot")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(505)
    }
}
impl Default for NodeVectorDot {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueVector::def()),
            b: ValueIn::new(ValueVector::def()),
        }
    }
}

/// 向量叉积(ID 506)
pub struct NodeVectorCross {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeVectorCross {
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
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 506 Vector_Cross")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 506 Vector_Cross")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(506)
    }
}
impl Default for NodeVectorCross {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueVector::def()),
            b: ValueIn::new(ValueVector::def()),
        }
    }
}

/// 向量转旋转(ID 519):由前向/上向量构造旋转
pub struct NodeVectorToRotation {
    forward: ValueIn,
    up: ValueIn,
}
impl INode for NodeVectorToRotation {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.forward, &self.up]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 519 Vector_To_Rotation")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 519 Vector_To_Rotation")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(519)
    }
}
impl Default for NodeVectorToRotation {
    fn default() -> Self {
        Self {
            forward: ValueIn::new(ValueVector::def()),
            up: ValueIn::new(ValueVector::def()),
        }
    }
}

/// 创建向量(ID 225):x/y/z 分量
pub struct NodeCreateVector {
    x: ValueIn,
    y: ValueIn,
    z: ValueIn,
}
impl INode for NodeCreateVector {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.x, &self.y, &self.z]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueVector::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 225 Create_Vector")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 225 Create_Vector")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(225)
    }
}
impl Default for NodeCreateVector {
    fn default() -> Self {
        Self {
            x: ValueIn::new(ValueFloat::def()),
            y: ValueIn::new(ValueFloat::def()),
            z: ValueIn::new(ValueFloat::def()),
        }
    }
}

// ========================================================================
// 数值二元运算(泛型 R<T>,按 Float 语义)
// ========================================================================

/// 加法(ID 200)
pub struct NodeAdd {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeAdd {
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
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 200 Add")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 200 Add")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(200)
    }
}
impl Default for NodeAdd {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 减法(ID 202)
pub struct NodeSubtract {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeSubtract {
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
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 202 Subtract")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 202 Subtract")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(202)
    }
}
impl Default for NodeSubtract {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 乘法(ID 204)
pub struct NodeMultiply {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeMultiply {
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
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 204 Multiply")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 204 Multiply")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(204)
    }
}
impl Default for NodeMultiply {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 除法(ID 206)
pub struct NodeDivide {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeDivide {
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
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 206 Divide")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 206 Divide")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(206)
    }
}
impl Default for NodeDivide {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 幂运算(ID 209)
pub struct NodePower {
    base: ValueIn,
    exponent: ValueIn,
}
impl INode for NodePower {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.base, &self.exponent]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 209 Power")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 209 Power")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(209)
    }
}
impl Default for NodePower {
    fn default() -> Self {
        Self {
            base: ValueIn::new(ValueFloat::def()),
            exponent: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 取大值(ID 211)
pub struct NodeMax {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeMax {
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
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 211 Max")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 211 Max")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(211)
    }
}
impl Default for NodeMax {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 取小值(ID 213)
pub struct NodeMin {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeMin {
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
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 213 Min")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 213 Min")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(213)
    }
}
impl Default for NodeMin {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 取余(ID 208):整数取模
pub struct NodeModulo {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeModulo {
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
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 208 Modulo")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 208 Modulo")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(208)
    }
}
impl Default for NodeModulo {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueInt::def()),
            b: ValueIn::new(ValueInt::def()),
        }
    }
}

// ========================================================================
// 一元运算与夹取
// ========================================================================

/// 绝对值(ID 216)
pub struct NodeAbs {
    value: ValueIn,
}
impl INode for NodeAbs {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 216 Abs")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 216 Abs")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(216)
    }
}
impl Default for NodeAbs {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueFloat::def()) }
    }
}

/// 取符号(ID 218):-1 / 0 / 1
pub struct NodeSign {
    value: ValueIn,
}
impl INode for NodeSign {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 218 Sign")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 218 Sign")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(218)
    }
}
impl Default for NodeSign {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueFloat::def()) }
    }
}

/// 夹取(ID 222):value 限制在 [min, max]
pub struct NodeClamp {
    value: ValueIn,
    min: ValueIn,
    max: ValueIn,
}
impl INode for NodeClamp {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value, &self.min, &self.max]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 222 Clamp")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 222 Clamp")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(222)
    }
}
impl Default for NodeClamp {
    fn default() -> Self {
        Self {
            value: ValueIn::new(ValueFloat::def()),
            min: ValueIn::new(ValueFloat::def()),
            max: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 四舍五入(ID 224):value 按舍入模式取整
pub struct NodeRound {
    value: ValueIn,
    mode: ValueIn,
}
impl INode for NodeRound {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value, &self.mode]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 224 Round")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 224 Round")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(224)
    }
}
impl Default for NodeRound {
    fn default() -> Self {
        Self {
            value: ValueIn::new(ValueFloat::def()),
            mode: ValueIn::new(ValueEnum::def()),
        }
    }
}

/// 平方根(ID 221)
pub struct NodeSqrt {
    value: ValueIn,
}
impl INode for NodeSqrt {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 221 Sqrt")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 221 Sqrt")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(221)
    }
}
impl Default for NodeSqrt {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueFloat::def()) }
    }
}

/// 对数(ID 215):log_base(value)
pub struct NodeLogarithm {
    base: ValueIn,
    value: ValueIn,
}
impl INode for NodeLogarithm {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.base, &self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 215 Log")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 215 Log")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(215)
    }
}
impl Default for NodeLogarithm {
    fn default() -> Self {
        Self {
            base: ValueIn::new(ValueFloat::def()),
            value: ValueIn::new(ValueFloat::def()),
        }
    }
}

// ========================================================================
// 三角函数
// ========================================================================

/// 正弦(ID 291)
pub struct NodeSin {
    angle: ValueIn,
}
impl INode for NodeSin {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.angle]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 291 Sin")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 291 Sin")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(291)
    }
}
impl Default for NodeSin {
    fn default() -> Self {
        Self { angle: ValueIn::new(ValueFloat::def()) }
    }
}

/// 余弦(ID 292)
pub struct NodeCos {
    angle: ValueIn,
}
impl INode for NodeCos {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.angle]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 292 Cos")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 292 Cos")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(292)
    }
}
impl Default for NodeCos {
    fn default() -> Self {
        Self { angle: ValueIn::new(ValueFloat::def()) }
    }
}

/// 正切(ID 293)
pub struct NodeTan {
    angle: ValueIn,
}
impl INode for NodeTan {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.angle]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 293 Tan")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 293 Tan")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(293)
    }
}
impl Default for NodeTan {
    fn default() -> Self {
        Self { angle: ValueIn::new(ValueFloat::def()) }
    }
}

/// 反正弦(ID 294)
pub struct NodeAsin {
    value: ValueIn,
}
impl INode for NodeAsin {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 294 Asin")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 294 Asin")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(294)
    }
}
impl Default for NodeAsin {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueFloat::def()) }
    }
}

/// 反余弦(ID 295)
pub struct NodeAcos {
    value: ValueIn,
}
impl INode for NodeAcos {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 295 Acos")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 295 Acos")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(295)
    }
}
impl Default for NodeAcos {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueFloat::def()) }
    }
}

/// 反正切(ID 296)
pub struct NodeAtan {
    value: ValueIn,
}
impl INode for NodeAtan {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 296 Atan")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 296 Atan")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(296)
    }
}
impl Default for NodeAtan {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueFloat::def()) }
    }
}

/// 弧度转角度(ID 321)
pub struct NodeRadToDeg {
    radians: ValueIn,
}
impl INode for NodeRadToDeg {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.radians]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 321 Rad_To_Deg")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 321 Rad_To_Deg")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(321)
    }
}
impl Default for NodeRadToDeg {
    fn default() -> Self {
        Self { radians: ValueIn::new(ValueFloat::def()) }
    }
}

/// 角度转弧度(ID 322)
pub struct NodeDegToRad {
    degrees: ValueIn,
}
impl INode for NodeDegToRad {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.degrees]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueFloat::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 322 Deg_To_Rad")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 322 Deg_To_Rad")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(322)
    }
}
impl Default for NodeDegToRad {
    fn default() -> Self {
        Self { degrees: ValueIn::new(ValueFloat::def()) }
    }
}

// ========================================================================
// 布尔逻辑
// ========================================================================

/// 与(ID 226)
pub struct NodeAnd {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeAnd {
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
        todo!("ID 226 And")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 226 And")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(226)
    }
}
impl Default for NodeAnd {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueBool::def()),
            b: ValueIn::new(ValueBool::def()),
        }
    }
}

/// 或(ID 227)
pub struct NodeOr {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeOr {
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
        todo!("ID 227 Or")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 227 Or")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(227)
    }
}
impl Default for NodeOr {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueBool::def()),
            b: ValueIn::new(ValueBool::def()),
        }
    }
}

/// 异或(ID 228)
pub struct NodeXor {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeXor {
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
        todo!("ID 228 Xor")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 228 Xor")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(228)
    }
}
impl Default for NodeXor {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueBool::def()),
            b: ValueIn::new(ValueBool::def()),
        }
    }
}

/// 非(ID 229)
pub struct NodeNot {
    value: ValueIn,
}
impl INode for NodeNot {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueBool::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 229 Not")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 229 Not")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(229)
    }
}
impl Default for NodeNot {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueBool::def()) }
    }
}

// ========================================================================
// 比较
// ========================================================================

/// 相等(ID 14,泛型)
pub struct NodeEqual {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeEqual {
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
        todo!("ID 14 Equal")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 14 Equal")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(14)
    }
}
impl Default for NodeEqual {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 枚举相等(ID 475)
pub struct NodeEnumEqual {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeEnumEqual {
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
        todo!("ID 475 Enum_Equal")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 475 Enum_Equal")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(475)
    }
}
impl Default for NodeEnumEqual {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueEnum::def()),
            b: ValueIn::new(ValueEnum::def()),
        }
    }
}

/// 小于(ID 230)
pub struct NodeLessThan {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeLessThan {
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
        todo!("ID 230 Less_Than")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 230 Less_Than")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(230)
    }
}
impl Default for NodeLessThan {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 小于等于(ID 231)
pub struct NodeLessEqual {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeLessEqual {
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
        todo!("ID 231 Less_Equal")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 231 Less_Equal")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(231)
    }
}
impl Default for NodeLessEqual {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 大于(ID 232)
pub struct NodeGreaterThan {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeGreaterThan {
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
        todo!("ID 232 Greater_Than")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 232 Greater_Than")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(232)
    }
}
impl Default for NodeGreaterThan {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}

/// 大于等于(ID 233)
pub struct NodeGreaterEqual {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeGreaterEqual {
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
        todo!("ID 233 Greater_Equal")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 233 Greater_Equal")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(233)
    }
}
impl Default for NodeGreaterEqual {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueFloat::def()),
            b: ValueIn::new(ValueFloat::def()),
        }
    }
}

// ========================================================================
// 位运算
// ========================================================================

/// 左移(ID 778)
pub struct NodeLeftShift {
    value: ValueIn,
    bits: ValueIn,
}
impl INode for NodeLeftShift {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value, &self.bits]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 778 Left_Shift")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 778 Left_Shift")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(778)
    }
}
impl Default for NodeLeftShift {
    fn default() -> Self {
        Self {
            value: ValueIn::new(ValueInt::def()),
            bits: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 右移(ID 779)
pub struct NodeRightShift {
    value: ValueIn,
    bits: ValueIn,
}
impl INode for NodeRightShift {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value, &self.bits]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 779 Right_Shift")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 779 Right_Shift")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(779)
    }
}
impl Default for NodeRightShift {
    fn default() -> Self {
        Self {
            value: ValueIn::new(ValueInt::def()),
            bits: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 按位与(ID 780)
pub struct NodeBitwiseAnd {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeBitwiseAnd {
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
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 780 Bitwise_And")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 780 Bitwise_And")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(780)
    }
}
impl Default for NodeBitwiseAnd {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueInt::def()),
            b: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 按位或(ID 781)
pub struct NodeBitwiseOr {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeBitwiseOr {
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
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 781 Bitwise_Or")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 781 Bitwise_Or")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(781)
    }
}
impl Default for NodeBitwiseOr {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueInt::def()),
            b: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 按位异或(ID 782)
pub struct NodeBitwiseXor {
    a: ValueIn,
    b: ValueIn,
}
impl INode for NodeBitwiseXor {
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
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 782 Bitwise_Xor")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 782 Bitwise_Xor")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(782)
    }
}
impl Default for NodeBitwiseXor {
    fn default() -> Self {
        Self {
            a: ValueIn::new(ValueInt::def()),
            b: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 按位非(ID 783)
pub struct NodeBitwiseNot {
    value: ValueIn,
}
impl INode for NodeBitwiseNot {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 783 Bitwise_Not")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 783 Bitwise_Not")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(783)
    }
}
impl Default for NodeBitwiseNot {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueInt::def()) }
    }
}

/// 写入位(ID 784):value 的第 bit 位置为 bit_value
pub struct NodeWriteBit {
    value: ValueIn,
    bit: ValueIn,
    bit_value: ValueIn,
}
impl INode for NodeWriteBit {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value, &self.bit, &self.bit_value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 784 Write_Bit")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 784 Write_Bit")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(784)
    }
}
impl Default for NodeWriteBit {
    fn default() -> Self {
        Self {
            value: ValueIn::new(ValueInt::def()),
            bit: ValueIn::new(ValueInt::def()),
            bit_value: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 读取位(ID 785):取出 value 的第 bit 位
pub struct NodeReadBit {
    value: ValueIn,
    bit: ValueIn,
}
impl INode for NodeReadBit {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value, &self.bit]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 785 Read_Bit")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 785 Read_Bit")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(785)
    }
}
impl Default for NodeReadBit {
    fn default() -> Self {
        Self {
            value: ValueIn::new(ValueInt::def()),
            bit: ValueIn::new(ValueInt::def()),
        }
    }
}

// ========================================================================
// 时间
// ========================================================================

/// 时间戳转时间(ID 752):timestamp → 年月日时分秒
pub struct NodeTimestampToTime {
    timestamp: ValueIn,
}
impl INode for NodeTimestampToTime {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.timestamp]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![
            ValueInt::def(), // 年
            ValueInt::def(), // 月
            ValueInt::def(), // 日
            ValueInt::def(), // 时
            ValueInt::def(), // 分
            ValueInt::def(), // 秒
        ]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 752 Timestamp_To_Time")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 752 Timestamp_To_Time")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(752)
    }
}
impl Default for NodeTimestampToTime {
    fn default() -> Self {
        Self { timestamp: ValueIn::new(ValueInt::def()) }
    }
}

/// 时间转时间戳(ID 753):年月日时分秒 → timestamp
pub struct NodeTimeToTimestamp {
    year: ValueIn,
    month: ValueIn,
    day: ValueIn,
    hour: ValueIn,
    minute: ValueIn,
    second: ValueIn,
}
impl INode for NodeTimeToTimestamp {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.year, &self.month, &self.day, &self.hour, &self.minute, &self.second]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 753 Time_To_Timestamp")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 753 Time_To_Timestamp")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(753)
    }
}
impl Default for NodeTimeToTimestamp {
    fn default() -> Self {
        Self {
            year: ValueIn::new(ValueInt::def()),
            month: ValueIn::new(ValueInt::def()),
            day: ValueIn::new(ValueInt::def()),
            hour: ValueIn::new(ValueInt::def()),
            minute: ValueIn::new(ValueInt::def()),
            second: ValueIn::new(ValueInt::def()),
        }
    }
}

/// 时间戳转星期(ID 754)
pub struct NodeTimestampToWeekday {
    timestamp: ValueIn,
}
impl INode for NodeTimestampToWeekday {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.timestamp]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 754 Timestamp_To_Weekday")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 754 Timestamp_To_Weekday")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(754)
    }
}
impl Default for NodeTimestampToWeekday {
    fn default() -> Self {
        Self { timestamp: ValueIn::new(ValueInt::def()) }
    }
}

// ========================================================================
// 列表 / 字典 / 类型转换
// ========================================================================

/// 组装列表(ID 169):把若干元素拼成列表,元素数量动态
pub struct NodeAssembleList {
    /// 元素输入(动态添加)
    items: Vec<ValueIn>,
}
impl Default for NodeAssembleList {
    fn default() -> Self {
        Self { items: vec![] }
    }
}
impl NodeAssembleList {
    /// 添加一个元素输入(泛型元素,按 Int 语义)
    pub fn add_item(&mut self, item: ValueIn) {
        self.items.push(item);
    }
}
impl INode for NodeAssembleList {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        self.items.iter().collect()
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueIntList::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 169 Assemble_List")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 169 Assemble_List")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(169)
    }
}

/// 类型转换(ID 180):K 类型值转为 V 类型值
pub struct NodeConvertType {
    value: ValueIn,
}
impl INode for NodeConvertType {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 180 Convert_Type")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 180 Convert_Type")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(180)
    }
}
impl Default for NodeConvertType {
    fn default() -> Self {
        Self { value: ValueIn::new(ValueInt::def()) }
    }
}

/// 创建字典(ID 1088):key 列表 + value 列表 → 字典
pub struct NodeCreateDictionary {
    keys: ValueIn,
    values: ValueIn,
}
impl INode for NodeCreateDictionary {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.keys, &self.values]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 1088 Create_Dictionary")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 1088 Create_Dictionary")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(1088)
    }
}
impl Default for NodeCreateDictionary {
    fn default() -> Self {
        Self {
            keys: ValueIn::new(ValueIntList::def()),
            values: ValueIn::new(ValueIntList::def()),
        }
    }
}

/// 组装字典(ID 1788):若干 K/V 对拼成字典,键值对数量动态
pub struct NodeAssembleDictionary {
    /// 键值对输入(动态添加)
    pairs: Vec<(ValueIn, ValueIn)>,
}
impl Default for NodeAssembleDictionary {
    fn default() -> Self {
        Self { pairs: vec![] }
    }
}
impl NodeAssembleDictionary {
    /// 添加一个键值对
    pub fn add_pair(&mut self, key: ValueIn, value: ValueIn) {
        self.pairs.push((key, value));
    }
}
impl INode for NodeAssembleDictionary {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        let mut v = Vec::with_capacity(self.pairs.len() * 2);
        for (k, val) in &self.pairs {
            v.push(k);
            v.push(val);
        }
        v
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueDict::new(ValueInt::default(), ValueInt::default()).into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 1788 Assemble_Dictionary")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 1788 Assemble_Dictionary")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(1788)
    }
}

// ========================================================================
// 结构体操作(meta 引脚,按 ValueStruct 语义设计)
// ========================================================================

/// 拼装结构体(ID 300002):字段值 → 结构体
pub struct NodeAssembleStruct {
    /// 字段值输入(动态添加,按结构体定义顺序)
    fields: Vec<ValueIn>,
}
impl Default for NodeAssembleStruct {
    fn default() -> Self {
        Self { fields: vec![] }
    }
}
impl NodeAssembleStruct {
    /// 添加一个字段值输入
    pub fn add_field(&mut self, field: ValueIn) {
        self.fields.push(field);
    }
}
impl INode for NodeAssembleStruct {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        self.fields.iter().collect()
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueStruct::new(0, vec![]).into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 300002 Assemble_Struct")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 300002 Assemble_Struct")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(300002)
    }
}

/// 拆分结构体(ID 300003):结构体 → 字段值(动态)
pub struct NodeSplitStruct {
    /// 结构体输入
    structure: ValueIn,
}
impl INode for NodeSplitStruct {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.structure]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 300003 Split_Struct")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 300003 Split_Struct")
    }
    fn get_type(&self) -> NodeType {
        NodeType::simple(300003)
    }
}
impl Default for NodeSplitStruct {
    fn default() -> Self {
        Self { structure: ValueIn::new(ValueStruct::new(0, vec![]).into()) }
    }
}
