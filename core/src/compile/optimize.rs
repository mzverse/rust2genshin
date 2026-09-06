use crate::asset::node_graph::control::NODE_IF;
use crate::asset::node_graph::execution::node_set_local;
use crate::asset::node_graph::query::node_local;
use crate::asset::node_graph::{Connection, Link, NodeGraph, NodeGraphExtra, NodeKind, NodeRef, ValueIn};
use crate::asset::value::{AnyValue, ValueBool};
use either::Either;
use std::collections::{HashMap, HashSet, VecDeque};


pub struct Optimizer<'a, E: NodeGraphExtra> {
    pub graph: &'a mut NodeGraph<E>,
    pub proxies: Vec<(usize, usize)>,
    pub provided: HashMap<usize, Either<AnyValue, usize>>,
}

impl<'a, E: NodeGraphExtra> Optimizer<'a, E> {
    #![allow(clippy::result_large_err)]

    pub fn new(graph: &'a mut NodeGraph<E>) -> Self {
        Self { graph, proxies: Default::default(), provided: Default::default() }
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
            let neighbors = self.graph.get_node(node).get_neighbors();
            if self.eliminate_solo(node).is_some() {
                continue;
            }
            for x in neighbors {
                if set.insert(x) {
                    queue.push_back(x);
                }
            }
        }
    }

    pub fn eliminate_solo(&mut self, node: NodeRef) -> Option<()> {
        self.eliminate_if(node)?;
        self.eliminate_set_local(node)?;
        self.eliminate_local(node)?;
        self.eliminate_calc(node)?;
        self.eliminate_unnecessary_local_setter(node)?;
        Some(())
    }

    pub fn eliminate_if(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind == *NODE_IF && n.values_in[0].link.is_none() {
            let n = self.graph.remove(node);
            let value = n.values_in[0].default.as_ref().unwrap().downcast_ref::<ValueBool>().unwrap().0;
            self.relink_controls(&n.controls_in[0], &n.controls_out[1 - value as usize]);
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_set_local(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.shell_eq(&node_set_local(ValueBool(false).into())) {
            let Link::Connection(source) = n.values_in[0].link.unwrap() else {
                return Some(());
            };
            let source = self.graph.get_node(source.node());
            if !source.values_out[1].is_empty() {
                return Some(());
            }
            let n = self.graph.remove(node);
            self.relink_controls(&n.controls_in[0], &n.controls_out[0]);
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_local(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.shell_eq(&node_local(ValueBool(false).into())) && n.values_out[0].is_empty() {
            let n = self.graph.remove(node);
            self.reset_values(&n.values_out[1], n.values_in[0].clone());
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_calc(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if is_calc(&n.kind) && n.values_out.iter().all(|x| x.is_empty()) {
            self.graph.remove(node);
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_unnecessary_local_setter(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if !n.kind.shell_eq(&node_set_local(ValueBool(false).into())) {
            return Some(());
        }
        let Link::Connection(Connection(local, _)) = n.values_in[0].link.unwrap() else {
            return Some(());
        };
        let n_local = self.graph.get_node(local);
        if n_local.values_out[0].len() != 1 || n_local.values_out[1].len() != 1 {
            return Some(());
        }
        let out = n_local.values_out[1][0];
        let mut queue = VecDeque::new();
        queue.push_back(out);
        let mut vis = HashSet::new();
        while let Some(link) = queue.pop_front() {
            let Link::Connection(Connection(next, _)) = link else {
                return Some(());
            };
            if !vis.insert(next) {
                return Some(());
            }
            let next = self.graph.get_node(next);
            if is_calc(&next.kind) {
                queue.extend(next.values_out.iter().flatten());
            } else {
                if !next.controls_in.iter().flatten().all(|x| matches!(*x, Link::Connection(Connection(x, _)) if x == node)) {
                    return Some(());
                }
            }
        }
        let n = self.graph.remove(node);
        self.relink_controls(&n.controls_in[0], &n.controls_out[0]);
        self.reset_value(out, n.values_in[1].clone());
        None
    }

    pub fn relink_controls(&mut self, from: &[Link], to: &[Link]) {
        for &f in from {
            for &t in to {
                self.relink_control(f, t);
            }
        }
    }

    pub fn reset_values(&mut self, to: &[Link], value: ValueIn) {
        for &t in to {
            self.reset_value(t, value.clone());
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

    pub fn reset_value(&mut self, to: Link, value: ValueIn) {
        match to {
            Link::Connection(to) => self.graph.set_value_in(to, value),
            Link::Export(to) => if let Some(from) = value.link {
                match from {
                    Link::Connection(from) =>
                        self.graph.export_value_out(from, to),
                    Link::Export(from) =>
                        _ = self.provided.insert(to, Either::Right(from)),
                }
            } else {
                self.provided.insert(to, Either::Left(value.default.unwrap()));
            }
        }
    }
}

fn is_calc(kind: &NodeKind) -> bool {
    kind.controls_in_num == 0 && kind.controls_out_num == 0
}
