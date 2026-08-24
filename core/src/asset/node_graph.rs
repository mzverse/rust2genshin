use downcast::{downcast, Any};
use std::mem;
use crate::asset::node_graph::control::NodeBreak;
use anyhow::{anyhow, bail, Context, Result};

mod control;
mod execution;

#[derive(Clone)]
enum Value {
    Bool(bool),
    Int(i32),
    String(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct NodeRef(u32);

impl From<NodeRef> for usize {
    fn from(value: NodeRef) -> usize {
        value.0 as usize
    }
}

#[derive(Clone, Copy)]
pub struct Link(NodeRef, u32);

type ControlOut = Vec<NodeRef>;
#[derive(Clone)]
struct ValueIn {
    has_default: bool,
    default: Value,
    link: Option<Link>,
}
impl ValueIn {
    pub fn new(default: Value) -> Self {
        Self {
            has_default: false,
            default,
            link: None,
        }
    }
    pub fn verify<>(&self, context: &Simulation) -> Result<()> {
        if let Some(link) = &self.link {
            if let Some(target) = context.get_node(link.0).get_values_out().get(link.1 as usize) {
                if mem::discriminant(target) != mem::discriminant(&self.default) {
                    return Err(anyhow!("type error"));
                }
            }
        }
        Ok(())
    }
    pub fn get(&self, context: &Simulation) -> Result<Value> {
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
enum LogType {
    Event,
    Info,
    Error,
}
struct Simulation {
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
                fn get_controls_in(&self) -> u32 {
                    0
                }
                fn get_controls_out(&self) -> Vec<ControlOut> {
                    vec![]
                }
                fn get_values_in(&self) -> Vec<&ValueIn> {
                    vec![]
                }
                fn get_values_out(&self) -> Vec<Value> {
                    vec![]
                }
                fn execute(&mut self, _context: &mut Simulation) -> Result<Vec<NodeRef>> {
                    Err(anyhow!("circular dependency"))
                }
                fn get_value(&self, _index: u32, _context: &Simulation) -> Result<Value> {
                    Err(anyhow!("circular dependency"))
                }
            }
            let mut node = mem::replace(i, Box::new(Using));
            for x in node.execute(self)?.iter().rev() {
                stack.push(*x);
            }
            self.nodes[usize::from(now)] = node;
        }
        Ok(())
    }
    pub fn get_value(&self, input: Link) -> Result<Value> {
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
                    } else if vis[now] || self.get_node(NodeRef(now as u32)).is::<NodeBreak>() {
                        continue;
                    }
                    vis[now] = true;
                    state[now] = true;
                    stack.push((now, false));
                    for x in self.get_node(NodeRef(now as u32)).get_controls_out().iter().flat_map(|x| x.iter()) {
                        stack.push((usize::from(*x), true));
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

trait INode: Any {
    fn get_controls_in(&self) -> u32;
    fn get_controls_out(&self) -> Vec<ControlOut>;

    fn get_values_in(&self) -> Vec<&ValueIn>;
    fn get_values_out(&self) -> Vec<Value>;

    fn execute(&mut self, context: &mut Simulation) -> Result<Vec<NodeRef>>;
    fn get_value(&self, index: u32, context: &Simulation) -> Result<Value>;

    fn verify(&self, context: &Simulation) -> Result<()> {
        Ok(())
    }
}
downcast!(dyn INode);
