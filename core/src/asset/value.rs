use crate::asset::Side;
use crate::asset::generated::structure_definition_data::var_def::Subtype;
use crate::asset::generated::type_definition::TypeDetail;
use crate::asset::generated::{pin_interface, structure_definition_data, type_definition, typed_value, vec3f, ClientTypeId, Enum, Flt, Id, Int, ListStorage, MapPairStorage, MapStorage, ServerTypeId, Str, TypeDefinition, TypedValue, Vec3f};
use crate::asset::structure::ValueStruct;
use anyhow::{Result, anyhow};
use downcast::{Any, downcast};
use std::any::TypeId;
use std::fmt::Debug;
use crate::asset::generated::structure_definition_data::var_def::value::{Dict, Val};

pub type AnyValue = Box<dyn Value>;
impl<T: Value> From<T> for AnyValue {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}
pub trait CloneValue {
    fn clone(&self) -> AnyValue;
}
pub trait Value: Any + CloneValue + Debug + Send + Sync {
    fn encode_type(&self, side: Side) -> TypeDetail {
        match side {
            Side::Server => TypeDetail::ServerSide(type_definition::ServerType {
                type_tag: self.get_server_type() as i32,
                r#impl: 0,
                schema: self.encode_schema(),
            }),
            Side::Client => TypeDetail::ClientSide(type_definition::ClientType {
                type_tag: self.get_client_type() as i32,
            }),
        }
    }

    fn encode_typed(&self, is_set: bool, side: Side) -> TypedValue {
        TypedValue {
            widget: self.get_widget_type() as i32,
            is_set,
            r#type: TypeDefinition {
                backend: match side {
                    Side::Server => type_definition::Backend::Server as i32,
                    Side::Client => type_definition::Backend::Client as i32,
                },
                type_detail: self.encode_type(side).into(),
            }.into(),
            tracker: None,
            storage: if is_set { self.encode_storage(side) } else { None },
        }
    }

    /// 展示控件类型:按服务端类型分发(参考导出里
    /// int→NUMBER_INPUT / string→TEXT_INPUT / float→DECIMAL_INPUT 等,
    /// 编辑器靠它决定如何渲染变量值,UNKNOWN 会显示为空)。
    fn get_widget_type(&self) -> typed_value::WidgetType {
        use typed_value::WidgetType::*;
        match self.get_server_type() {
            ServerTypeId::SInt => NumberInput,
            ServerTypeId::SFloat => DecimalInput,
            ServerTypeId::SString => TextInput,
            ServerTypeId::SBoolean | ServerTypeId::SEnumItem => EnumPicker,
            ServerTypeId::SGuid
            | ServerTypeId::SEntity
            | ServerTypeId::SFaction
            | ServerTypeId::SConfig
            | ServerTypeId::SPrefab
            | ServerTypeId::SLocalVarRef
            | ServerTypeId::SVarSnapshotRef => IdInput,
            ServerTypeId::SVector => VectorGroup,
            ServerTypeId::SGuidList
            | ServerTypeId::SIntList
            | ServerTypeId::SBooleanList
            | ServerTypeId::SFloatList
            | ServerTypeId::SStringList
            | ServerTypeId::SEntityList
            | ServerTypeId::SVectorList
            | ServerTypeId::SEnumList
            | ServerTypeId::SFactionList
            | ServerTypeId::SConfigList
            | ServerTypeId::SPrefabList
            | ServerTypeId::SStructList => ListGroup,
            ServerTypeId::SStruct => StructBlock,
            ServerTypeId::SDict => MapGroup,
            _ => Unknown,
        }
    }

    fn get_server_type(&self) -> ServerTypeId;
    fn get_client_type(&self) -> ClientTypeId;
    fn get_type_id(&self, side: Side) -> i32 {
        match side {
            Side::Server => self.get_server_type() as i32,
            Side::Client => self.get_client_type() as i32,
        }
    }

    fn encode_storage(&self, side: Side) -> Option<typed_value::Storage>;

    fn encode_schema(&self) -> Option<type_definition::server_type::Schema> {
        None
    }

    fn encode_type_detail(&self) -> Option<pin_interface::type_info::Detail> { // TODO: enum, List
        None
    }

    fn encode_subtype(&self) -> Option<Subtype> { // TODO: enum, List
        None
    }

    fn encode_field_value(&self) -> Val;

    fn is_instance(&self, value: &AnyValue) -> bool {
        value.type_id() == TypeId::of::<Self>()
    }
}

/// 用元素编码结果构造列表存储(ListStorage)
fn list_storage(elements: Vec<TypedValue>) -> ListStorage {
    ListStorage { element: elements }
}
impl ToOwned for dyn Value {
    type Owned = AnyValue;
    fn to_owned(&self) -> Self::Owned {
        CloneValue::clone(self)
    }
}
impl Clone for AnyValue {
    fn clone(&self) -> Self {
        self.as_ref().to_owned()
    }
}
downcast!(dyn Value);

pub trait ValueDefault: Value + Default {
    fn def() -> AnyValue {
        Self::default().into()
    }
}
impl<T: Default + Value> ValueDefault for T {
}

trait ValueClone: Value + Clone {
}
impl<T: Value + Clone> ValueClone for T {
}
impl<T: ValueClone> CloneValue for T {
    fn clone(&self) -> AnyValue {
        Clone::clone(self).into()
    }
}

#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueBool(pub bool);
impl ValueBool {
    pub fn encode(&self) -> Enum {
        Enum { value: self.0 as i64 }
    }
}
impl Value for ValueBool {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SBoolean
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CBoolean
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValEnum(self.encode()).into()
    }
    fn encode_type_detail(&self) -> Option<pin_interface::type_info::Detail> {
        pin_interface::type_info::Detail::EnumId(pin_interface::type_info::EnumId { val: 1 }).into()
    }

    fn encode_field_value(&self) -> Val {
        Val::BooleanVal(self.encode())
    }
}

#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueInt(pub i32);
impl ValueInt {
    pub fn encode(&self) -> Int {
        Int { value: self.0 }
    }
}
impl Value for ValueInt {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SInt
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CInt
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValInt(self.encode()).into()
    }

    fn encode_field_value(&self) -> Val {
        Val::IntVal(self.encode())
    }
}

#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueString(pub String);
impl ValueString {
    pub fn encode(&self) -> Str {
        Str { value: self.0.clone() }
    }
}
impl Value for ValueString {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SString
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CString
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValString(self.encode()).into()
    }

    fn encode_field_value(&self) -> Val {
        Val::StrVal(self.encode())
    }
}

// ========================================================================
// 其余内置类型(排除 SStruct / SStructList)
// ========================================================================

// ---------- 标量 ----------

/// 浮点数(SFloat=5 / CFloat=7)
#[derive(Clone, Debug, Default)]
pub struct ValueFloat(pub f32);
impl ValueFloat {
    fn encode(&self) -> Flt {
        Flt { value: self.0 }
    }
}
impl Value for ValueFloat {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SFloat
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CFloat
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValFloat(self.encode()).into()
    }

    fn encode_field_value(&self) -> Val {
        Val::FloatVal(self.encode())
    }
}

/// 三维向量(SVector=12 / CVector=11)
#[derive(Clone, Debug, Default)]
pub struct ValueVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
impl ValueVec3 {
    fn encode(&self) -> Vec3f {
        Vec3f {
            value: vec3f::Value {
                x: self.x,
                y: self.y,
                z: self.z,
            }.into(),
        }
    }
}
impl Value for ValueVec3 {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SVector
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CVector
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValVector(self.encode()).into()
    }

    fn encode_field_value(&self) -> Val {
        Val::Vec3Val(self.encode())
    }
}

/// 全局唯一 ID(SGuid=2 / CGuid=14)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueGuid(pub i64);
impl ValueGuid {
    fn encode(&self) -> Id {
        Id { value: self.0 }
    }
}
impl Value for ValueGuid {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SGuid
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CGuid
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValId(self.encode()).into()
    }

    fn encode_field_value(&self) -> Val {
        Val::GuidVal(self.encode())
    }
}

/// 运行时实体对象(句柄)(SEntity=1 / CEntity=1)
#[derive(Clone, Debug)]
pub struct ValueEntity;
impl Default for ValueEntity {
    fn default() -> Self {
        Self
    }
}
impl Value for ValueEntity {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SEntity
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CEntity
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        None
    }

    fn encode_field_value(&self) -> Val {
        panic!()
    }
}

/// 枚举项(SEnumItem=14 / CEnumItem=13)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueEnum {
    pub id: i64,
    pub index: i64,
}
impl ValueEnum {
    pub fn new(id: i64, index: i64) -> Self {
        Self { id, index }
    }
}
impl Value for ValueEnum {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SEnumItem
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CEnumItem
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValEnum(Enum { value: self.index }).into()
    }
    fn encode_type_detail(&self) -> Option<pin_interface::type_info::Detail> {
        pin_interface::type_info::Detail::EnumId(pin_interface::type_info::EnumId { val: self.id }).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }

    fn is_instance(&self, value: &AnyValue) -> bool {
        matches!(value.downcast_ref::<ValueEnum>(), Ok(value) if value.id == self.id)
    }
}

/// 阵营/势力(SFaction=17 / CFaction=16)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueFaction(pub i64);
impl Value for ValueFaction {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SFaction
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CFaction
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValId(Id { value: self.0 }).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

/// 配置表引用(SConfig=20 / CConfig=18)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueConfig(pub i64);
impl Value for ValueConfig {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SConfig
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CConfig
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValId(Id { value: self.0 }).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

/// 预制体引用(SPrefab=21 / CPrefab=19)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValuePrefab(pub i64);
impl Value for ValuePrefab {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SPrefab
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CPrefab
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValId(Id { value: self.0 }).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

// ---------- 运行时引用(仅服务器,客户端无对应类型) ----------

/// 局部变量引用(SLocalVarRef=16,运行时栈内存引用)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueLocalVarRef;
impl Value for ValueLocalVarRef {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SLocalVarRef
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        None
    }

    fn encode_field_value(&self) -> Val {
        panic!();
    }
}

/// 变量快照引用(SVarSnapshotRef=28,实体删除时访问原始数据)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueVarSnapshotRef;
impl Value for ValueVarSnapshotRef {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SVarSnapshotRef
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        None
    }

    fn encode_field_value(&self) -> Val {
        panic!();
    }
}

// ---------- 列表 ----------

/// 实体列表(SEntityList=13 / CEntityList=2)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueEntityList(pub Vec<i64>);
impl Value for ValueEntityList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SEntityList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CEntityList
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|_x| {
                let v = &ValueEntity;
                let side = Side::Server;
                v.encode_typed(true, side)
            }).collect(),
        )).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

/// GUID 列表(SGuidList=7 / CGuidList=15)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueGuidList(pub Vec<i64>);
impl Value for ValueGuidList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SGuidList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CGuidList
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueGuid(*x);
                let side = Side::Server;
                v.encode_typed(true, side)
            }).collect(),
        )).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

/// 整数列表(SIntList=8 / CIntList=4)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueIntList(pub Vec<i32>);
impl Value for ValueIntList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SIntList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CIntList
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueInt(*x);
                let side = Side::Server;
                v.encode_typed(true, side)
            }).collect(),
        )).into()
    }
    fn encode_field_value(&self) -> Val {
        Val::IntList(structure_definition_data::var_def::value::IntList {
            items: self.0.clone(),
        })
    }
}

/// 布尔列表(SBooleanList=9 / CBooleanList=6)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueBoolList(pub Vec<bool>);
impl Value for ValueBoolList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SBooleanList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CBooleanList
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueBool(*x);
                let side = Side::Server;
                v.encode_typed(true, side)
            }).collect(),
        )).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

/// 浮点列表(SFloatList=10 / CFloatList=8)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueFloatList(pub Vec<f32>);
impl Value for ValueFloatList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SFloatList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CFloatList
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueFloat(*x);
                let side = Side::Server;
                v.encode_typed(true, side)
            }).collect(),
        )).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

/// 字符串列表(SStringList=11 / CStringList=10)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueStringList(pub Vec<String>);
impl Value for ValueStringList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SStringList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CStringList
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueString(x.clone());
                let side = Side::Server;
                v.encode_typed(true, side)
            }).collect(),
        )).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

/// 向量列表(SVectorList=15 / CVectorList=12)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueVectorList(pub Vec<(f32, f32, f32)>);
impl Value for ValueVectorList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SVectorList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CVectorList
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValList(list_storage(
            self.0
                .iter()
                .map(|&(x, y, z)| {
                    let v = &ValueVec3 { x, y, z };
                    let side = Side::Server;
                    v.encode_typed(true, side)
                })
                .collect(),
        )).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

/// 枚举列表(SEnumList=18 / CEnumList=17)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueEnumList(pub Vec<ValueEnum>);
impl Value for ValueEnumList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SEnumList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CEnumList
    }
    fn encode_storage(&self, side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValList(list_storage(self.0.iter().map(|x| x.encode_typed(true, side)).collect())).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

/// 阵营列表(SFactionList=24;客户端无对应类型)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueFactionList(pub Vec<i64>);
impl Value for ValueFactionList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SFactionList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueFaction(*x);
                let side = Side::Server;
                v.encode_typed(true, side)
            }).collect(),
        )).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

/// 配置表列表(SConfigList=22 / CConfigList=20)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValueConfigList(pub Vec<i64>);
impl Value for ValueConfigList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SConfigList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CConfigList
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueConfig(*x);
                let side = Side::Server;
                v.encode_typed(true, side)
            }).collect(),
        )).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

/// 预制体列表(SPrefabList=23;客户端无对应类型)
#[derive(Clone, Debug)]
#[derive(Default)]
pub struct ValuePrefabList(pub Vec<i64>);
impl Value for ValuePrefabList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SPrefabList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, _side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValuePrefab(*x);
                let side = Side::Server;
                v.encode_typed(true, side)
            }).collect(),
        )).into()
    }

    fn encode_field_value(&self) -> Val {
        todo!()
    }
}

// ---------- 字典(仅服务器,SDict=27) ----------

/// 字典/哈希表(SDict=27;客户端不支持 Map)。
/// 非泛型:键/值类型由各自 `AnyValue` 自身携带,不另存类型参数。
#[derive(Clone, Debug)]
pub struct ValueDict {
    /// 键类型(空字典时仍需要,用于 schema 的 key_type)
    pub key_type: AnyValue,
    /// 值类型(用于 schema 的 value_type / value_struct_id)
    pub value_type: AnyValue,
    pub data: Vec<(AnyValue, AnyValue)>,
}

impl ValueDict {
    /// `impl Into<AnyValue>`:具体 Value 类型和 AnyValue 都能直接传
    pub fn new(key_type: impl Into<AnyValue>, value_type: impl Into<AnyValue>) -> Self {
        Self {
            key_type: key_type.into(),
            value_type: value_type.into(),
            data: Vec::new(),
        }
    }
    pub fn infer(data: Vec<(AnyValue, AnyValue)>) -> Result<Self> {
        if let Some(first) = data.first() {
            Ok(Self {
                key_type: CloneValue::clone(first.0.as_ref()),
                value_type: CloneValue::clone(first.1.as_ref()),
                data,
            })
        } else {
            Err(anyhow!("Cannot infer type"))
        }
    }
}

impl Value for ValueDict {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SDict
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, side: Side) -> Option<typed_value::Storage> {
        // MapStorage.pairs 是 TypedValue 列表,每个元素用 ValPair 包装键值对
        typed_value::Storage::ValMap(MapStorage {
            pairs: self
                .data
                .iter()
                .map(|(k, v)| {
                    TypedValue {
                        widget: typed_value::WidgetType::MapPairItem as i32,
                        is_set: true,
                        r#type: None,
                        tracker: None,
                        storage: Some(typed_value::Storage::ValPair(Box::new(MapPairStorage {
                            key: Some(Box::new(k.encode_typed(true, side))),
                            value: Some(Box::new(v.encode_typed(true, side))),
                        }))),
                    }
                })
                .collect(),
        }).into()
    }
    fn encode_schema(&self) -> Option<type_definition::server_type::Schema> {
        Some(type_definition::server_type::Schema::MapBinding(type_definition::MapKeyValueBinding {
            key_type: self.key_type.get_server_type() as i32,
            value_type: self.value_type.get_server_type() as i32,
            value_struct_id: self.value_type.as_ref().downcast_ref::<ValueStruct>().ok().map(|x| x.st.root.guid),
        }))
    }
    fn encode_type_detail(&self) -> Option<pin_interface::type_info::Detail> {
        pin_interface::type_info::Detail::MapType(pin_interface::type_info::MapType {
            key: self.key_type.get_server_type() as i32,
            value: self.value_type.get_server_type() as i32,
            value_id: self.value_type.as_ref().downcast_ref::<ValueStruct>().ok().map(|x| x.st.root.guid),
            unknown: 1,
            unknown1: 1,
        }).into()
    }
    fn is_instance(&self, value: &Box<dyn Value>) -> bool {
        matches!(value.downcast_ref::<ValueDict>(), Ok(value)
            if self.key_type.is_instance(&value.key_type) && self.value_type.is_instance(&value.value_type))
    }

    fn encode_subtype(&self) -> Option<Subtype> {
        Subtype {
            is_set: true,
            struct_id: 0x41000001, // TODO
            key: (self.key_type.get_server_type() as i32).into(),
            value: (self.value_type.get_server_type() as i32).into(),
            value_id: self.value_type.downcast_ref::<ValueStruct>().ok().map(ValueStruct::get_struct_id),
        }.into()
    }

    fn encode_field_value(&self) -> Val {
        if !self.data.is_empty() {
            todo!();
        }
        Val::Dict(Dict {
            pairs: vec![],
            keys: vec![],
            values: vec![],
            key_type: self.key_type.get_server_type() as i32,
            value_type: self.value_type.get_server_type() as i32,
            value_type_id: self.value_type.downcast_ref::<ValueStruct>().ok().map(ValueStruct::get_struct_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_types_encode_storage() {
        assert_eq!(ValueFloat(1.5).get_server_type(), ServerTypeId::SFloat);
        assert_eq!(ValueFloat(1.5).get_client_type(), ClientTypeId::CFloat);
        let f = ValueFloat(1.5).encode_typed(true, Side::Server);
        assert!(matches!(f.storage, Some(typed_value::Storage::ValFloat(_))));

        let v = ValueVec3 { x: 1.0, y: 2.0, z: 3.0 }.encode_typed(true, Side::Server);
        assert!(matches!(v.storage, Some(typed_value::Storage::ValVector(_))));

        let g = ValueGuid(42).encode_typed(true, Side::Server);
        assert!(matches!(g.storage, Some(typed_value::Storage::ValId(_))));
    }

    #[test]
    fn list_types_encode() {
        assert_eq!(ValueIntList(vec![]).get_server_type(), ServerTypeId::SIntList);
        let l = ValueIntList(vec![1, 2, 3]).encode_typed(true, Side::Server);
        let Some(typed_value::Storage::ValList(list)) = l.storage else {
            panic!("not a list");
        };
        assert_eq!(list.element.len(), 3);
        assert!(matches!(list.element[0].storage, Some(typed_value::Storage::ValInt(_))));

        let sl = ValueStringList(vec!["a".to_string()]).encode_typed(true, Side::Server);
        let Some(typed_value::Storage::ValList(sl_list)) = sl.storage else {
            panic!("not a list");
        };
        assert!(matches!(sl_list.element[0].storage, Some(typed_value::Storage::ValString(_))));
    }

    #[test]
    fn dict_encodes_pairs() {
        assert_eq!(ValueDict::new(ValueString::default(), ValueString::default()).get_server_type(), ServerTypeId::SDict);
        let d = ValueDict::infer(vec![(ValueString("k".to_string()).into(), ValueInt(1).into())])
            .unwrap()
            .encode_typed(true, Side::Server);
        let Some(typed_value::Storage::ValMap(map)) = d.storage else {
            panic!("not a map");
        };
        assert_eq!(map.pairs.len(), 1);
        assert!(matches!(map.pairs[0].storage, Some(typed_value::Storage::ValPair(_))));
    }

    #[test]
    fn server_only_types_use_client_unknown() {
        assert_eq!(ValueFactionList(vec![]).get_client_type(), ClientTypeId::ClientUnknown);
        assert_eq!(ValueDict::new(ValueString::default(), ValueString::default()).get_client_type(), ClientTypeId::ClientUnknown);
        assert_eq!(ValueLocalVarRef.get_client_type(), ClientTypeId::ClientUnknown);
    }
}
