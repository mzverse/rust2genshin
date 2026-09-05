use crate::asset::node_graph::{Link, Node, NodeGraph, NodeGraphExtra, NodeRef};
use std::collections::{HashSet, VecDeque};
use crate::asset::node_graph::control::NODE_IF;
use crate::asset::value::ValueBool;

pub struct Optimizer<'a, E: NodeGraphExtra> {
    pub graph: &'a mut NodeGraph<E>,
    pub proxies: Vec<(usize, usize)>,
}

impl<'a, E: NodeGraphExtra> Optimizer<'a, E> {
    pub fn new(graph: &'a mut NodeGraph<E>) -> Self {
        Self { graph, proxies: Default::default() }
    }

    pub fn optimize(&mut self) {
        self.eliminate_solos();
    }

    pub fn eliminate_solos(&mut self) {
        let mut queue: VecDeque<NodeRef> = self.graph.get_nodes().into();
        let mut set: HashSet<NodeRef> = queue.iter().copied().collect();
        while let Some(node) = queue.pop_front() {
            if !set.remove(&node) {
                unreachable!()
            }
            let Some(node) = self.eliminate_solo(node) else {
                continue;
            };
            for x in node.get_neighbors() {
                if set.insert(x) {
                    queue.push_back(x);
                }
            }
        }
    }

    pub fn eliminate_solo(&mut self, node: NodeRef) -> Option<Node> {
        self.eliminate_dead_if(node)?;
        None
    }

    pub fn eliminate_dead_if(&mut self, node: NodeRef) -> Option<Node> {
        let n = self.graph.get_node(node);
        if n.kind == *NODE_IF && n.values_in[0].link.is_none() {
            let n = self.graph.remove(node);
            let value = n.values_in[0].default.as_ref().unwrap().downcast_ref::<ValueBool>().unwrap().0;
            self.relink_controls(&n.controls_in[0], &n.controls_out[1 - value as usize]);
            Some(n)
        } else {
            None
        }
    }

    pub fn relink_controls(&mut self, from: &[Link], to: &[Link]) {
        for &f in from {
            for &t in to {
                self.relink_control(f, t);
            }
        }
    }

    pub fn relink_values(&mut self, from: Link, to: &[Link]) {
        for &t in to {
            self.relink_value(from, t);
        }
    }

    pub fn relink_control(&mut self, from: Link, to: Link) {
        match from {
            Link::Connection(from) => match to {
                Link::Connection(to) => self.graph.connect_control(from, to),
                Link::Export(to) => self.graph.export_control_out(from, to),
            },
            Link::Export(from) => match to {
                Link::Connection(to) => self.graph.export_control_in(to, from),
                Link::Export(to) => self.proxies.push((from, to)),
            }
        }
    }

    pub fn relink_value(&mut self, _from: Link, _to: Link) {
        todo!()
    }
}
