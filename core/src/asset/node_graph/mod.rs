use super::value::{AnyValue, Value};
use crate::asset::generated::{asset_data, identifier, node_graph_container, node_graph_data, pin_signature, type_definition, typed_value, AssetData, ClientTypeId, GraphVariable, Identifier, NodeConnection, NodeGraphContainer, NodeGraphData, NodeInstance, PinData, PinSignature, PolymorphicValue, ServerTypeId, TypedValue, TypeDefinition, DynamicTypeMetadata, dynamic_type_metadata};
use crate::asset::{Asset, AssetBundle, AssetRef, Side};
use slab::Slab;
use std::collections::{HashMap, HashSet};
use tap::Tap;

pub mod arithmetic;
pub mod client;
pub mod control;
pub mod execution;
pub mod hidden;
pub mod query;
pub mod trigger;
pub mod composite;
pub mod decl;

pub use pin_signature::Kind as PinType;
use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::node_instance::DependencyDeclaration;
use crate::asset::generated::structure_definition_data::var_def::value::Val;
use crate::asset::generated::type_definition::server_type::Schema;
use crate::asset::generated::type_definition::TypeDetail;
pub(crate) use crate::asset::node_graph::composite::CompositeNodeGraph;
use crate::asset::node_graph::decl::NodeDecl;
use crate::asset::structure::ValueStruct;

#[derive(Copy, Clone)]
pub enum NodeGraphKind {
    ServerEntity,
}
impl NodeGraphKind {
    pub fn side(&self) -> Side {
        match self {
            NodeGraphKind::ServerEntity => Side::Server,
        }
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
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

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum NodeId {
    Low {
        kind: identifier::AssetKind,
        id: i64,
    },
    Unreachable,
    Local,
    SetLocal,
    Assemble,
    Destructure,
    Modify,
}
#[derive(Clone, Debug)]
pub struct NodeKind {
    pub id: NodeId,
    pub kernel_id: i64,

    pub controls_in_num: usize,
    pub controls_out_num: usize,
    pub values_in_types: Vec<Option<AnyValue>>,
    pub values_out_types: Vec<AnyValue>,

    pub selectors_in: Vec<Option<i32>>,
    pub selectors_out: Vec<Option<i32>>,

    pub imps_out: Vec<type_definition::server_type::Implementation>,

    pub references: Vec<Identifier>,

    pub using_struct: Option<Box<DependencyDeclaration>>,
}
impl PartialEq for NodeKind {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.kernel_id == other.kernel_id
    }
}
impl Eq for NodeKind {}
impl NodeKind {
    pub fn new(
        id: i64,
        controls_in_num: usize,
        controls_out_num: usize,
        values_in_types: Vec<AnyValue>,
        values_out_types: Vec<AnyValue>,
    ) -> Self {
        Self::full(NodeId::Low {
            id,
            kind: identifier::AssetKind::SysCallStub,
        }, id, controls_in_num, controls_out_num,values_in_types.into_iter().map(Some).collect(), values_out_types)
    }
    pub fn full(
        id: NodeId,
        kernel_id: i64,
        controls_in_num: usize,
        controls_out_num: usize,
        values_in_types: Vec<Option<AnyValue>>,
        values_out_types: Vec<AnyValue>,
    ) -> Self {
        Self {
            id,
            kernel_id,
            controls_in_num,
            controls_out_num,
            selectors_in: vec![None; values_in_types.len()],
            selectors_out: vec![None; values_out_types.len()],
            imps_out: vec![type_definition::server_type::Implementation::Primitive; values_out_types.len()],
            values_in_types,
            values_out_types,
            references: vec![],
            using_struct: None,
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
        let NodeId::Low { id, kind, .. } = self.id else {
            panic!("{:?}", self.id);
        };
        Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: kind as i32,
            guid: 0,
            runtime_id: id,
        }
    }
    fn encode_kernel(&self) -> Option<Identifier> {
        if self.kernel_id == 0 {
            return None;
        }
        Identifier {
            runtime_id: self.kernel_id,
            ..self.encode_shell()
        }.into()
    }
}
pub struct Node {
    pub kind: NodeKind,
    pub controls_in: Vec<Vec<Link>>,
    pub controls_out: Vec<Vec<Link>>,
    pub values_in: Vec<ValueIn>,
    pub values_out: Vec<Vec<Link>>,
}
impl From<NodeKind> for Node {
    fn from(kind: NodeKind) -> Self {
        Self::new(kind)
    }
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
    #[must_use]
    pub fn get_neighbors(&self) -> HashSet<NodeRef> {
        self.controls_in.iter()
            .chain(self.controls_out.iter())
            .chain(self.values_out.iter())
            .flatten().copied()
            .chain(self.values_in.iter().flat_map(|x| x.link))
            .filter_map(Link::connection).map(|x| x.node()).collect()
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

pub struct NodeGraph {
    pub class: NodeGraphKind,
    pub name: String,
    pub nodes: Slab<Node>,
    pub embedded: HashMap<DependencyDeclaration, NodeDecl>,
}
impl NodeGraph {
    pub fn new(class: NodeGraphKind, name: impl Into<String>) -> Self {
        Self {
            class,
            name: name.into(),
            nodes: Default::default(),
            embedded: Default::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn get_node(&self, key: NodeRef) -> &Node {
        &self.nodes[key.into()]
    }

    pub fn get_node_mut(&mut self, key: NodeRef) -> &mut Node {
        &mut self.nodes[key.into()]
    }

    pub fn get_nodes(&self) -> Vec<NodeRef> {
        self.nodes.iter().map(|x| x.0.into()).collect()
    }

    pub fn insert(&mut self, node: Node) -> NodeRef {
        self.nodes.insert(node).into()
    }
    pub fn remove(&mut self, key: NodeRef) -> Node {
        let node = self.nodes.remove(key.into());
        for Connection(r, i) in node.values_in.iter().flat_map(|x| x.link).flat_map(Link::connection) {
            self.get_node_mut(r).values_out[i].retain(|x| !matches!(x, Link::Connection(Connection(x, _)) if *x == key));
        }
        for Connection(r, i) in node.values_out.iter().flatten().copied().flat_map(Link::connection) {
            let l = &mut self.get_node_mut(r).values_in[i].link;
            if let Some(Link::Connection(Connection(x, _))) = l && *x == key {
                *l = None;
            }
        }
        for Connection(r, i) in node.controls_in.iter().flatten().copied().flat_map(Link::connection) {
            self.get_node_mut(r).controls_out[i].retain(|x| !matches!(x, Link::Connection(Connection(x, _)) if *x == key));
        }
        for Connection(r, i) in node.controls_out.iter().flatten().copied().flat_map(Link::connection) {
            self.get_node_mut(r).controls_in[i].retain(|x| !matches!(x, Link::Connection(Connection(x, _)) if *x == key));
        }
        node
    }

    pub fn set_default(&mut self, place: Connection, value: AnyValue) {
        let node = &mut self.nodes[place.node().into()];
        if let Some(kind) = &node.kind.values_in_types[place.pin()] {
            assert!(kind.is_instance(&value), "{value:?} is not {:?}", node.kind.values_in_types[place.pin()]);
        }
        node.values_in[place.pin()].default = Some(value);
    }

    pub fn connect_value(&mut self, from: Connection, to: Connection) {
        let to_node = self.nodes.get_mut(to.node().into()).unwrap();
        if let Some(Link::Connection(Connection(f, i))) = to_node.values_in[to.pin()].link {
            self.get_node_mut(f).values_out[i].retain(|x| !matches!(*x, Link::Connection(t) if t == to));
        }
        let (from_node, to_node) = self.nodes.get2_mut(from.node().into(), to.node().into()).unwrap();
        if let Some(kind) = &to_node.kind.values_in_types[to.pin()] {
            let kind1 = &from_node.kind.values_out_types[from.pin()];
            assert!(kind.is_instance(kind1) || kind1.is_instance(kind), "Type error: {:?} and {:?}", kind1, kind);
        }
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
        if let Some(Link::Connection(Connection(f, i))) = self.nodes[inner.node().into()].values_in[inner.pin()].link {
            self.get_node_mut(f).values_out[i].retain(|x| !matches!(*x, Link::Connection(t) if t == inner));
        }
        let node = &mut self.nodes[inner.node().into()];
        node.values_in[inner.pin()].link = Some(Link::Export(outer));
    }

    pub fn export_value_out(&mut self, inner: Connection, outer: usize) {
        // TODO: optimize
        for x in self.nodes.iter_mut().flat_map(|(_, x)| x.values_out.iter_mut()) {
            x.retain(|x| *x != Link::Export(outer));
        }
        self.nodes[inner.node().into()].values_out[inner.pin()].push(Link::Export(outer));
    }

    fn apply(self, id: Identifier) -> AssetData {
        let mut references = vec![];
        for (_, x) in &self.nodes {
            references.extend(x.kind.references.clone());
        }
        let side = self.class.side();
        AssetData {
            id: id.into(),
            references,
            name: self.name.clone(),
            r#type: match self.class {
                NodeGraphKind::ServerEntity => asset_data::Type::EntityNodeGraph,
            } as i32,
            payload: Some(Payload::GraphData(NodeGraphContainer {
                inner: Some(node_graph_container::InnerWrapper {
                    graph: NodeGraphData {
                        id: Some(Identifier {
                            source: identifier::Source::UserDefined as i32,
                            category: identifier::Category::ServerBasic as i32,
                            kind: identifier::AssetKind::CustomGraph as i32,
                            guid: 0,
                            runtime_id: id.guid,
                        }),
                        display_name: self.name.clone(),
                        node: self.nodes.iter().map(|(i, n)| NodeInstance {
                            index: NodeRef::from(i).encode(),
                            shell_ref: Some(n.kind.encode_shell()),
                            kernel_ref: n.kind.encode_kernel(),
                            pins: vec![].tap_mut(|pins| {
                                for (i, x) in n.controls_out.iter().enumerate() {
                                    let sig = PinSignature {
                                        kind: PinType::OutControl as i32,
                                        index: i as i32,
                                        source_ref: None,
                                    };
                                    let connections = x.iter().copied().flat_map(Link::connection).collect::<Vec<_>>();
                                    if connections.is_empty() {
                                        continue;
                                    }
                                    pins.push(PinData {
                                        shell_sig: sig.into(),
                                        kernel_sig: sig.into(),
                                        value: None,
                                        r#type: None,
                                        connection: connections.iter().map(|Connection(target, j)| {
                                            let sig_tar = PinSignature {
                                                kind: PinType::InControl as i32,
                                                index: *j as i32,
                                                source_ref: None,
                                            };
                                            NodeConnection {
                                                target_node_index: target.encode(),
                                                target_pin_shell: sig_tar.into(),
                                                target_pin_kernel: sig_tar.into(),
                                            }
                                        }).collect(),
                                        binding_meta: None,
                                        persistent_pin_uid: None,
                                    })
                                }
                                let handle_value = |k1, k2, i, kernel, kind: &AnyValue, s, def: &Option<_>, link: &Vec<_>, imp| {
                                    let sig = PinSignature {
                                        kind: k1 as i32,
                                        index: i,
                                        source_ref: None,
                                    };
                                    PinData {
                                        shell_sig: sig.into(),
                                        kernel_sig: sig.tap_mut(|sig| sig.index = kernel).into(),
                                        value: ValueSelected::encode(def.clone().unwrap_or_else(|| kind.clone()), def.is_some(), s, side).tap_mut(|it|
                                            if let Some(TypedValue { storage: Some(typed_value::Storage::ValPoly(it)) , .. }) = it {
                                                if imp == type_definition::server_type::Implementation::Struct {
                                                    it.extra_meta = DynamicTypeMetadata {
                                                        version: 1,
                                                        config: dynamic_type_metadata::Config {
                                                            inner: dynamic_type_metadata::config::Inner {
                                                                container_style: typed_value::WidgetType::StructBlock as i32,
                                                                item_style: None,
                                                                schema_binding: dynamic_type_metadata::config::inner::SchemaBinding::TargetStructId(type_definition::StructReference {
                                                                    schema_id: kind.downcast_ref::<ValueStruct>().unwrap().get_struct_id(),
                                                                }).into(),
                                                            }.into(),
                                                        }.into(),
                                                    }.into();
                                                }
                                                if let PolymorphicValue { actual_value: Some(it), .. } = it.as_mut()
                                                        && let TypedValue { r#type: Some(TypeDefinition { type_detail: Some(TypeDetail::ServerSide(type_definition::ServerType { r#impl, .. })), .. }), .. } = it.as_mut() {
                                                    *r#impl = imp as i32;
                                                }
                                            }),
                                        r#type: Some(kind.get_type_id(side)),
                                        connection: link.iter().copied().filter_map(Link::connection).map(|Connection(target, j)| {
                                            let sig_tar = PinSignature {
                                                kind: k2 as i32,
                                                index: j as i32,
                                                source_ref: None,
                                            };
                                            NodeConnection {
                                                target_node_index: target.encode(),
                                                target_pin_shell: sig_tar.into(),
                                                target_pin_kernel: sig_tar.into(),
                                            }
                                        }).collect(),
                                        binding_meta: None,
                                        persistent_pin_uid: None,
                                    }
                                };
                                for (i, x) in n.values_out.iter().enumerate() {
                                    let Some(s) = n.kind.selectors_out[i] else {
                                        continue;
                                    };
                                    pins.push(handle_value(PinType::OutValue, PinType::InValue, i as i32, i as i32, &n.kind.values_out_types[i], Some(s), &None, x, n.kind.imps_out[i]));
                                }
                                let mut kernel = 0;
                                for (i, x) in n.values_in.iter().enumerate() {
                                    let Some(ref kind) = n.kind.values_in_types[i] else {
                                        continue;
                                    };
                                    if !x.is_unset() {
                                        pins.push(handle_value(PinType::InValue, PinType::OutValue, i as i32, kernel, kind, n.kind.selectors_in[i], &x.default, &x.link.iter().copied().collect(), type_definition::server_type::Implementation::Primitive));
                                    }
                                    kernel += 1;
                                }
                            }),
                            x_pos: 0.,
                            y_pos: 0.,
                            attached_comment: None, // TODO
                            context_declaration: None, // TODO
                            signal_version: None, // TODO
                            using_struct: n.kind.using_struct.clone().map(|x| *x), // TODO
                        }).collect(),
                        port_mapping: vec![],
                        comment: vec![],
                        blackboard: vec![],
                        embedded: self.embedded.into_iter().map(|(decl, v)| node_graph_data::Embedded {
                            decl: decl.into(),
                            unknown1: node_graph_data::embedded::Unknown1 { unknown1: 1 }.into(), // maybe 25?
                            inter: v.encode(None).into(),
                            data: None,
                        }).collect(),
                        entry_slot_index: None,
                        evaluation_interval: None,
                    }.into(),
                }),
            })),
        }
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
            storage_value: Some(self.value.encode_typed(self.is_set, Side::Server)),
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

pub struct MainNodeGraph {
    pub graph: NodeGraph,
    pub statics: Vec<NodeGraphStatic>,
}
impl MainNodeGraph {
    pub fn new(graph: NodeGraph) -> Self {
        Self {
            graph,
            statics: Default::default(),
        }
    }
}
impl Asset for MainNodeGraph {
    type RefData = ();

    fn apply(self, bundle: &mut AssetBundle) -> AssetRef<Self> {
        let id = bundle.alloc(identifier::Category::ServerNodeGraph, identifier::AssetKind::Basic);
        bundle.push(self.graph.apply(id).tap_mut(|data| {
            match data.payload.as_mut().unwrap() {
                Payload::GraphData(data) => data.inner.as_mut().unwrap().graph.as_mut().unwrap().blackboard = self.statics.iter().map(NodeGraphStatic::encode).collect(),
                _ => unreachable!(),
            }
        }));
        AssetRef::new(id, ())
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
    pub fn encode(value: AnyValue, is_set: bool, selected: Option<i32>, side: Side) -> Option<TypedValue> {
        match selected {
            None => if is_set { value.encode_typed(is_set, side).into() } else { None },
            Some(selected) => ValueSelected {
                index: selected,
                value,
                is_set,
            }.encode_typed(true, side).into(), // TODO
        }
    }
}
impl Value for ValueSelected {
    fn get_widget_type(&self) -> Option<typed_value::WidgetType> {
        typed_value::WidgetType::TypeSelector.into()
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
            actual_value: Some(self.value.encode_typed(self.is_set, side).into()),
            extra_meta: None,
        }.into()).into()
    }

    fn encode_field_value(&self) -> Val {
        panic!()
    }
}
