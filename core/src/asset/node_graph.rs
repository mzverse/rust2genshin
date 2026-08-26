use super::raw_node_graph::{NodeGraphClass, NodeType, RawLink, RawNode, RawNodeGraph, RawPin};
use super::value::{AnyValue, Value};
use crate::asset::generated::pin_signature;
use crate::asset::node_graph::control::NodeBreak;
use anyhow::{Context, Result, anyhow, bail};
use downcast::{Any, downcast};
use std::collections::HashMap;
use std::mem;

pub mod arithmetic;
pub mod client;
pub mod control;
pub mod execution;
pub mod hidden;
pub mod query;
pub mod trigger;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeRef(i32);

impl From<NodeRef> for usize {
    fn from(value: NodeRef) -> usize {
        value.0 as usize
    }
}

#[derive(Clone, Copy)]
pub struct Link(NodeRef, i32);

type ControlOut = Vec<NodeRef>;
#[derive(Clone)]
pub struct ValueIn {
    has_default: bool,
    default: AnyValue,
    link: Option<Link>,
}
impl ValueIn {
    pub fn new(default: AnyValue) -> Self {
        Self {
            has_default: false,
            default,
            link: None,
        }
    }
    pub fn verify(&self, context: &Simulation) -> Result<()> {
        if let Some(link) = &self.link
            && let Some(target) = context
                .get_node(link.0)
                .get_values_out()
                .get(link.1 as usize)
            && (target.get_server_type() != self.default.get_server_type()
                || target.get_client_type() != self.default.get_client_type())
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
            Ok(self.default.clone())
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
    nodes: Vec<Box<dyn INode>>,
    current: Option<NodeRef>,
    logs: Vec<(LogType, String)>,
}
impl Simulation {
    pub fn get_node(&self, id: NodeRef) -> &dyn INode {
        self.nodes.get(usize::from(id)).unwrap().as_ref()
    }
    pub fn get_node_mut(&mut self, id: NodeRef) -> &mut dyn INode {
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
            impl INode for Using {
                fn get_controls_in(&self) -> i32 {
                    0
                }
                fn get_controls_out(&self) -> Vec<ControlOut> {
                    vec![]
                }
                fn get_values_in(&self) -> Vec<&ValueIn> {
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

pub trait INode: Any {
    fn get_controls_in(&self) -> i32;
    fn get_controls_out(&self) -> Vec<ControlOut>;

    fn get_values_in(&self) -> Vec<&ValueIn>;
    fn get_values_out(&self) -> Vec<Box<dyn Value>>;

    fn execute(&mut self, context: &mut Simulation) -> Result<Vec<NodeRef>>;
    fn get_value(&self, index: i32, context: &Simulation) -> Result<AnyValue>;

    #[allow(unused_variables)]
    fn verify(&self, context: &Simulation) -> Result<()> {
        Ok(())
    }

    fn get_type(&self) -> NodeType;
}
downcast!(dyn INode);

pub struct NodeGraph {
    class: NodeGraphClass,
    name: String,
    nodes: Vec<Box<dyn INode>>,
}
impl NodeGraph {
    pub fn encode(&self) -> RawNodeGraph {
        let mut pins: Vec<
            HashMap<(pin_signature::Kind, i32), (Option<(AnyValue, bool)>, Vec<Link>)>,
        > = vec![HashMap::new(); self.nodes.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            for (j, p) in n.get_values_out().iter().enumerate() {
                pins[i].insert(
                    (pin_signature::Kind::OutParam, j as i32),
                    (Some((p.clone(), false)), Vec::new()),
                );
            }
            for (j, p) in n.get_values_in().iter().enumerate() {
                pins[i].insert(
                    (pin_signature::Kind::InParam, j as i32),
                    (
                        Some((p.default.clone(), p.has_default)),
                        if let Some(l) = p.link {
                            vec![l]
                        } else {
                            vec![]
                        },
                    ),
                );
                if let Some(link) = p.link {
                    pins[link.0.0 as usize]
                        .get_mut(&(pin_signature::Kind::OutParam, link.1))
                        .unwrap()
                        .1
                        .push(Link(NodeRef(i as i32), j as i32));
                }
            }
            for j in 0..n.get_controls_in() {
                pins[i].insert((pin_signature::Kind::InFlow, j), (None, Vec::new()));
            }
            for (j, p) in n.get_controls_out().iter().enumerate() {
                let k = if n.is::<NodeBreak>() { 1 } else { 0 };
                pins[i].insert(
                    (pin_signature::Kind::OutFlow, j as i32),
                    (
                        None,
                        p.iter().map(|NodeRef(t)| Link(NodeRef(*t), k)).collect(),
                    ),
                );
                for NodeRef(t) in p {
                    pins[*t as usize]
                        .get_mut(&(pin_signature::Kind::InFlow, k))
                        .unwrap()
                        .1
                        .push(Link(NodeRef(i as i32), j as i32));
                }
            }
        }
        RawNodeGraph {
            class: self.class,
            name: self.name.clone(),
            nodes: pins
                .into_iter()
                .enumerate()
                .map(|(i, pins)| {
                    RawNode {
                        ty: self.nodes[i].get_type(),
                        pos: (0.0, 0.0), // TODO
                        pins: pins
                            .into_iter()
                            .map(|((kind, index), (value, links))| RawPin {
                                ty: kind,
                                index,
                                value,
                                links: links
                                    .iter()
                                    .map(|Link(node, i)| RawLink {
                                        node: node.0,
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
                    }
                })
                .collect(),
        }
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

        // Arithmetic.Math.Add (ID 200):泛型变体,兜底 Int
        let add = NodeAdd::default();
        assert_eq!(add.get_values_in().len(), 2);
        assert_eq!(add.get_values_out().len(), 1);
    }

    /// 生成节点能进入 NodeGraph 并编码为 RawNodeGraph
    #[test]
    fn generated_nodes_encode() {
        let mut sw = NodeSwitch::default();
        sw.add_case(vec![NodeRef(0)]);
        let mut g = NodeGraph {
            class: NodeGraphClass::Entity,
            name: "gen".to_string(),
            nodes: vec![Box::new(sw), Box::new(NodeVectorAdd::default())],
        };
        let raw = g.encode();
        assert_eq!(raw.nodes.len(), 2);
        // Switch:1 InFlow + 2 InParam + 1 Default + 1 Case = 5 pins;Vector_Add:2 InParam + 1 OutParam
        assert_eq!(raw.nodes[0].pins.len(), 5);
        assert_eq!(raw.nodes[1].pins.len(), 3);
    }
}
