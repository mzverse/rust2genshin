use crate::compile::func::{FnDecl, NodeGraphIr, node_ir_native};
use crate::compile::helper::Helper;
use crate::compile::link::{CompiledFn, Linker, Target};
use crate::node::arithmetic::{NODE_NOT, node_cast, node_equal};
use crate::node::control::NODE_IF;
use crate::node::execution::node_set_local;
use crate::node::hidden::node_on_native_custom_value_change;
use crate::node::query::{NODE_RANDOM_FLOAT, NODE_RANDOM_INT, node_local};
use crate::node::{Link, LinkTarget, Lower, NativeNodeId, NodeGraph, NodeKind, NodeRef, PinType, ValueIn};
use crate::structure::{StructField, StructureDefinition, node_assemble_struct, node_destructure_struct, node_modify_struct};
use crate::unwrap;
use crate::value::NativeKind;
use either::Either;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{Display, Formatter};
use std::mem;
use std::mem::swap;
use panic_context::panic_context;
use tap::Tap;
use crate::asset::generated::identifier;

pub struct Optimizer {
    pub graph: NodeGraphIr,
    pub decl: FnDecl,
}

pub type NodeKindIr = NodeKind<IrNodeId, IrKind>;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum IrNodeId {
    Native(NativeNodeId),
    Unreachable,
    Local,
    SetLocal,
    Fn(String),
    Assemble,
    Destructure,
    Modify,
    BlackBox,
}
impl IrNodeId {
    pub fn as_native(&self) -> Option<&NativeNodeId> {
        match self {
            IrNodeId::Native(native) => native.into(),
            _ => None,
        }
    }
    pub fn into_native(self) -> Option<NativeNodeId> {
        match self {
            IrNodeId::Native(native) => native.into(),
            _ => None,
        }
    }
}

pub fn node_ir_black_box(kind: &IrKind) -> NodeKindIr {
    NodeKind::new(IrNodeId::BlackBox, 1, 1, vec![kind.clone().into()], vec![kind.clone()])
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum IrKind {
    Native(NativeKind),
    Never,
    LocalRef(Box<IrKind>),
    Adt(String),
    Mut(Box<IrKind>),
    PartialMut(String),
    Unsupported(String),
}
impl Display for IrKind {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            IrKind::Native(native) => write!(f, "{native}"),
            IrKind::Never => write!(f, "Never"),
            IrKind::LocalRef(kind) => write!(f, "Local<{kind}>"),
            IrKind::Adt(key) => write!(f, "{key}"),
            IrKind::Mut(kind) => write!(f, "Mut<{kind}>"),
            IrKind::PartialMut(kind) => write!(f, "PartialMut<{kind}>"),
            IrKind::Unsupported(info) => write!(f, "Unsupported<{info}>"),
        }
    }
}

impl IrKind {
    pub fn partial_mut(kind: IrKind) -> IrKind {
        if let Self::Adt(kind) = kind {
            IrKind::PartialMut(kind)
        } else {
            kind
        }
    }

    pub fn as_native(&self) -> Option<&NativeKind> {
        match self {
            IrKind::Native(native) => native.into(),
            _ => None,
        }
    }
    pub fn into_native(self) -> Option<NativeKind> {
        match self {
            IrKind::Native(native) => native.into(),
            _ => None,
        }
    }

    fn touch_adt(linker: &mut Linker, kind: String) -> NativeKind {
        if let Some(r) = linker.adts.get(&kind) {
            return r.to_kind();
        }
        let r = StructureDefinition {
            name: kind.clone(),
            version: 1,
            fields: linker.target.adts.get(&kind).unwrap().fields.clone().into_iter().map(|field| StructField {
                name: field.name,
                kind: field.kind.lower(linker),
                default: None,
            }).collect(),
        }.apply(&mut linker.assets);
        r.to_kind().tap(|_| _ = linker.adts.insert(kind, r))
    }

    pub fn lower(&self, linker: &mut Linker) -> NativeKind {
        match self {
            IrKind::Native(native) => native.clone(),
            | IrKind::Adt(kind)
            | IrKind::PartialMut(kind)
            => Self::touch_adt(linker, kind.clone()),
            IrKind::Mut(_) => panic!(),
            IrKind::Never => panic!(),
            IrKind::LocalRef(kind) => {
                if let Some(kind) = kind.as_native() && node_local(kind).is_some() {
                    NativeKind::LocalVarRef
                } else {
                    let key = self.to_string();
                    linker.target.adts.entry(key.clone()).or_insert_with(|| AdtInfo::new(vec![FieldInfo::new("value".into(), kind.as_ref().clone())]));
                    IrKind::Adt(key).lower(linker)
                }
            }
            IrKind::Unsupported(info) => panic!("Unsupported: {info}"),
        }
    }
}

pub fn node_ir_unreachable() -> NodeKindIr {
    NodeKind::new(IrNodeId::Unreachable, 1, 0, vec![], vec![])
}

pub fn node_ir_local(kind: &IrKind) -> NodeKindIr {
    NodeKind::new(IrNodeId::Local, 0, 0, vec![kind.clone().into()], vec![IrKind::LocalRef(kind.clone().into()), kind.clone()])
}

pub fn node_ir_set_local(kind: &IrKind) -> NodeKindIr {
    NodeKind::new(IrNodeId::SetLocal, 1, 1, vec![IrKind::LocalRef(kind.clone().into()).into(), kind.clone().into()], vec![])
}

#[derive(Clone)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct FieldInfo {
    pub name: String,
    pub kind: IrKind,
}
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AdtInfo {
    pub fields: Vec<FieldInfo>,
    pub variants: Vec<Vec<usize>>,
    pub discriminant: Option<usize>,
}
impl AdtInfo {
    pub fn new(fields: Vec<FieldInfo>) -> Self {
        Self {
            fields,
            variants: vec![],
            discriminant: None,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct FnInfo {
    pub description: String,
    pub graph: NodeGraphIr,
    pub decl: FnDecl,
    pub exported: bool,
}
impl FieldInfo {
    pub fn new(name: String, kind: IrKind) -> Self {
        Self { name, kind }
    }
    pub fn encode(&self, linker: &mut Linker) -> StructField {
        StructField {
            name: self.name.clone(),
            kind: self.kind.lower(linker),
            default: None,
        }
    }
}
pub fn struct_fields(target: &mut Target, kind: &IrKind) -> Vec<FieldInfo> {
    match kind {
        IrKind::Adt(intern) => target.adts.get(intern).unwrap().fields.clone(),
        IrKind::Mut(e) => vec![FieldInfo::new(0.to_string(), IrKind::Native(NativeKind::LocalVarRef)), FieldInfo::new(1.to_string(), IrKind::partial_mut(e.as_ref().clone()))],
        IrKind::PartialMut(e) => struct_fields(target, &IrKind::Adt(e.clone())).into_iter().map(|mut x| {
            x.kind = IrKind::partial_mut(x.kind);
            x
        }).collect(),
        _ => panic!("{kind:?}"),
    }
}

pub fn node_ir_assemble(target: &mut Target, kind: &IrKind) -> NodeKindIr {
    NodeKind::new(IrNodeId::Assemble, 0, 0, struct_fields(target, kind).into_iter().map(|FieldInfo { kind, .. }| Some(kind)).collect(), vec![kind.clone()])
}

pub fn node_ir_destructure(target: &mut Target, kind: &IrKind) -> NodeKindIr {
    NodeKind::new(IrNodeId::Destructure, 0, 0, vec![kind.clone().into()], struct_fields(target, kind).into_iter().map(|FieldInfo { kind, .. }| kind).collect())
}

pub fn node_ir_modify_struct(target: &mut Target, kind: &IrKind) -> NodeKindIr {
    let mut params = vec![kind.clone().into()];
    params.push(None);
    for x in struct_fields(target, kind) {
        params.push(x.kind.clone().into());
        params.push(IrKind::Native(NativeKind::Bool).into());
    }
    NodeKind::new(IrNodeId::Modify, 1, 1, params, vec![])
}

impl Optimizer {
    pub fn verify(&self, _helper: Helper) -> super::Result<()> {
        self.graph.verify();
        // TODO
        Ok(())
    }

    pub fn link_node(&mut self, linker: &mut Linker, x: NodeRef) {
        let IrNodeId::Fn(key) = &self.graph.get_node(x).kind.id else {
            return;
        };
        panic_context!("Linking calling {key}");
        let key = key.clone();
        let CompiledFn { id: _, node, decl } = linker.touch_fn(&key).clone();
        let low = self.graph.insert(node_ir_native(node));
        let high = self.graph.get_node(x).clone();

        let args = high.links.values_in.into_iter().enumerate()
            .flat_map(|(i, x)| decl.params[i].destructure_all(&mut linker.target, &mut self.graph, high.kind.values_in_types[i].as_ref().unwrap(), x)).collect::<Vec<_>>();
        for (i, &j) in decl.proxies_in.iter().enumerate() {
            self.graph.set_value_in(Link::node(low, i), args[j].clone());
        }
        let ret = decl.ret.assemble_all(&mut linker.target, &mut self.graph, &high.kind.values_out_types[0], &mut decl.proxies_out.iter().enumerate().map(
            |(i, j)| match j {
                Some(Either::Left(j)) => args[*j].clone(),
                Some(Either::Right(j)) => j.clone().map(ValueIn::value).unwrap_or_else(ValueIn::default),
                None => ValueIn::link(Link::node(low, i)),
            }));
        self.reset_values(high.links.values_out[0].clone(), ret);
        if decl.control {
            for &from in &high.links.controls_in[0] {
                self.graph.relink_controls(from, Link::node(x, 0), &[Link::node(low, 0)]);
            }
            for &to in &high.links.controls_out[0] {
                self.graph.link_control(Link::node(low, 0), to);
            }
        } else {
            self.relink_controls(x, 0, 0);
        }
        self.graph.remove(x);
    }

    pub fn lower(mut self, linker: &mut Linker) -> (NodeGraph, FnDecl) {
        // link
        for x in self.graph.get_nodes() {
            self.link_node(linker, x);
        }
        self.optimize();
        self.graph.verify();
        for x in self.graph.get_nodes() {
            let kind = &mut self.graph.get_node_mut(x).kind;
            *kind = node_ir_native(match kind.id {
                IrNodeId::Native(_) => continue,
                IrNodeId::Unreachable => {
                    self.graph.remove(x);
                    continue;
                }
                IrNodeId::Local => {
                    if let NativeKind::Struct(kind) = kind.values_out_types[0].lower(linker) {
                        let kind = kind.clone();
                        let n = node_on_native_custom_value_change(&mut self.graph, &kind.to_kind());
                        let n = self.graph.insert(node_ir_native(n));
                        let n_getter = self.graph.insert(node_ir_native(node_destructure_struct(&linker.assets, &kind)));
                        self.graph.link_value(Link::node(n, 3), Link::node(n_getter, 0));
                        let n0 = self.graph.get_node(x);
                        self.reset_values(n0.links.values_out[0].clone(), ValueIn::link(Link::node(n, 3)));
                        let n0 = self.graph.get_node(x);
                        self.reset_values(n0.links.values_out[1].clone(), ValueIn::link(Link::node(n_getter, 0)));
                        self.graph.remove(x);
                        continue;
                    } else {
                        node_local(&match &kind.values_out_types[1] {
                            IrKind::PartialMut(_) => panic!(),
                            other => other.lower(linker),
                        }).unwrap()
                    }
                },
                IrNodeId::SetLocal => {
                    if let NativeKind::Struct(kind) = kind.values_in_types[0].as_ref().unwrap().lower(linker) {
                        let kind = kind.clone();
                        let n = self.graph.insert(node_ir_native(node_modify_struct(&linker.assets, &kind)));
                        let n0 = self.graph.get_node(x);
                        for from in n0.links.controls_in[0].clone() {
                            self.graph.relink_controls(from, Link::node(x, 0), &[Link::node(n, 0)]);
                        }
                        let n0 = self.graph.get_node(x);
                        for to in n0.links.controls_out[0].clone() {
                            self.graph.link_control(Link::node(n, 0), to);
                        }
                        let n0 = self.graph.get_node(x).links.clone();
                        self.graph.set_value_in(Link::node(n, 0), n0.values_in[0].clone());
                        self.graph.set_value_in(Link::node(n, 2), n0.values_in[1].clone());
                        self.graph.set_value_in(Link::node(n, 3), ValueIn::value(true.into()));
                        self.graph.remove(x);
                        continue;
                    } else {
                        node_set_local(&kind.values_in_types[1].as_ref().unwrap().lower(linker))
                    }
                },
                IrNodeId::Assemble => {
                    let k = kind.values_out_types[0].lower(linker);
                    node_assemble_struct(&linker.assets, unwrap!(&k, NativeKind::Struct))
                },
                IrNodeId::Destructure => {
                    let k = kind.values_in_types[0].as_ref().unwrap().lower(linker);
                    node_destructure_struct(&linker.assets, unwrap!(&k, NativeKind::Struct))
                },
                IrNodeId::Modify => {
                    let k = kind.values_in_types[0].as_ref().unwrap().lower(linker);
                    node_modify_struct(&linker.assets, unwrap!(&k, NativeKind::Struct))
                },
                IrNodeId::Fn(_) => unreachable!(),
                IrNodeId::BlackBox => {
                    let n = self.graph.get_node(x);
                    self.reset_values(n.links.values_out[0].clone(), n.links.values_in[0].clone());
                    self.relink_controls(x, 0, 0);
                    self.graph.remove(x);
                    continue;
                }
            });
        }

        if self.graph.externals.controls_in.first() == Some(&vec![Link::export(0)])
            || self.graph.externals.controls_in.first() == Some(&vec![]) { // fn is unreachable
            self.decl.control = false;
            *self.graph.exports.get_mut(&PinType::InControl).unwrap() = vec![];
            *self.graph.exports.get_mut(&PinType::OutControl).unwrap() = vec![];
            self.graph.externals.controls_in = vec![];
            self.graph.externals.controls_out = vec![];
        }

        let active: Vec<_> = self.graph.externals.values_out.iter().map(|v| v.iter().any(|&l| l.target.is_node())).collect();
        let mut rm = HashMap::new();
        for (i, (j, _)) in active.iter().enumerate().filter(|(_, x)| **x).enumerate() {
            rm.insert(j, i);
        }
        for (_i, node) in self.graph.nodes.iter_mut() {
            for x in node.links.values_in.iter_mut() {
                if let Some(Link { target: LinkTarget::Export, index }) = &mut x.link {
                    *index = rm[index];
                }
            }
        }
        let mut i = 0;
        self.graph.exports.get_mut(&PinType::InValue).unwrap().retain(|_| active[i].tap(|_| i += 1));
        let mut i = 0;
        self.graph.externals.values_out.retain(|_| active[i].tap(|_| i += 1));
        let mut i = 0;
        self.decl.proxies_in.retain(|_| active[i].tap(|_| i += 1));

        let active: Vec<_> = self.graph.externals.values_in.iter().map(|v| matches!(v.link, Some(Link { target: LinkTarget::Node(..), .. }))).collect();
        let mut rm = HashMap::new();
        for (i, (j, _)) in active.iter().enumerate().filter(|(_, x)| **x).enumerate() {
            rm.insert(j, i);
        }
        for (_, node) in self.graph.nodes.iter_mut() {
            for x in node.links.values_out.iter_mut().flatten() {
                if let Link { target: LinkTarget::Export, index } = x {
                    *index = rm[index];
                }
            }
        }
        let mut i = 0;
        self.graph.exports.get_mut(&PinType::OutValue).unwrap().retain(|_| active[i].tap(|_| i += 1));
        let mut i = 0;
        self.graph.externals.values_in.retain(|_| active[i].tap(|_| i += 1));

        for x in &mut self.graph.externals.values_out {
            x.retain(|&l| l.target.is_node());
        }
        for x in &mut self.graph.externals.values_in {
            if let Some(link) = x.link.as_mut() && link.target.is_export() {
                x.link = None;
            }
        }

        // lower all kinds
        for x in self.graph.kinds_mut() {
            *x = IrKind::Native(x.lower(linker));
        }

        struct Lowerer;
        impl Lower for Lowerer {
            type NodeId0 = IrNodeId;
            type Kind0 = IrKind;
            type NodeId1 = NativeNodeId;
            type Kind1 = NativeKind;

            fn lower_node_id(&mut self, node_id: Self::NodeId0) -> Self::NodeId1 {
                node_id.into_native().unwrap()
            }

            fn lower_kind(&mut self, kind: Self::Kind0) -> Self::Kind1 {
                kind.into_native().unwrap()
            }
        }
        (Lowerer.lower_graph(self.graph), self.decl)
    }

    /// 完整优化:消除规则跑到不动点 + 控制边界收尾(清空空的控制导出、
    /// 压缩值导出的索引)。
    ///
    /// `pub` 给 viewer 的「一键优化」用 —— 它可以在同一张图上**反复点**。
    /// 下面那条分支会把 `controls_in` 清空,所以判空要用 `first()` 兜底:
    /// `[0]` 在第二次进来时越界(core 自己只在 `lower()` 里跑一次,碰不到)。
    pub fn optimize(&mut self) {
        while self.eliminate_solos() {
        }
    }

    pub fn eliminate_solos(&mut self) -> bool {
        let mut queue: VecDeque<NodeRef> = self.graph.get_nodes().into();
        let mut set: HashSet<NodeRef> = queue.iter().copied().collect();
        let mut result = false;
        while let Some(node) = queue.pop_front() {
            if !set.remove(&node) {
                unreachable!()
            }
            let neighbors = self.graph.get_node(node).links.get_neighbors();
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
        let n = self.graph.get_node(node);
        if n.kind.id.as_native() == Some(&NODE_IF.id) && n.links.values_in[0].link.is_none() {
            let tar = 1 - *n.links.values_in[0].default.as_ref().unwrap().downcast_ref::<bool>().unwrap() as usize;
            if n.links.controls_out[tar].first() == Some(&Link::node(node, 0)) {
                let from = n.links.controls_in[0].clone();
                let un = self.graph.insert(node_ir_unreachable());
                for from in from {
                    self.graph.relink_controls(from, Link::node(node, 0), &[Link::node(un, 0)]);
                }
                return None;
            }
            self.relink_controls(node, 0, tar);
            self.graph.remove(node);
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_set_local(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.id == IrNodeId::SetLocal {
            let Link { target: LinkTarget::Node(source), .. } = n.links.values_in[0].link.unwrap() else {
                return Some(());
            };
            let source = self.graph.get_node(source);
            if source.kind.id != IrNodeId::Local {
                return Some(());
            }
            if !source.links.values_out[1].is_empty() {
                return Some(());
            }
            self.relink_controls(node ,0, 0);
            self.graph.remove(node);
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_local(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.id == IrNodeId::Local && n.links.values_out[0].is_empty() {
            self.reset_values(n.links.values_out[1].clone(), n.links.values_in[0].clone());
            self.graph.remove(node);
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_calc(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if is_calc(&n.kind) && n.links.values_out.iter().all(|x| x.is_empty()) {
            self.graph.remove(node);
            None
        } else {
            Some(())
        }
    }

    pub fn eliminate_unnecessary_local_setter(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.id != IrNodeId::SetLocal {
            return Some(());
        }
        let Link { target: LinkTarget::Node(local), index: 0 } = n.links.values_in[0].link.unwrap() else {
            return Some(());
        };
        let n_local = self.graph.get_node(local);
        if n_local.kind.id != IrNodeId::Local {
            // TODO
            return Some(());
        }
        if n.links.values_in[1].link == Some(Link::node(local, 1)) {
            self.relink_controls(node, 0, 0);
            self.graph.remove(node);
            return None;
        }

        const DEFAULT_MAX: usize = 1;
        let max = if let IrKind::Native(native) = n.kind.values_in_types[1].as_ref().unwrap() && node_local(native).is_some() {
            DEFAULT_MAX
        } else {
            usize::MAX
        };

        let mut setters = HashSet::new();
        setters.extend(n_local.links.values_out[0].iter());
        let mut queue = VecDeque::new();
        queue.push_back(node);
        while let Some(now) = queue.pop_front() {
            for (x, i) in self.graph.get_node(now).links.values_in.iter().flat_map(|x| x.link).filter_map(|x| x.target.node().map(|y| (y, x.index))) {
                let n = self.graph.get_node(x);
                if is_random(&n.kind.id) {
                    return Some(());
                }
                if n.kind.id == IrNodeId::Local && i == 1 {
                    setters.extend(n.links.values_out[0].iter());
                } else if is_calc(&n.kind) {
                    queue.push_back(x);
                } else {
                    setters.insert(Link::node(x, i));
                }
            }
        }
        assert!(setters.remove(&Link::node(node, 0)));

        let ret = self.graph.externals.controls_in.iter().flatten().copied().collect::<Vec<_>>();
        let mut usages = vec![];
        'l1: for &x in &n_local.links.values_out[1] {
            let mut queue = VecDeque::new();
            queue.push_back(x);
            while let Some(now) = queue.pop_front() {
                let Some(now_node) = now.target.node() else {
                    queue.extend(ret.clone());
                    continue;
                };
                let l = &self.graph.get_node(now_node).links;
                if !l.controls_in.is_empty() && matches!(n.kind.values_in_types[1].as_ref().unwrap(), IrKind::Adt(..)) {
                    match self.graph.get_node(now_node).kind.id {
                        IrNodeId::Modify if now.index == 0 => return Some(()),
                        IrNodeId::Native(NativeNodeId { kind: identifier::AssetKind::GeneratedStub, .. }) => return Some(()), // TODO
                        _ => (),
                    }
                }
                if now_node == node {
                    continue;
                } else if setters.contains(&now) {
                    continue 'l1;
                }
                if let Some(l) = l.controls_in.first() {
                    for &x in l {
                        if x.target.is_export() {
                            continue 'l1;
                        }
                        queue.push_back(x);
                    }
                } else {
                    queue.extend(l.values_out.iter().flatten());
                }
            }
            usages.push(x);
        }
        if usages.len() > max {
            return Some(());
        }
        assert!(!usages.contains(&Link::node(node, 1)));
        let res = usages.is_empty().then_some(());
        self.reset_values(usages, n.links.values_in[1].clone());
        let n_local = self.graph.get_node(local);
        for &x in n_local.links.values_out[1].iter() {
            let mut queue = VecDeque::new();
            queue.push_back(x);
            while let Some(now) = queue.pop_front() {
                let Some(now_node) = now.target.node() else {
                    return res;
                };
                if now_node == node {
                    return res;
                } else if setters.contains(&now) {
                    if self.graph.get_node(now_node).kind.id == IrNodeId::SetLocal && now.index == 0 {
                        continue;
                    } else {
                        return res;
                    }
                }
                let l = &self.graph.get_node(now_node).links;
                if let Some(l) = l.controls_in.first() {
                    for &x in l {
                        if x.target.is_export() {
                            return res;
                        }
                        queue.push_back(x);
                    }
                } else {
                    queue.extend(l.values_out.iter().flatten());
                }
            }
        }
        self.relink_controls(node, 0, 0);
        self.graph.remove(node);
        None
    }

    pub fn eliminate_assemble(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.id != IrNodeId::Assemble {
            return Some(());
        }
        if n.links.values_in.is_empty() {
            self.graph.remove(node);
            return None;
        }
        let Some(Link { target: LinkTarget::Node(from), .. }) = n.links.values_in[0].link else {
            return Some(());
        };
        let from_node = self.graph.get_node(from);
        if from_node.kind.id != IrNodeId::Destructure || from_node.kind.values_in_types[0].as_ref() != Some(&n.kind.values_out_types[0]) {
            return Some(());
        }
        for (i, x) in n.links.values_in.iter().enumerate() {
            let node1 = from;
            if x.link != Some(Link::node(node1, i)) {
                return Some(());
            }
        }
        let from = from_node.links.values_in[0].clone();
        self.reset_values(n.links.values_out[0].clone(), from);
        self.graph.remove(node);
        None
    }

    pub fn eliminate_destructure(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.id != IrNodeId::Destructure {
            return Some(());
        }
        let Some(Link { target: LinkTarget::Node(from), .. }) = n.links.values_in[0].link else {
            return Some(());
        };
        let from = self.graph.get_node(from);
        if from.kind.id != IrNodeId::Assemble {
            return Some(());
        }
        let from = from.links.values_in.clone();
        for (i, x) in n.links.values_out.clone().into_iter().enumerate() {
            self.reset_values(x, from[i].clone());
        }
        self.graph.remove(node);
        None
    }

    // `eliminate_bool_eq_int` / `eliminate_not_not` 原本是私有的,和调用链里
    // 别的规则不一样。viewer 的「优化」面板要对**每条规则**单独点一次,
    // 统一成 pub(见 viewer/src/optimize.rs 的 RULES)。
    pub fn eliminate_bool_eq_int(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.id.as_native() != Some(&node_equal(NativeKind::Int).id) {
            return Some(());
        }
        let mut v1 = &n.links.values_in[0];
        let mut v2 = &n.links.values_in[1];
        if v1.link.is_none() {
            swap(&mut v1, &mut v2);
        }
        if v2.link.is_some() {
            return Some(());
        }
        let Some(Link { target: LinkTarget::Node(v1), index: _ }) = v1.link else {
            return Some(());
        };
        let n1 = self.graph.get_node(v1);
        if n1.kind.id.as_native() != Some(&node_cast(NativeKind::Bool, NativeKind::Int).unwrap().id) {
            return Some(());
        }
        let from = n1.links.values_in[0].clone();
        let to = n.links.values_out[0].clone();
        let v = match v2.default.as_ref().map(|x| *x.downcast_ref::<i32>().unwrap()).unwrap_or(0) {
            0 => {
                let node_not = self.graph.insert(node_ir_native(NODE_NOT.clone()));
                self.graph.set_value_in(Link::node(node_not, 0), from);
                ValueIn::link(Link::node(node_not, 0))
            },
            1 => from,
            _ => ValueIn::value(false.into()),
        };
        self.reset_values(to, v);
        None
    }

    pub fn eliminate_not_not(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.id.as_native() != Some(&NODE_NOT.id) {
            return Some(());
        }
        let Some(Link { target: LinkTarget::Node(from), index: _ }) = n.links.values_in[0].link else {
            return Some(());
        };
        let from = self.graph.get_node(from);
        if from.kind.id.as_native() != Some(&NODE_NOT.id) {
            return Some(());
        }
        self.reset_values(n.links.values_out[0].clone(), from.links.values_in[0].clone());
        None
    }

    pub fn relink_controls(&mut self, node: NodeRef, from: usize, to: usize) {
        let mut last = self.graph.get_node(node).links.clone();
        for x in mem::take(&mut last.controls_in[from]) {
            self.graph.relink_controls(x, Link::node(node, from), &last.controls_out[to]);
        }
    }

    pub fn reset_values(&mut self, tos: Vec<Link>, value: ValueIn) {
        for x in tos {
            self.graph.set_value_in(x, value.clone());
        }
    }
}

fn is_calc(kind: &NodeKindIr) -> bool {
    kind.controls_in_num == 0 && kind.controls_out_num == 0
}

fn is_random(id: &IrNodeId) -> bool {
    if let IrNodeId::Native(native) = *id {
        native == NODE_RANDOM_INT.id || native == NODE_RANDOM_FLOAT.id
    } else {
        false
    }
}
