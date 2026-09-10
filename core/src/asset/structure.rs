//! 节点图模型:节点标识、引脚、黑板变量、结构体定义。
//!
//! 分层设计:NodeGraph(深度封装,INode 节点,连线单向)→ RawNodeGraph(proto 的
//! 简单封装,包含全部信息)→ proto。`RawNodeGraph::encode` 把图编码为资产。

use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::identifier::{AssetKind, Category};
use crate::asset::generated::structure_definition_data::{self, var_def as sd_var_def};
use crate::asset::generated::*;
use crate::asset::node_graph::decl::NodeDecl;
use crate::asset::node_graph::{NodeKind, PinType};
use crate::asset::value::{AnyValue, Value, ValueBool, ValueFloat, ValueGuid, ValueInt, ValueString, ValueVector};
use crate::asset::{Asset, AssetBundle, AssetRef, Side};
use std::collections::HashMap;
use tap::Tap;

/// 拼装结构体: 字段值 → 结构体
pub fn node_assemble_struct(st: &ValueStruct) -> NodeKind {
    st.st.data.get(&StructureNodeDecl::AssembleServer).unwrap().data.clone()
}

pub fn node_destruct_struct(st: &ValueStruct) -> NodeKind {
    st.st.data.get(&StructureNodeDecl::DestructServer).unwrap().data.clone()
}

/// 结构体的一个字段(对标 GIA `StructDecl.fields[]`)
pub struct StructField {
    pub name: String,
    /// 字段值(类型由 AnyValue 自身携带,不另存 type id)
    pub value: AnyValue,
    /// 是否有默认值(is_set=false 时 typedef3.val 为空)
    pub is_set: bool,
}

impl StructField {
    /// 字段默认值 → proto `TypeDef.val`(Int / Bool / Str / Guid / Float / Vector)
    fn encode_val(v: &dyn Value) -> Option<sd_var_def::type_def::Val> {
        use sd_var_def::type_def::Val;
        if let Ok(i) = v.downcast_ref::<ValueInt>() {
            Some(Val::IntVal(Int { value: i.0 }))
        } else if let Ok(b) = v.downcast_ref::<ValueBool>() {
            Some(Val::BooleanVal(Enum { value: b.0 as i64 }))
        } else if let Ok(s) = v.downcast_ref::<ValueString>() {
            Some(Val::StrVal(Str { value: s.0.clone() }))
        } else if let Ok(g) = v.downcast_ref::<ValueGuid>() {
            // val oneof 字段 12 = Id{id:int64}(与我们的 Id{value} 同构)
            Some(Val::GuidVal(Id { value: g.0 }))
        } else if let Ok(f) = v.downcast_ref::<ValueFloat>() {
            // val oneof 字段 15 = Float{float}(与我们的 Flt{value} 同构)
            Some(Val::FloatVal(Flt { value: f.0 }))
        } else if let Ok(v3) = v.downcast_ref::<ValueVector>() {
            // val oneof 字段 22 = Vector{vec{x,y,z}}(与 Vec3f 同构)
            Some(Val::VectorVal(Vec3f {
                value: Some(vec3f::Value { x: v3.0, y: v3.1, z: v3.2 }),
            }))
        } else {
            None
        }
    }

    /// 字段 → proto `VarDef`(对齐真实导出的 wire 格式):
    ///   typedef1 = { type, subType {} }(空 subType,无 val)
    ///   typedef3 = { type, subType { type, xxxx_id {} }, val }
    fn encode_var_def(&self, index: i32) -> structure_definition_data::VarDef {
        let ty = self.value.get_server_type();
        let val = if self.is_set { Self::encode_val(self.value.as_ref()) } else { None };
        structure_definition_data::VarDef {
            typedef1: Some(sd_var_def::TypeDef {
                r#type: ty as i32,
                sub_type: Some(sd_var_def::type_def::SubType {
                    r#type: 0,
                    xxxx_id: 0,
                    key: 0,
                    value: 0,
                    value_id: 0,
                }),
                val: None,
            }),
            typedef3: Some(sd_var_def::TypeDef3 {
                r#type: ty as i32,
                sub_type: Some(sd_var_def::type_def3::SubType {
                    r#type: ty as i32,
                    xxxx_id: Some(sd_var_def::type_def3::sub_type::Any {}),
                    key: 0,
                    value: 0,
                    value_id: 0,
                }),
                val: val.clone().map(|v| match v {
                    sd_var_def::type_def::Val::IntVal(x) => sd_var_def::type_def3::Val::IntVal(x),
                    sd_var_def::type_def::Val::BooleanVal(x) => {
                        sd_var_def::type_def3::Val::BooleanVal(x)
                    }
                    sd_var_def::type_def::Val::StrVal(x) => sd_var_def::type_def3::Val::StrVal(x),
                    sd_var_def::type_def::Val::GuidVal(x) => sd_var_def::type_def3::Val::GuidVal(x),
                    sd_var_def::type_def::Val::FloatVal(x) => sd_var_def::type_def3::Val::FloatVal(x),
                    sd_var_def::type_def::Val::VectorVal(x) => {
                        sd_var_def::type_def3::Val::VectorVal(x)
                    }
                }),
            }),
            name: self.name.clone(),
            var_name: self.name.clone(),
            var_type: ty as i32,
            var_index: index,
        }
    }
}

/// 自定义结构体定义(对标 GIA 的 `interface` 声明 → StructDecl)
pub struct StructureDefinition {
    /// schema_id:被 GraphVariable.schema_ref_id / StructReference 引用
    pub name: String,
    pub version: i32,
    pub fields: Vec<StructField>,
}

impl StructureDefinition {
    /// 组装 proto `Field`(generic_field 与 concrete_field 相同,见 proto 注释)。
    /// `index` 为 Field.index,真实导出里它等于 structVersion。
    fn encode_field(&self, id: i64, index: i32) -> structure_definition_data::Field {
        structure_definition_data::Field {
            id,
            xxx: 0,
            var: self.fields.iter().enumerate().map(|(i, f)| f.encode_var_def((i as i32) + 1)).collect(),
            struct_name: self.name.clone(),
            class_base: 1,
            index,
        }
    }
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub enum StructureNodeDecl {
    AssembleServer,
    DestructServer,
}

// ---------- 结构体值(SStruct=25,仅服务器) ----------

/// 结构体值(SStruct=25;客户端不支持 Struct)。
/// `struct_id` 指向 StructureDefinition 的 schema_id;`fields` 为字段值,
/// 按结构体定义顺序排列。
#[derive(Clone, Debug)]
pub struct ValueStruct {
    pub st: AssetRef<StructureDefinition>,
    pub fields: Vec<AnyValue>,
}
impl ValueStruct {
    pub fn new(r: AssetRef<StructureDefinition>, fields: Vec<AnyValue>) -> Self {
        Self { st: r, fields }
    }
}
impl Value for ValueStruct {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SStruct
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValStruct(StructStorage {
            field: self.fields.iter().map(|f| f.encode(true, side)).collect(),
        }).into()
    }
    fn encode_schema(&self) -> Option<type_definition::server_type::Schema> {
        Some(type_definition::server_type::Schema::StructRef(type_definition::StructReference {
            schema_id: self.st.root.guid,
        }))
    }
    fn encode_type_detail(&self) -> Option<pin_interface::type_info::Detail> {
        Some(pin_interface::type_info::Detail::StructId(pin_interface::type_info::StructId { val: self.st.root.guid }),)
    }

    fn is_instance(&self, value: &Box<dyn Value>) -> bool {
        matches!(value.downcast_ref::<ValueStruct>(), Ok(value) if value.st == self.st)
    }
}

impl Asset for StructureDefinition {
    type RefData = HashMap<StructureNodeDecl, AssetRef<NodeDecl>>;

    fn apply(self, bundle: &mut AssetBundle) -> AssetRef<Self> {
        let mut result = AssetRef::new(bundle.alloc(Category::Default, AssetKind::Structure), Default::default());
        for (k, name, imp, pins) in [
            (StructureNodeDecl::AssembleServer, "Assemble Struct Server", node_interface::Implementation {
                category: node_interface::implementation::Category::StructAssembly as i32,
                template: node_interface::implementation::Template::AssembleStruct(node_interface::implementation::Id { id: result.root.guid }).into(),
            }, HashMap::default().tap_mut(|pins| {
                pins.insert(PinType::InValue, self.fields.iter().map(|x| (x.name.clone(), Some(x.value.clone()), None)).collect::<Vec<_>>());
                pins.insert(PinType::OutValue, vec![(self.name.clone(), Some(ValueStruct::new(result.clone(), vec![]).into()), PinSignature {
                    kind: pin_signature::Kind::StructRef as i32,
                    index: 0,
                    source_ref: None,
                }.into())]);
            })),
            (StructureNodeDecl::DestructServer, "Destruct Struct Server", node_interface::Implementation { // TODO
                category: node_interface::implementation::Category::StructSplit as i32,
                template: node_interface::implementation::Template::SplitStruct(node_interface::implementation::Id { id: result.root.guid }).into(),
            }, HashMap::default().tap_mut(|pins| {
                pins.insert(PinType::InValue, vec![(self.name.clone(), Some(ValueStruct::new(result.clone(), vec![]).into()), PinSignature {
                    kind: pin_signature::Kind::StructRef as i32,
                    index: 0,
                    source_ref: None,
                }.into())]);
                pins.insert(PinType::OutValue, self.fields.iter().map(|x| (x.name.clone(), Some(x.value.clone()), None)).collect::<Vec<_>>());
            })),
        ] {
            result.data.insert(k, NodeDecl {
                name: name.to_string(),
                description: "".to_string(),
                pins,
                implementation: imp,
                template_root: node_interface::TemplateRoot::Struct,
                template_sub: node_interface::TemplateSub::StructSub,
                references: vec![result.root],
            }.apply(bundle));
        }
        // Field.index 对齐 structVersion(真实导出中二者相等)
        let field = self.encode_field(result.root.guid, self.version);
        bundle.push(AssetData {
            id: result.root.into(),
            references: result.data.values().map(|x| x.root).collect(),
            name: self.name,
            r#type: asset_data::Type::Structure as i32,
            payload: Some(Payload::StructData(StructureDefinitionContainer {
                def: Some(StructureDefinitionData {
                    generic_field: Some(field.clone()),
                    concrete_field: Some(field),
                    struct_version: self.version,
                    item_count: self.fields.len() as i32,
                    unknown1: 1,
                }),
            })),
        });
        result
    }
}
