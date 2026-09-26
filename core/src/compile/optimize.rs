use crate::asset::generated::structure_definition_data::var_def::value::Val;
use crate::asset::generated::typed_value::Storage;
use crate::asset::generated::{ClientTypeId, ServerTypeId};
use crate::asset::node_graph::control::NODE_IF;
use crate::asset::node_graph::execution::node_set_local;
use crate::asset::node_graph::hidden::node_on_native_custom_value_change;
use crate::asset::node_graph::query::node_local;
use crate::asset::node_graph::{CompositeNodeGraph, Connection, Link, NodeId, NodeKind, NodeRef, PinType, ValueIn};
use crate::asset::structure::{node_assemble_struct, node_destructure_struct, node_modify_struct, ValueStruct, StructureDefinition, StructField};
use crate::asset::value::{AnyValue, Value, ValueBool, ValueDefault, ValueInt, ValueLocalVarRef};
use crate::asset::{Asset, Side};
use crate::compile::Compiler;
use crate::compile::func::FnDecl;
use either::Either;
use rustc_middle::ty::{Ty, TyKind, TypingEnv};
use rustc_span::DUMMY_SP;
use std::collections::{HashMap, HashSet, VecDeque};
use std::mem::swap;
use tap::Tap;
use crate::asset::node_graph::arithmetic::{node_cast, node_equal, NODE_NOT};

pub struct Optimizer<'a> {
    pub graph: &'a mut CompositeNodeGraph,
    pub decl: &'a mut FnDecl,
}

#[derive(Clone, Debug, Default)]
pub struct ValueIrNever;
impl Value for ValueIrNever {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::ServerUnknown
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, _side: Side) -> Option<Storage> {
        panic!()
    }
    fn encode_field_value(&self) -> Val {
        panic!()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ValueIrAdt(pub String);
impl Value for ValueIrAdt {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::ServerUnknown
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
    }
    fn encode_storage(&self, _side: Side) -> Option<Storage> {
        panic!()
    }
    fn encode_field_value(&self) -> Val {
        panic!()
    }
    fn is_instance(&self, value: &AnyValue) -> bool {
        if let Ok(value) = value.downcast_ref::<ValueIrAdt>() {
            self.0 == value.0
        } else {
            value.is::<ValueStruct>() || value.is::<ValueInt>() // lowered
        }
    }
}

#[derive(Clone, Debug)]
pub struct ValueIrMut(pub AnyValue);
impl Value for ValueIrMut {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::ServerUnknown
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::ClientUnknown
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

pub fn node_ir_unreachable() -> NodeKind {
    NodeKind::full(NodeId::Unreachable, 0, 1, 0, vec![], vec![])
}

fn ir_local_ref<'tcx>(compiler: &mut Compiler<'tcx>, kind: &AnyValue, ty: Ty<'tcx>) -> AnyValue {
    if node_local(kind).is_some() {
        ValueLocalVarRef::def()
    } else {
        compiler.compile_ty(DUMMY_SP, Ty::new_tup(compiler.tcx, &[ty])).unwrap()
    }
}

pub fn node_ir_local<'tcx>(compiler: &mut Compiler<'tcx>, kind: &AnyValue, ty: Ty<'tcx>) -> NodeKind {
    NodeKind::full(NodeId::Local, 0, 0, 0, vec![kind.clone().into()], vec![ir_local_ref(compiler, kind, ty), kind.clone()])
}

pub fn node_ir_set_local<'tcx>(compiler: &mut Compiler<'tcx>, kind: &AnyValue, ty: Ty<'tcx>) -> NodeKind {
    NodeKind::full(NodeId::SetLocal, 0, 1, 1, vec![ir_local_ref(compiler, kind, ty).into(), kind.clone().into()], vec![])
}

pub fn struct_fields<'tcx>(compiler: &mut Compiler<'tcx>, kind: &AnyValue) -> Vec<StructField> {
    if let Ok(kind) = kind.downcast_ref::<ValueIrMut>() {
        vec![StructField::new(0.to_string(), ValueLocalVarRef::def()), StructField::new(1.to_string(), kind.0.clone())]
    } else if let Ok(kind) = kind.downcast_ref::<ValueIrAdt>() {
        match compiler.interned_adts.get(&kind.0).unwrap().kind() {
            TyKind::Adt(d, a) => {
                if !d.is_struct() {
                    todo!();
                }
                return d.non_enum_variant().fields.iter().map(|x| StructField::new(x.name.to_string(), compiler.compile_ty(DUMMY_SP, compiler.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), x.ty(compiler.tcx, a))).unwrap())).collect();
            }
            TyKind::Closure(_d, a) => a.as_closure().upvar_tys(),
            TyKind::Tuple(es) => es,
            other => panic!("{other:?}"),
        }.iter().enumerate().map(|(i, x)| StructField::new(i.to_string(), compiler.compile_ty(DUMMY_SP, x).unwrap())).collect()
    } else {
        panic!("{kind:?}");
    }
}

pub fn node_ir_assemble<'tcx>(compiler: &mut Compiler<'tcx>, kind: &AnyValue) -> NodeKind {
    NodeKind::full(NodeId::Assemble, 0, 0, 0, struct_fields(compiler, kind).into_iter().map(|StructField { value, .. }| Some(value)).collect(), vec![kind.clone()])
}

pub fn node_ir_destructure<'tcx>(compiler: &mut Compiler<'tcx>, kind: &AnyValue) -> NodeKind {
    NodeKind::full(NodeId::Destructure, 0, 0, 0, vec![kind.clone().into()], struct_fields(compiler, kind).into_iter().map(|StructField { value, .. }| value).collect())
}

impl<'a> Optimizer<'a> {
    #![allow(clippy::result_large_err)]

    pub fn touch_adt(compiler: &mut Compiler, kind: &AnyValue) -> ValueStruct {
        let name = kind.downcast_ref::<ValueIrAdt>().unwrap().0.to_string();
        if let Some(r) = compiler.compiled_adts.get(&name) {
            return ValueStruct::new(r.clone());
        }
        let id = StructureDefinition {
            name: name.clone(),
            version: 1,
            fields: struct_fields(compiler, kind),
        }.apply(&mut compiler.assets);
        compiler.compiled_adts.insert(name, id.clone());
        ValueStruct::new(id)
    }

    pub fn lower(&mut self, compiler: &mut Compiler) {
        self.optimize();
        for x in self.graph.graph.nodes.iter_mut().map(|(_, x)| &mut x.kind).flat_map(|x|
            x.values_in_types.iter_mut().flatten().chain(x.values_out_types.iter_mut())
        ) {
            if x.downcast_ref::<ValueIrAdt>().is_ok() {
                *x = Self::touch_adt(compiler, x).into();
            }
        }
        for x in self.graph.graph.get_nodes() {
            let kind = &mut self.graph.graph.get_node_mut(x).kind;
            *kind = match kind.id {
                NodeId::Native { .. } => continue,
                NodeId::Unreachable => {
                    self.graph.graph.remove(x);
                    continue;
                }
                NodeId::Local => {
                    if let Ok(kind) = kind.values_out_types[0].downcast_ref::<ValueStruct>() {
                        let kind = kind.clone();
                        let n = node_on_native_custom_value_change(&mut self.graph.graph, &kind.clone().into());
                        let n = self.graph.graph.insert(n.into());
                        let n_getter = self.graph.graph.insert(node_destructure_struct(&kind.st).into());
                        self.graph.graph.connect_value(Connection(n, 3), Connection(n_getter, 0));
                        let n0 = self.graph.graph.remove(x);
                        self.reset_values(&n0.values_out[0], ValueIn::link(Connection(n, 3).into()));
                        self.reset_values(&n0.values_out[1], ValueIn::link(Connection(n_getter, 0).into()));
                        continue;
                    } else {
                        node_local(&kind.values_out_types[1]).unwrap()
                    }
                },
                NodeId::SetLocal => {
                    if let Ok(kind) = kind.values_in_types[0].as_ref().unwrap().downcast_ref::<ValueStruct>() {
                        let kind = kind.clone();
                        let n = self.graph.graph.insert(node_modify_struct(&kind.st).into());
                        let n0 = self.graph.graph.remove(x);
                        self.relink_controls(&n0.controls_in[0], &[Connection(n, 0).into()]);
                        self.relink_controls(&[Connection(n, 0).into()], &n0.controls_out[0]);
                        self.graph.graph.set_value_in(Connection(n, 0), n0.values_in[0].clone());
                        self.graph.graph.set_value_in(Connection(n, 2), n0.values_in[1].clone());
                        self.graph.graph.set_value_in(Connection(n, 3), ValueIn::value(ValueBool(true).into()));
                        continue;
                    } else {
                        node_set_local(kind.values_in_types[1].as_ref().unwrap())
                    }
                },
                NodeId::Assemble => node_assemble_struct(&kind.values_out_types[0].downcast_ref::<ValueStruct>().expect("low").st),
                NodeId::Destructure => node_destructure_struct(&kind.values_in_types[0].as_ref().unwrap().downcast_ref::<ValueStruct>().expect("low").st),
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
        self.eliminate_bool_eq_int(node)?;
        self.eliminate_not_not(node)?;
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
        if n.values_in[1].link == Some(Connection(local, 1).into()) {
            let n = self.graph.graph.remove(node);
            self.relink_controls(&n.controls_in[0], &n.controls_out[0]);
            return None;
        }
        let n_local = self.graph.graph.get_node(local);

        const DEFAULT_MAX: usize = 1;
        let max = if node_local(n.kind.values_in_types[1].as_ref().unwrap()).is_some() {
            DEFAULT_MAX
        } else {
            usize::MAX
        };

        let mut setters = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(node);
        while let Some(now) = queue.pop_front() {
            for Connection(x, _) in self.graph.graph.get_node(now).values_in.iter().flat_map(|x| x.link).flat_map(|x| x.connection()) {
                let n = self.graph.graph.get_node(x);
                if n.kind.id == NodeId::Local {
                    setters.extend(n.values_out[0].iter().flat_map(|x| x.connection()).map(|x| x.0));
                } else if is_calc(&n.kind) {
                    queue.push_back(x);
                } else {
                    setters.insert(x);
                }
            }
        }
        setters.remove(&node);

        let ret = self.graph.graph.nodes.iter().filter(|(_i, x)| x.controls_out.iter().flatten().find(|x| matches!(x, Link::Export(..))).is_some()).map(|(i, _)| NodeRef::from(i)).collect::<Vec<_>>();
        let usages = n_local.values_out[1].iter().copied().filter(|&x| {
            let mut queue = VecDeque::new();
            match x {
                Link::Connection(Connection(x, _)) => {
                    assert_ne!(x, node);
                    queue.push_back(x)
                },
                Link::Export(_) => queue.extend(ret.clone()),
            }
            while let Some(now) = queue.pop_front() {
                if now == node {
                    continue;
                } else if setters.contains(&now) {
                    return false;
                }
                for &x in self.graph.graph.get_node(now).controls_in.iter().flatten() {
                    let Link::Connection(Connection(x, _)) = x else {
                        return false;
                    };
                    queue.push_back(x);
                }
            }
            true
        }).collect::<Vec<_>>();
        if usages.len() > max {
            return Some(());
        }
        assert!(!usages.contains(&Connection(node, 1).into()));
        let res = usages.is_empty().then_some(());
        self.reset_values(&usages, n.values_in[1].clone()); // maybe update?
        let n_local = self.graph.graph.get_node(local);
        for &x in n_local.values_out[1].iter() {
            let Link::Connection(Connection(x, _)) = x else {
                return res;
            };
            let mut queue = VecDeque::new();
            queue.push_back(x);
            while let Some(now) = queue.pop_front() {
                if now == node {
                    return res;
                } else if setters.contains(&now) {
                    continue;
                }
                for &x in self.graph.graph.get_node(now).controls_in.iter().flatten() {
                    let Link::Connection(Connection(x, _)) = x else {
                        return res;
                    };
                    queue.push_back(x);
                }
            }
        }
        let n = self.graph.graph.remove(node);
        self.relink_controls(&n.controls_in[0], &n.controls_out[0]);
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

    fn eliminate_bool_eq_int(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.graph.get_node(node);
        if n.kind != node_equal(ValueInt::def()) {
            return Some(());
        }
        let mut v1 = &n.values_in[0];
        let mut v2 = &n.values_in[1];
        if v1.link.is_none() {
            swap(&mut v1, &mut v2);
        }
        if v2.link.is_some() {
            return Some(());
        }
        let Some(Link::Connection(Connection(v1, _))) = v1.link else {
            return Some(());
        };
        let n1 = self.graph.graph.get_node(v1);
        if n1.kind != node_cast(ValueBool::def(), ValueInt::def()).unwrap() {
            return Some(());
        }
        let from = n1.values_in[0].clone();
        let to = n.values_out[0].clone();
        let v = match v2.default.as_ref().map(|x| x.downcast_ref::<ValueInt>().unwrap().0).unwrap_or(0) {
            0 => {
                let node_not = self.graph.graph.insert(NODE_NOT.clone().into());
                self.graph.graph.set_value_in(Connection(node_not, 0), from);
                ValueIn::link(Connection(node_not, 0).into())
            },
            1 => from,
            _ => ValueIn::value(ValueBool(false).into()),
        };
        self.reset_values(&to, v);
        None
    }

    fn eliminate_not_not(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.graph.get_node(node);
        if n.kind != *NODE_NOT {
            return Some(());
        }
        let Some(Link::Connection(Connection(from, _))) = n.values_in[0].link else {
            return Some(());
        };
        let from = self.graph.graph.get_node(from);
        if from.kind != *NODE_NOT {
            return Some(());
        }
        self.reset_values(&n.values_out[0].clone(), from.values_in[0].clone());
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

    // FIXME: orders
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
                    Link::Export(from) => {
                        // TODO: optimize
                        for x in self.graph.graph.nodes.iter_mut().flat_map(|(_, x)| x.values_out.iter_mut()) {
                            x.retain(|x| *x != Link::Export(to));
                        }
                        self.decl.proxies_out[to] = Some(Either::Left(from))
                    },
                }
            } else {
                // TODO: optimize
                for x in self.graph.graph.nodes.iter_mut().flat_map(|(_, x)| x.values_out.iter_mut()) {
                    x.retain(|x| *x != Link::Export(to));
                }
                self.decl.proxies_out[to] = Some(Either::Right(value.default));
            }
        }
    }
}

fn is_calc(kind: &NodeKind) -> bool {
    kind.controls_in_num == 0 && kind.controls_out_num == 0
}
