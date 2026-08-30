use super::raw_node_graph::{GraphVariable, NodeGraphClass, NodeType, RawLink, RawNode, RawNodeGraph, RawPin};
use super::value::{AnyValue, Value, ValueSelected};
use crate::asset::generated::{AssetData, Identifier, InterfaceMapping, NodeInterface, NodeInterfaceContainer, PinInterface, PinSignature, asset_data, identifier, node_interface, node_interface_container, pin_interface, pin_signature};
use crate::asset::node_graph::control::NodeBreak;
use anyhow::{Context, Result, anyhow, bail};
use downcast::{Any, downcast};
use slab::Slab;
use std::collections::HashMap;
use std::mem;
use crate::asset::Asset;

pub mod arithmetic;
pub mod client;
pub mod control;
pub mod execution;
pub mod hidden;
pub mod query;
pub mod trigger;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct NodeRef(i32);

impl From<NodeRef> for usize {
    fn from(value: NodeRef) -> usize {
        value.0 as usize
    }
}
impl From<usize> for NodeRef {
    fn from(value: usize) -> NodeRef {
        NodeRef(value as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Link(NodeRef, i32);

impl Link {
    pub fn new(node: NodeRef, pin: i32) -> Self {
        Self(node, pin)
    }
    pub fn node(&self) -> NodeRef {
        self.0
    }
    pub fn pin(&self) -> i32 {
        self.1
    }
}

pub type ControlOut = Vec<NodeRef>;
#[derive(Clone)]
pub struct ValueIn {
    pub(crate) value: AnyValue,
    pub(crate) has_default: bool,
    pub(crate) link: Option<Link>,
}
impl ValueIn {
    pub fn new(default: AnyValue) -> Self {
        Self {
            has_default: false,
            value: default,
            link: None,
        }
    }
    pub fn into_selected(self, f: impl FnOnce(AnyValue) -> Result<i32>) -> Result<Self> {
        Ok(Self {
            has_default: true,
            value: self.value.into_selected(self.has_default, f)?.into(),
            link: self.link,
        })
    }
    /// 带来源构造:有连接则无默认值,无连接则默认值生效
    pub fn linked(default: AnyValue, link: Option<Link>) -> Self {
        Self {
            has_default: link.is_none(),
            value: default,
            link,
        }
    }
    pub fn verify(&self, context: &Simulation) -> Result<()> {
        if let Some(link) = &self.link
            && let Some(target) = context
                .get_node(link.0)
                .get_values_out()
                .get(link.1 as usize)
            && (target.get_server_type() != self.value.get_server_type()
                || target.get_client_type() != self.value.get_client_type())
        {
            return Err(anyhow!("type error"));
        }
        // TODO
        Ok(())
    }
    pub fn get(&self, context: &Simulation) -> Result<AnyValue> {
        if let Some(link) = self.link {
            return context.get_value(link);
        }
        if self.has_default {
            Ok(self.value.clone())
        } else {
            Err(anyhow!("No value"))
        }
    }
}

#[repr(u32)]
pub enum LogType {
    Event,
    Info,
    Error,
}
pub struct Simulation {
    nodes: Vec<Box<dyn Node>>,
    current: Option<NodeRef>,
    logs: Vec<(LogType, String)>,
}
impl Simulation {
    pub fn get_node(&self, id: NodeRef) -> &dyn Node {
        self.nodes.get(usize::from(id)).unwrap().as_ref()
    }
    pub fn get_node_mut(&mut self, id: NodeRef) -> &mut dyn Node {
        self.nodes.get_mut(usize::from(id)).unwrap().as_mut()
    }
    pub fn execute(&mut self, start: NodeRef) -> Result<()> {
        let mut stack = vec![start];
        while let Some(now) = stack.pop() {
            self.current = Some(now);
            let Some(i) = self.nodes.get_mut(usize::from(now)) else {
                bail!("Node not found");
            };
            struct Using;
            impl Node for Using {
                fn get_controls_in(&self) -> i32 {
                    0
                }
                fn get_controls_out(&self) -> Vec<ControlOut> {
                    vec![]
                }
                fn get_values_in(&self) -> Vec<ValueIn> {
                    vec![]
                }
                fn get_values_out(&self) -> Vec<Box<dyn Value>> {
                    vec![]
                }
                fn execute(&mut self, _context: &mut Simulation) -> Result<Vec<NodeRef>> {
                    Err(anyhow!("circular dependency"))
                }
                fn get_value(&self, _index: i32, _context: &Simulation) -> Result<Box<dyn Value>> {
                    Err(anyhow!("circular dependency"))
                }
                fn get_type(&self) -> NodeType {
                    panic!("circular dependency")
                }
            }
            let mut node = mem::replace(i, Box::new(Using));
            let result = node.execute(self);
            self.nodes[usize::from(now)] = node;
            for x in result?.iter().rev() {
                stack.push(*x);
            }
        }
        Ok(())
    }
    pub fn get_value(&self, input: Link) -> Result<Box<dyn Value>> {
        let Link(node, index) = input;
        self.nodes[usize::from(node)].get_value(index, self)
    }
    pub fn verify(&self) -> Result<()> {
        let mut vis: Vec<bool> = vec![false; self.nodes.len()];
        for now in 0..self.nodes.len() {
            if vis[now] {
                continue;
            }
            let mut state: Vec<bool> = vec![false; self.nodes.len()];
            let mut stack = vec![(now, true)];
            while let Some((now, flag)) = stack.pop() {
                if flag {
                    if state[now] {
                        return Err(anyhow!("Circular control flow: {:?}", now));
                    } else if vis[now] || self.get_node(NodeRef(now as i32)).is::<NodeBreak>() {
                        continue;
                    }
                    vis[now] = true;
                    state[now] = true;
                    stack.push((now, false));
                    for x in self
                        .get_node(NodeRef(now as i32))
                        .get_controls_out()
                        .iter()
                        .flat_map(|x| x.iter())
                    {
                        stack.push((usize::from(*x), true));
                    }
                    for x in self
                        .get_node(NodeRef(now as i32))
                        .get_values_in()
                        .iter()
                        .flat_map(|x| x.link)
                    {
                        stack.push((usize::from(x.0), true));
                    }
                } else {
                    state[now] = false;
                }
            }
        }
        for i in 0..self.nodes.len() {
            for j in self.nodes[i].get_values_in() {
                j.verify(self)?;
            }
            self.nodes[i].verify(self).context(anyhow!("Node: {}", i))?;
        }
        Ok(())
    }
}

pub trait Node: Any {
    fn get_controls_in(&self) -> i32;
    fn get_controls_out(&self) -> Vec<ControlOut>;

    fn get_values_in(&self) -> Vec<ValueIn>;
    fn get_values_out(&self) -> Vec<Box<dyn Value>>;

    fn execute(&mut self, context: &mut Simulation) -> Result<Vec<NodeRef>>;
    fn get_value(&self, index: i32, context: &Simulation) -> Result<AnyValue>;

    fn get_type(&self) -> NodeType;

    #[allow(unused_variables)]
    fn verify(&self, context: &Simulation) -> Result<()> {
        Ok(())
    }

    /// 节点引用的资产(如主图复合节点引用其复合声明);
    /// encode_basic 收集为图的 references。
    fn get_references(&self) -> Vec<Identifier> {
        vec![]
    }

    /// 可变访问流输出(用于滞后建立控制流连接);
    /// 普通节点不支持时 panic。
    fn get_controls_out_mut(&mut self) -> Vec<&mut ControlOut> {
        unimplemented!("delayed control-out wiring not supported by this node")
    }
}
downcast!(dyn Node);
impl<T: Node> From<T> for Box<dyn Node> {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

pub struct NodeGraphBasic {
    pub(crate) class: NodeGraphClass,
    pub(crate) name: String,
    pub(crate) nodes: Slab<Box<dyn Node>>,
}
impl NodeGraphBasic {
    fn new(class: NodeGraphClass, name: impl Into<String>) -> Self {
        Self {
            class,
            name: name.into(),
            nodes: Slab::new(),
        }
    }
    fn insert(&mut self, node: Box<dyn Node>) -> NodeRef {
        self.nodes.insert(node).into()
    }
    fn get_mut(&mut self, key: NodeRef) -> Option<&mut Box<dyn Node>> {
        self.nodes.get_mut(key.into())
    }
    fn remove(&mut self, key: NodeRef) -> Box<dyn Node> {
        self.nodes.remove(key.into())
    }

    fn encode(&self) -> RawNodeGraph {
        let mut references = Vec::new();
        for (_, n) in &self.nodes {
            references.extend(n.get_references());
        }
        let mut pins: HashMap<usize, HashMap<(pin_signature::Kind, i32), (Option<(AnyValue, bool)>, Vec<Link>)>> = HashMap::new();
        for (i, n) in self.nodes.iter() {
            pins.insert(i, HashMap::new());
            for (j, p) in n.get_values_out().iter().enumerate() {
                pins.get_mut(&i).unwrap().insert(
                    (pin_signature::Kind::OutParam, j as i32),
                    (Some((p.clone(), p.is::<ValueSelected>())), Vec::new()),
                );
            }
            for j in 0..n.get_controls_in() {
                pins.get_mut(&i).unwrap().insert((pin_signature::Kind::InFlow, j), (None, Vec::new()));
            }
        }
        for (i, n) in self.nodes.iter() {
            for (j, p) in n.get_values_in().iter().enumerate() {
                pins.get_mut(&i).unwrap().insert(
                    (pin_signature::Kind::InParam, j as i32),
                    (
                        Some((p.value.clone(), p.has_default)),
                        if let Some(l) = p.link {
                            vec![l]
                        } else {
                            vec![]
                        },
                    ),
                );
                if let Some(link) = p.link {
                    pins.get_mut(&(link.0.0 as usize)).unwrap()
                        .get_mut(&(pin_signature::Kind::OutParam, link.1))
                        .unwrap()
                        .1
                        .push(Link(NodeRef(i as i32), j as i32));
                }
            }
            for (j, p) in n.get_controls_out().iter().enumerate() {
                let k = if n.is::<NodeBreak>() { 1 } else { 0 };
                pins.get_mut(&i).unwrap().insert(
                    (pin_signature::Kind::OutFlow, j as i32),
                    (
                        None,
                        p.iter().map(|NodeRef(t)| Link(NodeRef(*t), k)).collect(),
                    ),
                );
                for NodeRef(t) in p {
                    pins.get_mut(&(*t as usize)).unwrap()
                        .get_mut(&(pin_signature::Kind::InFlow, k))
                        .unwrap()
                        .1
                        .push(Link(NodeRef(i as i32), j as i32));
                }
            }
        }
        RawNodeGraph {
            class: self.class,
            name: self.name.to_string(),
            nodes: pins
                .into_iter()
                .map(|(i, pins)| {
                    (i as i32, RawNode {
                        ty: self.nodes[i].get_type(),
                        pos: (0.0, 0.0), // TODO
                        pins: pins
                            .into_iter()
                            .map(|((kind, index), (value, links))| RawPin {
                                ty: kind,
                                index,
                                value,
                                uid: None,
                                links: links
                                    .iter()
                                    .map(|Link(node, i)| RawLink {
                                        // NodeRef 为节点下标(0 起),连接目标用图内 index(1 起)
                                        node: node.0 + 1,
                                        ty: match kind {
                                            pin_signature::Kind::InFlow => {
                                                pin_signature::Kind::OutFlow
                                            }
                                            pin_signature::Kind::OutFlow => {
                                                pin_signature::Kind::InFlow
                                            }
                                            pin_signature::Kind::InParam => {
                                                pin_signature::Kind::OutParam
                                            }
                                            pin_signature::Kind::OutParam => {
                                                pin_signature::Kind::InParam
                                            }
                                            it => panic!("Unsupported kind {:?}", it), // TODO
                                        },
                                        index: *i,
                                    })
                                    .collect(),
                            })
                            .collect(),
                    })
                })
                .collect(),
            blackboard: vec![],
            references,
            port_mapping: vec![],
            graph_kind: identifier::AssetKind::CustomGraph,
        }
    }
}

pub trait NodeGraph: Asset {
    fn get_basic_mut(&mut self) -> &mut NodeGraphBasic;
    fn insert(&mut self, node: Box<dyn Node>) -> NodeRef {
        self.get_basic_mut().insert(node)
    }
    fn remove(&mut self, key: NodeRef) -> Box<dyn Node> {
        self.get_basic_mut().remove(key)
    }
    fn connect_control(&mut self, from: Link, to: NodeRef) {
        if from.pin() >= self.get_basic_mut().get_mut(to).unwrap().get_controls_in() {
            panic!();
        }
        self.get_basic_mut().get_mut(from.node()).unwrap().get_controls_out_mut()[from.pin() as usize].push(to);
    }
}

pub struct NodeGraphMain {
    pub(crate) basic: NodeGraphBasic,
    /// 节点图变量(黑板变量,全局变量)
    pub(crate) blackboard: Vec<GraphVariable>,
}
impl NodeGraphMain {
    pub fn new(
        class: NodeGraphClass,
        name: impl Into<String>,
    ) -> Self {
        Self { basic: NodeGraphBasic::new(class, name) , blackboard: vec![] }
    }

    pub fn with_blackboard(mut self, blackboard: Vec<GraphVariable>) -> Self {
        self.blackboard = blackboard;
        self
    }
}
impl NodeGraph for NodeGraphMain {
    fn get_basic_mut(&mut self) -> &mut NodeGraphBasic {
        &mut self.basic
    }
}
impl Asset for NodeGraphMain {
    fn encode(&self, id: i64) -> Vec<AssetData> {
        let mut result = self.basic.encode();
        result.blackboard = self.blackboard.clone();
        result.encode(id)
    }
}

/// 复合内部图(与 NodeGraphMain 只有 port_mapping 之差,graph_kind 固定 COMPOSITE_GRAPH):
/// 节点与 NodeGraphMain 一样是 INode(NodeStub),字段对齐。
pub struct NodeGraphComposite {
    pub(crate) basic: NodeGraphBasic,
    pub(crate) description: String,
    /// 复合接口引脚 → 内部节点引脚的穿透映射
    pub(crate) pins: HashMap<pin_signature::Kind, Vec<(String, Vec<Link>)>>,
}
impl NodeGraph for NodeGraphComposite {
    fn get_basic_mut(&mut self) -> &mut NodeGraphBasic {
        &mut self.basic
    }
}
impl Asset for NodeGraphComposite {
    fn encode(&self, id: i64) -> Vec<AssetData> {
        let mut result = self.basic.encode();
        result.graph_kind = identifier::AssetKind::CompositeGraph;
        fn mapping(kind: pin_signature::Kind, id: i32, link: Link) -> InterfaceMapping {
            let sig = |idx: i32| PinSignature {
                kind: kind as i32,
                index: idx,
                source_ref: None,
            };
            InterfaceMapping {
                external_port: Some(sig(id)),
                // Link 的 NodeRef 是节点下标(0 起),穿透目标用图内 index(1 起)
                internal_target_node_handle: link.0.0 + 1,
                internal_port_shell: Some(sig(link.1)),
                internal_port_kernel: Some(sig(link.1)),
            }
        }
        for (&kind, p) in &self.pins {
            for (i, (_name, links)) in p.iter().enumerate() {
                for link in links {
                    result.port_mapping.push(mapping(kind, i as i32, *link));
                }
            }
        }
        let mut result = result.encode(id);
        result.push(self.encode_decl(id));
        result
    }
}
impl NodeGraphComposite {
    pub const DECL_OFFSET: i64 = 0x100000;

    pub fn new(class: NodeGraphClass, name: impl Into<String>) -> Self {
        let mut pins = HashMap::new();
        pins.insert(pin_signature::Kind::InFlow, vec![]);
        pins.insert(pin_signature::Kind::OutFlow, vec![]);
        pins.insert(pin_signature::Kind::InParam, vec![]);
        pins.insert(pin_signature::Kind::OutParam, vec![]);
        Self {
            basic: NodeGraphBasic::new(class, name),
            description: String::new(),
            pins,
        }
    }
    fn get_pin_type(&self, kind: pin_signature::Kind, link: Link) -> Option<AnyValue> {
        let node = &self.basic.nodes[link.0.0 as usize];
        match kind {
            pin_signature::Kind::InParam => Some(node.get_values_in()[link.1 as usize].value.clone()),
            pin_signature::Kind::OutParam => Some(node.get_values_out()[link.1 as usize].clone()),
            _ => None,
        }
    }
    /// 复合接口声明(替代独立的 CompositeNode):由本图(接口引脚)编码为资产。
    pub fn encode_decl(&self, id: i64) -> AssetData {
        let decl_id = Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: identifier::AssetKind::GeneratedStub as i32,
            guid: 0,
            runtime_id: id + Self::DECL_OFFSET,
        };
        let mut persistent_uid = 0;
        let mut encode_pin = |kind: pin_signature::Kind, index: i32, name: String, link: Link| {
            // 穿透锚点:接口内全局自增(参考导出 flow 引脚为 1、2…)
            let tar = self.get_pin_type(kind, link);
            (PinInterface {
                name: name.clone(),
                // 对齐参考导出:对外引脚 visibility=1
                visibility_mask: 1,
                sig: Some(PinSignature {
                    kind: kind as i32,
                    index,
                    source_ref: None,
                }),
                r#type: tar.map(|x| pin_interface::TypeInfo {
                    ui_class: Some(x.downcast_ref::<ValueSelected>().map(|it| &it.value).unwrap_or(&x).get_widget_type() as i32),
                    var_type_shell: Some(x.get_server_type() as i32),
                    var_type_kernel: Some(x.get_server_type() as i32),
                    placeholder: None,
                    display_state: None,
                    detail: x.encode_type_detail(), // TODO: enum ...
                }),
                meta_sig_type: None,
                persistent_pin_uid: persistent_uid,
            }, persistent_uid += 1).0
        };
        AssetData {
            id: Some(Identifier {
                source: 0,
                category: identifier::Category::NodeDecl as i32,
                kind: 0,
                guid: id + Self::DECL_OFFSET,
                runtime_id: 0,
            }),
            reference: vec![Identifier {
                source: 0,
                category: identifier::Category::ServerNodeGraph as i32,
                kind: 0,
                guid: id,
                runtime_id: 0,
            }],
            name: self.basic.name.clone(),
            r#type: asset_data::Type::CompositeNodeDecl as i32,
            payload: Some(asset_data::Payload::InterfaceData(NodeInterfaceContainer {
                inner: Some(node_interface_container::InnerWrapper {
                    interface: Some(NodeInterface {
                        id: Some(node_interface::Signature {
                            shell_ref: Some(decl_id.clone()),
                            kernel_ref: Some(decl_id),
                            graph_ref: Some(Identifier {
                                source: identifier::Source::UserDefined as i32,
                                category: identifier::Category::ServerBasic as i32,
                                kind: identifier::AssetKind::CompositeGraph as i32,
                                guid: 0,
                                runtime_id: id,
                            }),
                            signal_version: None,
                        }),
                        inflows: self.pins[&pin_signature::Kind::InFlow].iter().enumerate().map(|(i, (name, link))| encode_pin(pin_signature::Kind::InFlow, i as i32, name.clone(), link[0])).collect(),
                        outflows: self.pins[&pin_signature::Kind::OutFlow].iter().enumerate().map(|(i, (name, link))| encode_pin(pin_signature::Kind::OutFlow, i as i32, name.clone(), link[0])).collect(),
                        inputs: self.pins[&pin_signature::Kind::InParam].iter().enumerate().map(|(i, (name, link))| encode_pin(pin_signature::Kind::InParam, i as i32, name.clone(), link[0])).collect(),
                        outputs: self.pins[&pin_signature::Kind::OutParam].iter().enumerate().map(|(i, (name, link))| encode_pin(pin_signature::Kind::OutParam, i as i32, name.clone(), link[0])).collect(),
                        meta_pins: vec![], // TODO
                        r#impl: Some(node_interface::Implementation {
                            category: node_interface::implementation::Category::Composite as i32,
                            template: None,
                        }),
                        name: self.basic.name.clone(),
                        description: self.description.clone(),
                        template_root: node_interface::TemplateRoot::UserComposite as i32,
                        template_sub: node_interface::TemplateSub::None as i32,
                    }),
                }),
            })),
        }
    }
}

/// 编译路径的节点:以 NodeType + 完整引脚定义构造,封装为 INode。
/// 引脚类型通过挂载的占位值表达;连线单向写在引脚 links(目标侧),
/// encode_basic 推导源侧的反向连接;复合节点声明对自身资产的引用。
pub struct NodeComposite {
    pub(crate) id: i64,
    pub(crate) controls_in: i32,
    pub(crate) controls_out: Vec<ControlOut>,
    pub(crate) values_in: Vec<ValueIn>,
    pub(crate) values_out: Vec<AnyValue>,
}
impl Node for NodeComposite {
    fn get_controls_in(&self) -> i32 {
        self.controls_in
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        self.controls_out.clone()
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        self.values_in.clone()
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        self.values_out.clone()
    }
    fn execute(&mut self, _c: &mut Simulation) -> Result<Vec<NodeRef>> {
        todo!("compiled node is not executable")
    }
    fn get_value(&self, _i: i32, _c: &Simulation) -> Result<AnyValue> {
        todo!("compiled node is not executable")
    }
    fn get_type(&self) -> NodeType {
        let id = Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: identifier::AssetKind::GeneratedStub as i32,
            guid: 0,
            runtime_id: self.id + NodeGraphComposite::DECL_OFFSET,
        };
        NodeType {
            shell: id.clone(),
            kernel: id,
        }
    }
    fn get_references(&self) -> Vec<Identifier> {
        vec![Identifier {
            source: 0,
            category: identifier::Category::NodeDecl as i32,
            kind: 0,
            guid: self.id + NodeGraphComposite::DECL_OFFSET,
            runtime_id: 0,
        }]
    }
    fn get_controls_out_mut(&mut self) -> Vec<&mut ControlOut> {
        self.controls_out.iter_mut().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::node_graph::arithmetic::{NodeAdd, NodeVectorAdd};
    use crate::asset::node_graph::control::NodeSwitch;
    use crate::asset::raw_node_graph::NodeGraphClass;

    /// 生成节点的引脚签名与 GIA 文档一致(样本:Switch / Vector_Add / Add)
    #[test]
    fn generated_nodes_pin_signatures() {
        // Control.General.Switch (ID 3):1 flow in,key+cases 值入;分支动态
        let mut sw = NodeSwitch::default();
        assert_eq!(sw.get_controls_in(), 1);
        assert_eq!(sw.get_controls_out().len(), 1); // 只有 default
        assert_eq!(sw.get_values_in().len(), 2);
        sw.add_case(vec![]);
        sw.add_case(vec![]);
        assert_eq!(sw.get_controls_out().len(), 3); // default + 2 case

        // Arithmetic.Math.Vector_Add (ID 10):纯值节点,a+b Vec → Vec
        let va = NodeVectorAdd::default();
        assert_eq!(va.get_controls_in(), 0);
        assert_eq!(va.get_controls_out().len(), 0);
        assert_eq!(va.get_values_in().len(), 2);
        assert_eq!(va.get_values_out().len(), 1);
        assert!(matches!(
            va.get_values_out()[0].get_server_type(),
            crate::asset::generated::ServerTypeId::SVector
        ));

        // Arithmetic.Math.Add (ID 200):泛型变体,需显式类型实例化
        let add = NodeAdd::new(crate::asset::value::ValueInt(0).into());
        assert_eq!(add.get_values_in().len(), 2);
        assert_eq!(add.get_values_out().len(), 1);
        assert_eq!(add.get_values_out()[0].get_server_type(), crate::asset::generated::ServerTypeId::SInt);
    }

    /// 生成节点能进入 NodeGraph 并编码为 RawNodeGraph
    #[test]
    fn generated_nodes_encode() {
        let mut sw = NodeSwitch::default();
        sw.add_case(vec![NodeRef(0)]);
        let mut g = NodeGraphMain::new(NodeGraphClass::Entity, "gen".to_string());
        g.basic.insert(Box::new(sw));
        g.basic.insert(Box::new(NodeVectorAdd::default()));
        let raw = g.basic.encode();
        assert_eq!(raw.nodes.len(), 2);
        // Switch:1 InFlow + 2 InParam + 1 Default + 1 Case = 5 pins;Vector_Add:2 InParam + 1 OutParam
        assert_eq!(raw.nodes[&0].pins.len(), 5);
        assert_eq!(raw.nodes[&1].pins.len(), 3);
    }
}
