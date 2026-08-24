use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::*;
use crate::asset::IAsset;
use crate::asset::value::{Side, Value};

pub enum NodeGraphClass {
    Entity,
}

pub struct RawNodeGraph {
    class: NodeGraphClass,
    id: i64,
    name: String,
    nodes: Vec<RawNode>,
}

impl IAsset for RawNodeGraph {
    fn encode(&self) -> AssetData {
        AssetData {
            id: Some(Identifier {
                source: 0,
                category: identifier::Category::ServerNodeGraph as i32,
                kind: 0,
                guid: self.id,
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
                            guid: self.id,
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

pub struct NodeType {
    shell: Identifier,
    kernel: Identifier,
}
impl NodeType {
    fn id_simple(value: i64) -> Identifier {
        Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: identifier::AssetKind::SysCallStub as i32,
            guid: 0,
            runtime_id: value,
        }
    }
    fn simple(id: i64) -> Self {
        Self {
            shell: Self::id_simple(id),
            kernel: Self::id_simple(id),
        }
    }
}
pub struct RawNode {
    ty: NodeType,
    pos: (f32, f32),
    pins: Vec<RawPin>,
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
    ty: pin_signature::Kind,
    index: i32,
    value: Box<dyn Value>,
    is_set: bool,
}
impl RawPin {
    pub fn encode(&self, side: Side) -> PinData {
        let sig = PinSignature {
            kind: self.ty as i32,
            index: self.index,
            source_ref: None, // TODO
        };
        PinData {
            shell_sig: Some(sig),
            kernel_sig: Some(sig),
            value: Some(self.value.encode(self.is_set, side)),
            r#type: None,
            connection: vec![],
            binding_meta: None,
            persistent_pin_uid: None,
        }
    }
}

// TODO
pub struct StructureDefinition {
}
impl IAsset for StructureDefinition {
    fn encode(&self) -> AssetData {
        // TODO
        AssetData {
            id: None,
            reference: vec![],
            name: "".to_string(),
            r#type: asset_data::Type::Structure as i32,
            payload: None,
        }
    }
}
