use crate::asset::generated::identifier;
use crate::asset::generated::pin_signature::Kind as PinType;
use crate::compile::func::{FnDecl, node_ir_native};
use crate::compile::helper::Helper;
use crate::compile::ir;
use crate::compile::ir::{IrKind, IrNodeId, Optimizer};
use crate::compile::link::{CompiledFn, Linker};
use crate::node::arithmetic::{NODE_NOT, node_cast, node_equal};
use crate::node::control::{node_switch, NODE_IF};
use crate::node::execution::node_set_local;
use crate::node::hidden::node_on_native_custom_value_change;
use crate::node::list::{node_list_assemble, node_list_get_at_index, node_list_modify_index};
use crate::node::query::node_local;
use crate::node::{Link, LinkTarget, Lower, NativeNodeId, NodeGraph, NodeRef, ValueIn};
use crate::structure::{node_assemble_struct, node_destructure_struct, node_modify_struct};
use crate::value::NativeKind;
use either::Either;
use std::collections::{HashMap, HashSet, VecDeque};
use std::mem;
use std::mem::swap;
use tap::Tap;
use crate::unwrap;

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
        panic_context::panic_context!("Linking calling {key}");
        let key = key.clone();
        let CompiledFn { id: _, node, decl } = linker.touch_fn(&key).clone();
        let low = self.graph.insert(node_ir_native(node));
        let high = self.graph.get_node(x).clone();

        let args = high
            .links
            .values_in
            .into_iter()
            .enumerate()
            .flat_map(|(i, x)| {
                decl.params[i].destructure_all(
                    &linker.target,
                    &mut self.graph,
                    high.kind.values_in_types[i].as_ref().unwrap(),
                    x,
                )
            })
            .collect::<Vec<_>>();
        for (i, &j) in decl.proxies_in.iter().enumerate() {
            self.graph.set_value_in(Link::node(low, i), args[j].clone());
        }
        let ret = decl.ret.assemble_all(
            &linker.target,
            &mut self.graph,
            &high.kind.values_out_types[0],
            &mut decl.proxies_out.iter().enumerate().map(|(i, j)| match j {
                Some(Either::Left(j)) => args[*j].clone(),
                Some(Either::Right(j)) => j
                    .clone()
                    .map(ValueIn::value)
                    .unwrap_or_else(ValueIn::default),
                None => ValueIn::link(Link::node(low, i)),
            }),
        );
        self.reset_values(high.links.values_out[0].clone(), ret);
        if decl.control {
            for &from in &high.links.controls_in[0] {
                self.graph
                    .relink_control(from, Link::node(x, 0), &[Link::node(low, 0)]);
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
        // 控制流里的环改写成「有限循环」节点。放在 optimize 之后：优化器
        // 先消掉 Nop / IF 并重连，改写面对的是干净的控制流。
        if let Err(e) = crate::compile::loops::lower_loops(&mut self.graph) {
            panic!("控制流改写失败：{e}");
        }
        self.graph.verify();
        // 列表类 IR 节点要先按元素类型展开成真正的列表节点。它们要往图里插
        // 结构体依赖声明(见 `node_struct`),不能像下面那段一样在持有节点可变
        // 借用时顺手改图,所以单独走一遍:先克隆出节点,再改图。
        for x in self.graph.get_nodes() {
            let kind = self.graph.get_node(x).kind.clone();
            let lowered = match &kind.id {
                IrNodeId::ListAssemble => {
                    let count = kind.values_in_types.len();
                    let elem = kind.values_out_types[0].list_elem().expect("拼装列表的输出应当是列表");
                    // 原节点前面还有一个隐藏的「元素个数」引脚,IR 里没有:
                    // 这里补出来,默认值就是元素个数,已有连线整体后移一位。
                    self.graph.insert_value_in_front(x);
                    self.graph.set_default(Link::node(x, 0), Some((count as i32).into()));
                    Some(node_list_assemble(&mut self.graph, elem.into_native().unwrap(), count))
                }
                IrNodeId::ListGet => {
                    let elem = kind.values_in_types[0].as_ref().unwrap().list_elem().expect("按下标取值的输入应当是列表");
                    Some(node_list_get_at_index(&mut self.graph, elem.into_native().unwrap()))
                }
                IrNodeId::ListSet => {
                    let elem = kind.values_in_types[0].as_ref().unwrap().list_elem().expect("按下标改值的输入应当是列表");
                    Some(node_list_modify_index(&mut self.graph, elem.into_native().unwrap()))
                }
                _ => None,
            };
            if let Some(node) = lowered {
                self.graph.get_node_mut(x).kind = node_ir_native(node);
            }
        }
        for x in self.graph.get_nodes() {
            let node = self.graph.get_node_mut(x);
            let kind = &mut node.kind;
            *kind = node_ir_native(match &kind.id {
                IrNodeId::Native(_) => continue,
                IrNodeId::Nop => panic!(),
                IrNodeId::Unreachable => {
                    self.graph.remove(x);
                    continue;
                }
                IrNodeId::Local => {
                    if let NativeKind::Struct(kind) = kind.values_out_types[0].lower(linker) {
                        let kind = kind.clone();
                        let n =
                            node_on_native_custom_value_change(&mut self.graph, &kind.to_kind());
                        let n = self.graph.insert(node_ir_native(n));
                        self.graph.move_comment(x, n);
                        let n_getter = self.graph.insert(node_ir_native(node_destructure_struct(
                            &linker.assets,
                            &kind,
                        )));
                        self.graph
                            .link_value(Link::node(n, 3), Link::node(n_getter, 0));
                        let n0 = self.graph.get_node(x);
                        self.reset_values(
                            n0.links.values_out[0].clone(),
                            ValueIn::link(Link::node(n, 3)),
                        );
                        let n0 = self.graph.get_node(x);
                        self.reset_values(
                            n0.links.values_out[1].clone(),
                            ValueIn::link(Link::node(n_getter, 0)),
                        );
                        self.graph.remove(x);
                        continue;
                    } else {
                        node_local(&match &kind.values_out_types[1] {
                            IrKind::PartialMut(_) => panic!(),
                            other => other.lower(linker),
                        })
                        .unwrap()
                    }
                }
                IrNodeId::SetLocal => {
                    if let NativeKind::Struct(kind) =
                        kind.values_in_types[0].as_ref().unwrap().lower(linker)
                    {
                        let kind = kind.clone();
                        let n = self
                            .graph
                            .insert(node_ir_native(node_modify_struct(&linker.assets, &kind)));
                        self.graph.move_comment(x, n);
                        let n0 = self.graph.get_node(x);
                        for from in n0.links.controls_in[0].clone() {
                            self.graph
                                .relink_control(from, Link::node(x, 0), &[Link::node(n, 0)]);
                        }
                        let n0 = self.graph.get_node(x);
                        for to in n0.links.controls_out[0].clone() {
                            self.graph.link_control(Link::node(n, 0), to);
                        }
                        let n0 = self.graph.get_node(x).links.clone();
                        self.graph
                            .set_value_in(Link::node(n, 0), n0.values_in[0].clone());
                        self.graph
                            .set_value_in(Link::node(n, 2), n0.values_in[1].clone());
                        self.graph
                            .set_value_in(Link::node(n, 3), ValueIn::value(true.into()));
                        self.graph.remove(x);
                        continue;
                    } else {
                        node_set_local(&kind.values_in_types[1].as_ref().unwrap().lower(linker))
                    }
                }
                IrNodeId::Switch(values) => {
                    assert!(!values.is_empty()); // optimized
                    match kind.values_in_types[0].as_ref().unwrap().lower(linker) {
                        NativeKind::Bool => {
                            let last = node.links.clone();
                            let br_false = *values[0].downcast_ref::<bool>().unwrap() as usize;
                            let n1 = self.graph.insert(node_ir_native(NODE_IF.clone()));
                            self.graph.move_comment(x, n1);
                            self.graph.set_value_in(Link::node(n1, 0), last.values_in[0].clone());
                            self.graph.relink_controls(&last.controls_in[0], Link::node(x, 0), &[Link::node(n1, 0)]);
                            self.graph.link_controls(Link::node(n1, 0), &last.controls_out[1 - br_false]);
                            self.graph.link_controls(Link::node(n1, 1), &last.controls_out[br_false]);
                        },
                        k => {
                            // TODO: when values.len() > 10
                            let last = node.links.clone();
                            let values = values.clone();
                            let n1 = self.graph.insert(node_ir_native(node_switch(k, values.len())));
                            self.graph.move_comment(x, n1);
                            self.graph.set_value_in(Link::node(n1, 0), last.values_in[0].clone());
                            self.graph.relink_controls(&last.controls_in[0], Link::node(x, 0), &[Link::node(n1, 0)]);
                            self.graph.link_controls(Link::node(n1, 0), last.controls_out.last().unwrap());
                            for i in 0..values.len() {
                                self.graph.link_controls(Link::node(n1, 1 + i), &last.controls_out[i]);
                            }
                            self.graph.set_default(Link::node(n1, 1), Some(values.into()));
                        }
                    }
                    self.graph.remove(x);
                    continue;
                },
                IrNodeId::Assemble => {
                    let k = kind.values_out_types[0].lower(linker);
                    node_assemble_struct(&linker.assets, unwrap!(&k, NativeKind::Struct))
                }
                // 这三种在前面的单独一遍里就展开成 Native 节点了
                | IrNodeId::ListAssemble
                | IrNodeId::ListGet
                | IrNodeId::ListSet
                => unreachable!("列表节点已经在 lowering 前展开"),
                IrNodeId::Destructure => {
                    let k = kind.values_in_types[0].as_ref().unwrap().lower(linker);
                    node_destructure_struct(&linker.assets, unwrap!(&k, NativeKind::Struct))
                }
                IrNodeId::Modify => {
                    let k = kind.values_in_types[0].as_ref().unwrap().lower(linker);
                    node_modify_struct(&linker.assets, unwrap!(&k, NativeKind::Struct))
                }
                IrNodeId::FieldMut(idx) => panic!("{idx}"),
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
            || self.graph.externals.controls_in.first() == Some(&vec![])
        {
            // fn is unreachable
            self.decl.control = false;
            *self.graph.exports.get_mut(&PinType::InControl).unwrap() = vec![];
            *self.graph.exports.get_mut(&PinType::OutControl).unwrap() = vec![];
            self.graph.externals.controls_in = vec![];
            self.graph.externals.controls_out = vec![];
        }

        let active: Vec<_> = self
            .graph
            .externals
            .values_out
            .iter()
            .map(|v| v.iter().any(|&l| l.target.is_node()))
            .collect();
        let mut rm = HashMap::new();
        for (i, (j, _)) in active.iter().enumerate().filter(|(_, x)| **x).enumerate() {
            rm.insert(j, i);
        }
        for (_i, node) in self.graph.nodes.iter_mut() {
            for x in node.links.values_in.iter_mut() {
                if let Some(Link {
                    target: LinkTarget::Export,
                    index,
                }) = &mut x.link
                {
                    *index = rm[index];
                }
            }
        }
        let mut i = 0;
        self.graph
            .exports
            .get_mut(&PinType::InValue)
            .unwrap()
            .retain(|_| active[i].tap(|_| i += 1));
        let mut i = 0;
        self.graph
            .externals
            .values_out
            .retain(|_| active[i].tap(|_| i += 1));
        let mut i = 0;
        self.decl.proxies_in.retain(|_| active[i].tap(|_| i += 1));
        self.decl.proxies_out = self
            .graph
            .externals
            .values_in
            .iter()
            .map(|x| match x.link {
                Some(Link {
                    target: LinkTarget::Node(..),
                    ..
                }) => None,
                Some(Link {
                    target: LinkTarget::Export,
                    index,
                }) => Either::Left(index).into(),
                None => Either::Right(x.default.clone()).into(),
            })
            .collect();

        let active: Vec<_> = self
            .graph
            .externals
            .values_in
            .iter()
            .map(|v| {
                matches!(
                    v.link,
                    Some(Link {
                        target: LinkTarget::Node(..),
                        ..
                    })
                )
            })
            .collect();
        let mut rm = HashMap::new();
        for (i, (j, _)) in active.iter().enumerate().filter(|(_, x)| **x).enumerate() {
            rm.insert(j, i);
        }
        for (_, node) in self.graph.nodes.iter_mut() {
            for x in node.links.values_out.iter_mut().flatten() {
                if let Link {
                    target: LinkTarget::Export,
                    index,
                } = x
                {
                    *index = rm[index];
                }
            }
        }
        let mut i = 0;
        self.graph
            .exports
            .get_mut(&PinType::OutValue)
            .unwrap()
            .retain(|_| active[i].tap(|_| i += 1));
        let mut i = 0;
        self.graph
            .externals
            .values_in
            .retain(|_| active[i].tap(|_| i += 1));

        for x in &mut self.graph.externals.values_out {
            x.retain(|&l| l.target.is_node());
        }
        for x in &mut self.graph.externals.values_in {
            if let Some(link) = x.link.as_mut()
                && link.target.is_export()
            {
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
        while self.eliminate_solos() {}
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
        self.eliminate_nop(node)?;
        self.raise_if(node)?;
        self.eliminate_branches(node)?;
        self.eliminate_switch(node)?;
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

    pub fn eliminate_nop(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.id != IrNodeId::Nop {
            return Some(());
        }
        if n.links.controls_out[0].first() == Some(&Link::node(node, 0)) {
            let from = n.links.controls_in[0].clone();
            let un = self.graph.insert(ir::node_ir_unreachable());
            self.graph
                .relink_controls(&from, Link::node(node, 0), &[Link::node(un, 0)]);
            return None;
        }
        self.relink_controls(node, 0, 0);
        self.graph.remove(node);
        None
    }

    pub fn raise_if(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.id.as_native() != Some(&NODE_IF.id) {
            return Some(());
        }
        let last = n.links.clone();
        let n1 = self.graph.insert(ir::node_ir_switch(
            IrKind::Native(NativeKind::Bool),
            vec![true.into()],
        ));
        self.graph.relink_controls(
            &last.controls_in[0],
            Link::node(node, 0),
            &[Link::node(n1, 0)],
        );
        self.graph
            .link_controls(Link::node(n1, 0), &last.controls_out[0]);
        self.graph
            .link_controls(Link::node(n1, 1), &last.controls_out[1]);
        self.graph.remove(node);
        None
    }

    pub fn eliminate_branches(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        let IrNodeId::Switch(values) = &n.kind.id else {
            return Some(());
        };
        let reachable: Vec<_> = n
            .links
            .controls_out
            .iter()
            .map(|x| {
                x.first()
                    .map(|x| {
                        x.target
                            .node()
                            .map(|x| self.graph.get_node(x).kind.id != IrNodeId::Unreachable)
                            .unwrap_or(true)
                    })
                    .unwrap_or(true)
            })
            .collect();
        let alive: usize = reachable.iter().copied().map(usize::from).sum();
        if alive == n.links.controls_out.len() {
            return Some(());
        }
        if alive == 0 {
            let from = n.links.controls_in[0].clone();
            let n1 = self.graph.insert(ir::node_ir_unreachable());
            self.graph.relink_controls(&from, Link::node(node, 0), &[Link::node(n1, 0)]);
            self.graph.remove(node);
            return None;
        }
        let mut values: Vec<_> = values.iter().enumerate().filter_map(|(i, x)| if reachable[i] { x.into() } else { None }).cloned().collect();
        if !reachable.last().unwrap() {
            values.pop();
        }
        let outs: Vec<_> = n.links.controls_out.iter().enumerate().filter_map(|(i, x)| if reachable[i] { x.into() } else { None }).cloned().collect();
        let last_control = n.links.controls_in[0].clone();
        let last_value = n.links.values_in[0].clone();
        let n1 = self.graph.insert(ir::node_ir_switch(n.kind.values_in_types[0].clone().unwrap(), values));
        self.graph.relink_controls(&last_control, Link::node(node, 0), &[Link::node(n1, 0)]);
        self.graph.set_value_in(Link::node(n1, 0), last_value);
        for (i, x) in outs.iter().enumerate() {
            self.graph.link_controls(Link::node(n1, i), x);
        }
        self.graph.remove(node);
        None
    }

    pub fn eliminate_switch(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        let IrNodeId::Switch(values) = &n.kind.id else {
            return Some(());
        };
        let br = if values.is_empty() || n.links.controls_out.iter().all(|x| *x == n.links.controls_out[0]) {
            0
        } else if let Some(v) = &n.links.values_in[0].default {
            let mut i = 0;
            while i < values.len() {
                if v == &values[i] {
                    break;
                }
                i += 1;
            }
            i
        } else {
            return Some(());
        };
        self.relink_controls(node, 0, br);
        self.graph.remove(node);
        None
    }

    pub fn eliminate_set_local(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.id == IrNodeId::SetLocal {
            let Link {
                target: LinkTarget::Node(source),
                ..
            } = n.links.values_in[0].link.unwrap()
            else {
                return Some(());
            };
            let source = self.graph.get_node(source);
            if source.kind.id != IrNodeId::Local {
                return Some(());
            }
            if !source.links.values_out[1].is_empty() {
                return Some(());
            }
            self.relink_controls(node, 0, 0);
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
        if n.kind.is_calc() && n.links.values_out.iter().all(|x| x.is_empty()) {
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
        let Link {
            target: LinkTarget::Node(local),
            index: 0,
        } = n.links.values_in[0].link.unwrap()
        else {
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
        let max = if n.kind.values_in_types[1].as_ref().unwrap().is_storable() {
            DEFAULT_MAX
        } else {
            usize::MAX
        };

        // ---- 步骤 1：找到所有 setters ----
        //
        // `local_setters` 是写这个 local 的节点（`Local` 的句柄输出被它们消费）。
        // 句柄被别的东西消费了（复合节点的调用、ADT 的修改……）—— 它可能改这个
        // 变量，值从哪来说不清 —— 直接拒绝优化。
        let mut local_setters: HashSet<NodeRef> = HashSet::new();
        // `opaque` 是「说不清它求值时是什么值」的节点（见步骤 1b）。
        // 控制流回溯时碰到它们，就说明那个执行点读到的值来源不明，拒绝优化。
        let mut opaque: HashSet<NodeRef> = HashSet::new();
        for x in n_local
            .links
            .values_out[0]
            .iter()
            .filter_map(|x| x.target.node())
        {
            if self.graph.get_node(x).kind.id == IrNodeId::SetLocal {
                local_setters.insert(x);
            } else {
                // 句柄被拿去改这个变量了：值来源不明
                return Some(());
            }
        }
        assert!(local_setters.contains(&node));

        // ---- 步骤 1b：写入的值从哪来 ----
        //
        // 消除这个 setter 相当于把写入的值挪到 usage 那里求值，所以得先看这个
        // 值是从哪来的。从 setter 的 `value in[1]` 的来源开始 BFS（值流不会有环，
        // 用 visited 防重入）：
        //
        // - 来自 extern：跳过；
        // - 不是 calc 节点：执行节点，它什么时候执行会影响到这个值 —— opaque；
        // - 是 `Local` 节点：读的是另一个变量，它所有的 setter 都是 opaque；
        //   它的值（`values_out[1]`）的每一处使用也要一起看 —— 入队，继续 BFS；
        // - 是其它不纯的节点：每次求值可能不同，挪了就变了 —— 拒绝优化；
        // - 是 pure 节点：把它的所有 `value in` 入队，继续。
        let mut queue = VecDeque::new();
        let mut visited: HashSet<NodeRef> = HashSet::new();
        if let Some(v) = n.links.values_in[1].link {
            queue.push_back(v);
        }
        while let Some(now) = queue.pop_front() {
            let Some(x) = now.target.node() else {
                // 来自图外
                continue;
            };
            if !visited.insert(x) {
                continue;
            }
            let nx = self.graph.get_node(x);
            if nx.kind.id == IrNodeId::Local {
                // 读的是另一个变量：它所有的 setter 都是 opaque
                for s in nx.links.values_out[0].iter().filter_map(|l| l.target.node()) {
                    opaque.insert(s);
                }
                // 它的值的每一处使用也要一起看
                for u in &nx.links.values_out[1] {
                    queue.push_back(*u);
                }
            } else if !nx.kind.is_calc() {
                // 执行节点：语义不明
                opaque.insert(x);
            } else if !nx.kind.is_pure() {
                // 不纯的值挪了就变了
                return Some(());
            } else {
                // pure：看它的输入
                for v in &nx.links.values_in {
                    if let Some(link) = v.link {
                        queue.push_back(link);
                    }
                }
            }
        }

        // ---- 步骤 2：遍历每一处 usage ----
        //
        // usage 是「读这个 local 的值」的那条边。每处 usage 先沿数据流找出所有
        // executors（触发它求值的执行节点），再沿控制流回溯，判断它读到的值
        // 到底由谁决定：
        //
        // - 完全取决于当前 setter：可以把它改接到写入的值上；
        // - 完全与当前 setter 无关（它前面执行的都是别的 setter）：不动它；
        // - 不明确（比如有分支，两边写的值不一样）：拒绝优化。
        let mut usages = vec![];
        for &x in &n_local.links.values_out[1] {
            // --- 2a. 沿数据流找 executors ---
            //
            // 队列里带上了「这条边的值是什么类型」—— 值的类型由它的来源
            // （输出引脚）决定。
            let mut queue = VecDeque::new();
            queue.push_back((n_local.kind.values_out_types.get(1).cloned(), x));
            let mut executors: HashSet<LinkTarget> = HashSet::new();
            // 通过引用写这个值的执行点（见下）。
            let mut writes: Vec<LinkTarget> = vec![];
            // 值流不可能有环，不用 visited 防重入。
            while let Some((ty, now)) = queue.pop_front() {
                let Some(now_node) = now.target.node() else {
                    // 值走出图外
                    executors.insert(LinkTarget::Export);
                    continue;
                };
                let n_now = self.graph.get_node(now_node);
                if n_now.kind.controls_in_num == 0 {
                    // 没有控制输入：不是执行点，沿数据流继续往下找
                    for (p, edges) in n_now.links.values_out.iter().enumerate() {
                        let ty = n_now.kind.values_out_types.get(p).cloned();
                        for e in edges {
                            queue.push_back((ty.clone(), *e));
                        }
                    }
                } else {
                    // 有控制输入：这是一个执行点
                    //
                    // `&mut`（`PartialMut`）的值被交给 `Modify`（改字段）或函数
                    // 调用时，那是在**通过引用写**它，不是读 —— 不算执行点。
                    if matches!(ty, Some(IrKind::PartialMut(..)))
                        && maybe_write(&n_now.kind.id, now.index)
                    {
                        writes.push(now.target);
                        continue;
                    }
                    executors.insert(now.target);
                }
            }
            let mut depends_on_us = false;
            let mut unrelated = false;
            if executors.is_empty() {
                if writes.is_empty() {
                    // 找不到执行点：值来源不明
                    return Some(());
                }
                // 这处 usage 只是「写」，不是「读」—— 没有执行点，但输入同样得
                // 改接到写入的值上，否则这个 local 永远消不掉。写之前读过谁的
                // 值仍然按控制流判断；判断不出来的那处就不动它。
                for &w in &writes {
                    if let Ok(Some(s)) = self.last_setter_before(w, &local_setters, &opaque) {
                        if s.len() == 1 && s.contains(&node) {
                            depends_on_us = true;
                        }
                    }
                }
                if depends_on_us {
                    usages.push(x);
                }
                continue;
            }
            // --- 2b. 沿控制流回溯，判断这个 usage 的值由谁决定 ---
            //
            // 对每个执行点 e 问一句话：e 执行之前，最后一个写这个 local 的
            // setter 是谁？沿 e 的控制输入往回走：
            //
            // - 走到写这个 local 的 setter S：S 执行之后的值就是 S 写的，它
            //   之前的 setter 都被盖掉了，路径到此为止；
            // - 走到别的节点：合并各条控制输入路径的结果。
            //
            // 合并出多个不同的 setter，说明有分支、两边写的值不一样 —— 不明确。
            for &e in &executors {
                match self.last_setter_before(e, &local_setters, &opaque) {
                    // 成环，或者路径上有可能改掉这个变量的节点：值来源不明
                    Err(()) => return Some(()),
                    // 之前没有 setter 写过这个变量：与当前 setter 无关
                    Ok(None) => unrelated = true,
                    Ok(Some(s)) => {
                        if s.contains(&node) {
                            if s.len() > 1 {
                                // 既可能是当前 setter 写的，也可能是别的 setter
                                // 写的（分支导致）—— 不明确
                                return Some(());
                            }
                            depends_on_us = true;
                        } else {
                            // 它前面执行的都是别的 setter：与当前 setter 无关
                            unrelated = true;
                        }
                    }
                }
            }
            if depends_on_us && unrelated {
                // 不明确
                return Some(());
            }
            if depends_on_us {
                usages.push(x);
            }
        }
        if usages.is_empty() {
            // 没有 usage 依赖当前 setter：留着它，别把变量的唯一写者删掉。
            return Some(());
        }
        if usages.len() > max {
            return Some(());
        }
        assert!(!usages.contains(&Link::node(node, 1)));
        // 被写入的值不能依赖这个 local 的旧值 —— 否则把 usage 改接到写入的
        // 值上会造环（`i += 1` 就是这种：写入值是 `i + 1`，而 `i` 正是 usage）。
        if let Some(v) = n.links.values_in[1].link {
            if self.value_depends_on(v, Link::node(local, 1)) {
                return Some(());
            }
        }
        // 把要改接的 usage 直接接到写入的值上，然后删掉这个 setter。
        let value = n.links.values_in[1].clone();
        self.reset_values(usages, value);
        self.relink_controls(node, 0, 0);
        self.graph.remove(node);
        None
    }

    /// `root` 执行之前，最后一个写这个 local 的 setter 是谁。
    ///
    /// `root` 是「值的读取点」：它自己如果是 setter，它写下的值在读取之后，
    /// 所以不能算作「之前的 setter」—— 只看它的控制输入。
    ///
    /// - `Ok(None)`：之前没有 setter 写过这个变量。
    /// - `Ok(Some(s))`：候选的最后一个 setter 在 `s` 里；多于一个说明有分支。
    /// - `Err(())`：控制流成环，或路径上有可能改掉这个变量的节点 —— 值来源不明。
    fn last_setter_before(
        &self,
        root: LinkTarget,
        local_setters: &HashSet<NodeRef>,
        opaque: &HashSet<NodeRef>,
    ) -> Result<Option<HashSet<NodeRef>>, ()> {
        let mut memo: HashMap<LinkTarget, Result<Option<HashSet<NodeRef>>, ()>> = HashMap::new();
        let mut stack: HashSet<LinkTarget> = HashSet::new();
        self.merge_setters(
            self.graph
                .get_links(root)
                .controls_in
                .iter()
                .flatten()
                .map(|x| x.target),
            local_setters,
            opaque,
            &mut memo,
            &mut stack,
        )
    }

    /// 合并若干条路径的结果：路径上没有 setter 写过（`None`）和有 setter 写过
    /// 撞在一起，说明两条路径的值不一样 —— 不明确。
    fn merge_setters(
        &self,
        preds: impl Iterator<Item = LinkTarget>,
        local_setters: &HashSet<NodeRef>,
        opaque: &HashSet<NodeRef>,
        memo: &mut HashMap<LinkTarget, Result<Option<HashSet<NodeRef>>, ()>>,
        stack: &mut HashSet<LinkTarget>,
    ) -> Result<Option<HashSet<NodeRef>>, ()> {
        let mut acc: Option<HashSet<NodeRef>> = None;
        for pred in preds {
            match self.walk(pred, local_setters, opaque, memo, stack)? {
                None => {
                    if acc.is_some() {
                        return Err(());
                    }
                }
                Some(s) => {
                    match acc.as_mut() {
                        None => acc = Some(s),
                        Some(a) => a.extend(s),
                    }
                }
            }
        }
        Ok(acc)
    }

    /// `now` 执行之后的值由谁决定。见 `last_setter_before`。
    fn walk(
        &self,
        now: LinkTarget,
        local_setters: &HashSet<NodeRef>,
        opaque: &HashSet<NodeRef>,
        memo: &mut HashMap<LinkTarget, Result<Option<HashSet<NodeRef>>, ()>>,
        stack: &mut HashSet<LinkTarget>,
    ) -> Result<Option<HashSet<NodeRef>>, ()> {
        if !stack.insert(now) {
            // 控制流成环：「之前」没法定义
            return Err(());
        }
        if let Some(r) = memo.get(&now) {
            stack.remove(&now);
            return r.clone();
        }
        let res = (|| {
            if let Some(x) = now.node() {
                if local_setters.contains(&x) {
                    // 它自己就是 setter：它执行之后的值就是它写的，它之前的
                    // setter 都被盖掉了
                    return Ok(Some(HashSet::from([x])));
                }
                if opaque.contains(&x) {
                    // 有可能改掉这个变量的节点：值来源不明
                    return Err(());
                }
            }
            self.merge_setters(
                self.graph
                    .get_links(now)
                    .controls_in
                    .iter()
                    .flatten()
                    .map(|x| x.target),
                local_setters,
                opaque,
                memo,
                stack,
            )
        })();
        stack.remove(&now);
        memo.insert(now, res.clone());
        res
    }

    /// `link` 的值依赖闭包里有没有 `target`。
    fn value_depends_on(&self, link: Link, target: Link) -> bool {
        let mut queue = VecDeque::new();
        queue.push_back(link);
        let mut visited = HashSet::new();
        while let Some(now) = queue.pop_front() {
            if !visited.insert(now) {
                continue;
            }
            if now == target {
                return true;
            }
            let Some(n) = now.target.node() else {
                continue;
            };
            queue.extend(
                self.graph
                    .get_node(n)
                    .links
                    .values_in
                    .iter()
                    .flat_map(|x| x.link),
            );
        }
        false
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
        let Some(Link {
            target: LinkTarget::Node(from),
            ..
        }) = n.links.values_in[0].link
        else {
            return Some(());
        };
        let from_node = self.graph.get_node(from);
        if from_node.kind.id != IrNodeId::Destructure
            || from_node.kind.values_in_types[0].as_ref() != Some(&n.kind.values_out_types[0])
        {
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
        let Some(Link {
            target: LinkTarget::Node(from),
            ..
        }) = n.links.values_in[0].link
        else {
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
        let Some(Link {
            target: LinkTarget::Node(v1),
            index: _,
        }) = v1.link
        else {
            return Some(());
        };
        let n1 = self.graph.get_node(v1);
        if n1.kind.id.as_native() != Some(&node_cast(NativeKind::Bool, NativeKind::Int).unwrap().id)
        {
            return Some(());
        }
        let from = n1.links.values_in[0].clone();
        let to = n.links.values_out[0].clone();
        let v = match v2
            .default
            .as_ref()
            .map(|x| *x.downcast_ref::<i32>().unwrap())
            .unwrap_or(0)
        {
            0 => {
                let node_not = self.graph.insert(node_ir_native(NODE_NOT.clone()));
                self.graph.set_value_in(Link::node(node_not, 0), from);
                ValueIn::link(Link::node(node_not, 0))
            }
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
        let Some(Link {
            target: LinkTarget::Node(from),
            index: _,
        }) = n.links.values_in[0].link
        else {
            return Some(());
        };
        let from = self.graph.get_node(from);
        if from.kind.id.as_native() != Some(&NODE_NOT.id) {
            return Some(());
        }
        self.reset_values(
            n.links.values_out[0].clone(),
            from.links.values_in[0].clone(),
        );
        None
    }

    pub fn relink_controls(&mut self, node: NodeRef, from: usize, to: usize) {
        let mut last = self.graph.get_node(node).links.clone();
        for x in mem::take(&mut last.controls_in[from]) {
            self.graph
                .relink_control(x, Link::node(node, from), &last.controls_out[to]);
        }
    }

    pub fn reset_values(&mut self, tos: Vec<Link>, value: ValueIn) {
        for x in tos {
            self.graph.set_value_in(x, value.clone());
        }
    }
}

/// 可能是通过引用**写**这个值的节点：
///
/// - `Modify` 的 `value in[0]`：改它的字段；
/// - 函数调用（复合节点 `Fn`、原生调用的桩 `GeneratedStub`）：把它当 `&mut` 传进去。
///
/// `&mut`（`PartialMut`）的值落到这些地方时，写的是被引用的东西，不是读这个值
/// 本身 —— 所以这种连接不算「读」。
fn maybe_write(id: &IrNodeId, index: usize) -> bool {
    match id {
        IrNodeId::Modify => index == 0,
        IrNodeId::Fn(_) => true,
        IrNodeId::Native(NativeNodeId {
            kind: identifier::AssetKind::GeneratedStub,
            ..
        }) => true,
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::func::NodeGraphIr;
    use crate::compile::ir::{node_ir_local, node_ir_set_local, node_ir_switch};
    use crate::node::arithmetic::node_add;
    use crate::node::{ExportDecl, NodeGraphKind, NodeKind};
    use crate::value::NativeKind;

    fn int() -> IrKind {
        IrKind::Native(NativeKind::Int)
    }

    /// `i += 1`：`add` 读的是 `i` 的旧值，而 `i` 的新值又是 `add` 的输出。
    ///
    /// `eliminate_unnecessary_local_setter` 曾经把 `add` 的输入改接到它自己的
    /// 输出上，造出「自己连自己」—— 因为它没分清「执行者之前的 setter」和
    /// 「执行者本身」。
    #[test]
    fn local_setter_does_not_connect_the_addition_to_itself() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "i_plus_1");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));

        let i = g.insert(node_ir_local(int()));
        // `i = 0`
        let init = g.insert(node_ir_set_local(int()));
        g.link_value(Link::node(i, 0), Link::node(init, 0));
        // `i + 1`：读 `i` 的旧值
        let add = g.insert(node_ir_native(node_add(NativeKind::Int)));
        g.link_value(Link::node(i, 1), Link::node(add, 0));
        // `i = i + 1`
        let set = g.insert(node_ir_set_local(int()));
        g.link_value(Link::node(i, 0), Link::node(set, 0));
        g.link_value(Link::node(add, 0), Link::node(set, 1));

        g.link_control(Link::export(0), Link::node(init, 0));
        g.link_control(Link::node(init, 0), Link::node(set, 0));
        g.link_control(Link::node(set, 0), Link::export(0));

        let mut opt = Optimizer {
            graph: g,
            decl: FnDecl::default(),
        };
        opt.eliminate_unnecessary_local_setter(set);

        // 没有任何节点连自己
        for x in opt.graph.get_nodes() {
            let node = opt.graph.get_node(x);
            for (i, v) in node.links.values_in.iter().enumerate() {
                if let Some(l) = v.link
                    && l.target.node() == Some(x)
                {
                    panic!("节点 {x:?} 的值入 {i} 连到了自己");
                }
            }
        }
        // `add` 仍然读 `i` 的旧值
        assert_eq!(
            opt.graph.get_node(add).links.values_in[0].link,
            Some(Link::node(i, 1)),
            "`add` 的输入不该被改接"
        );
    }

    /// `i = 0; i = 1; use(i)`：读 `i` 的那个执行点在 `i = 1` 之后，它读到的
    /// 值完全取决于当前 setter —— 当前 setter 可以消掉，读的地方直接用写入的值。
    ///
    /// 这里「执行点之前最后一个 setter」是当前 setter 自己，之前那个
    /// （`i = 0`）被它盖掉了，不算。
    #[test]
    fn local_setter_is_removed_when_the_usage_depends_on_it() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "i_then_use");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));

        let i = g.insert(node_ir_local(int()));
        // `i = 0`
        let init = g.insert(node_ir_set_local(int()));
        g.link_value(Link::node(i, 0), Link::node(init, 0));
        // `i = 1`
        let set = g.insert(node_ir_set_local(int()));
        g.link_value(Link::node(i, 0), Link::node(set, 0));
        // 用 `i`：`switch` 是个执行点
        let sw = g.insert(node_ir_switch(int(), vec![1i32.into()]));
        g.link_value(Link::node(i, 1), Link::node(sw, 0));

        g.link_control(Link::export(0), Link::node(init, 0));
        g.link_control(Link::node(init, 0), Link::node(set, 0));
        g.link_control(Link::node(set, 0), Link::node(sw, 0));
        g.link_control(Link::node(sw, 0), Link::export(0));

        let mut opt = Optimizer {
            graph: g,
            decl: FnDecl::default(),
        };
        opt.eliminate_unnecessary_local_setter(set);

        // 当前 setter 被消掉了
        assert!(
            opt.graph.get_nodes().iter().all(|x| *x != set),
            "当前 setter 应该被消掉"
        );
        // 读的地方改接到写入的值上（写入值没有 link，就是默认值）
        assert_eq!(opt.graph.get_node(sw).links.values_in[0].link, None);
    }

    /// `&mut` 的值被交给 `Modify`（改字段）或函数调用时，那是在**通过引用写**它，
    /// 不是读它 —— 那处 usage 不算执行点，不能因此拒绝整个优化。
    #[test]
    fn writing_through_a_reference_is_not_an_executor() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "by_ref");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));

        let pm = IrKind::PartialMut("Test".into());
        let s = g.insert(node_ir_local(pm.clone()));
        // `s = &mut 0`
        let init = g.insert(node_ir_set_local(pm.clone()));
        g.link_value(Link::node(s, 0), Link::node(init, 0));
        // `s = &mut 1`
        let set = g.insert(node_ir_set_local(pm.clone()));
        g.link_value(Link::node(s, 0), Link::node(set, 0));
        // 通过 `s` 改字段：`Modify` 的 `value in[0]` 接的是 `s` 的值
        let modify = g.insert(NodeKind::new(
            IrNodeId::Modify,
            1,
            1,
            vec![IrKind::Adt("Test".into()).into(), None],
            vec![],
        ));
        g.link_value(Link::node(s, 1), Link::node(modify, 0));
        // 把 `s` 传给函数：函数可能通过它写
        let func = g.insert(NodeKind::new(
            IrNodeId::Fn("f".into()),
            1,
            1,
            vec![pm.clone().into()],
            vec![],
        ));
        g.link_value(Link::node(s, 1), Link::node(func, 0));
        // 读 `s`：`switch` 是个执行点
        let sw = g.insert(node_ir_switch(pm.clone(), vec![]));
        g.link_value(Link::node(s, 1), Link::node(sw, 0));

        g.link_control(Link::export(0), Link::node(init, 0));
        g.link_control(Link::node(init, 0), Link::node(set, 0));
        g.link_control(Link::node(set, 0), Link::node(sw, 0));
        g.link_control(Link::node(set, 0), Link::node(modify, 0));
        g.link_control(Link::node(set, 0), Link::node(func, 0));
        g.link_control(Link::node(sw, 0), Link::node(modify, 0));
        g.link_control(Link::node(modify, 0), Link::node(func, 0));
        g.link_control(Link::node(func, 0), Link::export(0));

        let mut opt = Optimizer {
            graph: g,
            decl: FnDecl::default(),
        };
        opt.eliminate_unnecessary_local_setter(set);

        // 「写」那两处 usage 不算执行点，所以当前 setter 还是被消掉了
        assert!(
            opt.graph.get_nodes().iter().all(|x| *x != set),
            "当前 setter 应该被消掉"
        );
        // 读的地方改接到写入的值上（写入值没有 link，就是默认值）
        assert_eq!(opt.graph.get_node(sw).links.values_in[0].link, None);
        // 「写」那两处 usage 不算执行点，但也得改接到写入的值上 —— 否则这个
        // local 永远消不掉（`&mut` 的 read 半边是非 storable 的 `PartialMut`）
        assert_eq!(opt.graph.get_node(modify).links.values_in[0].link, None);
        assert_eq!(opt.graph.get_node(func).links.values_in[0].link, None);
    }
}
