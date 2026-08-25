use crate::asset::IAsset;
use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::structure_definition_data::{self, var_def as sd_var_def};
use crate::asset::generated::*;
use crate::asset::value::{
    AnyValue, Side, Value, ValueBool, ValueFloat, ValueGuid, ValueInt, ValueString, ValueVector,
};

#[derive(Copy, Clone)]
pub enum NodeGraphClass {
    Entity,
}

pub struct RawNodeGraph {
    pub(super) class: NodeGraphClass,
    pub(super) name: String,
    pub(super) nodes: Vec<RawNode>,
}

impl IAsset for RawNodeGraph {
    fn encode(&self, id: i64) -> AssetData {
        AssetData {
            id: Some(Identifier {
                source: 0,
                category: identifier::Category::ServerNodeGraph as i32,
                kind: 0,
                guid: id,
                runtime_id: 0,
            }),
            reference: vec![],
            name: self.name.clone(),
            r#type: match self.class {
                NodeGraphClass::Entity => asset_data::Type::EntityNodeGraph,
            } as i32,
            payload: Some(Payload::GraphData(NodeGraphContainer {
                inner: Some(node_graph_container::InnerWrapper {
                    graph: Some(NodeGraphData {
                        id: Some(Identifier {
                            source: identifier::Source::UserDefined as i32,
                            category: identifier::Category::ServerBasic as i32,
                            kind: identifier::AssetKind::CustomGraph as i32,
                            guid: id,
                            runtime_id: 0,
                        }),
                        display_name: self.name.clone(),
                        node: self.nodes.iter().enumerate().map(|(i, n)| n.encode(i as i32, Side::Server /* TODO */)).collect(),
                        port_mapping: vec![],
                        comment: vec![],
                        blackboard: vec![],
                        entry_slot_index: None,
                        evaluation_interval: None,
                    }),
                }),
            })),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NodeType {
    shell: Identifier,
    kernel: Identifier,
}
impl NodeType {
    pub fn id_simple(value: i64) -> Identifier {
        Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: identifier::AssetKind::SysCallStub as i32,
            guid: 0,
            runtime_id: value,
        }
    }
    pub fn simple(id: i64) -> Self {
        Self {
            shell: Self::id_simple(id),
            kernel: Self::id_simple(id),
        }
    }
}
pub struct RawNode {
    pub(super) ty: NodeType,
    pub(super) pos: (f32, f32),
    pub(super) pins: Vec<RawPin>,
}
impl RawNode {
    pub fn encode(&self, index: i32, side: Side) -> NodeInstance {
        NodeInstance {
            index,
            shell_ref: Some(self.ty.shell),
            kernel_ref: Some(self.ty.kernel),
            pins: self.pins.iter().map(|it| it.encode(side)).collect(),
            x_pos: self.pos.0,
            y_pos: self.pos.1,
            attached_comment: None, // TODO
            context_declaration: None, // TODO
            signal_version: None, // TODO
            using_structs: vec![], // TODO
        }
    }
}

pub struct RawPin {
    pub(super) ty: pin_signature::Kind,
    pub(super) index: i32,
    pub(super) value: Option<(AnyValue, bool)>,
    pub(super) links: Vec<RawLink>,
}
pub struct RawLink {
    pub(super) node: i32,
    pub(super) ty: pin_signature::Kind,
    pub(super) index: i32,
}
impl RawPin {
    pub fn encode(&self, side: Side) -> PinData {
        fn signature(ty: pin_signature::Kind, index: i32) -> PinSignature {
            PinSignature {
                kind: ty as i32,
                index,
                source_ref: None, // TODO: 疑似复合节点
            }
        }
        let sig = signature(self.ty, self.index);
        PinData {
            shell_sig: Some(sig),
            kernel_sig: Some(sig),
            value: self.value.as_ref().map(|(v, t)| v.encode(*t, side)),
            r#type: self.value.as_ref().map(|(v, _)| v.get_type_id(side)),
            connection: self.links.iter().map(|it| {
                let sig = signature(it.ty, it.index);
                NodeConnection {
                    target_node_index: it.node,
                    target_pin_shell: Some(sig),
                    target_pin_kernel: Some(sig),
                }
            }).collect(),
            binding_meta: None,
            persistent_pin_uid: None,
        }
    }
}

// ========================================================================
// 结构体定义 (StructureDefinition)
//
// 对标 GIA 工具集的 StructDecl(DSL `interface` 声明):
//   interface StructName { name: type = defaultValue; }
// 序列化为 StructureDefinitionData(proto, AssetData.Type = STRUCTURE)。
// 结构体 ID 被 GraphVariable.schema_ref_id / StructReference 引用。
// ========================================================================

/// 结构体的一个字段(对标 GIA `StructDecl.fields[]`)
pub struct StructField {
    pub name: String,
    /// 字段类型(服务端类型 ID;复杂类型如 S_STRUCT 由 var_type 表达)
    pub var_type: ServerTypeId,
    /// 默认值(ValueBool / ValueInt / ValueString / ValueGuid / ValueFloat / ValueVector;暂无默认值为 None)
    pub default: Option<AnyValue>,
}

impl StructField {
    /// 字段默认值 → proto `TypeDef.val`(Int / Bool / Str / Guid / Float / Vector)
    fn encode_val(v: &dyn Value) -> Option<sd_var_def::type_def::Val> {
        use sd_var_def::type_def::Val;
        if let Some(i) = v.downcast_ref::<ValueInt>().ok() {
            Some(Val::IntVal(Int { value: i.0 }))
        } else if let Some(b) = v.downcast_ref::<ValueBool>().ok() {
            Some(Val::BooleanVal(Enum { value: b.0 as i64 }))
        } else if let Some(s) = v.downcast_ref::<ValueString>().ok() {
            Some(Val::StrVal(Str { value: s.0.clone() }))
        } else if let Some(g) = v.downcast_ref::<ValueGuid>().ok() {
            // val oneof 字段 12 = Id{id:int64}(与我们的 Id{value} 同构)
            Some(Val::GuidVal(Id { value: g.0 }))
        } else if let Some(f) = v.downcast_ref::<ValueFloat>().ok() {
            // val oneof 字段 15 = Float{float}(与我们的 Flt{value} 同构)
            Some(Val::FloatVal(Flt { value: f.0 }))
        } else if let Some(v3) = v.downcast_ref::<ValueVector>().ok() {
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
        let val = self.default.as_deref().and_then(Self::encode_val);
        structure_definition_data::VarDef {
            typedef1: Some(sd_var_def::TypeDef {
                r#type: self.var_type as i32,
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
                r#type: self.var_type as i32,
                sub_type: Some(sd_var_def::type_def3::SubType {
                    r#type: self.var_type as i32,
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
            var_type: self.var_type as i32,
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

impl IAsset for StructureDefinition {
    fn encode(&self, id: i64) -> AssetData {
        // Field.index 对齐 structVersion(真实导出中二者相等)
        let field = self.encode_field(id, self.version);
        AssetData {
            // 对齐真实导出:source_domain 省略(0)、runtime_id 省略(0)
            id: Some(Identifier {
                source: 0,
                category: identifier::Category::Default as i32,
                kind: identifier::AssetKind::Structure as i32,
                guid: id,
                runtime_id: 0,
            }),
            reference: vec![],
            name: self.name.clone(),
            r#type: asset_data::Type::Structure as i32,
            payload: Some(Payload::StructData(StructureDefinitionContainer {
                def: Some(StructureDefinitionData {
                    generic_field: Some(field.clone()),
                    concrete_field: Some(field),
                    struct_version: self.version,
                    item_count: self.fields.len() as i32,
                    // 参考真实导出没有字段 5(unknown1),写 0 即省略
                    unknown1: 0,
                }),
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_definition_encodes() {
        let def = StructureDefinition {
            name: "Player".to_string(),
            version: 1,
            fields: vec![
                StructField {
                    name: "hp".to_string(),
                    var_type: ServerTypeId::SInt,
                    default: Some(ValueInt(100).into()),
                },
                StructField {
                    name: "name".to_string(),
                    var_type: ServerTypeId::SString,
                    default: Some(ValueString("alice".to_string()).into()),
                },
                StructField {
                    name: "pos".to_string(),
                    var_type: ServerTypeId::SVector,
                    default: Some(ValueVector(114.0, 514.0, 191.0).into()),
                },
                StructField {
                    name: "uid".to_string(),
                    var_type: ServerTypeId::SGuid,
                    default: Some(ValueGuid(46456416).into()),
                },
                StructField {
                    name: "ratio".to_string(),
                    var_type: ServerTypeId::SFloat,
                    default: Some(ValueFloat(5.145).into()),
                },
            ],
        };
        let data = def.encode(100);
        assert_eq!(data.r#type, asset_data::Type::Structure as i32);
        assert_eq!(data.id.as_ref().unwrap().kind, identifier::AssetKind::Structure as i32);
        let Payload::StructData(container) = data.payload.unwrap() else {
            panic!("payload is not struct data");
        };
        let sdef = container.def.unwrap();
        assert_eq!(sdef.item_count, 5);
        assert_eq!(sdef.struct_version, 1);
        let field = sdef.generic_field.unwrap();
        assert_eq!(field.struct_name, "Player");
        assert_eq!(field.var.len(), 5);
        assert_eq!(field.var[0].var_name, "hp");
        assert_eq!(field.var[0].var_type, ServerTypeId::SInt as i32);
        // Field.index 与 structVersion 一致;varIndex 从 1 起
        assert_eq!(field.index, sdef.struct_version);
        assert_eq!(field.var[0].var_index, 1);
        assert_eq!(field.var[4].var_index, 5);
        // typedef1 只有 type + 空 subType(无 val);默认值在 typedef3
        let td = field.var[0].typedef1.as_ref().unwrap();
        assert!(td.val.is_none());
        assert!(td.sub_type.is_some());
        let td3 = field.var[0].typedef3.as_ref().unwrap();
        assert!(matches!(td3.val.as_ref(), Some(sd_var_def::type_def3::Val::IntVal(_))));
        assert!(td3.sub_type.as_ref().unwrap().xxxx_id.is_some());
        let td3b = field.var[1].typedef3.as_ref().unwrap();
        assert!(matches!(td3b.val.as_ref(), Some(sd_var_def::type_def3::Val::StrVal(_))));
        // 向量默认值 → typedef3 val 的 VectorVal(Vec3f)
        let td3v = field.var[2].typedef3.as_ref().unwrap();
        let Some(sd_var_def::type_def3::Val::VectorVal(v)) = td3v.val.as_ref() else {
            panic!("vector val not encoded");
        };
        let vv = v.value.as_ref().unwrap();
        assert_eq!(vv.x, 114.0);
        assert_eq!(vv.y, 514.0);
        assert_eq!(vv.z, 191.0);
        // guid 默认值 → GuidVal(Id)
        let td3g = field.var[3].typedef3.as_ref().unwrap();
        assert!(matches!(td3g.val.as_ref(), Some(sd_var_def::type_def3::Val::GuidVal(_))));
        // float 默认值 → FloatVal(Flt)
        let td3f = field.var[4].typedef3.as_ref().unwrap();
        assert!(matches!(td3f.val.as_ref(), Some(sd_var_def::type_def3::Val::FloatVal(_))));
    }
}
