//! 节点图模型:节点标识、引脚、黑板变量、结构体定义。
//!
//! 分层设计:NodeGraph(深度封装,INode 节点,连线单向)→ RawNodeGraph(proto 的
//! 简单封装,包含全部信息)→ proto。`RawNodeGraph::encode` 把图编码为资产。

use std::collections::HashMap;
use crate::asset::Asset;
use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::structure_definition_data::{self, var_def as sd_var_def};
use crate::asset::generated::*;
use crate::asset::generated::type_definition::server_type::Schema;
use crate::asset::value::{
    AnyValue, Side, Value, ValueBool, ValueFloat, ValueGuid, ValueInt, ValueString, ValueVector,
};

#[derive(Copy, Clone)]
pub enum NodeGraphClass {
    Entity,
}

/// 节点图(proto `NodeGraphData` 的简单封装,信息完整)。
/// 由 `NodeGraph`(深度封装)encode 生成;`IAsset::encode` 再编码为资产。
pub struct RawNodeGraph {
    pub(super) class: NodeGraphClass,
    pub(super) name: String,
    pub(super) nodes: HashMap<i32, RawNode>,
    /// 节点图变量(黑板变量)
    pub(super) blackboard: Vec<GraphVariable>,
    /// 资产级引用(对齐参考:主图 reference → 用到的复合节点资产)
    pub(super) references: Vec<Identifier>,
    /// 内部图:外部(复合接口)引脚 → 内部节点的穿透映射
    pub(super) port_mapping: Vec<crate::asset::generated::InterfaceMapping>,
    /// 图标识 kind:普通图 = CUSTOM_GRAPH(21001);复合内部图 = COMPOSITE_GRAPH(21002)
    pub(super) graph_kind: identifier::AssetKind,
}

impl Asset for RawNodeGraph {
    fn encode(&self, id: i64) -> Vec<AssetData> {
        vec![AssetData {
            id: Some(Identifier {
                source: 0,
                category: identifier::Category::ServerNodeGraph as i32,
                kind: 0,
                guid: id,
                runtime_id: 0,
            }),
            reference: self.references.clone(),
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
                            kind: self.graph_kind as i32,
                            guid: 0,
                            runtime_id: id,
                        }),
                        display_name: self.name.clone(),
                        // 节点 index 从 1 开始(参考 export.gia:node[0].index=1)
                        node: self.nodes.iter().map(|(i, n)| n.encode((i + 1) as i32, Side::Server /* TODO */)).collect(),
                        port_mapping: self.port_mapping.clone(),
                        comment: vec![],
                        blackboard: self.blackboard.iter().map(|v| v.encode(Side::Server)).collect(),
                        entry_slot_index: None,
                        evaluation_interval: None,
                    }),
                }),
            })),
        }]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NodeType {
    pub(super) shell: Identifier,
    pub(super) kernel: Identifier,
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

    /// 泛型节点特化:shell 固定、kernel 随类型变化
    /// (如 Add:Int 特化 kernel=200、Flt 特化 kernel=201,shell 恒为 200)
    pub fn variant(shell: i64, kernel: i64) -> Self {
        Self {
            shell: Self::id_simple(shell),
            kernel: Self::id_simple(kernel),
        }
    }

    /// 是否引用复合节点(GeneratedStub)
    pub fn is_composite(&self) -> bool {
        self.shell.kind == identifier::AssetKind::GeneratedStub as i32
    }

    /// 节点运行时 id(复合节点 = 复合资产 id;系统节点 = 系统内置 id)
    pub fn runtime_id(&self) -> i64 {
        self.shell.runtime_id
    }
}
/// 图中的节点实例(proto `NodeInstance` 的简单封装)
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

#[derive(Clone)]
pub struct RawPin {
    pub(crate) ty: pin_signature::Kind,
    pub(crate) index: i32,
    pub(crate) value: Option<(AnyValue, bool)>,
    pub(crate) links: Vec<RawLink>,
    /// 持久化引脚 ID(复合节点引脚的锚点;参考导出主图复合节点 OUT_FLOW uid=2)
    pub(crate) uid: Option<i32>,
}
#[derive(Clone)]
pub struct RawLink {
    pub(crate) node: i32,
    pub(crate) ty: pin_signature::Kind,
    pub(crate) index: i32,
}
impl RawPin {
    /// 空引脚(无默认值/无连线/无 uid/无类型),字段可随后直接赋值
    pub fn new(ty: pin_signature::Kind, index: i32) -> Self {
        Self { ty, index, value: None, links: vec![], uid: None }
    }

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
            // 优先显式类型,否则从挂载值推导
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
            persistent_pin_uid: self.uid,
        }
    }
}

// ========================================================================
// 节点图变量(黑板变量)
//
// 对标 GIA 工具集的 `GraphVariable`(make_graph_variable):
//   NodeGraphData.blackboard[] = GraphVariable
//     { var_name, base_type, storage_value: TypedValue, is_public,
//       schema_ref_id?, container_key_type, container_value_type }
// 全局变量被 Execution/Query/Trigger 的 Set/Get/OnChange GraphVariable
// 节点引用(见 game_nodes.ts 的 CustomVariable 域)。
// ========================================================================

/// 节点图变量(黑板变量)
#[derive(Clone)]
pub struct GraphVariable {
    pub name: String,
    /// 变量值(类型由 AnyValue 自身携带,不另存 type id)
    pub value: AnyValue,
    /// 是否已设置(is_set:0=空值, 1=已设置)
    pub is_set: bool,
    /// 是否暴露为外部可配置参数
    pub is_public: bool,
}

impl GraphVariable {
    pub fn new(name: impl Into<String>, value: AnyValue) -> Self {
        Self {
            name: name.into(),
            value,
            is_set: true,
            is_public: false,
        }
    }

    fn encode(&self, side: Side) -> crate::asset::generated::GraphVariable {
        let mut result = crate::asset::generated::GraphVariable {
            var_name: self.name.clone(),
            base_type: self.value.get_server_type() as i32,
            storage_value: Some(self.value.encode(self.is_set, side)),
            is_public: self.is_public,
            schema_ref_id: None,
            container_key_type: 0,
            container_value_type: 0,
        };
        if let Some(schema) = self.value.encode_schema() {
            match schema {
                Schema::StructRef(s) => result.schema_ref_id = Some(s.schema_id),
                Schema::MapBinding(m) => {
                    result.container_key_type = m.key_type;
                    result.container_value_type = m.value_type;
                    result.schema_ref_id = m.value_struct_id; // FIXME: 待验证
                }
            }
        }
        result
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
    /// 字段值(类型由 AnyValue 自身携带,不另存 type id)
    pub value: AnyValue,
    /// 是否有默认值(is_set=false 时 typedef3.val 为空)
    pub is_set: bool,
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

impl Asset for StructureDefinition {
    fn encode(&self, id: i64) -> Vec<AssetData> {
        // Field.index 对齐 structVersion(真实导出中二者相等)
        let field = self.encode_field(id, self.version);
        vec![AssetData {
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
        }]
    }
}

#[cfg(test)]
mod tests {
    use crate::asset::value::ValueStruct;
    use super::*;

    /// 节点图变量编码:对齐参考实现 make_graph_variable
    #[test]
    fn graph_variable_encodes() {
        let v = GraphVariable::new("hp", ValueInt(100).into());
        let proto = v.encode(Side::Server);
        assert_eq!(proto.var_name, "hp");
        // base_type 从值自身推导
        assert_eq!(proto.base_type, ServerTypeId::SInt as i32);
        assert!(!proto.is_public);
        // 容器子类型:非 Map 变量默认 0(由值的 schema 推导,仅 Map 填充)
        assert_eq!(proto.container_key_type, 0);
        assert_eq!(proto.container_value_type, 0);
        assert!(proto.schema_ref_id.is_none());
        // storage_value:初始值 100(is_set=true,带类型定义与存储)
        let sv = proto.storage_value.unwrap();
        assert!(sv.is_set);
        assert!(sv.r#type.is_some());
        assert!(sv.storage.is_some());
    }

    /// 结构体变量带 schema_ref_id;is_set=false 时 storage_value 不带存储
    #[test]
    fn graph_variable_struct_schema_ref() {
        let mut v = GraphVariable::new("player", ValueInt(0).into());
        v.value = ValueStruct::new(0x4040_0001, vec![]).into();
        let proto = v.encode(Side::Server);
        assert_eq!(proto.schema_ref_id, Some(0x4040_0001));

        let mut unset = GraphVariable::new("x", ValueInt(0).into());
        unset.is_set = false;
        let sv = unset.encode(Side::Server).storage_value.unwrap();
        assert!(!sv.is_set);
        assert!(sv.storage.is_none());
    }

    /// 节点图编码携带 blackboard
    #[test]
    fn node_graph_blackboard_encodes() {
        let g = RawNodeGraph {
            class: NodeGraphClass::Entity,
            name: "g".to_string(),
            nodes: HashMap::new(),
            blackboard: vec![
                GraphVariable::new("hp", ValueInt(100).into()),
                GraphVariable::new("name", ValueString("alice".to_string()).into()),
            ],
            references: vec![],
            port_mapping: vec![],
            graph_kind: crate::asset::generated::identifier::AssetKind::CustomGraph,
        };
        let data = g.encode(7).pop().unwrap();
        let Payload::GraphData(container) = data.payload.unwrap() else {
            panic!("payload is not graph data");
        };
        let graph = container.inner.unwrap().graph.unwrap();
        assert_eq!(graph.blackboard.len(), 2);
        assert_eq!(graph.blackboard[0].var_name, "hp");
        assert_eq!(graph.blackboard[0].base_type, ServerTypeId::SInt as i32);
        assert_eq!(graph.blackboard[1].var_name, "name");
        assert_eq!(graph.blackboard[1].base_type, ServerTypeId::SString as i32);
    }

    #[test]
    fn structure_definition_encodes() {
        let def = StructureDefinition {
            name: "Player".to_string(),
            version: 1,
            fields: vec![
                StructField {
                    name: "hp".to_string(),
                    value: ValueInt(100).into(),
                    is_set: true,
                },
                StructField {
                    name: "name".to_string(),
                    value: ValueString("alice".to_string()).into(),
                    is_set: true,
                },
                StructField {
                    name: "pos".to_string(),
                    value: ValueVector(114.0, 514.0, 191.0).into(),
                    is_set: true,
                },
                StructField {
                    name: "uid".to_string(),
                    value: ValueGuid(46456416).into(),
                    is_set: true,
                },
                StructField {
                    name: "ratio".to_string(),
                    value: ValueFloat(5.145).into(),
                    is_set: true,
                },
            ],
        };
        let data = def.encode(100).pop().unwrap();
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
