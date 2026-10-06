//! 节点图模型:节点标识、引脚、黑板变量、结构体定义。
//!
//! 分层设计:NodeGraph(深度封装,INode 节点,连线单向)→ RawNodeGraph(proto 的
//! 简单封装,包含全部信息)→ proto。`RawNodeGraph::encode` 把图编码为资产。

use crate::asset::AssetBundle;
use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::structure_definition_data::{self, var_def as sd_var_def};
use crate::asset::generated::*;
use crate::node::decl::{DeclPin, NodeDecl, node_declared};
use crate::node::{NodeKind, PinType};
use crate::value::{NativeKind, NativeValue};
use std::collections::HashMap;
use std::fmt::Debug;
use tap::Tap;

/// 拼装结构体: 字段值 → 结构体
pub fn node_assemble_struct(assets: &AssetBundle, st: &StructRef) -> NodeKind {
    let id = get_struct_node_id(assets, st.id, node_interface::implementation::Category::StructAssembly);
    node_declared(id, 0, 0, st.fields.iter().map(|x| x.1.clone()).map(Some).collect(), vec![NativeKind::Struct(st.clone())])
}

pub fn node_destructure_struct(assets: &AssetBundle, st: &StructRef) -> NodeKind {
    let id = get_struct_node_id(assets, st.id, node_interface::implementation::Category::StructSplit);
    node_declared(id, 0, 0, vec![NativeKind::Struct(st.clone()).into()], st.fields.iter().map(|x| x.1.clone()).collect())
}

pub fn node_modify_struct(assets: &AssetBundle, st: &StructRef) -> NodeKind {
    let id = get_struct_node_id(assets, st.id, node_interface::implementation::Category::StructModify);
    let mut params = vec![NativeKind::Struct(st.clone()).into()];
    params.push(None);
    for (_, x) in &st.fields {
        params.push(x.clone().into());
        params.push(NativeKind::Bool.into());
    }
    node_declared(id, 1, 1, params, vec![])
}

pub fn get_struct_node_id(assets: &AssetBundle, st: Identifier, imp: node_interface::implementation::Category) -> Identifier {
    for &x in &assets.get(st).unwrap().references {
        let node = assets.get(x).unwrap();
        if node.id.unwrap().kind != identifier::AssetKind::Basic as i32
            || node.id.unwrap().category != identifier::Category::NodeDecl as i32 {
            continue;
        }
        let Payload::InterfaceData(payload) = node.payload.as_ref().unwrap() else { panic!() };
        if payload.inner.as_ref().unwrap()
            .interface.as_ref().unwrap()
            .r#impl.as_ref().unwrap()
            .category == imp as i32 {
            return x;
        }
    }
    panic!();
}

pub struct StructField {
    pub name: String,
    pub kind: NativeKind,
    pub default: Option<Box<dyn NativeValue>>,
}

impl StructField {
    pub fn new(name: String, kind: NativeKind, default: Option<Box<dyn NativeValue>>) -> Self {
        Self { name, kind, default }
    }
    /// 字段 → proto `VarDef`(对齐真实导出的 wire 格式):
    ///   typedef1 = { type, subType {} }(空 subType,无 val)
    ///   typedef3 = { type, subType { type, xxxx_id {} }, val }
    fn encode_var_def(&self, index: i32) -> structure_definition_data::VarDef {
        let ty = self.kind.get_server_id();
        let kind = structure_definition_data::var_def::Kind {
            primary: ty as i32,
            sub: self.kind.encode_subtype().unwrap_or_default().into(),
        };
        structure_definition_data::VarDef {
            kind: kind.into(),
            name: self.name.clone(),
            var_name: self.name.clone(),
            var_type: ty as i32,
            var_index: index,
            def: sd_var_def::Value {
                r#type: ty as i32,
                kind: kind.into(),
                name: None,
                val: self.kind.encode_field_value(self.kind.default().as_ref()).into(),
            }.into(),
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
    fn encode_fields(&self, id: i64, index: i32) -> structure_definition_data::Fields {
        structure_definition_data::Fields {
            id,
            xxx: 0,
            var: self.fields.iter().enumerate().map(|(i, f)| f.encode_var_def((i as i32) + 1)).collect(),
            struct_name: self.name.clone(),
            class_base: 1,
            index,
        }
    }
}

pub fn id_struct(id: i64) -> Identifier {
    Identifier {
        source: 0,
        category: identifier::Category::Default as i32,
        kind: identifier::AssetKind::Structure as i32,
        guid: id,
        runtime_id: 0,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct StructRef {
    pub id: Identifier,
    pub fields: Vec<(String, NativeKind)>,
}
impl StructRef {
    pub fn to_kind(&self) -> NativeKind {
        NativeKind::Struct(self.clone())
    }
}
impl StructureDefinition {
    pub fn apply(self, bundle: &mut AssetBundle) -> StructRef {
        let result = StructRef {
            id: bundle.alloc(identifier::Category::Default, identifier::AssetKind::Structure),
            fields: self.fields.iter().map(|x| (x.name.clone(), x.kind.clone())).collect(),
        };
        let mut references = Vec::new();
        for (name, imp, pins) in [
            ("Assemble Struct Server", node_interface::Implementation {
                category: node_interface::implementation::Category::StructAssembly as i32,
                template: node_interface::implementation::Template::AssembleStruct(node_interface::implementation::Id { id: result.id.guid }).into(),
            }, HashMap::default().tap_mut(|pins| {
                pins.insert(PinType::InValue, self.fields.iter().map(|x| DeclPin {
                    name: x.name.clone(),
                    kind: x.kind.clone().into(),
                    meta: None,
                }).collect::<Vec<_>>());
                pins.insert(PinType::OutValue, vec![DeclPin {
                    name: self.name.clone(),
                    kind: result.to_kind().into(),
                    meta: pin_signature::Kind::StructRef.into(),
                }]);
            })),
            ("Destructure Struct Server", node_interface::Implementation {
                category: node_interface::implementation::Category::StructSplit as i32,
                template: node_interface::implementation::Template::SplitStruct(node_interface::implementation::Id { id: result.id.guid }).into(),
            }, HashMap::default().tap_mut(|pins| {
                pins.insert(PinType::InValue, vec![DeclPin {
                    name: self.name.clone(),
                    kind: result.to_kind().into(),
                    meta: pin_signature::Kind::StructRef.into(),
                }]);
                pins.insert(PinType::OutValue, self.fields.iter().map(|x| DeclPin {
                    name: x.name.clone(),
                    kind: Some(x.kind.clone()),
                    meta: None,
                }).collect::<Vec<_>>());
            })),
            ("Modify Struct", node_interface::Implementation {
                category: node_interface::implementation::Category::StructModify as i32,
                template: node_interface::implementation::Template::ModifyStruct(node_interface::implementation::Id { id: result.id.guid }).into(),
            }, HashMap::default().tap_mut(|pins| { // TODO
                pins.insert(PinType::InControl, vec![DeclPin { name: "".to_string(), kind: None, meta: None }]);
                pins.insert(PinType::OutControl, vec![DeclPin { name: "".to_string(), kind: None, meta: None }]);
                pins.insert(PinType::InValue, vec![].tap_mut(|pins| {
                    pins.push(DeclPin {
                        name: self.name.clone(),
                        kind: Some(result.to_kind()),
                        meta: pin_signature::Kind::StructRef.into(),
                    });
                    pins.push(DeclPin {
                        name: "Struct Key Select".to_string(),
                        kind: None, // virtual pin
                        meta: pin_signature::Kind::StructKeySelect.into(),
                    });
                    for field in &self.fields {
                        pins.push(DeclPin {
                            name: format!(".{}", field.name),
                            kind: field.kind.clone().into(),
                            meta: pin_signature::Kind::StructKeySet.into(),
                        });
                        pins.push(DeclPin {
                            name: format!("modify({})", field.name),
                            kind: NativeKind::Bool.into(),
                            meta: pin_signature::Kind::StructKeyMod.into(),
                        });
                    }
                }));
            })),
        ] {
            let (id, _node) = NodeDecl {
                name: name.to_string(),
                description: "".to_string(),
                pins,
                implementation: imp,
                template_root: node_interface::TemplateRoot::Struct,
                template_sub: node_interface::TemplateSub::StructSub,
                references: vec![result.id],
            }.apply(bundle);
            references.push(id);
        }
        // Field.index 对齐 structVersion(真实导出中二者相等)
        let field = self.encode_fields(result.id.guid, self.version);
        bundle.push(AssetData {
            id: result.id.into(),
            references,
            name: "".to_string(),
            r#type: asset_data::Type::Structure as i32,
            payload: Some(Payload::StructData(StructureDefinitionContainer {
                def: Some(StructureDefinitionData {
                    generic_fields: Some(field.clone()),
                    concrete_fields: Some(field),
                    struct_version: self.version,
                    item_count: self.fields.len() as i32,
                    unknown1: 1,
                }),
            })),
        });
        result
    }
}
