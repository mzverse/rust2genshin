use crate::asset::Side;
use crate::asset::generated::structure_definition_data::var_def::value::Val;
use crate::asset::generated::typed_value::Storage;
use crate::asset::generated::{ClientTypeId, ServerTypeId};
use crate::asset::node_graph::control::NODE_IF;
use crate::asset::node_graph::execution::node_set_local;
use crate::asset::node_graph::query::node_local;
use crate::asset::node_graph::{CompositeNodeGraph, Connection, Link, NodeId, NodeKind, NodeRef, PinType, ValueIn};
use crate::asset::structure::{ValueStruct, node_assemble_struct, node_destructure_struct};
use crate::asset::value::{AnyValue, Value, ValueBool, ValueDefault, ValueLocalVarRef};
use crate::compile::func::FnDecl;
use std::collections::{HashMap, HashSet, VecDeque};
use either::Either;
use tap::Tap;

pub struct Optimizer<'a> {
    pub graph: &'a mut CompositeNodeGraph,
    pub decl: &'a mut FnDecl,
}

#[derive(Clone, Debug, Default)]
pub struct ValueIrNever;
impl Value for ValueIrNever {
    fn get_server_type(&self) -> ServerTypeId {
        panic!()
    }
    fn get_client_type(&self) -> ClientTypeId {
        panic!()
    }
    fn encode_storage(&self, _side: Side) -> Option<Storage> {
        panic!()
    }
    fn encode_field_value(&self) -> Val {
        panic!()
    }
}

#[derive(Clone, Debug)]
pub struct ValueIrMut(pub AnyValue);
impl Value for ValueIrMut {
    fn get_server_type(&self) -> ServerTypeId {
        panic!()
    }
    fn get_client_type(&self) -> ClientTypeId {
        panic!()
    }
    fn encode_storage(&self, _side: Side) -> Option<Storage> {
        panic!()
    }
    fn encode_field_value(&self) -> Val {
        panic!()
    }
    fn is_instance(&self, value: &AnyValue) -> bool {
        if let Ok(value) = value.downcast_ref::<ValueIrMut>() {
            self.0.is_instance(&value.0)
        } else {
            false
        }
    }
}

pub fn node_ir_local(kind: &AnyValue) -> NodeKind {
    NodeKind::full(NodeId::Local, 0, 0, 0, vec![kind.clone().into()], vec![ValueLocalVarRef::def(), kind.clone()])
}

pub fn node_ir_set_local(kind: &AnyValue) -> NodeKind {
    NodeKind::full(NodeId::SetLocal, 0, 1, 1, vec![ValueLocalVarRef::def().into(), kind.clone().into()], vec![])
}

pub fn node_ir_assemble(kind: &AnyValue) -> NodeKind {
    if let Ok(kind) = kind.downcast_ref::<ValueIrMut>() {
        NodeKind::full(NodeId::Assemble, 0, 0, 0, vec![ValueLocalVarRef::def().into(), kind.0.clone().into()], vec![kind.clone().into()])
    } else {
        node_assemble_struct(&kind.downcast_ref::<ValueStruct>().unwrap().st).tap_mut(|x| x.id = NodeId::Assemble)
    }
}

pub fn node_ir_destructure(kind: &AnyValue) -> NodeKind {
    if let Ok(kind) = kind.downcast_ref::<ValueIrMut>() {
        NodeKind::full(NodeId::Destructure, 0, 0, 0, vec![Some(kind.clone().into())], vec![ValueLocalVarRef::def(), kind.0.clone()])
    } else {
        node_destructure_struct(&kind.downcast_ref::<ValueStruct>().unwrap().st).tap_mut(|x| x.id = NodeId::Destructure)
    }
}

impl<'a> Optimizer<'a> {
    #![allow(clippy::result_large_err)]

    pub fn lower(&mut self) {
        self.optimize();
        for (_, node) in self.graph.graph.nodes.iter_mut() {
            node.kind = match node.kind.id {
                NodeId::Low { .. } => continue,
                NodeId::Local => node_local(node.kind.values_in_types[0].as_ref().unwrap()),
                NodeId::SetLocal => node_set_local(node.kind.values_in_types[1].as_ref().unwrap()),
                NodeId::Assemble => node_assemble_struct(&node.kind.values_out_types[0].downcast_ref::<ValueStruct>().expect("low").st),
                NodeId::Destructure => node_destructure_struct(&node.kind.values_in_types[0].as_ref().unwrap().downcast_ref::<ValueStruct>().expect("low").st),
                NodeId::Modify => todo!(),
            };
        }
    }

    pub fn optimize(&mut self) {
        while self.eliminate_solos() {
        }

        let mut active = vec![false; self.decl.proxies_in.len()];
        for (_i, node) in self.graph.graph.nodes.iter() {
            for i in node.values_in.iter() {
                if let Some(Link::Export(i)) = i.link {
                    active[i] = true;
                }
            }
        }
        let mut rm = HashMap::new();
        for (i, (j, _)) in active.iter().enumerate().filter(|(_, x)| **x).enumerate() {
            rm.insert(j, i);
        }
        for (_i, node) in self.graph.graph.nodes.iter_mut() {
            for i in node.values_in.iter_mut() {
                if let Some(Link::Export(i)) = &mut i.link {
                    *i = rm[i];
                }
            }
        }
        let mut i = 0;
        self.graph.pins.get_mut(&PinType::InValue).unwrap().retain(|_| active[i].tap(|_| i += 1));
        let mut i = 0;
        self.decl.proxies_in.retain(|_| active[i].tap(|_| i += 1));

        let mut active = vec![false; self.decl.proxies_out.len()];
        for (_i, node) in self.graph.graph.nodes.iter() {
            for i in node.values_out.iter().flatten() {
                if let Link::Export(i) = i {
                    active[*i] = true;
                }
            }
        }
        let mut rm = HashMap::new();
        for (i, (j, _)) in active.iter().enumerate().filter(|(_, x)| **x).enumerate() {
            rm.insert(j, i);
        }
        for (_i, node) in self.graph.graph.nodes.iter_mut() {
            for i in node.values_out.iter_mut().flatten() {
                if let Link::Export(i) = i {
                    *i = rm[i];
                }
            }
        }
        let mut i = 0;
        self.graph.pins.get_mut(&PinType::OutValue).unwrap().retain(|_| active[i].tap(|_| i += 1));
    }

    pub fn eliminate_solos(&mut self) -> bool {
        let mut queue: VecDeque<NodeRef> = self.graph.graph.get_nodes().into();
        let mut set: HashSet<NodeRef> = queue.iter().copied().collect();
        let mut result = false;
        while let Some(node) = queue.pop_front() {
            if !set.remove(&node) {
                unreachable!()
            }
            let neighbors = self.graph.graph.get_node(node).get_neighbors();
            if self.eliminate_solo(node).is_some() {
                continue;
            }
            result = true;
            for x in neighbors {
                if set.insert(x) {
                    queue.push_back(x);
                }
            }
        }
        result
    }

    pub fn eliminate_solo(&mut self, node: NodeRef) -> Option<()> {
        self.eliminate_if(node)?;
        self.eliminate_set_local(node)?;
        self.eliminate_local(node)?;
        self.eliminate_calc(node)?;
        self.eliminate_unnecessary_local_setter(node)?;
        self.eliminate_assemble(node)?;
        self.eliminate_destructure(node)?;
        Some(())
    }

    pub fn eliminate_if(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.graph.get_node(node);
        if n.kind.id == NODE_IF.id && n.values_in[0].link.is_none() {
            let n = self.graph.graph.remove(node);
            let value = n.values_in[0].default.as_ref().unwrap().downcast_ref::<ValueBool>().unwrap().0;
            self.relink_controls(&n.controls_in[0], &n.controls_out[1 - value as usize]);
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_set_local(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.graph.get_node(node);
        if n.kind.id == NodeId::SetLocal {
            let Link::Connection(source) = n.values_in[0].link.unwrap() else {
                return Some(());
            };
            let source = self.graph.graph.get_node(source.node());
            if !source.values_out[1].is_empty() {
                return Some(());
            }
            let n = self.graph.graph.remove(node);
            self.relink_controls(&n.controls_in[0], &n.controls_out[0]);
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_local(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.graph.get_node(node);
        if n.kind.id == NodeId::Local && n.values_out[0].is_empty() {
            let n = self.graph.graph.remove(node);
            self.reset_values(&n.values_out[1], n.values_in[0].clone());
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_calc(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.graph.get_node(node);
        if is_calc(&n.kind) && n.values_out.iter().all(|x| x.is_empty()) {
            self.graph.graph.remove(node);
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_unnecessary_local_setter(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.graph.get_node(node);
        if n.kind.id != NodeId::SetLocal {
            return Some(());
        }
        let Link::Connection(Connection(local, _)) = n.values_in[0].link.unwrap() else {
            return Some(());
        };
        let n_local = self.graph.graph.get_node(local);
        if n_local.values_out[0].len() != 1 || n_local.values_out[1].len() != 1 {
            return Some(());
        }
        let out = n_local.values_out[1][0];
        let mut next = out;
        loop {
            let Link::Connection(Connection(next_node, _)) = next else {
                if let [Link::Export(_)] = n.controls_out[0].as_slice() {
                    break;
                } else {
                    return Some(());
                }
            };
            let next_node = self.graph.graph.get_node(next_node);
            if is_calc(&next_node.kind) {
                match next_node.values_out.iter().flatten().collect::<Vec<_>>().as_slice() {
                    [it] => next = **it,
                    [] => break,
                    _ => return Some(()),
                }
            } else if let [x] = next_node.controls_in.iter().flatten().collect::<Vec<_>>().as_slice() && matches!(*x, Link::Connection(Connection(x, _)) if *x == node) {
                break;
            } else {
                return Some(());
            }
        }
        let n = self.graph.graph.remove(node);
        self.relink_controls(&n.controls_in[0], &n.controls_out[0]);
        self.reset_value(Connection(local, 0).into(), n.values_in[1].clone());
        None
    }

    pub fn eliminate_assemble(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.graph.get_node(node);
        if n.kind.id != NodeId::Assemble {
            return Some(());
        }
        if n.values_in.is_empty() {
            return None;
        }
        let Some(Link::Connection(from)) = n.values_in[0].link else {
            return Some(());
        };
        let from_node = self.graph.graph.get_node(from.0);
        if from_node.kind.id != NodeId::Destructure || !from_node.kind.values_in_types[0].as_ref().unwrap().is_instance(&n.kind.values_out_types[0]) {
            return Some(());
        }
        for (i, x) in n.values_in.iter().enumerate() {
            if x.link != Some(Connection(from.0, i).into()) {
                return Some(());
            }
        }
        let from = from_node.values_in[0].clone();
        let n = self.graph.graph.remove(node);
        self.reset_values(&n.values_out[0], from);
        None
    }

    pub fn eliminate_destructure(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.graph.get_node(node);
        if n.kind.id != NodeId::Destructure {
            return Some(());
        }
        let Some(Link::Connection(from)) = n.values_in[0].link else {
            return Some(());
        };
        let from = self.graph.graph.get_node(from.0);
        if from.kind.id != NodeId::Assemble {
            return Some(());
        }
        let from = from.values_in.clone();
        let n = self.graph.graph.remove(node);
        for (i, x) in n.values_out.iter().enumerate() {
            self.reset_values(x, from[i].clone());
        }
        None
    }

    pub fn relink_controls(&mut self, from: &[Link], to: &[Link]) {
        for &f in from {
            for &t in to {
                self.relink_control(f, t);
            }
        }
    }

    pub fn reset_values(&mut self, to: &Vec<Link>, value: ValueIn) {
        for t in to {
            self.reset_value(*t, value.clone());
        }
    }

    pub fn relink_control(&mut self, from: Link, to: Link) {
        match from {
            Link::Connection(from) => match to {
                Link::Connection(to) => self.graph.graph.connect_control(from, to),
                Link::Export(to) => self.graph.graph.export_control_out(from, to),
            },
            Link::Export(from) => match to {
                Link::Connection(to) => self.graph.graph.export_control_in(to, from),
                Link::Export(_) => self.decl.control = false,
            }
        }
    }

    pub fn reset_value(&mut self, to: Link, value: ValueIn) {
        match to {
            Link::Connection(to) => self.graph.graph.set_value_in(to, value),
            Link::Export(to) => if let Some(from) = value.link {
                match from {
                    Link::Connection(from) =>
                        self.graph.graph.export_value_out(from, to),
                    Link::Export(from) =>
                        self.decl.proxies_out[to] = Some(Either::Left(from)),
                }
            } else {
                self.decl.proxies_out[to] = Some(Either::Right(value.default.unwrap()));
            }
        }
    }
}

fn is_calc(kind: &NodeKind) -> bool {
    kind.controls_in_num == 0 && kind.controls_out_num == 0
}
