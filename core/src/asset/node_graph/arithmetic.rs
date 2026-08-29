//! 算术域节点(Server,Arithmetic)
//!
//! 人工设计,替换自动生成版本:
//! - 引脚按语义命名(a/b/vector/scale/x/y/z 等),不穷举 out_0/out_1
//! - 动态数量结构用 `Vec`(Assemble_List / Assemble_Dictionary 的参数不穷举字段)
//! - 泛型 `R<T>` 数值按 Float 语义;execute/get_value 仅模拟(todo!())

use std::any::{Any, TypeId};
use std::borrow::Cow;
use crate::asset::generated::ServerTypeId;
use crate::asset::node_graph::{ControlOut, Node, Link, NodeRef, Simulation, ValueIn};
use crate::asset::raw_node_graph::NodeType;
use crate::asset::value::{
    AnyValue, ValueBool, ValueBoolList, ValueConfig, ValueConfigList, ValueDefault, ValueDict,
    ValueEntity, ValueEntityList, ValueEnum, ValueFloat, ValueFloatList, ValueGuid, ValueGuidList,
    ValueInt, ValueIntList, ValuePrefab, ValuePrefabList, ValueSelected, ValueString,
    ValueStringList, ValueStruct, ValueVector, ValueVectorList, unwrap_selected,
};
use anyhow::{bail, Result};

// ========================================================================
// 向量运算
// ========================================================================

/// 拆分向量为分量(Arithmetic.Math.Split_Vector,ID 9):Vec → x/y/z
pub struct NodeSplitVector {
    vector: ValueIn,
}
impl Node for NodeSplitVector {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.vector.clone()]
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
impl Node for NodeVectorAdd {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
impl Node for NodeVectorSubtract {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
impl Node for NodeVectorScale {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.vector.clone(), self.scale.clone()]
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
impl Node for NodeVectorAngle {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
impl Node for NodeVectorNormalize {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.vector.clone()]
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
impl Node for NodeVectorLength {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.vector.clone()]
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
impl Node for NodeDistance {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
impl Node for NodeVectorRotate {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.vector.clone(), self.rotation.clone()]
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
impl Node for NodeVectorDot {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
impl Node for NodeVectorCross {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
impl Node for NodeVectorToRotation {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.forward.clone(), self.up.clone()]
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
impl Node for NodeCreateVector {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.x.clone(), self.y.clone(), self.z.clone()]
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

/// 加法(ID 200,泛型变体):shell 固定 200,kernel 随类型(Int→200、Flt→201)。
/// `ty` 是结果类型(AnyValue),`get_values_out` 返回该类型的值。
pub struct NodeAdd {
    pub a: ValueIn,
    pub b: ValueIn,
}
impl NodeAdd {
    pub fn new(ty: AnyValue) -> Self {
        Self { a: ValueIn::new(ty.clone()), b: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    /// 设置第 index 个输入(0=a、1=b)的值来源:连接或默认值
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.a } else { &mut self.b };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeAdd {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone().into_selected(Self::select).unwrap(), self.b.clone().into_selected(Self::select).unwrap()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![self.a.value.clone().into_selected(false, Self::select).unwrap().into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 200 Add")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 200 Add")
    }
    fn get_type(&self) -> NodeType {
        NodeType::variant(200, if unwrap_selected(&self.a.value).is::<ValueInt>() {
            200
        } else if unwrap_selected(&self.a.value).is::<ValueFloat>() {
            201
        } else {
            panic!("Unsupported type: {:?}", self.a.value);
        })
    }
    fn verify(&self, context: &Simulation) -> Result<()> {
        if Self::select(self.a.value.clone())? != Self::select(self.b.value.clone())? {
            bail!("The types of the two addends must be the same");
        }
        Ok(())
    }
}

/// 减法(ID 202,泛型变体):shell 固定 202,kernel 随类型(Int→202、Flt→203)。
pub struct NodeSubtract {
    pub a: ValueIn,
    pub b: ValueIn,
}
impl NodeSubtract {
    pub fn new(ty: AnyValue) -> Self {
        Self { a: ValueIn::new(ty.clone()), b: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.a } else { &mut self.b };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeSubtract {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.a.clone().into_selected(Self::select).unwrap(),
            self.b.clone().into_selected(Self::select).unwrap(),
        ]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![self.a.value.clone().into_selected(false, Self::select).unwrap().into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 202 Subtract")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 202 Subtract")
    }
    fn get_type(&self) -> NodeType {
        NodeType::variant(202, if unwrap_selected(&self.a.value).is::<ValueInt>() {
            202
        } else if unwrap_selected(&self.a.value).is::<ValueFloat>() {
            203
        } else {
            panic!("Unsupported type: {:?}", self.a.value);
        })
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        if Self::select(self.a.value.clone())?
            != Self::select(self.b.value.clone())?
        {
            bail!("The types of the two operands must be the same");
        }
        Ok(())
    }
}

/// 乘法(ID 204,泛型变体):shell 固定 204,kernel 随类型(Int→204、Flt→205)。
pub struct NodeMultiply {
    pub a: ValueIn,
    pub b: ValueIn,
}
impl NodeMultiply {
    pub fn new(ty: AnyValue) -> Self {
        Self { a: ValueIn::new(ty.clone()), b: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.a } else { &mut self.b };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeMultiply {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.a.clone().into_selected(Self::select).unwrap(),
            self.b.clone().into_selected(Self::select).unwrap(),
        ]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![self.a.value.clone().into_selected(false, Self::select).unwrap().into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 204 Multiply")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 204 Multiply")
    }
    fn get_type(&self) -> NodeType {
        NodeType::variant(204, if unwrap_selected(&self.a.value).is::<ValueInt>() {
            204
        } else if unwrap_selected(&self.a.value).is::<ValueFloat>() {
            205
        } else {
            panic!("Unsupported type: {:?}", self.a.value);
        })
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        if Self::select(self.a.value.clone())?
            != Self::select(self.b.value.clone())?
        {
            bail!("The types of the two operands must be the same");
        }
        Ok(())
    }
}

/// 除法(ID 206,泛型变体):shell 固定 206,kernel 随类型(Int→206、Flt→207)。
pub struct NodeDivide {
    pub a: ValueIn,
    pub b: ValueIn,
}
impl NodeDivide {
    pub fn new(ty: AnyValue) -> Self {
        Self { a: ValueIn::new(ty.clone()), b: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.a } else { &mut self.b };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeDivide {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.a.clone().into_selected(Self::select).unwrap(),
            self.b.clone().into_selected(Self::select).unwrap(),
        ]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![self.a.value.clone().into_selected(false, Self::select).unwrap().into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 206 Divide")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 206 Divide")
    }
    fn get_type(&self) -> NodeType {
        NodeType::variant(206, if unwrap_selected(&self.a.value).is::<ValueInt>() {
            206
        } else if unwrap_selected(&self.a.value).is::<ValueFloat>() {
            207
        } else {
            panic!("Unsupported type: {:?}", self.a.value);
        })
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        if Self::select(self.a.value.clone())?
            != Self::select(self.b.value.clone())?
        {
            bail!("The types of the two operands must be the same");
        }
        Ok(())
    }
}

/// 幂运算(ID 209,泛型变体):shell 固定 209,kernel 随类型(Int→209、Flt→210)。
pub struct NodePower {
    base: ValueIn,
    exponent: ValueIn,
}
impl NodePower {
    pub fn new(ty: AnyValue) -> Self {
        Self { base: ValueIn::new(ty.clone()), exponent: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.base } else { &mut self.exponent };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodePower {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.base.clone().into_selected(Self::select).unwrap(),
            self.exponent.clone().into_selected(Self::select).unwrap(),
        ]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![self.base.value.clone().into_selected(false, Self::select).unwrap().into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 209 Power")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 209 Power")
    }
    fn get_type(&self) -> NodeType {
        NodeType::variant(209, if unwrap_selected(&self.base.value).is::<ValueInt>() {
            209
        } else if unwrap_selected(&self.base.value).is::<ValueFloat>() {
            210
        } else {
            panic!("Unsupported type: {:?}", self.base.value);
        })
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        if Self::select(self.base.value.clone())?
            != Self::select(self.exponent.value.clone())?
        {
            bail!("The types of the two operands must be the same");
        }
        Ok(())
    }
}

/// 取大值(ID 211,泛型变体):shell 固定 211,kernel 随类型(Int→211、Flt→212)。
pub struct NodeMax {
    a: ValueIn,
    b: ValueIn,
}
impl NodeMax {
    pub fn new(ty: AnyValue) -> Self {
        Self { a: ValueIn::new(ty.clone()), b: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.a } else { &mut self.b };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeMax {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.a.clone().into_selected(Self::select).unwrap(),
            self.b.clone().into_selected(Self::select).unwrap(),
        ]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![self.a.value.clone().into_selected(false, Self::select).unwrap().into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 211 Max")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 211 Max")
    }
    fn get_type(&self) -> NodeType {
        NodeType::variant(211, if unwrap_selected(&self.a.value).is::<ValueInt>() {
            211
        } else if unwrap_selected(&self.a.value).is::<ValueFloat>() {
            212
        } else {
            panic!("Unsupported type: {:?}", self.a.value);
        })
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        if Self::select(self.a.value.clone())?
            != Self::select(self.b.value.clone())?
        {
            bail!("The types of the two operands must be the same");
        }
        Ok(())
    }
}

/// 取小值(ID 213,泛型变体):shell 固定 213,kernel 随类型(Int→213、Flt→214)。
pub struct NodeMin {
    a: ValueIn,
    b: ValueIn,
}
impl NodeMin {
    pub fn new(ty: AnyValue) -> Self {
        Self { a: ValueIn::new(ty.clone()), b: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.a } else { &mut self.b };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeMin {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.a.clone().into_selected(Self::select).unwrap(),
            self.b.clone().into_selected(Self::select).unwrap(),
        ]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![self.a.value.clone().into_selected(false, Self::select).unwrap().into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 213 Min")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 213 Min")
    }
    fn get_type(&self) -> NodeType {
        NodeType::variant(213, if unwrap_selected(&self.a.value).is::<ValueInt>() {
            213
        } else if unwrap_selected(&self.a.value).is::<ValueFloat>() {
            214
        } else {
            panic!("Unsupported type: {:?}", self.a.value);
        })
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        if Self::select(self.a.value.clone())?
            != Self::select(self.b.value.clone())?
        {
            bail!("The types of the two operands must be the same");
        }
        Ok(())
    }
}

/// 取余(ID 208):整数取模
pub struct NodeModulo {
    pub a: ValueIn,
    pub b: ValueIn,
}
impl Node for NodeModulo {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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

/// 绝对值(ID 216,泛型变体):shell 固定 216,kernel 随类型(Int→216、Flt→217)。
pub struct NodeAbs {
    value: ValueIn,
}
impl NodeAbs {
    pub fn new(ty: AnyValue) -> Self {
        Self { value: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, value: AnyValue, link: Option<Link>) {
        self.value.value = value;
        self.value.has_default = link.is_none();
        self.value.link = link;
    }
}
impl Node for NodeAbs {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone().into_selected(Self::select).unwrap()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![self.value.value.clone().into_selected(false, Self::select).unwrap().into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 216 Abs")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 216 Abs")
    }
    fn get_type(&self) -> NodeType {
        NodeType::variant(216, if unwrap_selected(&self.value.value).is::<ValueInt>() {
            216
        } else if unwrap_selected(&self.value.value).is::<ValueFloat>() {
            217
        } else {
            panic!("Unsupported type: {:?}", self.value.value);
        })
    }
}

/// 取符号(ID 218,泛型变体):-1 / 0 / 1;shell 固定 218,kernel 随类型(Int→218、Flt→219)。
pub struct NodeSign {
    value: ValueIn,
}
impl NodeSign {
    pub fn new(ty: AnyValue) -> Self {
        Self { value: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, value: AnyValue, link: Option<Link>) {
        self.value.value = value;
        self.value.has_default = link.is_none();
        self.value.link = link;
    }
}
impl Node for NodeSign {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone().into_selected(Self::select).unwrap()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![self.value.value.clone().into_selected(false, Self::select).unwrap().into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 218 Sign")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 218 Sign")
    }
    fn get_type(&self) -> NodeType {
        NodeType::variant(218, if unwrap_selected(&self.value.value).is::<ValueInt>() {
            218
        } else if unwrap_selected(&self.value.value).is::<ValueFloat>() {
            219
        } else {
            panic!("Unsupported type: {:?}", self.value.value);
        })
    }
}

/// 夹取(ID 222,泛型变体):value 限制在 [min, max];
/// shell 固定 222,kernel 随类型(Int→222、Flt→223)。
pub struct NodeClamp {
    value: ValueIn,
    min: ValueIn,
    max: ValueIn,
}
impl NodeClamp {
    pub fn new(ty: AnyValue) -> Self {
        Self {
            value: ValueIn::new(ty.clone()),
            min: ValueIn::new(ty.clone()),
            max: ValueIn::new(ty),
        }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = match index {
            0 => &mut self.value,
            1 => &mut self.min,
            _ => &mut self.max,
        };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeClamp {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.value.clone().into_selected(Self::select).unwrap(),
            self.min.clone().into_selected(Self::select).unwrap(),
            self.max.clone().into_selected(Self::select).unwrap(),
        ]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![self.value.value.clone().into_selected(false, Self::select).unwrap().into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 222 Clamp")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 222 Clamp")
    }
    fn get_type(&self) -> NodeType {
        NodeType::variant(222, if unwrap_selected(&self.value.value).is::<ValueInt>() {
            222
        } else if unwrap_selected(&self.value.value).is::<ValueFloat>() {
            223
        } else {
            panic!("Unsupported type: {:?}", self.value.value);
        })
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        let i = Self::select(self.value.value.clone())?;
        if Self::select(self.min.value.clone())? != i
            || Self::select(self.max.value.clone())? != i
        {
            bail!("The types of the three operands must be the same");
        }
        Ok(())
    }
}

/// 四舍五入(ID 224):value 按舍入模式取整
pub struct NodeRound {
    value: ValueIn,
    mode: ValueIn,
}
impl Node for NodeRound {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone(), self.mode.clone()]
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
impl Node for NodeSqrt {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone()]
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
impl Node for NodeLogarithm {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.base.clone(), self.value.clone()]
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
impl Node for NodeSin {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.angle.clone()]
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
impl Node for NodeCos {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.angle.clone()]
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
impl Node for NodeTan {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.angle.clone()]
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
impl Node for NodeAsin {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone()]
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
impl Node for NodeAcos {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone()]
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
impl Node for NodeAtan {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone()]
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
impl Node for NodeRadToDeg {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.radians.clone()]
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
impl Node for NodeDegToRad {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.degrees.clone()]
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
impl Node for NodeAnd {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
impl Node for NodeOr {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
    pub a: ValueIn,
    pub b: ValueIn,
}
impl Node for NodeXor {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
impl Node for NodeNot {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone()]
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

/// 相等(ID 14,泛型变体):shell 固定 14,kernel 随类型(Str→14、Gid→15、
/// Ety→16、Vec→17、Int→370、Flt→371、Cfg→581、Pfb→582、Bol→786);输出 Bol。
pub struct NodeEqual {
    a: ValueIn,
    b: ValueIn,
}
impl NodeEqual {
    pub fn new(ty: AnyValue) -> Self {
        Self { a: ValueIn::new(ty.clone()), b: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        // selected index 按变体顺序(Str,Gid,Ety,Vec,Fct,Int,Flt,Cfg,Pfb,Bol),与 kernel 无关
        Ok(if value.is::<ValueString>() {
            0
        } else if value.is::<ValueGuid>() {
            1
        } else if value.is::<ValueEntity>() {
            2
        } else if value.is::<ValueVector>() {
            3
        } else if value.is::<ValueInt>() {
            5
        } else if value.is::<ValueFloat>() {
            6
        } else if value.is::<ValueConfig>() {
            7
        } else if value.is::<ValuePrefab>() {
            8
        } else if value.is::<ValueBool>() {
            9
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.a } else { &mut self.b };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeEqual {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.a.clone().into_selected(Self::select).unwrap(),
            self.b.clone().into_selected(Self::select).unwrap(),
        ]
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
        let k = if unwrap_selected(&self.a.value).is::<ValueString>() {
            14
        } else if unwrap_selected(&self.a.value).is::<ValueGuid>() {
            15
        } else if unwrap_selected(&self.a.value).is::<ValueEntity>() {
            16
        } else if unwrap_selected(&self.a.value).is::<ValueVector>() {
            17
        } else if unwrap_selected(&self.a.value).is::<ValueInt>() {
            370
        } else if unwrap_selected(&self.a.value).is::<ValueFloat>() {
            371
        } else if unwrap_selected(&self.a.value).is::<ValueConfig>() {
            581
        } else if unwrap_selected(&self.a.value).is::<ValuePrefab>() {
            582
        } else if unwrap_selected(&self.a.value).is::<ValueBool>() {
            786
        } else {
            panic!("Unsupported type: {:?}", self.a.value);
        };
        NodeType::variant(14, k)
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        if Self::select(self.a.value.clone())?
            != Self::select(self.b.value.clone())?
        {
            bail!("The types of the two operands must be the same");
        }
        Ok(())
    }
}

/// 枚举相等(ID 475)
pub struct NodeEnumEqual {
    a: ValueIn,
    b: ValueIn,
}
impl Node for NodeEnumEqual {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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

/// 小于(ID 230,泛型变体):shell 固定 230,kernel 随类型(Int→230、Flt→235);输出 Bol。
pub struct NodeLessThan {
    a: ValueIn,
    b: ValueIn,
}
impl NodeLessThan {
    pub fn new(ty: AnyValue) -> Self {
        Self { a: ValueIn::new(ty.clone()), b: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.a } else { &mut self.b };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeLessThan {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.a.clone().into_selected(Self::select).unwrap(),
            self.b.clone().into_selected(Self::select).unwrap(),
        ]
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
        NodeType::variant(230, if unwrap_selected(&self.a.value).is::<ValueInt>() {
            230
        } else if unwrap_selected(&self.a.value).is::<ValueFloat>() {
            235
        } else {
            panic!("Unsupported type: {:?}", self.a.value);
        })
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        if Self::select(self.a.value.clone())?
            != Self::select(self.b.value.clone())?
        {
            bail!("The types of the two operands must be the same");
        }
        Ok(())
    }
}

/// 小于等于(ID 231,泛型变体):shell 固定 231,kernel 随类型(Int→231、Flt→236);输出 Bol。
pub struct NodeLessEqual {
    a: ValueIn,
    b: ValueIn,
}
impl NodeLessEqual {
    pub fn new(ty: AnyValue) -> Self {
        Self { a: ValueIn::new(ty.clone()), b: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.a } else { &mut self.b };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeLessEqual {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.a.clone().into_selected(Self::select).unwrap(),
            self.b.clone().into_selected(Self::select).unwrap(),
        ]
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
        NodeType::variant(231, if unwrap_selected(&self.a.value).is::<ValueInt>() {
            231
        } else if unwrap_selected(&self.a.value).is::<ValueFloat>() {
            236
        } else {
            panic!("Unsupported type: {:?}", self.a.value);
        })
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        if Self::select(self.a.value.clone())?
            != Self::select(self.b.value.clone())?
        {
            bail!("The types of the two operands must be the same");
        }
        Ok(())
    }
}

/// 大于(ID 232,泛型变体):shell 固定 232,kernel 随类型(Int→232、Flt→237);输出 Bol。
pub struct NodeGreaterThan {
    a: ValueIn,
    b: ValueIn,
}
impl NodeGreaterThan {
    pub fn new(ty: AnyValue) -> Self {
        Self { a: ValueIn::new(ty.clone()), b: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.a } else { &mut self.b };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeGreaterThan {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.a.clone().into_selected(Self::select).unwrap(),
            self.b.clone().into_selected(Self::select).unwrap(),
        ]
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
        NodeType::variant(232, if unwrap_selected(&self.a.value).is::<ValueInt>() {
            232
        } else if unwrap_selected(&self.a.value).is::<ValueFloat>() {
            237
        } else {
            panic!("Unsupported type: {:?}", self.a.value);
        })
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        if Self::select(self.a.value.clone())?
            != Self::select(self.b.value.clone())?
        {
            bail!("The types of the two operands must be the same");
        }
        Ok(())
    }
}

/// 大于等于(ID 233,泛型变体):shell 固定 233,kernel 随类型(Int→233、Flt→238);输出 Bol。
pub struct NodeGreaterEqual {
    a: ValueIn,
    b: ValueIn,
}
impl NodeGreaterEqual {
    pub fn new(ty: AnyValue) -> Self {
        Self { a: ValueIn::new(ty.clone()), b: ValueIn::new(ty) }
    }
    fn select(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        let vi = if index == 0 { &mut self.a } else { &mut self.b };
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeGreaterEqual {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![
            self.a.clone().into_selected(Self::select).unwrap(),
            self.b.clone().into_selected(Self::select).unwrap(),
        ]
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
        NodeType::variant(233, if unwrap_selected(&self.a.value).is::<ValueInt>() {
            233
        } else if unwrap_selected(&self.a.value).is::<ValueFloat>() {
            238
        } else {
            panic!("Unsupported type: {:?}", self.a.value);
        })
    }
    fn verify(&self, _context: &Simulation) -> Result<()> {
        if Self::select(self.a.value.clone())?
            != Self::select(self.b.value.clone())?
        {
            bail!("The types of the two operands must be the same");
        }
        Ok(())
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
impl Node for NodeLeftShift {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone(), self.bits.clone()]
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
impl Node for NodeRightShift {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone(), self.bits.clone()]
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
    pub a: ValueIn,
    pub b: ValueIn,
}
impl Node for NodeBitwiseAnd {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
    pub a: ValueIn,
    pub b: ValueIn,
}
impl Node for NodeBitwiseOr {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
    pub a: ValueIn,
    pub b: ValueIn,
}
impl Node for NodeBitwiseXor {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.a.clone(), self.b.clone()]
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
impl Node for NodeBitwiseNot {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone()]
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
impl Node for NodeWriteBit {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone(), self.bit.clone(), self.bit_value.clone()]
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
impl Node for NodeReadBit {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone(), self.bit.clone()]
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
impl Node for NodeTimestampToTime {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.timestamp.clone()]
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
impl Node for NodeTimeToTimestamp {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.year.clone(), self.month.clone(), self.day.clone(), self.hour.clone(), self.minute.clone(), self.second.clone()]
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
impl Node for NodeTimestampToWeekday {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.timestamp.clone()]
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
/// 组装列表(ID 169,泛型变体):shell 固定 169,kernel 随元素类型(Int→169、
/// Str→170、Ety→171、Gid→172、Flt→173、Vec→174、Bol→175、Cfg→568、Pfb→569);
/// 输出 L<R<T>>(按元素类型返回列表占位)。元素输入动态添加。
pub struct NodeAssembleList {
    /// 元素输入(动态添加)
    items: Vec<ValueIn>,
    ty: AnyValue,
}
impl NodeAssembleList {
    pub fn new(ty: AnyValue) -> Self {
        Self { items: vec![], ty }
    }
    /// 元素类型的 selected index(Int→0、Str→1、Ety→2、Gid→3、Flt→4、Vec→5、Bol→6、Cfg→7、Pfb→8)
    fn index_of(ty: &AnyValue) -> Result<i32> {
        Ok(match ty.get_server_type() {
            ServerTypeId::SInt => 0,
            ServerTypeId::SString => 1,
            ServerTypeId::SEntity => 2,
            ServerTypeId::SGuid => 3,
            ServerTypeId::SFloat => 4,
            ServerTypeId::SVector => 5,
            ServerTypeId::SBoolean => 6,
            ServerTypeId::SConfig => 7,
            ServerTypeId::SPrefab => 8,
            other => bail!("Unsupported type: {other:?}"),
        })
    }
    fn select(value: AnyValue) -> Result<i32> {
        Self::index_of(&value)
    }
    /// 添加一个元素输入
    pub fn add_item(&mut self, item: ValueIn) {
        self.items.push(item);
    }
    /// 设置第 index 个元素输入的值来源
    pub fn set_item(&mut self, index: usize, value: AnyValue, link: Option<Link>) {
        while self.items.len() <= index {
            self.items.push(ValueIn::new(self.ty.clone()));
        }
        let vi = &mut self.items[index];
        vi.value = value;
        vi.has_default = link.is_none();
        vi.link = link;
    }
}
impl Node for NodeAssembleList {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        self.items
            .iter()
            .map(|i| i.clone().into_selected(Self::select).unwrap())
            .collect()
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        // 输出 L<R<T>>:selected index 按元素类型,值按元素类型给列表占位
        let index = Self::index_of(&self.ty).unwrap();
        let list: AnyValue = match self.ty.get_server_type() {
            ServerTypeId::SInt => ValueIntList::default().into(),
            ServerTypeId::SString => ValueStringList::default().into(),
            ServerTypeId::SEntity => ValueEntityList::default().into(),
            ServerTypeId::SGuid => ValueGuidList::default().into(),
            ServerTypeId::SFloat => ValueFloatList::default().into(),
            ServerTypeId::SVector => ValueVectorList::default().into(),
            ServerTypeId::SBoolean => ValueBoolList::default().into(),
            ServerTypeId::SConfig => ValueConfigList::default().into(),
            ServerTypeId::SPrefab => ValuePrefabList::default().into(),
            other => panic!("Arithmetic.General.Assemble_List does not support type {other:?}"),
        };
        vec![ValueSelected { index, value: list, has_default: false }.into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 169 Assemble_List")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 169 Assemble_List")
    }
    fn get_type(&self) -> NodeType {
        let k = match self.ty.get_server_type() {
            ServerTypeId::SInt => 169,
            ServerTypeId::SString => 170,
            ServerTypeId::SEntity => 171,
            ServerTypeId::SGuid => 172,
            ServerTypeId::SFloat => 173,
            ServerTypeId::SVector => 174,
            ServerTypeId::SBoolean => 175,
            ServerTypeId::SConfig => 568,
            ServerTypeId::SPrefab => 569,
            other => panic!("Arithmetic.General.Assemble_List does not support type {other:?}"),
        };
        NodeType::variant(169, k)
    }
}

/// 类型转换(ID 180,泛型变体):K 类型值转为 V 类型值;
/// shell 固定 180,kernel 随 (K,V) 组合(11 种,见特判);输出 R<V>。
pub struct NodeConvertType {
    value: ValueIn,
    from_ty: AnyValue,
    to_ty: AnyValue,
}
impl NodeConvertType {
    pub fn new(from_ty: AnyValue, to_ty: AnyValue) -> Self {
        Self { value: ValueIn::new(from_ty.clone()), from_ty, to_ty }
    }
    /// 输入 R<K> 的 selected index(Int→0、Ety→1、Gid→2、Bol→3、Flt→4、Vec→5)
    fn select_in(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueInt>() {
            0
        } else if value.is::<ValueEntity>() {
            1
        } else if value.is::<ValueGuid>() {
            2
        } else if value.is::<ValueBool>() {
            3
        } else if value.is::<ValueFloat>() {
            4
        } else if value.is::<ValueVector>() {
            5
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    /// 输出 R<V> 的 selected index(Bol→0、Flt→1、Str→2、Int→3)
    fn select_out(value: AnyValue) -> Result<i32> {
        Ok(if value.is::<ValueBool>() {
            0
        } else if value.is::<ValueFloat>() {
            1
        } else if value.is::<ValueString>() {
            2
        } else if value.is::<ValueInt>() {
            3
        } else {
            bail!("Unsupported type: {value:?}");
        })
    }
    pub fn set_input(&mut self, value: AnyValue, link: Option<Link>) {
        self.value.value = value;
        self.value.has_default = link.is_none();
        self.value.link = link;
    }
}
impl Node for NodeConvertType {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.value.clone().into_selected(Self::select_in).unwrap()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![self.to_ty.clone().into_selected(false, Self::select_out).unwrap().into()]
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("ID 180 Convert_Type")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("ID 180 Convert_Type")
    }
    fn get_type(&self) -> NodeType {
        // kernel 由 (K,V) 组合决定(11 种)
        let k = match (self.from_ty.get_server_type(), self.to_ty.get_server_type()) {
            (ServerTypeId::SInt, ServerTypeId::SBoolean) => 180,
            (ServerTypeId::SInt, ServerTypeId::SFloat) => 181,
            (ServerTypeId::SInt, ServerTypeId::SString) => 182,
            (ServerTypeId::SEntity, ServerTypeId::SString) => 183,
            (ServerTypeId::SGuid, ServerTypeId::SString) => 184,
            (ServerTypeId::SBoolean, ServerTypeId::SInt) => 185,
            (ServerTypeId::SBoolean, ServerTypeId::SString) => 186,
            (ServerTypeId::SFloat, ServerTypeId::SInt) => 187,
            (ServerTypeId::SFloat, ServerTypeId::SString) => 188,
            (ServerTypeId::SVector, ServerTypeId::SString) => 189,
            (from, to) => panic!("Arithmetic.General.Convert_Type does not support {from:?} -> {to:?}"),
        };
        NodeType::variant(180, k)
    }
}

/// 创建字典(ID 1088):key 列表 + value 列表 → 字典
pub struct NodeCreateDictionary {
    keys: ValueIn,
    values: ValueIn,
}
impl Node for NodeCreateDictionary {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.keys.clone(), self.values.clone()]
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
impl Node for NodeAssembleDictionary {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        let mut v = Vec::with_capacity(self.pairs.len() * 2);
        for (k, val) in &self.pairs {
            v.push(k.clone());
            v.push(val.clone());
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
impl Node for NodeAssembleStruct {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        self.fields.clone()
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
impl Node for NodeSplitStruct {
    fn get_controls_in(&self) -> i32 {
        0
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.structure.clone()]
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
