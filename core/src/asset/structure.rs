//! 节点图模型:节点标识、引脚、黑板变量、结构体定义。
//!
//! 分层设计:NodeGraph(深度封装,INode 节点,连线单向)→ RawNodeGraph(proto 的
//! 简单封装,包含全部信息)→ proto。`RawNodeGraph::encode` 把图编码为资产。

use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::identifier::{AssetKind, Category};
use crate::asset::generated::structure_definition_data::var_def::Subtype;
use crate::asset::generated::structure_definition_data::var_def::value::{StructVal, Val, struct_val};
use crate::asset::generated::structure_definition_data::{self, var_def as sd_var_def};
use crate::asset::generated::*;
use crate::asset::node_graph::decl::{DeclPin, NodeDecl};
use crate::asset::node_graph::{NodeKind, PinType};
use crate::asset::value::{AnyValue, Value, ValueBool, ValueDefault};
use crate::asset::{Asset, AssetBundle, AssetRef, Side};
use std::collections::HashMap;
use tap::Tap;

/// 拼装结构体: 字段值 → 结构体
pub fn node_assemble_struct(st: &ValueStruct) -> NodeKind {
    st.st.data.decls.get(&StructureNodeDecl::AssembleServer).unwrap().data.clone()
}

pub fn node_destruct_struct(st: &ValueStruct) -> NodeKind {
    st.st.data.decls.get(&StructureNodeDecl::DestructServer).unwrap().data.clone()
}

pub fn node_modify_struct(st: &ValueStruct) -> NodeKind {
    st.st.data.decls.get(&StructureNodeDecl::Modify).unwrap().data.clone()
}

/// 结构体的一个字段(对标 GIA `StructDecl.fields[]`)
pub struct StructField {
    pub name: String,
    /// 字段值(类型由 AnyValue 自身携带,不另存 type id)
    pub value: AnyValue,
}

impl StructField {
    /// 字段 → proto `VarDef`(对齐真实导出的 wire 格式):
    ///   typedef1 = { type, subType {} }(空 subType,无 val)
    ///   typedef3 = { type, subType { type, xxxx_id {} }, val }
    fn encode_var_def(&self, index: i32) -> structure_definition_data::VarDef {
        let ty = self.value.get_server_type();
        let kind = structure_definition_data::var_def::Kind {
            primary: ty as i32,
            sub: self.value.encode_subtype().or(Subtype {
                is_set: false,
                struct_id: 0,
                key: None,
                value: None,
                value_id: None,
            }.into()),
        };
        structure_definition_data::VarDef {
            kind: kind.into(),
            name: self.name.clone(),
            var_name: self.name.clone(),
            var_type: ty as i32,
            var_index: index,
            def: if self.value.encode_storage(Side::Server).is_some() { sd_var_def::Value {
                r#type: ty as i32,
                kind: kind.into(),
                name: None,
                val: self.value.encode_field_value().into(),
            }.into() } else { None },
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

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub enum StructureNodeDecl {
    AssembleServer,
    DestructServer,
    Modify, // server only
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
    pub fn new(r: AssetRef<StructureDefinition>) -> Self {
        Self {
            fields: r.data.fields.clone(),
            st: r,
        }
    }
    pub fn get_struct_id(&self) -> i64 {
        self.st.root.guid
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
            field: self.fields.iter().map(|f| f.encode_typed(true, side)).collect(),
        }).into()
    }
    fn encode_schema(&self) -> Option<type_definition::server_type::Schema> {
        Some(type_definition::server_type::Schema::StructRef(type_definition::StructReference {
            schema_id: self.st.root.guid,
        }))
    }
    fn encode_type_detail(&self) -> Option<pin_interface::type_info::Detail> {
        Some(pin_interface::type_info::Detail::StructId(pin_interface::type_info::StructId { val: self.get_struct_id() }),)
    }

    fn encode_subtype(&self) -> Option<Subtype> {
        Subtype {
            is_set: true,
            struct_id: self.get_struct_id(),
            key: None,
            value: None,
            value_id: None,
        }.into()
    }

    fn encode_field_value(&self) -> Val {
        Val::Structure(StructVal {
            fields: vec![],
            struct_id: self.get_struct_id(),
            id: struct_val::Id {
                kind: ServerTypeId::SVarSnapshotRef as i32,
                id: 0x40000001, // TODO
            }.into(),
        })
    }

    fn is_instance(&self, value: &Box<dyn Value>) -> bool {
        matches!(value.downcast_ref::<ValueStruct>(), Ok(value) if value.st == self.st)
    }
}

#[derive(Clone, Debug)]
pub struct StructureRef {
    fields: Vec<AnyValue>,
    decls: HashMap<StructureNodeDecl, AssetRef<NodeDecl>>,
}
impl Asset for StructureDefinition {
    type RefData = StructureRef;

    fn apply(self, bundle: &mut AssetBundle) -> AssetRef<Self> {
        let mut result = AssetRef::new(bundle.alloc(Category::Default, AssetKind::Structure), StructureRef {
            fields: self.fields.iter().map(|x| x.value.clone()).collect(),
            decls: Default::default(),
        });
        for (k, name, imp, pins) in [
            (StructureNodeDecl::AssembleServer, "Assemble Struct Server", node_interface::Implementation {
                category: node_interface::implementation::Category::StructAssembly as i32,
                template: node_interface::implementation::Template::AssembleStruct(node_interface::implementation::Id { id: result.root.guid }).into(),
            }, HashMap::default().tap_mut(|pins| {
                pins.insert(PinType::InValue, self.fields.iter().map(|x| DeclPin {
                    name: x.name.clone(),
                    kind: x.value.clone().into(),
                    meta: None,
                }).collect::<Vec<_>>());
                pins.insert(PinType::OutValue, vec![DeclPin {
                    name: self.name.clone(),
                    kind: Some(ValueStruct::new(result.clone()).into()),
                    meta: pin_signature::Kind::StructRef.into(),
                }]);
            })),
            (StructureNodeDecl::DestructServer, "Destruct Struct Server", node_interface::Implementation {
                category: node_interface::implementation::Category::StructSplit as i32,
                template: node_interface::implementation::Template::SplitStruct(node_interface::implementation::Id { id: result.root.guid }).into(),
            }, HashMap::default().tap_mut(|pins| {
                pins.insert(PinType::InValue, vec![DeclPin {
                    name: self.name.clone(),
                    kind: Some(ValueStruct::new(result.clone()).into()),
                    meta: pin_signature::Kind::StructRef.into(),
                }]);
                pins.insert(PinType::OutValue, self.fields.iter().map(|x| DeclPin {
                    name: x.name.clone(),
                    kind: Some(x.value.clone()),
                    meta: None,
                }).collect::<Vec<_>>());
            })),
            (StructureNodeDecl::Modify, "Modify Struct", node_interface::Implementation {
                category: node_interface::implementation::Category::StructModify as i32,
                template: node_interface::implementation::Template::ModifyStruct(node_interface::implementation::Id { id: result.root.guid }).into(),
            }, HashMap::default().tap_mut(|pins| { // TODO
                pins.insert(PinType::InControl, vec![DeclPin { name: "".to_string(), kind: None, meta: None }]);
                pins.insert(PinType::OutControl, vec![DeclPin { name: "".to_string(), kind: None, meta: None }]);
                pins.insert(PinType::InValue, vec![].tap_mut(|pins| {
                    pins.push(DeclPin {
                        name: self.name.clone(),
                        kind: Some(ValueStruct::new(result.clone()).into()),
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
                            kind: field.value.clone().into(),
                            meta: pin_signature::Kind::StructKeySet.into(),
                        });
                        pins.push(DeclPin {
                            name: format!("modify({})", field.name),
                            kind: ValueBool::def().into(),
                            meta: pin_signature::Kind::StructKeyMod.into(),
                        });
                    }
                }));
            })),
        ] {
            result.data.decls.insert(k, NodeDecl {
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
        let field = self.encode_fields(result.root.guid, self.version);
        bundle.push(AssetData {
            id: result.root.into(),
            references: result.data.decls.values().map(|x| x.root).collect(),
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
