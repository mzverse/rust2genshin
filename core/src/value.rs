use crate::asset::{Identifier, Side};
use crate::asset::generated::{pin_interface, structure_definition_data, type_definition, typed_value, vec3f, ClientTypeId, DynamicTypeMetadata, Enum, Flt, Id, Int, ListStorage, MapPairStorage, MapStorage, ServerTypeId, Str, StructStorage, TypeDefinition, TypedValue, Vec3f, dynamic_type_metadata};
use crate::structure::StructRef;
use downcast::{Any, downcast};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum NativeKind {
    Bool,
    Int,
    Float,
    String,
    Vec3,
    Guid,
    Faction,
    Config,
    Prefab,
    Entity,
    Enum(i32),
    LocalVarRef,
    VarSnapshotRef,
    List(Box<NativeKind>),
    Dict {
        key: Box<NativeKind>,
        value: Box<NativeKind>,
    },
    Struct(StructRef),
}
impl NativeKind {
    pub fn dict(key: Self, value: Self) -> Self {
        Self::Dict {
            key: key.into(),
            value: value.into(),
        }
    }
}
impl Display for NativeKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NativeKind::Enum(id) => write!(f, "Enum<{id}>"),
            NativeKind::List(ele) => write!(f, "List<{ele}>"),
            NativeKind::Dict { key, value } => write!(f, "Dict<{key},{value}>"),
            NativeKind::Struct(r) => panic!("{r:?}"),
            _ => Debug::fmt(self, f),
        }
    }
}

#[typetag::serde]
pub trait NativeValue: Any + value_super::NativeValueSuper + Debug {
}
downcast!(dyn NativeValue);

impl<T: NativeValue> From<T> for Box<dyn NativeValue> {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

mod value_super {
    use super::*;

    pub trait NativeValueSuper {
        fn clone(&self) -> Box<dyn NativeValue>;
        fn eq(&self, other: &dyn NativeValue) -> bool;
        fn partial_cmp(&self, other: &dyn NativeValue) -> Option<Ordering>;
    }
    impl<T: NativeValue + Clone + PartialOrd> NativeValueSuper for T {
        fn clone(&self) -> Box<dyn NativeValue> {
            Clone::clone(self).into()
        }

        fn eq(&self, other: &dyn NativeValue) -> bool {
            if let Ok(other) = other.downcast_ref::<Self>() {
                PartialEq::eq(self, other)
            } else {
                false
            }
        }

        fn partial_cmp(&self, other: &dyn NativeValue) -> Option<Ordering> {
            if let Ok(other) = other.downcast_ref::<Self>() {
                PartialOrd::partial_cmp(self, other)
            } else {
                None
            }
        }
    }
    impl Clone for Box<dyn NativeValue> {
        fn clone(&self) -> Self {
            self.as_ref().clone()
        }
    }
    impl PartialEq for dyn NativeValue {
        fn eq(&self, other: &Self) -> bool {
            self.eq(other)
        }
    }
    impl Eq for dyn NativeValue {}
    #[allow(clippy::non_canonical_partial_ord_impl)]
    impl PartialOrd for dyn NativeValue {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            self.partial_cmp(other)
        }
    }
    impl Ord for dyn NativeValue {
        fn cmp(&self, other: &Self) -> Ordering {
            self.partial_cmp(other).unwrap()
        }
    }
}

type StructValue = Vec<Box<dyn NativeValue>>;

impl NativeKind {
    // pub fn decode_server_id(id: ServerTypeId) -> Option<Self> {
    //     use NativeKind::*;
    //     use ServerTypeId::*;
    //     match id {
    //         ServerUnknown => panic!(),
    //         SEntity => Entity,
    //         SGuid => Guid,
    //         SInt => Int,
    //         SBoolean => Bool,
    //         SFloat => Float,
    //         SString => String,
    //         SVector => Vec3,
    //         SGuidList => List(Guid.into()),
    //         SIntList => List(Int.into()),
    //         SBooleanList => List(Bool.into()),
    //         SFloatList => List(Float.into()),
    //         SStringList => List(String.into()),
    //         SEntityList => List(Entity.into()),
    //         SVectorList => List(Vec3.into()),
    //         SFaction => Faction,
    //         SConfig => Config,
    //         SPrefab => Prefab,
    //         SConfigList => List(Config.into()),
    //         SPrefabList => List(Prefab.into()),
    //         SFactionList => List(Faction.into()),
    //         SLocalVarRef => LocalVarRef,
    //         SVarSnapshotRef => VarSnapshotRef,
    //         SEnumItem |
    //         SEnumList |
    //         SStruct |
    //         SStructList |
    //         SDict => return None,
    //     }.into()
    // }
    //
    // pub fn decode_server_id_with_st(id: ServerTypeId, st: Option<i64>) -> Self {
    //     if let Some(result) = Self::decode_server_id(id) {
    //         return result;
    //     };
    //     assert_eq!(id, ServerTypeId::SStruct);
    //     NativeKind::Struct(id_struct(st.unwrap()))
    // }
    //
    // pub fn decode_type_info(info: pin_interface::TypeInfo) -> Self {
    //     let pin_interface::TypeInfo {
    //         var_type_kernel, detail, ..
    //     } = info;
    //     Self::decode_type_detail(var_type_kernel.unwrap().try_into().unwrap(), detail)
    // }
    //
    // pub fn decode_type_detail(id: ServerTypeId, detail: Option<pin_interface::type_info::Detail>) -> Self {
    //     if let Some(result) = Self::decode_server_id(id) {
    //         return result;
    //     };
    //     use NativeKind::*;
    //     use ServerTypeId::*;
    //     use pin_interface::type_info::Detail;
    //     #[allow(unreachable_patterns)]
    //     match detail.unwrap() {
    //         Detail::EnumId(result) => {
    //             assert_eq!(id, SEnumItem);
    //             Enum(result.val)
    //         }
    //         Detail::ListItemType(ele) => {
    //             let result = Self::decode_type_info(*ele.item_type.unwrap());
    //             match id {
    //                 SEnumList => assert_matches!(result, Enum(_)),
    //                 SStructList => assert_matches!(result, Struct(_)),
    //                 _ => panic!(),
    //             }
    //             List(result.into())
    //         },
    //         Detail::StructId(result) => {
    //             assert_eq!(id, SStruct);
    //             Struct(id_struct(result.val))
    //         }
    //         Detail::MapType(ele) => {
    //             assert_eq!(id, SDict);
    //             Dict {
    //                 key: Self::decode_server_id(ele.key.try_into().unwrap()).unwrap().into(),
    //                 value: Self::decode_server_id_with_st(ele.value.try_into().unwrap(), ele.value_id).into(),
    //             }
    //         }
    //         _ => todo!(),
    //     }
    // }

    pub fn encode_type(&self, side: Side) -> type_definition::TypeDetail {
        match side {
            Side::Server => type_definition::TypeDetail::ServerSide(type_definition::ServerType {
                type_tag: self.get_server_id() as i32,
                r#impl: type_definition::server_type::Implementation::Primitive as i32,
                schema: self.encode_schema(),
            }),
            Side::Client => type_definition::TypeDetail::ClientSide(type_definition::ClientType {
                type_tag: self.get_client_id() as i32,
            }),
        }
    }

    pub fn encode_typed_value(&self, side: Side, value: Option<&dyn NativeValue>) -> TypedValue {
        TypedValue {
            widget: self.get_widget_type().unwrap_or(typed_value::WidgetType::Unknown) as i32,
            is_set: value.is_some(),
            r#type: TypeDefinition {
                backend: match side {
                    Side::Server => type_definition::Backend::Server as i32,
                    Side::Client => type_definition::Backend::Client as i32,
                },
                type_detail: self.encode_type(side).into(),
            }.into(),
            tracker: None,
            // 官方导出里多态引脚的「未设置」值也带一个空 storage
            //(`val_struct {}` / `val_list {}` / `val_map {}`)。
            storage: value.and_then(|x| self.encode_storage(side, x)).or_else(|| match self {
                NativeKind::Struct(_) => Some(typed_value::Storage::ValStruct(StructStorage { field: vec![] })),
                NativeKind::List(_) => Some(typed_value::Storage::ValList(ListStorage { element: vec![] })),
                NativeKind::Dict { .. } => Some(typed_value::Storage::ValMap(MapStorage { pairs: vec![] })),
                _ => None,
            }),
        }
    }

    pub fn get_reference(&self) -> Option<Identifier> {
        use NativeKind::*;
        match self {
            Struct(r) => r.id.into(),
            List(e) => e.get_reference(),
            Dict { key: _, value } => value.get_reference(),
            _ => None,
        }
    }

    /// 展示控件类型:按服务端类型分发(参考导出里
    /// int→NUMBER_INPUT / string→TEXT_INPUT / float→DECIMAL_INPUT 等,
    /// 编辑器靠它决定如何渲染变量值,UNKNOWN 会显示为空)。
    pub fn get_widget_type(&self) -> Option<typed_value::WidgetType> {
        use NativeKind::*;
        use typed_value::WidgetType::*;
        match self {
            Int => NumberInput,
            Float => DecimalInput,
            String => TextInput,
            Bool | Enum(..) => EnumPicker,
            | Guid
            | Faction
            | Config
            | Prefab
            => IdInput,
            Vec3 => VectorGroup,
            List(..) => ListGroup,
            Struct(..) => StructBlock,
            Dict { .. } => MapGroup,
            _ => return None,
        }.into()
    }

    pub fn get_server_id(&self) -> ServerTypeId {
        use NativeKind::*;
        use ServerTypeId::*;
        match self {
            Bool => SBoolean,
            Int => SInt,
            Float => SFloat,
            String => SString,
            Vec3 => SVector,
            Guid => SGuid,
            Faction => SFaction,
            Config => SConfig,
            Prefab => SPrefab,
            Entity => SEntity,
            Enum(_) => SEnumItem,
            LocalVarRef => SLocalVarRef,
            VarSnapshotRef => SVarSnapshotRef,
            List(ele) => match **ele {
                Bool => SBooleanList,
                Int => SIntList,
                Float => SFloatList,
                String => SStringList,
                Vec3 => SVectorList,
                Guid => SGuidList,
                Faction => SFactionList,
                Config => SConfigList,
                Prefab => SPrefabList,
                Entity => SEntityList,
                Enum(_) => SEnumList,
                LocalVarRef => panic!(),
                VarSnapshotRef => panic!(),
                List(_) => panic!(),
                Dict { .. } => panic!(),
                Struct { .. } => SStructList,
            },
            Dict { .. } => SDict,
            Struct { .. } => SStruct,
        }
    }
    fn get_client_id(&self) -> ClientTypeId {
        use ClientTypeId::*;
        use NativeKind::*;
        match self {
            Bool => CBoolean,
            Int => CInt,
            Float => CFloat,
            String => CString,
            Vec3 => CVector,
            Guid => CGuid,
            Faction => CFaction,
            Config => CConfig,
            Prefab => CPrefab,
            Entity => CEntity,
            Enum(_) => CEnumItem,
            LocalVarRef => panic!(),
            VarSnapshotRef => panic!(),
            List(ele) => match **ele {
                Bool => CBooleanList,
                Int => CIntList,
                Float => CFloatList,
                String => CStringList,
                Vec3 => CVectorList,
                Guid => CGuidList,
                Faction => panic!(),
                Config => CConfigList,
                Prefab => panic!(),
                Entity => CEntityList,
                Enum(_) => CEnumList,
                LocalVarRef => panic!(),
                VarSnapshotRef => panic!(),
                List(_) => panic!(),
                Dict { .. } => panic!(),
                Struct { .. } => panic!(),
            },
            Dict { .. } => panic!(),
            Struct { .. } => panic!(),
        }
    }
    pub fn get_server_shell_type(&self) -> i32 {
        use NativeKind::*;
        match self {
            Enum(id) => 10000 + *id,
            _ => self.get_server_id() as i32,
        }
    }
    pub fn get_type_id(&self, side: Side) -> i32 {
        match side {
            Side::Server => self.get_server_id() as i32,
            Side::Client => self.get_client_id() as i32,
        }
    }

    pub fn default(&self) -> Box<dyn NativeValue> {
        use NativeKind::*;
        match self {
            Bool => bool::default().into(),
            Int => i32::default().into(),
            Float => f32::default().into(),
            String => std::string::String::default().into(),
            Vec3 => self::Vec3::default().into(),
            Enum(_) => i32::default().into(),
            Guid |
            Faction |
            Config |
            Prefab => i64::default().into(),
            Entity |
            LocalVarRef |
            VarSnapshotRef => ().into(),
            List(_) => Vec::<Box<dyn NativeValue>>::default().into(),
            Dict { .. } => BTreeMap::<Box<dyn NativeValue>, Box<dyn NativeValue>>::default().into(),
            Struct(r) => r.fields.iter().map(|(_name, kind)| kind.default()).collect::<StructValue>().into(),
        }
    }

    fn encode_storage(&self, side: Side, value: &dyn NativeValue) -> Option<typed_value::Storage> {
        use NativeKind::*;
        use typed_value::Storage::*;
        match self {
            Bool => ValEnum(encode_bool(value)),
            Int => ValInt(encode_int(value)),
            Float => ValFloat(encode_float(value)),
            String => ValString(encode_string(value)),
            Vec3 => ValVector(encode_vec3(value)),
            Enum(_) => ValEnum(encode_enum(value)),
            Guid |
            Faction |
            Config |
            Prefab => ValId(encode_id(value)),
            Entity |
            LocalVarRef |
            VarSnapshotRef => {
                assert!(value.is::<()>());
                return None;
            },
            List(ele) => ValList(encode_list(side, ele, value)),
            Dict { key: k, value: v } => ValMap(encode_dict(side, k, v, value)),
            Struct(st) => {
                let value = value.downcast_ref::<StructValue>().unwrap();
                ValStruct(StructStorage {
                    field: st.fields.iter().enumerate().map(|(i, (_name, kind))| kind.encode_typed_value(side, Some(value[i].as_ref()))).collect(),
                })
            }
        }.into()
    }

    pub fn encode_field_value(&self, value: &dyn NativeValue) -> Option<structure_definition_data::var_def::value::Val> {
        use NativeKind::*;
        use structure_definition_data::var_def::value::Val::*;
        match self {
            Bool => BooleanVal(encode_bool(value)),
            Int => IntVal(encode_int(value)),
            Float => FloatVal(encode_float(value)),
            String => StrVal(encode_string(value)),
            Vec3 => Vec3Val(encode_vec3(value)),
            Guid => GuidVal(encode_id(value)),
            Enum(_) => todo!(),
            Faction => todo!(),
            Config => todo!(),
            Prefab => todo!(),
            | Entity
            | LocalVarRef
            | VarSnapshotRef
            => return None,
            List(ele) => encode_field_list(ele, value),
            NativeKind::Dict { key: k, value: v } => encode_field_dict(k, v, value),
            Struct(st) => Structure(encode_struct_val(st, value)),
        }.into()
    }

    pub fn encode_schema(&self) -> Option<type_definition::server_type::Schema> {
        use NativeKind::*;
        use type_definition::server_type::Schema::*;
        match self {
            Dict { key, value } => MapBinding(type_definition::MapKeyValueBinding {
                key_type: key.get_server_id() as i32,
                value_type: value.get_server_id() as i32,
                value_struct_id: match value.as_ref() {
                    Struct(st) => st.id.guid.into(),
                    _ => None,
                },
            }).into(),
            Struct(st) => StructRef(type_definition::StructReference {
                schema_id: st.id.guid,
            }).into(),
            List(ele) => ele.encode_schema(),
            _ => None,
        }
    }

    pub fn encode_type_detail(&self) -> Option<pin_interface::type_info::Detail> {
        use NativeKind::*;
        use pin_interface::type_info;
        use type_info::Detail::*;
        match self {
            Bool => EnumId(type_info::EnumId { val: 1 }).into(),
            Dict { key, value } => MapType(type_info::MapType {
                key: key.get_server_id() as i32,
                value: value.get_server_id() as i32,
                value_id: match value.as_ref() {
                    Struct(st) => st.id.guid.into(),
                    _ => None,
                },
                unknown: 1,
                unknown1: 1,
            }).into(),
            Struct(st) => StructId(type_info::StructId { val: st.id.guid }).into(),
            // 列表要带上元素类型:结构体列表没有这一项时游戏拿到的是结构体 0。
            List(ele) => ListItemType(Box::new(type_info::ListType {
                item_type: Some(Box::new(pin_interface::TypeInfo {
                    ui_class: ele.get_widget_type().map(|x| x as i32),
                    var_type_shell: Some(ele.get_server_shell_type()),
                    var_type_kernel: Some(ele.get_server_id() as i32),
                    placeholder: None,
                    display_state: None,
                    detail: ele.encode_type_detail(),
                })),
            })).into(),
            _ => None, // TODO: enum
        }
    }

    pub fn encode_subtype(&self) -> Option<structure_definition_data::var_def::Subtype> { // TODO: enum, List
        use NativeKind::*;
        use structure_definition_data::var_def::Subtype;
        match self {
            Dict { key, value } => Subtype {
                is_set: true,
                struct_id: 0x41000001, // TODO
                key: Some(key.get_server_id() as i32),
                value: Some(value.get_server_id() as i32),
                value_id: match value.as_ref() {
                    Struct(st) => st.id.guid.into(),
                    _ => None,
                },
            }.into(),
            Struct(st) => Subtype {
                is_set: true,
                struct_id: st.id.guid,
                key: None,
                value: None,
                value_id: None,
            }.into(),
            // 结构体列表同样要带元素的结构体 id:官方导出里
            // `kind { primary: S_STRUCT_LIST, sub { is_set, struct_id } }`。
            List(ele) => match ele.as_ref() {
                Struct(st) => Subtype {
                    is_set: true,
                    struct_id: st.id.guid,
                    key: None,
                    value: None,
                    value_id: None,
                }.into(),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn encode_metadata(&self) -> DynamicTypeMetadata {
        use NativeKind::*;
        use dynamic_type_metadata::config::inner::SchemaBinding::*;
        let mut item_style = None;
        let schema = match self {
            Struct(kind) => TargetStructId(type_definition::StructReference {
                schema_id: kind.id.guid,
            }),
            List(ele) if let Struct(kind) = ele.as_ref() => {
                item_style = ele.get_widget_type();
                ListItemStructId(type_definition::StructReference {
                    schema_id: kind.id.guid,
                })
            },
            Dict { .. } => todo!(),
            _ => panic!("{self:?}"),
        };
        DynamicTypeMetadata {
            version: 1,
            config: dynamic_type_metadata::Config {
                inner: dynamic_type_metadata::config::Inner {
                    container_style: self.get_widget_type().unwrap() as i32,
                    item_style: item_style.map(|x| x as i32),
                    schema_binding: schema.into(),
                }.into(),
            }.into(),
        }
    }
}

#[typetag::serde(name = "unit")]
impl NativeValue for () {
}
#[typetag::serde]
impl NativeValue for bool {
}
#[typetag::serde]
impl NativeValue for i32 {
}
#[typetag::serde]
impl NativeValue for f32 {
}
#[typetag::serde]
impl NativeValue for String {
}
#[typetag::serde]
impl NativeValue for Vec3 {
}
#[typetag::serde]
impl NativeValue for i64 {
}
#[typetag::serde]
impl NativeValue for Vec<Box<dyn NativeValue>> {
}
#[typetag::serde]
impl NativeValue for BTreeMap<Box<dyn NativeValue>, Box<dyn NativeValue>> {
}

#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Debug, PartialEq, Clone, Copy, PartialOrd, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
fn encode_bool(value: &dyn NativeValue) -> Enum {
    Enum { value: *value.downcast_ref::<bool>().unwrap() as i32 }
}

fn encode_int(value: &dyn NativeValue) -> Int {
    Int { value: *value.downcast_ref::<i32>().unwrap() }
}

fn encode_float(value: &dyn NativeValue) -> Flt {
    Flt { value: *value.downcast_ref::<f32>().unwrap() }
}

fn encode_string(value: &dyn NativeValue) -> Str {
    Str { value: value.downcast_ref::<String>().unwrap().clone() }
}

fn encode_vec3(value: &dyn NativeValue) -> Vec3f {
    let value = value.downcast_ref::<Vec3>().unwrap();
    Vec3f {
        value: vec3f::Value {
            x: value.x,
            y: value.y,
            z: value.z,
        }.into(),
    }
}

fn encode_id(value: &dyn NativeValue) -> Id {
    Id { value: *value.downcast_ref::<i64>().unwrap() }
}

fn encode_enum(value: &dyn NativeValue) -> Enum {
    Enum { value: *value.downcast_ref::<i32>().unwrap() }
}

fn encode_list(side: Side, kind: &NativeKind, value: &dyn NativeValue) -> ListStorage {
    ListStorage { element: value.downcast_ref::<Vec<Box<dyn NativeValue>>>().unwrap().iter()
        .map(|x| kind.encode_typed_value(side, x.as_ref().into())).collect() }
}

fn encode_dict(side: Side, key: &NativeKind, value: &NativeKind, data: &dyn NativeValue) -> MapStorage {
    MapStorage {
        pairs: data.downcast_ref::<HashMap<Box<dyn NativeValue>, Box<dyn NativeValue>>>().unwrap().iter()
            .map(|(k, v)| {
                TypedValue {
                    widget: typed_value::WidgetType::MapPairItem as i32,
                    is_set: true,
                    r#type: None,
                    tracker: None,
                    storage: Some(typed_value::Storage::ValPair(Box::new(MapPairStorage {
                        key: Some(Box::new(key.encode_typed_value(side, k.as_ref().into()))),
                        value: Some(Box::new(value.encode_typed_value(side, v.as_ref().into()))),
                    }))),
                }
            })
            .collect(),
    }
}

/// 结构体字段默认值里的列表:每种元素类型对应 `Val` 的一个变体。
fn encode_field_list(ele: &NativeKind, data: &dyn NativeValue) -> structure_definition_data::var_def::value::Val {
    use NativeKind::*;
    use structure_definition_data::var_def::value::{self, Val::*};
    let items = data.downcast_ref::<Vec<Box<dyn NativeValue>>>().unwrap();
    match ele {
        Guid => GuidList(value::GuidList {
            items: items.iter().map(|x| *x.downcast_ref::<i64>().unwrap()).collect(),
        }),
        Int => IntList(value::IntList {
            items: items.iter().map(|x| *x.downcast_ref::<i32>().unwrap()).collect(),
        }),
        Bool => BoolList(value::BoolList {
            items: items.iter().map(|x| *x.downcast_ref::<bool>().unwrap()).collect(),
        }),
        Float => FloatList(value::FloatList {
            items: items.iter().map(|x| *x.downcast_ref::<f32>().unwrap()).collect(),
        }),
        String => StrList(value::StrList {
            items: items.iter().map(|x| x.downcast_ref::<std::string::String>().unwrap().clone()).collect(),
        }),
        Faction => FactionList(value::FactionList {
            items: items.iter().map(|x| x.downcast_ref::<std::string::String>().unwrap().clone()).collect(),
        }),
        // 实体在存档里没有可写的值,默认值就是空引用。
        Entity => EntityList(value::EntityList {
            items: items.iter().map(|_| 0).collect(),
        }),
        Vec3 => Vec3List(value::Vec3List {
            items: items.iter().map(|x| encode_vec3(x.as_ref())).collect(),
        }),
        Config => ConfigList(value::ConfigList {
            items: items.iter().map(|x| value::RefId { id: *x.downcast_ref::<i64>().unwrap() }).collect(),
        }),
        Prefab => PrefabList(value::PrefabList {
            items: items.iter().map(|x| value::RefId { id: *x.downcast_ref::<i64>().unwrap() }).collect(),
        }),
        Struct(st) => StructList(value::StructList {
            items: items.iter().map(|x| encode_struct_value(st, x.as_ref())).collect(),
            // 501:元素结构体的 id(官方导出里和 items 的 `structId` 一致)。
            struct_id: Some(st.id.guid),
        }),
        _ => todo!("列表默认值:{ele}"),
    }.into()
}

/// 结构体列表的元素:一个完整的 `Value`(带自己的类型与 subtype)。
fn encode_struct_value(st: &StructRef, value: &dyn NativeValue) -> structure_definition_data::var_def::Value {
    let kind = NativeKind::Struct(st.clone());
    let ty = kind.get_server_id();
    structure_definition_data::var_def::Value {
        r#type: ty as i32,
        kind: structure_definition_data::var_def::Kind {
            primary: ty as i32,
            sub: kind.encode_subtype().unwrap_or_default().into(),
        }.into(),
        name: None,
        val: Some(structure_definition_data::var_def::value::Val::Structure(encode_struct_val(st, value))),
    }
}

fn encode_struct_val(st: &StructRef, value: &dyn NativeValue) -> structure_definition_data::var_def::value::StructVal {
    use structure_definition_data::var_def::value::struct_val;
    let value = value.downcast_ref::<StructValue>().unwrap();
    structure_definition_data::var_def::value::StructVal {
        fields: st.fields.iter().enumerate().map(|(i, (name, kind))| structure_definition_data::var_def::Value {
            r#type: kind.get_type_id(Side::Server),
            kind: structure_definition_data::var_def::Kind {
                primary: kind.get_type_id(Side::Server),
                sub: kind.encode_subtype().unwrap_or_default().into(),
            }.into(),
            name: name.to_string().into(),
            val: kind.encode_field_value(value[i].as_ref()),
        }).collect(),
        struct_id: st.id.guid,
        id: struct_val::Id {
            kind: ServerTypeId::SVarSnapshotRef as i32, // ?
            id: 0x40000001, // FIXME
        }.into(),
    }
}

fn encode_field_dict(key: &NativeKind, value: &NativeKind, data: &dyn NativeValue) -> structure_definition_data::var_def::value::Val {
    let data = data.downcast_ref::<HashMap<Box<dyn NativeValue>, Box<dyn NativeValue>>>().unwrap();
    if !data.is_empty() {
        todo!();
    }
    structure_definition_data::var_def::value::Val::Dict(structure_definition_data::var_def::value::Dict {
        pairs: vec![],
        keys: vec![],
        values: vec![],
        key_type: key.get_server_id() as i32,
        value_type: value.get_server_id() as i32,
        value_type_id: match value {
            NativeKind::Struct(st) => st.id.guid.into(),
            _ => None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn 结构体(id: i64) -> NativeKind {
        NativeKind::Struct(StructRef {
            id: crate::structure::id_struct(id),
            fields: vec![],
        })
    }

    #[test]
    fn 结构体列表的服务端类型是_s_struct_list() {
        let list = NativeKind::List(结构体(7).into());
        assert_eq!(list.get_server_id(), ServerTypeId::SStructList);
        // 元素本身仍是 S_STRUCT
        assert_eq!(结构体(7).get_server_id(), ServerTypeId::SStruct);
    }

    /// 每种元素类型的列表默认值都要落到自己那个 `Val` 变体上（字段号
    /// 见 `asset.proto`，从真实导出里实测）。
    #[test]
    fn 列表字段的默认值落在各自的_val_变体上() {
        use prost::Message;
        use structure_definition_data::var_def::value::Val;
        let cases: Vec<(NativeKind, fn(&Val) -> bool)> = vec![
            (NativeKind::Guid, |v| matches!(v, Val::GuidList(_))),
            (NativeKind::Int, |v| matches!(v, Val::IntList(_))),
            (NativeKind::Bool, |v| matches!(v, Val::BoolList(_))),
            (NativeKind::Float, |v| matches!(v, Val::FloatList(_))),
            (NativeKind::String, |v| matches!(v, Val::StrList(_))),
            (NativeKind::Entity, |v| matches!(v, Val::EntityList(_))),
            (NativeKind::Vec3, |v| matches!(v, Val::Vec3List(_))),
            (NativeKind::Config, |v| matches!(v, Val::ConfigList(_))),
            (NativeKind::Prefab, |v| matches!(v, Val::PrefabList(_))),
            (NativeKind::Faction, |v| matches!(v, Val::FactionList(_))),
            (结构体(7), |v| matches!(v, Val::StructList(_))),
        ];
        for (ele, check) in cases {
            let list = NativeKind::List(ele.clone().into());
            let val = list.encode_field_value(list.default().as_ref()).unwrap();
            let msg = structure_definition_data::var_def::Value {
                val: Some(val),
                ..Default::default()
            };
            let decoded = structure_definition_data::var_def::Value::decode(msg.encode_to_vec().as_slice()).unwrap();
            assert!(check(decoded.val.as_ref().unwrap()), "{ele:?} 的默认值变体不对");
        }
    }

    /// 结构体列表作为结构体字段时,`kind.sub` 要带元素的结构体 id。
    #[test]
    fn 结构体列表的_subtype_带元素结构体() {
        let list = NativeKind::List(结构体(7).into());
        let sub = list.encode_subtype().expect("结构体列表应当有 subtype");
        assert!(sub.is_set);
        assert_eq!(sub.struct_id, 7);
        // 其他列表没有 subtype
        assert!(NativeKind::List(NativeKind::Int.into()).encode_subtype().is_none());
    }

    /// 结构体列表的默认值:`structList { items, 501: 元素结构体 id }`。
    #[test]
    fn 结构体列表的默认值带结构体_id() {
        use prost::Message;
        use structure_definition_data::var_def::value::Val;
        let list = NativeKind::List(结构体(7).into());
        let val = list.encode_field_value(list.default().as_ref()).unwrap();
        let msg = structure_definition_data::var_def::Value {
            val: Some(val),
            ..Default::default()
        };
        let decoded = structure_definition_data::var_def::Value::decode(msg.encode_to_vec().as_slice()).unwrap();
        let Val::StructList(list) = decoded.val.unwrap() else {
            panic!("默认值应当是 structList");
        };
        assert_eq!(list.struct_id, Some(7), "501 要写元素的结构体 id");
    }

    #[test]
    fn 结构体列表的_schema_指回元素结构体() {
        let list = NativeKind::List(结构体(7).into());
        match list.encode_schema() {
            Some(type_definition::server_type::Schema::StructRef(r)) => assert_eq!(r.schema_id, 7),
            other => panic!("结构体列表应当带 struct_ref,实际是 {other:?}"),
        }
        // 其他列表没有 schema
        assert!(NativeKind::List(NativeKind::Int.into()).encode_schema().is_none());
    }
}
