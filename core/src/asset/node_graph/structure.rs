//! 节点图模型:节点标识、引脚、黑板变量、结构体定义。
//!
//! 分层设计:NodeGraph(深度封装,INode 节点,连线单向)→ RawNodeGraph(proto 的
//! 简单封装,包含全部信息)→ proto。`RawNodeGraph::encode` 把图编码为资产。

use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::identifier::{AssetKind, Category};
use crate::asset::generated::structure_definition_data::{self, var_def as sd_var_def};
use crate::asset::generated::*;
use crate::asset::node_graph::{NodeKind, PinType};
use crate::asset::node_graph::composite::{encode_node_decl, node_decl, NodeGraphDecl};
use crate::asset::value::{AnyValue, Value, ValueBool, ValueFloat, ValueGuid, ValueInt, ValueString, ValueStruct, ValueVector};
use crate::asset::{Asset, Side};
use std::collections::HashMap;


/// 拼装结构体: 字段值 → 结构体
pub fn node_assemble_struct(st: ValueStruct) -> NodeKind {
    node_decl(st.struct_id + StructureDefinition::OFFSET_ASSEMBLE, 0, 0, st.fields.clone(), vec![st.into()])
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
    pub const OFFSET_ASSEMBLE: i64 = 0x100000;
    pub const OFFSET_DESTRUCT: i64 = Self::OFFSET_ASSEMBLE * 2;
    pub const OFFSET_MODIFY: i64 = Self::OFFSET_ASSEMBLE * 3;

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

    fn encode_decl_assemble(&self, id: i64) -> AssetData {
        let mut pins = HashMap::default();
        pins.insert(PinType::InValue, self.fields.iter().map(|x| (x.name.clone(), Some(x.value.clone()), None)).collect::<Vec<_>>());
        pins.insert(PinType::OutValue, vec![(self.name.clone(), Some(ValueStruct::new(id, vec![]).into()), PinSignature {
            kind: pin_signature::Kind::StructRef as i32,
            index: 0,
            source_ref: None,
        }.into())]);
        encode_node_decl(id + Self::OFFSET_ASSEMBLE, &NodeGraphDecl {
            name: "Assemble Struct".to_string(),
            description: "".to_string(),
            pins,
            implementation: node_interface::Implementation {
                category: node_interface::implementation::Category::StructAssembly as i32,
                template: node_interface::implementation::Template::AssembleStruct(node_interface::implementation::Id { id }).into(),
            },
            template_root: node_interface::TemplateRoot::Struct,
            template_sub: node_interface::TemplateSub::StructSub,
        }, vec![Identifier {
            source: 0,
            category: Category::Default as i32,
            kind: AssetKind::Structure as i32,
            guid: id,
            runtime_id: 0,
        }])
    }
}

impl Asset for StructureDefinition {
    fn encode(&self, _side: Side, id: i64) -> Vec<AssetData> {
        // Field.index 对齐 structVersion(真实导出中二者相等)
        let field = self.encode_field(id, self.version);
        let mut result = vec![
            self.encode_decl_assemble(id),
        ];
        result.push(AssetData {
            // 对齐真实导出:source_domain 省略(0)、runtime_id 省略(0)
            id: Some(Identifier {
                source: 0,
                category: Category::Default as i32,
                kind: AssetKind::Structure as i32,
                guid: id,
                runtime_id: 0,
            }),
            references: result.iter().map(|x: &AssetData| x.id.unwrap()).collect(),
            name: self.name.clone(),
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
