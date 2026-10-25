use crate::asset::generated::{Annotation, AssetData, GraphVariable, Identifier, InterfaceMapping, NodeConnection, NodeGraphContainer, NodeGraphData, NodeInstance, PinData, PinSignature, PolymorphicValue, TypeDefinition, TypedValue, asset_data, identifier, node_graph_container, node_graph_data, pin_signature, type_definition, typed_value};
use crate::asset::{AssetBundle, Side};
use crate::value::{NativeKind, NativeValue};
use slab::Slab;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use tap::{Pipe, Tap};

pub mod arithmetic;
pub mod client;
pub mod control;
pub mod execution;
pub mod hidden;
pub mod list;
pub mod query;
pub mod trigger;
pub mod composite;
pub mod decl;
pub mod layout;

pub use pin_signature::Kind as PinType;
use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::node_instance::DependencyDeclaration;
use crate::asset::generated::pin_signature::Kind;
use crate::asset::generated::type_definition::TypeDetail;
use crate::asset::generated::type_definition::server_type::Schema;
use crate::node::decl::NodeDecl;
use crate::node::list::{ASSEMBLE_SHELL, MAX_ASSEMBLE_ELEMENTS};

#[derive(Copy, Clone)]
#[derive(serde::Serialize, serde::Deserialize)]
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
#[derive(serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Eq, Debug)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct NativeNodeId {
    pub kind: identifier::AssetKind,
    pub id: i64,
    pub kernel: i64,

    pub selectors_in: Vec<Option<i32>>,
    pub selectors_out: Vec<Option<i32>>,

    pub imps_in: Vec<type_definition::server_type::Implementation>,
    pub imps_out: Vec<type_definition::server_type::Implementation>,

    pub references: Vec<Identifier>,

    pub using_struct: Option<Box<DependencyDeclaration>>,
}
impl NativeNodeId {
    pub fn shell_eq(&self, other: &Self) -> bool {
        self.id == other.id && self.kind == other.kind
    }
}
impl PartialEq for NativeNodeId {
    fn eq(&self, other: &Self) -> bool {
        self.shell_eq(other) && self.kernel == other.kernel
    }
}
impl Hash for NativeNodeId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.kind.hash(state);
        self.kernel.hash(state);
    }
}

#[derive(Clone, Debug)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct NodeKind<NodeId = NativeNodeId, Kind = NativeKind> {
    pub id: NodeId,

    pub controls_in_num: usize,
    pub controls_out_num: usize,
    pub values_in_types: Vec<Option<Kind>>,
    pub values_out_types: Vec<Kind>,
}
impl PartialEq for NodeKind {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for NodeKind {}
impl<NodeId, Kind> NodeKind<NodeId, Kind> {
    pub fn new(
        id: NodeId,
        controls_in_num: usize,
        controls_out_num: usize,
        values_in_types: Vec<Option<Kind>>,
        values_out_types: Vec<Kind>,
    ) -> Self {
        Self {
            id,
            controls_in_num,
            controls_out_num,
            values_in_types,
            values_out_types,
        }
    }
}
impl NodeKind {
    pub fn native(
        kind: identifier::AssetKind,
        id: i64,
        kernel: i64,
        controls_in_num: usize,
        controls_out_num: usize,
        values_in_types: Vec<Option<NativeKind>>,
        values_out_types: Vec<NativeKind>,
    ) -> Self {
        Self::new(NativeNodeId {
            kind,
            id,
            kernel,
            selectors_in: vec![None; values_in_types.len()],
            selectors_out: vec![None; values_out_types.len()],
            imps_in: vec![type_definition::server_type::Implementation::Primitive; values_in_types.len()],
            imps_out: vec![type_definition::server_type::Implementation::Primitive; values_out_types.len()],
            references: vec![],
            using_struct: None,
        }, controls_in_num, controls_out_num, values_in_types, values_out_types)
    }
    pub fn simple(
        id: i64,
        controls_in_num: usize,
        controls_out_num: usize,
        values_in_types: Vec<NativeKind>,
        values_out_types: Vec<NativeKind>,
    ) -> Self {
        Self::native(
            identifier::AssetKind::SysCallStub,
            id,
            0,
            controls_in_num,
            controls_out_num,
            values_in_types.into_iter().map(Some).collect(),
            values_out_types
        )
    }
    pub fn expr(id: i64, values_in_types: Vec<NativeKind>, value_out_type: NativeKind) -> Self {
        Self::simple(id, 0, 0, values_in_types, vec![value_out_type])
    }
    pub fn func(id: i64, values_in_types: Vec<NativeKind>, value_out_type: NativeKind) -> Self {
        Self::simple(id, 1, 1, values_in_types, vec![value_out_type])
    }
    pub fn procedure(id: i64, values_in_types: Vec<NativeKind>) -> Self {
        Self::simple(id, 1, 1, values_in_types, vec![])
    }
    pub fn trigger(id: i64, value_out_type: Vec<NativeKind>) -> Self {
        Self::simple(id, 0, 1, vec![], value_out_type)
    }

    fn encode_shell(&self) -> Identifier {
        Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: self.id.kind as i32,
            guid: 0,
            runtime_id: self.id.id,
        }
    }
    fn encode_kernel(&self) -> Option<Identifier> {
        if self.id.kernel == 0 {
            return None;
        }
        Identifier {
            runtime_id: self.id.kernel,
            ..self.encode_shell()
        }.into()
    }
}
#[derive(Clone)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Node<NodeId = NativeNodeId, Kind = NativeKind> {
    pub kind: NodeKind<NodeId, Kind>,
    pub links: Links,
    /// 自动整理算出的坐标(左上角),默认 (0,0)。`.gia` 的 `x_pos` / `y_pos`
    /// 就是从这里编码的;viewer 与 core 的布局共用 [`layout`](layout::layout)。
    pub position: (f32, f32),
    pub comment: Option<String>,
}
#[derive(Default, Clone)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Links {
    pub controls_out: Vec<Vec<Link>>,
    pub controls_in: Vec<Vec<Link>>,
    pub values_out: Vec<Vec<Link>>,
    pub values_in: Vec<ValueIn>,
}
impl Links {
    pub fn new<NodeId, Kind>(kind: &NodeKind<NodeId, Kind>) -> Self {
        Self {
            controls_in: vec![Default::default(); kind.controls_in_num],
            controls_out: vec![Default::default(); kind.controls_out_num],
            values_in: vec![Default::default(); kind.values_in_types.len()],
            values_out: vec![Default::default(); kind.values_out_types.len()],
        }
    }
    pub fn is_empty(&self) -> bool {
        self.controls_out.iter().all(Vec::is_empty) && self.controls_in.iter().all(Vec::is_empty) && self.values_out.iter().all(Vec::is_empty) && self.values_in.iter().all(|x| x.link.is_none())
    }
    #[must_use]
    pub fn get_neighbors(&self) -> HashSet<NodeRef> {
        self.controls_in.iter()
            .chain(self.controls_out.iter())
            .chain(self.values_out.iter())
            .flatten().copied()
            .chain(self.values_in.iter().flat_map(|x| x.link))
            .filter_map(|link| link.target.node()).collect()
    }
}

impl<NodeId, Kind> Node<NodeId, Kind> {
    pub fn new(kind: NodeKind<NodeId, Kind>) -> Self {
        Self {
            links: Links::new(&kind),
            kind,
            position: (0., 0.),
            comment: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum LinkTarget {
    Node(NodeRef),
    Export,
}
impl LinkTarget {
    pub fn node(self) -> Option<NodeRef> {
        if let LinkTarget::Node(node) = self {
            Some(node)
        } else {
            None
        }
    }
    pub fn is_export(self) -> bool {
        matches!(self, LinkTarget::Export)
    }
    pub fn is_node(self) -> bool {
        !self.is_export()
    }
}
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Link {
    pub target: LinkTarget,
    pub index: usize,
}
impl Link {
    pub fn new(target: LinkTarget, index: usize) -> Self {
        Self { target, index }
    }
    pub fn node(node: NodeRef, index: usize) -> Self {
        Self::new(LinkTarget::Node(node), index)
    }
    pub fn export(index: usize) -> Self {
        Self::new(LinkTarget::Export, index)
    }
}

#[derive(Clone, Debug, Default)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ValueIn {
    pub default: Option<Box<dyn NativeValue>>,
    pub link: Option<Link>,
}
impl ValueIn {
    pub fn value(default: Box<dyn NativeValue>) -> Self {
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

pub trait Lower {
    type NodeId0;
    type Kind0;
    type NodeId1;
    type Kind1;

    fn lower_node_id(&mut self, node_id: Self::NodeId0) -> Self::NodeId1;
    fn lower_kind(&mut self, kind: Self::Kind0) -> Self::Kind1;

    fn lower_node_kind(&mut self, node_kind: NodeKind<Self::NodeId0, Self::Kind0>) -> NodeKind<Self::NodeId1, Self::Kind1> {
        NodeKind {
            id: self.lower_node_id(node_kind.id),
            values_in_types: node_kind.values_in_types.into_iter().map(|x| x.map(|x| self.lower_kind(x))).collect(),
            values_out_types: node_kind.values_out_types.into_iter().map(|x| self.lower_kind(x)).collect(),
            ..node_kind
        }
    }

    fn lower_node(&mut self, node: Node<Self::NodeId0, Self::Kind0>) -> Node<Self::NodeId1, Self::Kind1> {
        Node {
            kind: self.lower_node_kind(node.kind),
            ..node
        }
    }

    fn lower_export_decl(&mut self, export: ExportDecl<Self::Kind0>) -> ExportDecl<Self::Kind1> {
        ExportDecl {
            kind: export.kind.map(|x| self.lower_kind(x)),
            ..export
        }
    }

    fn lower_graph(&mut self, graph: NodeGraph<Self::NodeId0, Self::Kind0>) -> NodeGraph<Self::NodeId1, Self::Kind1> {
        NodeGraph {
            nodes: graph.nodes.into_iter().map(|(i, x)| (i, self.lower_node(x))).collect(),
            exports: graph.exports.into_iter().map(|(i, x)| (i, x.into_iter().map(|x| self.lower_export_decl(x)).collect())).collect(),
            ..graph
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ExportDecl<Kind = NativeKind> {
    name: String,
    kind: Option<Kind>,
}
impl<Kind> ExportDecl<Kind> {
    pub fn new(name: String, kind: Option<Kind>) -> Self {
        Self {
            name,
            kind,
        }
    }

    pub fn kind(&self) -> Option<&Kind> {
        self.kind.as_ref()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct NodeGraph<NodeId = NativeNodeId, Kind = NativeKind> {
    pub class: NodeGraphKind,
    pub name: String,
    pub nodes: Slab<Node<NodeId, Kind>>,
    pub exports: HashMap<PinType, Vec<ExportDecl<Kind>>>,
    pub externals: Links,
    pub embedded: HashMap<DependencyDeclaration, NodeDecl>,
}
impl<NodeId, Kind> NodeGraph<NodeId, Kind> {
    pub fn new(class: NodeGraphKind, name: impl Into<String>) -> Self {
        Self {
            class,
            name: name.into(),
            nodes: Default::default(),
            exports: HashMap::new().tap_mut(|exports| {
                exports.insert(PinType::InControl, vec![]);
                exports.insert(PinType::OutControl, vec![]);
                exports.insert(PinType::InValue, vec![]);
                exports.insert(PinType::OutValue, vec![]);
            }),
            externals: Default::default(),
            embedded: Default::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn get_node(&self, key: NodeRef) -> &Node<NodeId, Kind> {
        &self.nodes[key.into()]
    }

    pub fn get_node_mut(&mut self, key: NodeRef) -> &mut Node<NodeId, Kind> {
        &mut self.nodes[key.into()]
    }

    pub fn kinds_mut(&mut self) -> impl Iterator<Item = &mut Kind> {
        self.nodes.iter_mut().map(|(_, x)| &mut x.kind).flat_map(|kind|
            kind.values_in_types.iter_mut().flatten().chain(kind.values_out_types.iter_mut())
        ).chain(self.exports.values_mut().flatten().flat_map(|d| d.kind.as_mut()))
    }

    pub fn get_nodes(&self) -> Vec<NodeRef> {
        self.nodes.iter().map(|x| x.0.into()).collect()
    }

    pub fn insert(&mut self, node: NodeKind<NodeId, Kind>) -> NodeRef {
        self.nodes.insert(Node::new(node)).into()
    }

    pub fn push_export_value_in(&mut self, decl: ExportDecl<Kind>) {
        self.exports.get_mut(&PinType::InValue).unwrap().push(decl);
        self.externals.values_out.push(Default::default());
    }

    pub fn push_export_value_out(&mut self, decl: ExportDecl<Kind>) {
        self.exports.get_mut(&PinType::OutValue).unwrap().push(decl);
        self.externals.values_in.push(Default::default());
    }

    pub fn push_export_control_in(&mut self, decl: ExportDecl<Kind>) {
        self.exports.get_mut(&PinType::InControl).unwrap().push(decl);
        self.externals.controls_out.push(Default::default());
    }

    pub fn push_export_control_out(&mut self, decl: ExportDecl<Kind>) {
        self.exports.get_mut(&PinType::OutControl).unwrap().push(decl);
        self.externals.controls_in.push(Default::default());
    }

    pub fn get_links(&self, key: LinkTarget) -> &Links {
        match key {
            LinkTarget::Node(node) => &self.get_node(node).links,
            LinkTarget::Export => &self.externals,
        }
    }
    
    fn get_links_mut(&mut self, key: LinkTarget) -> &mut Links {
        match key {
            LinkTarget::Node(node) => &mut self.get_node_mut(node).links,
            LinkTarget::Export => &mut self.externals,
        }
    }

    pub fn set_comment(&mut self, node: NodeRef, comment: Option<String>) {
        self.get_node_mut(node).comment = comment;
    }

    pub fn move_comment(&mut self, from: NodeRef, to: NodeRef) {
        let comment = self.get_node_mut(from).comment.take();
        self.set_comment(to, comment);
    }

    pub fn controls_out_mut(&mut self, link: Link) -> &mut Vec<Link> {
        &mut self.get_links_mut(link.target).controls_out[link.index]
    }
    pub fn controls_in_mut(&mut self, link: Link) -> &mut Vec<Link> {
        &mut self.get_links_mut(link.target).controls_in[link.index]
    }
    pub fn values_out_mut(&mut self, link: Link) -> &mut Vec<Link> {
        &mut self.get_links_mut(link.target).values_out[link.index]
    }
    pub fn value_in_mut(&mut self, link: Link) -> &mut ValueIn {
        &mut self.get_links_mut(link.target).values_in[link.index]
    }

    pub fn unlink_control(&mut self, from: Link, to: Link) {
        self.controls_out_mut(from).retain(|x| *x != to);
        self.controls_in_mut(to).retain(|x| *x != from);
    }

    pub fn unlink_value(&mut self, from: Link, to: Link) {
        self.values_out_mut(from).retain(|&x| x != to);
        self.value_in_mut(to).pipe(|x| if x.link == Some(from) { x.link = None; });
    }

    pub fn unlink_value_in(&mut self, place: Link) {
        if let Some(from) = self.value_in_mut(place).link {
            self.unlink_value(from, place);
        }
    }

    pub fn remove(&mut self, key: NodeRef) -> NodeKind<NodeId, Kind> {
        let links = self.get_node(key).links.clone();
        for (i, x) in links.controls_out.into_iter().enumerate() {
            for to in x {
                self.unlink_control(Link::node(key, i), to);
            }
        }
        for (i, x) in links.controls_in.into_iter().enumerate() {
            for from in x {
                self.unlink_control(from, Link::node(key, i));
            }
        }
        for (i, x) in links.values_out.into_iter().enumerate() {
            for to in x {
                self.unlink_value(Link::node(key, i), to);
            }
        }
        for (i, x) in links.values_in.into_iter().enumerate() {
            if let Some(from) = x.link {
                self.unlink_value(from, Link::node(key, i));
            }
        }
        assert!(self.get_node(key).links.is_empty());
        self.nodes.remove(key.into()).kind
    }

    pub fn set_default(&mut self, place: Link, value: Option<Box<dyn NativeValue>>) {
        self.value_in_mut(place).default = value;
    }

    pub fn link_value(&mut self, from: Link, to: Link) {
        self.unlink_value_in(to);
        self.values_out_mut(from).push(to);
        self.value_in_mut(to).link = Some(from);
    }

    /// 在节点的值入口最前面插一个空引脚,原来的入线整体后移一位。
    ///
    /// 连接是双向记账的(`values_in.link` 与上游的 `values_out`),所以不能只对
    /// `values_in` 做 `insert`:上游记的旧下标得一起改,否则 [`Self::verify`]
    /// 和所有顺着 `values_out` 走的优化器都会看错引脚。
    pub fn insert_value_in_front(&mut self, node: NodeRef) {
        let incoming: Vec<Option<Link>> = self
            .get_node(node)
            .links
            .values_in
            .iter()
            .map(|x| x.link)
            .collect();
        for (i, from) in incoming.into_iter().enumerate() {
            let Some(from) = from else { continue };
            for to in self.values_out_mut(from) {
                if *to == Link::node(node, i) {
                    *to = Link::node(node, i + 1);
                }
            }
        }
        self.get_node_mut(node).links.values_in.insert(0, ValueIn::default());
    }

    pub fn set_value_in(&mut self, place: Link, value: ValueIn) {
        self.set_default(place, value.default);
        if let Some(from) = value.link {
            self.link_value(from, place);
        } else {
            self.unlink_value_in(place);
        }
    }

    pub fn link_control(&mut self, from: Link, to: Link) {
        self.controls_out_mut(from).push(to);
        self.controls_in_mut(to).push(from);
    }

    pub fn link_controls(&mut self, from: Link, to: &[Link]) {
        for &to in to {
            self.link_control(from, to);
        }
    }

    pub fn relink_controls(&mut self, from: &[Link], to0: Link, to1: &[Link]) {
        for &from in from {
            self.relink_control(from, to0, to1);
        }
    }

    pub fn relink_control(&mut self, from: Link, to0: Link, to1: &[Link]) {
        let mut a = false;
        self.controls_out_mut(from).pipe(|from| *from = from.iter().flat_map(|&x| if x == to0 {
            assert!(!a);
            a = true;
            to1.to_vec()
        } else { vec![x] }).collect());
        if a {
            self.controls_in_mut(to0).retain(|&x| x != from);
            for x in to1 {
                self.controls_in_mut(*x).push(from);
            }
        }
    }

    pub fn verify(&self) {
        for (i, x) in &self.nodes {
            for (j, y) in x.links.values_in.iter().enumerate() {
                if let Some(z) = &y.link && let Some(w) = z.target.node() {
                    assert!(self.get_node(w).links.values_out[z.index].contains(&Link::node(i.into(), j)));
                }
            }
            for (j, y) in x.links.values_out.iter().enumerate() {
                for z in y {
                    if let Some(w) = z.target.node() {
                        assert_eq!(self.get_node(w).links.values_in[z.index].link, Some(Link::node(i.into(), j)));
                    }
                }
            }
            for (j, y) in x.links.controls_in.iter().enumerate() {
                for z in y {
                    if let Some(w) = z.target.node() {
                        assert!(self.get_node(w).links.controls_out[z.index].contains(&Link::node(i.into(), j)));
                    }
                }
            }
            for (j, y) in x.links.controls_out.iter().enumerate() {
                for z in y {
                    if let Some(w) = z.target.node() {
                        assert!(self.get_node(w).links.controls_in[z.index].contains(&Link::node(i.into(), j)));
                    }
                }
            }
        }
    }
}
impl NodeGraph {
    fn apply(self, id: Identifier) -> AssetData {
        const POS_SCALA: f32 = 5.;
        let mut references = vec![];
        for (_, x) in &self.nodes {
            references.extend(x.kind.id.references.clone());
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
                                for (i, x) in n.links.controls_out.iter().enumerate() {
                                    let sig = PinSignature {
                                        kind: PinType::OutControl as i32,
                                        index: i as i32,
                                        source_ref: None,
                                    };
                                    let connections = x.iter().copied().filter_map(|x| x.target.node().map(|y| (y, x.index))).collect::<Vec<_>>();
                                    if connections.is_empty() {
                                        continue;
                                    }
                                    pins.push(PinData {
                                        shell_sig: sig.into(),
                                        kernel_sig: sig.into(),
                                        value: None,
                                        r#type: None,
                                        connection: connections.iter().map(|(target, j)| {
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
                                let handle_value = |k1, k2, i, kernel, kind: &NativeKind, s, def: Option<&dyn NativeValue>, link: &Vec<Link>, imp| {
                                    let sig = PinSignature {
                                        kind: k1 as i32,
                                        index: i,
                                        source_ref: None,
                                    };
                                    PinData {
                                        shell_sig: sig.into(),
                                        kernel_sig: sig.tap_mut(|sig| sig.index = kernel).into(),
                                        value: encode_selected(kind, def, s, side).tap_mut(|it|
                                            if let TypedValue { storage: Some(typed_value::Storage::ValPoly(it)) , .. } = it {
                                                if imp == type_definition::server_type::Implementation::Struct {
                                                    it.extra_meta = kind.encode_metadata().into();
                                                }
                                                if let PolymorphicValue { actual_value: Some(it), .. } = it.as_mut()
                                                        && let TypedValue { r#type: Some(TypeDefinition { type_detail: Some(TypeDetail::ServerSide(type_definition::ServerType { r#impl, .. })), .. }), .. } = it.as_mut() {
                                                    *r#impl = imp as i32;
                                                }
                                            }).into(),
                                        r#type: Some(kind.get_type_id(side)),
                                        connection: link.iter().copied().filter_map(|x| x.target.node().map(|y| (y, x.index))).map(|(target, j)| {
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
                                // 拼装列表在 `.gia` 里固定占满节点定义的 MAX 个元素槽
                                // (没连的隐藏),IR 里只保留实际用到的元素引脚 ——
                                // 多出来的槽在这里补,类型与选择器照抄最后一个真实元素引脚。
                                let slots = if n.kind.id.id == ASSEMBLE_SHELL {
                                    (MAX_ASSEMBLE_ELEMENTS + 1).max(n.links.values_in.len())
                                } else {
                                    n.links.values_in.len()
                                };
                                let pad_kind = n.kind.values_in_types.last().cloned().flatten();
                                let pad_selector = n.kind.id.selectors_in.last().copied().flatten();
                                let pad_imp = n.kind.id.imps_in.last().copied()
                                    .unwrap_or(type_definition::server_type::Implementation::Primitive);
                                let unset = ValueIn::default();
                                let mut kernel = 0;
                                for i in 0..slots {
                                    let x = n.links.values_in.get(i).unwrap_or(&unset);
                                    let (kind, selector, imp) = match n.kind.values_in_types.get(i) {
                                        Some(Some(kind)) =>
                                            (kind, n.kind.id.selectors_in[i], n.kind.id.imps_in[i]),
                                        Some(None) => continue,
                                        None => match &pad_kind {
                                            Some(kind) => (kind, pad_selector, pad_imp),
                                            None => continue,
                                        },
                                    };
                                    // 泛型引脚即使没连线也输出:官方导出里节点定义的
                                    // 每个元素槽都带着类型选择器模板(隐藏)。
                                    if !x.is_unset() || selector.is_some() {
                                        pins.push(handle_value(PinType::InValue, PinType::OutValue, i as i32, kernel, kind, selector, x.default.as_deref(), &x.link.iter().copied().collect(), imp));
                                    }
                                    kernel += 1;
                                }
                                // 输出引脚排在输入之后(与官方导出一致)。
                                for (i, x) in n.links.values_out.iter().enumerate() {
                                    let Some(s) = n.kind.id.selectors_out[i] else {
                                        continue;
                                    };
                                    pins.push(handle_value(PinType::OutValue, PinType::InValue, i as i32, i as i32, &n.kind.values_out_types[i], Some(s), None, x, n.kind.id.imps_out[i]));
                                }
                            }),
                            x_pos: n.position.0 * POS_SCALA,
                            y_pos: n.position.1 * POS_SCALA,
                            attached_comment: n.comment.clone().map(|x| Annotation {
                                text: x,
                                x_pos: None,
                                y_pos: None,
                            }),
                            context_declaration: None, // TODO
                            signal_version: None, // TODO
                            using_struct: n.kind.id.using_struct.clone().map(|x| *x), // TODO
                        }).collect(),
                        port_mapping: vec![].tap_mut(|port_mapping| {
                            for (&kind, p) in self.exports.iter() {
                                for (i, _decl) in p.iter().enumerate() {
                                    for link in match kind {
                                        Kind::InControl => Box::new(self.externals.controls_out[i].iter()) as Box<dyn Iterator<Item = &Link>>,
                                        Kind::OutControl => Box::new(self.externals.controls_in[i].iter()),
                                        Kind::InValue => Box::new(self.externals.values_out[i].iter()),
                                        Kind::OutValue => Box::new(self.externals.values_in[i].link.iter()),
                                        _ => panic!(),
                                    } {
                                        let LinkTarget::Node(target) = link.target else {
                                            panic!();
                                        };
                                        let sig = |idx: usize| PinSignature {
                                            kind: kind as i32,
                                            index: idx as i32,
                                            source_ref: None,
                                        };
                                        port_mapping.push(InterfaceMapping {
                                            external_port: Some(sig(i)),
                                            // Link 的 NodeRef 是节点下标(0 起),穿透目标用图内 index(1 起)
                                            internal_target_node_handle: target.encode(),
                                            internal_port_shell: Some(sig(link.index)),
                                            internal_port_kernel: Some(sig(link.index)),
                                        });
                                    }
                                }
                            }
                        }),
                        comment: vec![],
                        blackboard: vec![],
                        embedded: self.embedded.into_iter().map(|(decl, v)| node_graph_data::Embedded {
                            decl: decl.into(),
                            unknown1: node_graph_data::embedded::Unknown1 { unknown1: 1 }.into(), // FIXME: 25 for node_on_native_custom_value_change? 78 for node_list_clear?
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
    pub kind: NativeKind,
    pub default: Option<Box<dyn NativeValue>>,
    pub is_public: bool,
}

impl NodeGraphStatic {
    pub fn new(name: impl Into<String>, kind: NativeKind, default: Option<Box<dyn NativeValue>>) -> Self {
        Self {
            name: name.into(),
            kind,
            default,
            is_public: false,
        }
    }

    fn encode(&self) -> GraphVariable {
        let mut result = GraphVariable {
            var_name: self.name.clone(),
            base_type: self.kind.get_server_id() as i32,
            storage_value: self.kind.encode_typed_value(Side::Server, self.default.as_deref()).into(),
            is_public: self.is_public,
            schema_ref_id: None,
            container_key_type: 0,
            container_value_type: 0,
        };
        if let Some(schema) = self.kind.encode_schema() {
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
impl MainNodeGraph {
    pub fn apply(self, bundle: &mut AssetBundle) -> Identifier {
        let id = bundle.alloc(identifier::Category::ServerNodeGraph, identifier::AssetKind::Basic);
        bundle.push(self.graph.apply(id).tap_mut(|data| {
            match data.payload.as_mut().unwrap() {
                Payload::GraphData(data) => data.inner.as_mut().unwrap().graph.as_mut().unwrap().blackboard = self.statics.iter().map(NodeGraphStatic::encode).collect(),
                _ => unreachable!(),
            }
        }));
        id
    }
}

pub fn encode_selected(kind: &NativeKind, value: Option<&dyn NativeValue>, selected: Option<i32>, side: Side) -> TypedValue {
    if let Some(selected) = selected {
        TypedValue {
            widget: typed_value::WidgetType::TypeSelector as i32,
            is_set: true,
            // 多态引脚不带顶层 `type`:类型全在 `val_poly.actual_value.type` 里
            //(官方导出的写法;带上会让游戏按错误的类型找结构体)。
            r#type: None,
            tracker: None,
            storage: typed_value::Storage::ValPoly(PolymorphicValue {
                chosen_type_index: selected,
                actual_value: Some(kind.encode_typed_value(side, value).into()),
                extra_meta: None,
            }.into()).into(),
        }
    } else {
        kind.encode_typed_value(side, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 拼装列表的 IR 只带实际元素引脚,编码 `.gia` 时补满节点定义的槽。
    #[test]
    fn 拼装列表编码时补满元素槽() {
        let mut g = NodeGraph::new(NodeGraphKind::ServerEntity, "test");
        let kind = crate::node::list::node_list_assemble(&mut g, NativeKind::Int, 2);
        // 元素个数引脚 + 2 个元素
        assert_eq!(kind.values_in_types.len(), 3);
        g.insert(kind);
        let data = g.apply(Identifier {
            source: identifier::Source::UserDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: identifier::AssetKind::CustomGraph as i32,
            guid: 1,
            runtime_id: 1,
        });
        let Payload::GraphData(container) = data.payload.unwrap() else { panic!() };
        let graph = container.inner.unwrap().graph.unwrap();
        // 元素个数引脚没设默认值、也没有选择器,不输出;其余 100 个元素槽补满,
        // 再加 1 个列表出参。
        assert_eq!(graph.node[0].pins.len(), MAX_ASSEMBLE_ELEMENTS + 1);
    }
}
