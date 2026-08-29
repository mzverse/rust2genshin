use std::fmt::Debug;
use crate::asset::generated::{pin_interface, type_definition, typed_value, vec3f, ClientTypeId, Enum, Flt, Id, Int, ListStorage, MapPairStorage, MapStorage, PolymorphicValue, ServerTypeId, Str, StructStorage, TypeDefinition, TypedValue, Vec3f};
use anyhow::{Result, anyhow};
use downcast::{Any, downcast};
use crate::asset::generated::typed_value::{Storage, WidgetType};

#[derive(Clone, Copy)]
pub enum Side {
    Server,
    Client,
}

pub type AnyValue = Box<dyn Value>;
impl<T: Value> From<T> for AnyValue {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}
impl dyn Value {
    pub(crate) fn into_selected(self: Box<Self>, is_set: bool, f: impl FnOnce(AnyValue) -> Result<i32>) -> Result<ValueSelected> {
        Ok(ValueSelected {
            index: f(self.clone())?,
            value: self,
            has_default: is_set,
        })
    }
}
pub trait CloneValue {
    fn clone(&self) -> AnyValue;
}
pub trait Value: Any + CloneValue + Debug {
    fn encode(&self, is_set: bool, side: Side) -> TypedValue {
        TypedValue {
            widget: self.get_widget_type() as i32,
            is_set,
            r#type: Some(TypeDefinition {
                backend: match side {
                    Side::Server => type_definition::Backend::Server as i32,
                    Side::Client => type_definition::Backend::Client as i32,
                },
                type_detail: Some(match side {
                    Side::Server => type_definition::TypeDetail::ServerSide(type_definition::ServerType {
                        type_tag: self.get_server_type() as i32,
                        r#impl: 0,
                        schema: self.encode_schema(),
                    }),
                    Side::Client => type_definition::TypeDetail::ClientSide(type_definition::ClientType {
                        type_tag: self.get_client_type() as i32,
                    }),
                }),
            }),
            tracker: None,
            storage: if is_set { Some(self.encode_storage(side)) } else { None },
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

    fn encode_storage(&self, side: Side) -> typed_value::Storage;

    fn encode_schema(&self) -> Option<type_definition::server_type::Schema> {
        None
    }

    fn encode_type_detail(&self) -> Option<pin_interface::type_info::Detail> {
        None
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
    fn def() -> Box<Self> {
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
pub struct ValueSelected {
    pub index: i32,
    pub value: AnyValue,
    pub has_default: bool,
}

/// 多态值(ValueSelected)解包取实际值;普通值原样返回。
/// 泛型节点的 make_selected / get_type 判型前先解包。
pub fn unwrap_selected(value: &AnyValue) -> AnyValue {
    if value.is::<ValueSelected>() {
        value.downcast_ref::<ValueSelected>().unwrap().value.clone()
    } else {
        value.clone()
    }
}

impl Value for ValueSelected {
    fn get_widget_type(&self) -> WidgetType {
        WidgetType::TypeSelector
    }

    fn get_server_type(&self) -> ServerTypeId {
        self.value.get_server_type()
    }

    fn get_client_type(&self) -> ClientTypeId {
        self.value.get_client_type()
    }

    fn encode_storage(&self, side: Side) -> Storage {
        Storage::ValPoly(PolymorphicValue {
            chosen_type_index: self.index,
            actual_value: Some(self.value.encode(self.has_default, side).into()),
            extra_meta: None,
        }.into())
    }
}

#[derive(Clone, Debug)]
pub struct ValueBool(pub bool);
impl Default for ValueBool {
    fn default() -> Self {
        Self(Default::default())
    }
}
impl Value for ValueBool {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SBoolean
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CBoolean
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValEnum(Enum { value: match self.0 {
            true => 1,
            false => 0
        }})
    }
}

#[derive(Clone, Debug)]
pub struct ValueInt(pub i32);
impl Default for ValueInt {
    fn default() -> Self {
        Self(Default::default())
    }
}
impl Value for ValueInt {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SInt
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CInt
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValInt(Int { value: self.0 })
    }
}

#[derive(Clone, Debug)]
pub struct ValueString(pub String);
impl Default for ValueString {
    fn default() -> Self {
        Self(Default::default())
    }
}
impl Value for ValueString {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SString
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CString
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValString(Str { value: self.0.clone() })
    }
}

// ========================================================================
// 其余内置类型(排除 SStruct / SStructList)
// ========================================================================

// ---------- 标量 ----------

/// 浮点数(SFloat=5 / CFloat=7)
#[derive(Clone, Debug)]
pub struct ValueFloat(pub f32);
impl Default for ValueFloat {
    fn default() -> Self {
        Self(0.0)
    }
}
impl Value for ValueFloat {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SFloat
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CFloat
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValFloat(Flt { value: self.0 })
    }
}

/// 三维向量(SVector=12 / CVector=11)
#[derive(Clone, Debug)]
pub struct ValueVector(pub f32, pub f32, pub f32);
impl Default for ValueVector {
    fn default() -> Self {
        Self(0.0, 0.0, 0.0)
    }
}
impl Value for ValueVector {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SVector
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CVector
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValVector(Vec3f {
            value: Some(vec3f::Value { x: self.0, y: self.1, z: self.2 }),
        })
    }
}

/// 全局唯一 ID(SGuid=2 / CGuid=14)
#[derive(Clone, Debug)]
pub struct ValueGuid(pub i64);
impl Default for ValueGuid {
    fn default() -> Self {
        Self(0)
    }
}
impl Value for ValueGuid {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SGuid
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CGuid
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValId(Id { value: self.0 })
    }
}

/// 运行时实体对象(句柄)(SEntity=1 / CEntity=1)
#[derive(Clone, Debug)]
pub struct ValueEntity(pub i64);
impl Default for ValueEntity {
    fn default() -> Self {
        Self(0)
    }
}
impl Value for ValueEntity {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SEntity
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CEntity
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValId(Id { value: self.0 })
    }
}

/// 枚举项(SEnumItem=14 / CEnumItem=13)
#[derive(Clone, Debug)]
pub struct ValueEnum(pub i64);
impl Default for ValueEnum {
    fn default() -> Self {
        Self(0)
    }
}
impl Value for ValueEnum {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SEnumItem
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CEnumItem
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValEnum(Enum { value: self.0 })
    }
}

/// 阵营/势力(SFaction=17 / CFaction=16)
#[derive(Clone, Debug)]
pub struct ValueFaction(pub i64);
impl Default for ValueFaction {
    fn default() -> Self {
        Self(0)
    }
}
impl Value for ValueFaction {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SFaction
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CFaction
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValId(Id { value: self.0 })
    }
}

/// 配置表引用(SConfig=20 / CConfig=18)
#[derive(Clone, Debug)]
pub struct ValueConfig(pub i64);
impl Default for ValueConfig {
    fn default() -> Self {
        Self(0)
    }
}
impl Value for ValueConfig {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SConfig
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CConfig
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValId(Id { value: self.0 })
    }
}

/// 预制体引用(SPrefab=21 / CPrefab=19)
#[derive(Clone, Debug)]
pub struct ValuePrefab(pub i64);
impl Default for ValuePrefab {
    fn default() -> Self {
        Self(0)
    }
}
impl Value for ValuePrefab {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SPrefab
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CPrefab
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValId(Id { value: self.0 })
    }
}

// ---------- 运行时引用(仅服务器,客户端无对应类型) ----------

/// 局部变量引用(SLocalVarRef=16,运行时栈内存引用)
#[derive(Clone, Debug)]
pub struct ValueLocalVarRef(pub u32);
impl Default for ValueLocalVarRef {
    fn default() -> Self {
        Self(0)
    }
}
impl Value for ValueLocalVarRef {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SLocalVarRef
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValId(Id { value: self.0 as i64 })
    }
}

/// 变量快照引用(SVarSnapshotRef=28,实体删除时访问原始数据)
#[derive(Clone, Debug)]
pub struct ValueVarSnapshotRef(pub u32);
impl Default for ValueVarSnapshotRef {
    fn default() -> Self {
        Self(0)
    }
}
impl Value for ValueVarSnapshotRef {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SVarSnapshotRef
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValId(Id { value: self.0 as i64 })
    }
}

// ---------- 列表 ----------

/// 实体列表(SEntityList=13 / CEntityList=2)
#[derive(Clone, Debug)]
pub struct ValueEntityList(pub Vec<i64>);
impl Default for ValueEntityList {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl Value for ValueEntityList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SEntityList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CEntityList
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueEntity(*x);
                let side = Side::Server;
                v.encode(true, side)
            }).collect(),
        ))
    }
}

/// GUID 列表(SGuidList=7 / CGuidList=15)
#[derive(Clone, Debug)]
pub struct ValueGuidList(pub Vec<i64>);
impl Default for ValueGuidList {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl Value for ValueGuidList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SGuidList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CGuidList
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueGuid(*x);
                let side = Side::Server;
                v.encode(true, side)
            }).collect(),
        ))
    }
}

/// 整数列表(SIntList=8 / CIntList=4)
#[derive(Clone, Debug)]
pub struct ValueIntList(pub Vec<i32>);
impl Default for ValueIntList {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl Value for ValueIntList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SIntList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CIntList
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueInt(*x);
                let side = Side::Server;
                v.encode(true, side)
            }).collect(),
        ))
    }
}

/// 布尔列表(SBooleanList=9 / CBooleanList=6)
#[derive(Clone, Debug)]
pub struct ValueBoolList(pub Vec<bool>);
impl Default for ValueBoolList {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl Value for ValueBoolList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SBooleanList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CBooleanList
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueBool(*x);
                let side = Side::Server;
                v.encode(true, side)
            }).collect(),
        ))
    }
}

/// 浮点列表(SFloatList=10 / CFloatList=8)
#[derive(Clone, Debug)]
pub struct ValueFloatList(pub Vec<f32>);
impl Default for ValueFloatList {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl Value for ValueFloatList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SFloatList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CFloatList
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueFloat(*x);
                let side = Side::Server;
                v.encode(true, side)
            }).collect(),
        ))
    }
}

/// 字符串列表(SStringList=11 / CStringList=10)
#[derive(Clone, Debug)]
pub struct ValueStringList(pub Vec<String>);
impl Default for ValueStringList {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl Value for ValueStringList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SStringList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CStringList
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueString(x.clone());
                let side = Side::Server;
                v.encode(true, side)
            }).collect(),
        ))
    }
}

/// 向量列表(SVectorList=15 / CVectorList=12)
#[derive(Clone, Debug)]
pub struct ValueVectorList(pub Vec<(f32, f32, f32)>);
impl Default for ValueVectorList {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl Value for ValueVectorList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SVectorList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CVectorList
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValList(list_storage(
            self.0
                .iter()
                .map(|&(x, y, z)| {
                    let v = &ValueVector(x, y, z);
                    let side = Side::Server;
                    v.encode(true, side)
                })
                .collect(),
        ))
    }
}

/// 枚举列表(SEnumList=18 / CEnumList=17)
#[derive(Clone, Debug)]
pub struct ValueEnumList(pub Vec<i64>);
impl Default for ValueEnumList {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl Value for ValueEnumList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SEnumList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CEnumList
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueEnum(*x);
                let side = Side::Server;
                v.encode(true, side)
            }).collect(),
        ))
    }
}

/// 阵营列表(SFactionList=24;客户端无对应类型)
#[derive(Clone, Debug)]
pub struct ValueFactionList(pub Vec<i64>);
impl Default for ValueFactionList {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl Value for ValueFactionList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SFactionList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueFaction(*x);
                let side = Side::Server;
                v.encode(true, side)
            }).collect(),
        ))
    }
}

/// 配置表列表(SConfigList=22 / CConfigList=20)
#[derive(Clone, Debug)]
pub struct ValueConfigList(pub Vec<i64>);
impl Default for ValueConfigList {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl Value for ValueConfigList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SConfigList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CConfigList
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValueConfig(*x);
                let side = Side::Server;
                v.encode(true, side)
            }).collect(),
        ))
    }
}

/// 预制体列表(SPrefabList=23;客户端无对应类型)
#[derive(Clone, Debug)]
pub struct ValuePrefabList(pub Vec<i64>);
impl Default for ValuePrefabList {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl Value for ValuePrefabList {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SPrefabList
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, _side: Side) -> typed_value::Storage {
        typed_value::Storage::ValList(list_storage(
            self.0.iter().map(|x| {
                let v = &ValuePrefab(*x);
                let side = Side::Server;
                v.encode(true, side)
            }).collect(),
        ))
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
        if let Some(first) = data.get(0) {
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
    fn encode_storage(&self, side: Side) -> typed_value::Storage {
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
                            key: Some(Box::new(k.encode(true, side))),
                            value: Some(Box::new(v.encode(true, side))),
                        }))),
                    }
                })
                .collect(),
        })
    }
    fn encode_schema(&self) -> Option<type_definition::server_type::Schema> {
        Some(type_definition::server_type::Schema::MapBinding(type_definition::MapKeyValueBinding {
            key_type: self.key_type.get_server_type() as i32,
            value_type: self.value_type.get_server_type() as i32,
            value_struct_id: if let Ok(value) = self.value_type.as_ref().downcast_ref::<ValueStruct>() {
                Some(value.struct_id)
            } else {
                None
            },
        }))
    }
    fn encode_type_detail(&self) -> Option<pin_interface::type_info::Detail> {
        Some(pin_interface::type_info::Detail::MapType(pin_interface::type_info::MapType {
            key: self.key_type.get_server_type() as i32,
            value: self.value_type.get_server_type() as i32,
            struct_id: if let Ok(value) = self.value_type.as_ref().downcast_ref::<ValueStruct>() {
                Some(value.struct_id)
            } else {
                None
            },
        }))
    }
}

// ---------- 结构体值(SStruct=25,仅服务器) ----------

/// 结构体值(SStruct=25;客户端不支持 Struct)。
/// `struct_id` 指向 StructureDefinition 的 schema_id;`fields` 为字段值,
/// 按结构体定义顺序排列。
#[derive(Clone, Debug)]
pub struct ValueStruct {
    pub struct_id: i64,
    pub fields: Vec<AnyValue>,
}
impl ValueStruct {
    pub fn new(struct_id: i64, fields: Vec<AnyValue>) -> Self {
        Self { struct_id, fields }
    }
}
impl Value for ValueStruct {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SStruct
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, side: Side) -> typed_value::Storage {
        typed_value::Storage::ValStruct(StructStorage {
            field: self.fields.iter().map(|f| f.encode(true, side)).collect(),
        })
    }
    fn encode_schema(&self) -> Option<type_definition::server_type::Schema> {
        Some(type_definition::server_type::Schema::StructRef(type_definition::StructReference {
            schema_id: self.struct_id,
        }))
    }
    fn encode_type_detail(&self) -> Option<pin_interface::type_info::Detail> {
        Some(pin_interface::type_info::Detail::StructId(pin_interface::type_info::StructId { val: self.struct_id }),)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_types_encode_storage() {
        assert_eq!(ValueFloat(1.5).get_server_type(), ServerTypeId::SFloat);
        assert_eq!(ValueFloat(1.5).get_client_type(), ClientTypeId::CFloat);
        let f = ValueFloat(1.5).encode(true, Side::Server);
        assert!(matches!(f.storage, Some(typed_value::Storage::ValFloat(_))));

        let v = ValueVector(1.0, 2.0, 3.0).encode(true, Side::Server);
        assert!(matches!(v.storage, Some(typed_value::Storage::ValVector(_))));

        let g = ValueGuid(42).encode(true, Side::Server);
        assert!(matches!(g.storage, Some(typed_value::Storage::ValId(_))));

        let e = ValueEnum(7).encode(true, Side::Server);
        assert!(matches!(e.storage, Some(typed_value::Storage::ValEnum(_))));
    }

    #[test]
    fn list_types_encode() {
        assert_eq!(ValueIntList(vec![]).get_server_type(), ServerTypeId::SIntList);
        let l = ValueIntList(vec![1, 2, 3]).encode(true, Side::Server);
        let Some(typed_value::Storage::ValList(list)) = l.storage else {
            panic!("not a list");
        };
        assert_eq!(list.element.len(), 3);
        assert!(matches!(list.element[0].storage, Some(typed_value::Storage::ValInt(_))));

        let sl = ValueStringList(vec!["a".to_string()]).encode(true, Side::Server);
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
            .encode(true, Side::Server);
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
        assert_eq!(ValueLocalVarRef(0).get_client_type(), ClientTypeId::ClientUnknown);
    }

    #[test]
    fn struct_value_encodes() {
        assert_eq!(ValueStruct::new(100, vec![]).get_server_type(), ServerTypeId::SStruct);
        assert_eq!(ValueStruct::new(100, vec![]).get_client_type(), ClientTypeId::ClientUnknown);

        let s = ValueStruct::new(100, vec![Box::new(ValueInt(10)) as AnyValue]).encode(true, Side::Server);
        let Some(typed_value::Storage::ValStruct(st)) = s.storage else {
            panic!("not a struct");
        };
        assert_eq!(st.field.len(), 1);
        assert!(matches!(st.field[0].storage, Some(typed_value::Storage::ValInt(_))));

        // schema 携带 struct_id(StructReference)
        let Some(TypeDefinition { type_detail: Some(type_definition::TypeDetail::ServerSide(ty)), .. }) = s.r#type else {
            panic!("no server type");
        };
        assert!(matches!(
            ty.schema,
            Some(type_definition::server_type::Schema::StructRef(ref r)) if r.schema_id == 100
        ));
    }
}
