use super::value::{AnyValue, Value};
use crate::asset::generated::{AssetData, ClientTypeId, GraphVariable, Identifier, NodeConnection, NodeGraphContainer, NodeGraphData, NodeInstance, PinData, PinSignature, PolymorphicValue, ServerTypeId, TypedValue, asset_data, identifier, node_graph_container, pin_signature, typed_value};
use crate::asset::{Asset, Side};
use slab::Slab;
use std::any::TypeId;
use tap::Tap;

pub mod arithmetic;
pub mod client;
pub mod control;
pub mod execution;
pub mod hidden;
pub mod query;
pub mod trigger;
pub mod composite;
pub mod structure;

pub use pin_signature::Kind as PinType;
use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::type_definition::server_type::Schema;
pub(crate) use crate::asset::node_graph::composite::NodeGraphComposite;

#[derive(Copy, Clone)]
pub enum NodeGraphClass {
    Entity,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct NodeRef(usize);

const NODE_ID_BEGIN: i32 = 1;
impl From<NodeRef> for usize {
    fn from(value: NodeRef) -> usize {
        value.0
    }
}
impl From<usize> for NodeRef {
    fn from(value: usize) -> NodeRef {
        NodeRef(value)
    }
}
impl NodeRef {
    pub fn decode(value: i32) -> Self {
        Self((value - NODE_ID_BEGIN) as usize)
    }
    pub fn encode(&self) -> i32 {
        self.0 as i32 + NODE_ID_BEGIN
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Connection(pub NodeRef, pub usize);

impl Connection {
    pub fn node(&self) -> NodeRef {
        self.0
    }
    pub fn pin(&self) -> usize {
        self.1
    }
}
impl From<Connection> for Link {
    fn from(connection: Connection) -> Link {
        Link::Connection(connection)
    }
}

#[derive(Clone)]
pub struct NodeKind {
    pub id: i64,
    pub kernel_id: i64,
    pub kind: identifier::AssetKind,

    pub controls_in_num: usize,
    pub controls_out_num: usize,
    pub values_in_types: Vec<AnyValue>,
    pub values_out_types: Vec<AnyValue>,

    pub selectors_in: Vec<Option<i32>>,
    pub selectors_out: Vec<Option<i32>>,

    pub references: Vec<Identifier>,
}
impl NodeKind {
    pub fn new(
        id: i64,
        controls_in_num: usize,
        controls_out_num: usize,
        values_in_types: Vec<AnyValue>,
        values_out_types: Vec<AnyValue>,
    ) -> Self {
        Self {
            id,
            kernel_id: id,
            kind: identifier::AssetKind::SysCallStub,
            controls_in_num,
            controls_out_num,
            selectors_in: vec![None; values_in_types.len()],
            selectors_out: vec![None; values_out_types.len()],
            values_in_types,
            values_out_types,
            references: vec![],
        }
    }
    pub fn expr(id: i64, values_in_types: Vec<AnyValue>, value_out_type: AnyValue) -> Self {
        Self::new(id, 0, 0, values_in_types, vec![value_out_type])
    }
    pub fn func(id: i64, values_in_types: Vec<AnyValue>, value_out_type: AnyValue) -> Self {
        Self::new(id, 1, 1, values_in_types, vec![value_out_type])
    }
    pub fn procedure(id: i64, values_in_types: Vec<AnyValue>) -> Self {
        Self::new(id, 1, 1, values_in_types, vec![])
    }
    pub fn trigger(id: i64, value_out_type: Vec<AnyValue>) -> Self {
        Self::new(id, 0, 1, vec![], value_out_type)
    }

    fn encode_shell(&self) -> Identifier {
        Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: self.kind as i32,
            guid: 0,
            runtime_id: self.id,
        }
    }
    fn encode_kernel(&self) -> Identifier {
        Identifier {
            runtime_id: self.kernel_id,
            ..self.encode_shell()
        }
    }
}
pub struct Node {
    pub kind: NodeKind,
    controls_in: Vec<Vec<Link>>,
    controls_out: Vec<Vec<Link>>,
    values_in: Vec<ValueIn>,
    values_out: Vec<Vec<Link>>,
}
impl Node {
    pub fn new(kind: NodeKind) -> Node {
        Node {
            controls_in: vec![Default::default(); kind.controls_in_num],
            controls_out: vec![Default::default(); kind.controls_out_num],
            values_in: vec![Default::default(); kind.values_in_types.len()],
            values_out: vec![Default::default(); kind.values_out_types.len()],
            kind,
        }
    }
}
impl From<NodeKind> for Node {
    fn from(kind: NodeKind) -> Self {
        Self::new(kind)
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Link {
    Connection(Connection),
    Export(usize),
}
impl Link {
    pub fn connection(self) -> Option<Connection> {
        match self {
            Link::Connection(it) => it.into(),
            Link::Export(_) => None,
        }
    }
    pub fn export(self) -> Option<usize> {
        match self {
            Link::Connection(_) => None,
            Link::Export(it) => it.into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ValueIn {
    pub default: Option<AnyValue>,
    pub link: Option<Link>,
}
impl ValueIn {
    pub fn value(default: AnyValue) -> Self {
        Self {
            default: Some(default),
            link: None,
        }
    }
    pub fn link(link: Link) -> Self {
        Self {
            default: None,
            link: Some(link),
        }
    }
    pub fn is_unset(&self) -> bool {
        self.default.is_none() && self.link.is_none()
    }
}

pub trait NodeGraphExtra: Sized {
    type Data;
    fn encode(&self, graph: &NodeGraph<Self>, data: &mut NodeGraphData) -> Self::Data;
    fn encode_extra(&self, _data: Self::Data, _id: i64) -> Vec<AssetData> {
        vec![]
    }
}
pub struct NodeGraph<T: NodeGraphExtra> {
    pub class: NodeGraphClass,
    pub name: String,
    nodes: Slab<Node>,
    pub extra: T,
}
impl<T: NodeGraphExtra> NodeGraph<T> {
    pub fn new(class: NodeGraphClass, name: impl Into<String>, extra: T) -> Self {
        Self {
            class,
            name: name.into(),
            nodes: Default::default(),
            extra,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn get_node(&self, key: NodeRef) -> &Node {
        &self.nodes[key.into()]
    }

    pub fn insert(&mut self, node: Node) -> NodeRef {
        self.nodes.insert(node).into()
    }
    pub fn remove(&mut self, key: NodeRef) -> Node {
        // TODO
        self.nodes.remove(key.into())
    }

    pub fn set_default(&mut self, place: Connection, value: AnyValue) {
        let node = &mut self.nodes[place.node().into()];
        assert!(node.kind.values_in_types[place.pin()].is_instance(&value), "{value:?} is not {:?}", node.kind.values_in_types[place.pin()]);
        node.values_in[place.pin()].default = Some(value);
    }

    pub fn connect_value(&mut self, from: Connection, to: Connection) {
        let (from_node, to_node) = self.nodes.get2_mut(from.node().into(), to.node().into()).unwrap();
        assert!(to_node.values_in[to.pin()].link.is_none(), "has connected");
        assert!(to_node.kind.values_in_types[to.pin()].is_instance(&from_node.kind.values_out_types[from.pin()]), "Type error: {:?} and {:?}", from_node.kind.values_out_types[from.pin()], to_node.kind.values_in_types[to.pin()]);
        from_node.values_out[from.pin()].push(to.into());
        to_node.values_in[to.pin()].link = Some(from.into());
    }

    pub fn set_value_in(&mut self, place: Connection, value: ValueIn) {
        if let Some(default) = value.default {
            self.set_default(place, default);
        }
        if let Some(link) = value.link {
            match link {
                Link::Connection(con) => self.connect_value(con, place),
                Link::Export(exp) => self.export_value_in(place, exp),
            }
        }
    }

    pub fn connect_control(&mut self, from: Connection, to: Connection) {
        let (from_node, to_node) = self.nodes.get2_mut(from.node().into(), to.node().into()).unwrap();
        from_node.controls_out[from.pin()].push(to.into());
        to_node.controls_in[to.pin()].push(from.into());
    }

    pub fn export_control_in(&mut self, inner: Connection, outer: usize) {
        let node = &mut self.nodes[inner.node().into()];
        node.controls_in[inner.pin()].push(Link::Export(outer));
    }

    pub fn export_control_out(&mut self, inner: Connection, outer: usize) {
        let node = &mut self.nodes[inner.node().into()];
        node.controls_out[inner.pin()].push(Link::Export(outer));
    }

    pub fn export_value_in(&mut self, inner: Connection, outer: usize) {
        let node = &mut self.nodes[inner.node().into()];
        let link = &mut node.values_in[inner.pin()].link;
        assert!(link.is_none(), "has connected");
        *link = Some(Link::Export(outer));
    }

    pub fn export_value_out(&mut self, inner: Connection, outer: usize) {
        let node = &mut self.nodes[inner.node().into()];
        node.values_out[inner.pin()].push(Link::Export(outer));
    }
}
impl<T: NodeGraphExtra + 'static> Asset for NodeGraph<T> {
    fn encode(&self, side: Side, id: i64) -> Vec<AssetData> {
        let mut references = vec![];
        for (_, x) in &self.nodes {
            references.extend(x.kind.references.clone());
        }
        let extra: T::Data;
        let mut result = vec![AssetData {
            id: Some(Identifier {
                source: 0,
                category: identifier::Category::ServerNodeGraph as i32,
                kind: 0,
                guid: id,
                runtime_id: 0,
            }),
            references,
            name: self.name.clone(),
            r#type: match self.class {
                NodeGraphClass::Entity => asset_data::Type::EntityNodeGraph,
            } as i32,
            payload: Some(Payload::GraphData(NodeGraphContainer {
                inner: Some(node_graph_container::InnerWrapper {
                    graph: Some({
                        let mut data = NodeGraphData {
                            id: Some(Identifier {
                                source: identifier::Source::UserDefined as i32,
                                category: identifier::Category::ServerBasic as i32,
                                kind: if TypeId::of::<T>() != TypeId::of::<NodeGraphComposite>() { identifier::AssetKind::CustomGraph } else { identifier::AssetKind::CompositeGraph } as i32,
                                guid: 0,
                                runtime_id: id,
                            }),
                            display_name: self.name.clone(),
                            node: self.nodes.iter().map(|(i, n)| NodeInstance {
                                index: NodeRef::from(i).encode(),
                                shell_ref: Some(n.kind.encode_shell()),
                                kernel_ref: Some(n.kind.encode_kernel()),
                                pins: vec![].tap_mut(|pins| {
                                    process_controls(pins, &n.controls_in, PinType::InControl, PinType::OutControl);
                                    process_controls(pins, &n.controls_out, PinType::OutControl, PinType::InControl);
                                    for (i, x) in n.values_in.iter().enumerate() {
                                        let sig = PinSignature {
                                            kind: PinType::InValue as i32,
                                            index: i as i32,
                                            source_ref: None,
                                        };
                                        pins.push(PinData {
                                            shell_sig: sig.into(),
                                            kernel_sig: sig.into(),
                                            value: Some(ValueSelected::encode(x.default.clone().unwrap_or(n.kind.values_in_types[i].clone()), x.default.is_some(), n.kind.selectors_in[i], side)),
                                            r#type: Some(n.kind.values_in_types[i].get_type_id(side)),
                                            connection: vec![].tap_mut(|cs| {
                                                if let Some(Connection(target, j)) = x.link.and_then(Link::connection) {
                                                    let sig_tar = PinSignature {
                                                        kind: PinType::OutValue as i32,
                                                        index: j as i32,
                                                        source_ref: None,
                                                    };
                                                    cs.push(NodeConnection {
                                                        target_node_index: target.encode(),
                                                        target_pin_shell: sig_tar.into(),
                                                        target_pin_kernel: sig_tar.into(),
                                                    })
                                                }
                                            }),
                                            binding_meta: None,
                                            persistent_pin_uid: None,
                                        })
                                    }
                                    for (i, x) in n.values_out.iter().enumerate() {
                                        let sig = PinSignature {
                                            kind: PinType::OutValue as i32,
                                            index: i as i32,
                                            source_ref: None,
                                        };
                                        pins.push(PinData {
                                            shell_sig: sig.into(),
                                            kernel_sig: sig.into(),
                                            value: Some(ValueSelected::encode(n.kind.values_out_types[i].clone(), false, n.kind.selectors_out[i], side)),
                                            r#type: Some(n.kind.values_out_types[i].get_type_id(side)),
                                            connection: vec![].tap_mut(|cs| {
                                                for Connection(target, j) in x.iter().copied().flat_map(Link::connection) {
                                                    let sig_tar = PinSignature {
                                                        kind: PinType::InValue as i32,
                                                        index: j as i32,
                                                        source_ref: None,
                                                    };
                                                    cs.push(NodeConnection {
                                                        target_node_index: target.encode(),
                                                        target_pin_shell: sig_tar.into(),
                                                        target_pin_kernel: sig_tar.into(),
                                                    })
                                                }
                                            }),
                                            binding_meta: None,
                                            persistent_pin_uid: None,
                                        })
                                    }
                                    fn process_controls<'a, S: 'a>(pins: &mut Vec<PinData>, controls: &'a [S], kind: PinType, tar: PinType)
                                    where
                                        &'a S: IntoIterator<Item = &'a Link>,
                                    {
                                        for (i, x) in controls.iter().enumerate() {
                                            let sig = PinSignature {
                                                kind: kind as i32,
                                                index: i as i32,
                                                source_ref: None,
                                            };
                                            pins.push(PinData {
                                                shell_sig: sig.into(),
                                                kernel_sig: sig.into(),
                                                value: None,
                                                r#type: None,
                                                connection: vec![].tap_mut(|cs| {
                                                    for Connection(target, j) in x.into_iter().copied().flat_map(Link::connection) {
                                                        let sig_tar = PinSignature {
                                                            kind: tar as i32,
                                                            index: j as i32,
                                                            source_ref: None,
                                                        };
                                                        cs.push(NodeConnection {
                                                            target_node_index: target.encode(),
                                                            target_pin_shell: sig_tar.into(),
                                                            target_pin_kernel: sig_tar.into(),
                                                        })
                                                    }
                                                }),
                                                binding_meta: None,
                                                persistent_pin_uid: None,
                                            })
                                        }
                                    }
                                }),
                                x_pos: 0.,
                                y_pos: 0.,
                                attached_comment: None, // TODO
                                context_declaration: None, // TODO
                                signal_version: None, // TODO
                                using_structs: vec![], // TODO
                            }).collect(),
                            port_mapping: vec![],
                            comment: vec![],
                            blackboard: vec![],
                            entry_slot_index: None,
                            evaluation_interval: None,
                        };
                        extra = self.extra.encode(self, &mut data);
                        data
                    }),
                }),
            })),
        }];
        result.extend(self.extra.encode_extra(extra, id));
        result
    }
}

/// 节点图变量(黑板变量)
#[derive(Clone)]
pub struct NodeGraphStatic {
    pub name: String,
    pub value: AnyValue,
    pub is_set: bool,
    pub is_public: bool,
}

impl NodeGraphStatic {
    pub fn new(name: impl Into<String>, value: AnyValue) -> Self {
        Self {
            name: name.into(),
            value,
            is_set: true,
            is_public: false,
        }
    }

    fn encode(&self) -> GraphVariable {
        let mut result = GraphVariable {
            var_name: self.name.clone(),
            base_type: self.value.get_server_type() as i32,
            storage_value: Some(self.value.encode(self.is_set, Side::Server)),
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
impl NodeGraphExtra for Vec<NodeGraphStatic> {
    type Data = ();
    fn encode(&self, _graph: &NodeGraph<Self>, data: &mut NodeGraphData) {
        data.blackboard = self.iter().map(NodeGraphStatic::encode).collect();
    }
}

#[derive(Clone, Debug)]
struct ValueSelected {
    pub index: i32,
    pub value: AnyValue,
    /// is content value set
    pub is_set: bool,
}
impl ValueSelected {
    pub fn encode(value: AnyValue, is_set: bool, selected: Option<i32>, side: Side) -> TypedValue {
        match selected {
            None => value.encode(is_set, side),
            Some(selected) => ValueSelected {
                index: selected,
                value,
                is_set,
            }.encode(true, side),
        }
    }
}
impl Value for ValueSelected {
    fn get_widget_type(&self) -> typed_value::WidgetType {
        typed_value::WidgetType::TypeSelector
    }

    fn get_server_type(&self) -> ServerTypeId {
        self.value.get_server_type()
    }

    fn get_client_type(&self) -> ClientTypeId {
        self.value.get_client_type()
    }

    fn encode_storage(&self, side: Side) -> Option<typed_value::Storage> {
        typed_value::Storage::ValPoly(PolymorphicValue {
            chosen_type_index: self.index,
            actual_value: Some(self.value.encode(self.is_set, side).into()),
            extra_meta: None,
        }.into()).into()
    }
}
