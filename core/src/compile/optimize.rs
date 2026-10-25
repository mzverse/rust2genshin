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
use crate::node::list::{node_list_assemble, node_list_get_at_index, node_list_get_length, node_list_insert, node_list_modify_index};
use crate::node::query::node_local;
use crate::node::{Link, LinkTarget, Lower, NativeNodeId, NodeGraph, NodeKind, NodeRef, ValueIn};
use crate::structure::{node_assemble_struct, node_destructure_struct, node_modify_struct};
use crate::value::NativeKind;
use either::Either;
use std::collections::{HashMap, HashSet, VecDeque};
use std::mem;
use std::mem::swap;
use tap::Tap;
use crate::unwrap;

/// 一处 usage 该怎么处理。
enum UsageVerdict {
    /// 改接到写入值上。
    Rewrite,
    /// 与当前 setter 无关，不动它。
    Skip,
    /// 来源不明，整个优化放弃。
    Reject,
}

/// 一条起点边（局部变量值出口的某一条连线）在数据流上的去向。
struct Usage {
    /// 起点边本身 —— 优化时改接的就是它。
    start: Link,
    /// 读到这个值的执行节点（路径上经过 `Copy`）。
    users: Vec<LinkTarget>,
    /// 只拿到视图、通过引用写的执行节点。
    modifiers: Vec<LinkTarget>,
}

impl Optimizer {
    pub fn verify(&self, _helper: Helper) -> super::Result<()> {
        self.graph.verify();
        self.verify_types();
        Ok(())
    }

    /// 全图类型校验：每条数据流连接两端的引脚类型必须完全相等。
    ///
    /// 这是 IR 层的「类型检查」：`Copy` / `PartialMut` / `LocalRef` 这些
    /// 不能靠 wire 上同形来蒙混，必须在标称类型上就对得上。
    fn verify_types(&self) {
        let graph = &self.graph;
        for (i, node) in &graph.nodes {
            // 原生节点的引脚类型是 wire 层的 `NativeKind`，和 IR 类型不是一套
            // —— `link_node` 展开复合节点时本来就会混接，这里不校验。
            if node.kind.id.as_native().is_some() {
                continue;
            }
            for (j, value) in node.links.values_in.iter().enumerate() {
                let Some(link) = value.link else { continue };
                let Some(expected) = node.kind.values_in_types[j].as_ref() else {
                    panic!("节点 {:?} 的值入 {j} 没有类型,却接了连线", NodeRef::from(i));
                };
                let actual = match link.target {
                    LinkTarget::Node(src) => {
                        let src = graph.get_node(src);
                        // 和上面同理:一端是原生节点时比不了。
                        if src.kind.id.as_native().is_some() {
                            continue;
                        }
                        src.kind.values_out_types[link.index].clone()
                    }
                    LinkTarget::Export => self.export_kind(PinType::InValue, link.index),
                };
                assert_eq!(actual, *expected, "节点 {:?} ({:?}) 的值入 {j} 类型不符;来源 {:?} ({:?});节点类型 {:?}", NodeRef::from(i), node.kind.id, link.target, link.target.node().map(|x| self.graph.get_node(x).kind.id.clone()), node.kind.values_in_types);
            }
        }
        // 图出口：出参的类型也必须和来源引脚一致。
        for (i, decl) in self.graph.exports.get(&PinType::OutValue).into_iter().flatten().enumerate() {
            let Some(expected) = decl.kind() else { continue };
            let Some(link) = self.graph.externals.values_in[i].link else { continue };
            let LinkTarget::Node(src) = link.target else { continue };
            let actual = &self.graph.get_node(src).kind.values_out_types[link.index];
            assert_eq!(actual, expected, "出口 {i} 类型不符");
        }
        // 图入口：入参接到节点引脚,类型同样要对上。
        for (i, decl) in self.graph.exports.get(&PinType::InValue).into_iter().flatten().enumerate() {
            let Some(expected) = decl.kind() else { continue };
            for target in &self.graph.externals.values_out[i] {
                let LinkTarget::Node(dst) = target.target else { continue };
                let actual = self.graph.get_node(dst).kind.values_in_types[target.index].as_ref().unwrap();
                assert_eq!(actual, expected, "入口 {i} 类型不符");
            }
        }
    }

    fn export_kind(&self, kind: PinType, index: usize) -> IrKind {
        self.graph
            .exports
            .get(&kind)
            .and_then(|x| x.get(index))
            .and_then(|x| x.kind())
            .cloned()
            .unwrap_or_else(|| panic!("出口 {kind:?}[{index}] 没有类型"))
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
        // 优化也会改接数据流（`ToPartialMut` / `Copy` / 解构替换）—— 改接后的
        // 类型契约同样要成立。放在这里，优化器插错线会当场炸出来。
        self.verify_types();
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
                    Some(node_list_assemble(&mut self.graph, elem.to_native().expect("列表元素必须是原生类型"), count))
                }
                IrNodeId::ListGet => {
                    let elem = list_input_elem(kind.values_in_types[0].as_ref().unwrap());
                    Some(node_list_get_at_index(&mut self.graph, elem.to_native().expect("列表元素必须是原生类型")))
                }
                IrNodeId::ListSet => {
                    let elem = list_input_elem(kind.values_in_types[0].as_ref().unwrap());
                    Some(node_list_modify_index(&mut self.graph, elem.to_native().expect("列表元素必须是原生类型")))
                }
                IrNodeId::ListInsert => {
                    let elem = list_input_elem(kind.values_in_types[0].as_ref().unwrap());
                    Some(node_list_insert(&mut self.graph, elem.to_native().expect("列表元素必须是原生类型")))
                }
                IrNodeId::ListGetLength => {
                    let elem = list_input_elem(kind.values_in_types[0].as_ref().unwrap());
                    Some(node_list_get_length(&mut self.graph, elem.to_native().expect("列表元素必须是原生类型")))
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
                // 列表类节点在上面那一轮已经展开成真正的列表节点了。
                IrNodeId::ListAssemble |
                IrNodeId::ListGet |
                IrNodeId::ListSet |
                IrNodeId::ListInsert |
                IrNodeId::ListGetLength => panic!(),
                IrNodeId::Unreachable => {
                    self.graph.remove(x);
                    continue;
                }
                IrNodeId::Local => {
                    // 变量类型看句柄引脚里的 `LocalRef`。视图等中间形态在这里
                    // 由 `LocalRef::lower` 的 `!is_storable` 检查报出来。
                    let variable = kind.values_out_types[0]
                        .local_ref()
                        .expect("local 节点的输出 0 应当是 LocalRef");
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
                        node_local(&variable.lower(linker)).unwrap()
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
                        let variable = kind.values_in_types[0]
                            .as_ref()
                            .and_then(IrKind::local_ref)
                            .expect("set_local 的输入 0 应当是 LocalRef");
                        node_set_local(&variable.lower(linker))
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
                IrNodeId::Destructure => {
                    let k = kind.values_in_types[0].as_ref().unwrap().lower(linker);
                    node_destructure_struct(&linker.assets, unwrap!(&k, NativeKind::Struct))
                }
                IrNodeId::Modify => {
                    let k = kind.values_in_types[0].as_ref().unwrap().lower(linker);
                    node_modify_struct(&linker.assets, unwrap!(&k, NativeKind::Struct))
                }
                IrNodeId::Unsupported => panic!("Unsupported 节点不该走到降级"),
                IrNodeId::FieldMut { idx, .. } => panic!("{idx}"),
                IrNodeId::Fn(_) => unreachable!(),
                // 读值 / 标视图都只是 IR 层的写法,节点图里没有额外动作,
                // 也没有控制流要接。
                IrNodeId::Copy | IrNodeId::ToPartialMut => {
                    let n = self.graph.get_node(x);
                    self.reset_values(n.links.values_out[0].clone(), n.links.values_in[0].clone());
                    self.graph.remove(x);
                    continue;
                }
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
        self.eliminate_copy(node)?;
        self.eliminate_unnecessary_local_setter(node)?;
        self.eliminate_assemble(node)?;
        self.eliminate_destructure(node)?;
        self.eliminate_field_mut(node)?;
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
            // 初始值没接东西时不能消：`let` 局部变量的值来自 set_local，不是初始值。
            // 消掉它会把值用户改接到空的初始值引脚上 —— 断连。
            if n.links.values_in[0].link.is_none() {
                return Some(());
            }
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

    /// `Copy(ToPartialMut(x))`:先标成视图又立刻读回来 —— 直接接 `x`。
    ///
    /// `eliminate_unnecessary_local_setter` 改接时会在值前面补一层 `ToPartialMut`,
    /// 读那一侧原本就带着 `Copy`,两层正好互逆。
    pub fn eliminate_copy(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        if n.kind.id != IrNodeId::Copy {
            return Some(());
        }
        let Some(Link {
            target: LinkTarget::Node(pm),
            ..
        }) = n.links.values_in[0].link
        else {
            return Some(());
        };
        let n_pm = self.graph.get_node(pm);
        if n_pm.kind.id != IrNodeId::ToPartialMut {
            return Some(());
        }
        let from = n_pm.links.values_in[0].clone();
        self.reset_values(n.links.values_out[0].clone(), from);
        self.graph.remove(node);
        None
    }

    /// 把「通过 `&mut` 写结构体字段」改写成 `Modify`。
    ///
    /// `FieldMut` 给出的 setter 在节点图里没有对应物,只是 IR 层的占位。
    /// 这里沿句柄流走一遍:句柄经过 `Destructure` 出来、可以存进局部变量再
    /// 取出来,最终落到某个 `SetLocal` 的句柄输入上 —— 那一处就是写,换成
    /// `Modify`;读字段的那一侧改成对 owner 视图的解构。剩下的 `FieldMut`
    /// 降级时还会报错,不会静默走错。
    pub fn eliminate_field_mut(&mut self, node: NodeRef) -> Option<()> {
        let n = self.graph.get_node(node);
        let IrNodeId::FieldMut { idx, fields } = n.kind.id.clone() else {
            return Some(());
        };
        let owner = n.links.values_in[0].clone();
        let owner_kind = n.kind.values_in_types[0].clone().unwrap();
        if owner.link.is_none() {
            return Some(());
        }

        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        enum Role {
            /// `Mut<字段>` 结构本身。
            Mut,
            /// 结构里的 setter 槽(`LocalRef<字段>`)。
            Handle,
            /// 结构里的读槽(字段的视图)。
            Read,
        }

        let mut queue: VecDeque<(Link, Role)> = n
            .links
            .values_out[0]
            .iter()
            .map(|x| (*x, Role::Mut))
            .collect();
        let mut visited: HashSet<(Link, Role)> = HashSet::new();
        // 通过这个句柄写的 `SetLocal`,要换成 `Modify`。
        let mut writes: Vec<NodeRef> = vec![];
        // 读字段的边,要改接到 owner 视图的解构上。
        let mut partial_muts: Vec<Link> = vec![];
        // 句柄 / 视图经过的解构节点,最后删掉。
        let mut destructures: Vec<NodeRef> = vec![];
        while let Some((link, role)) = queue.pop_front() {
            if !visited.insert((link, role)) {
                continue;
            }
            let Some(x) = link.target.node() else {
                // 句柄或字段视图流出图外:这里没有相应机制,放弃。
                return Some(());
            };
            let nx = self.graph.get_node(x);
            match role {
                Role::Mut => {
                    // `Mut` 结构只能被解构:0 号出是句柄,1 号出是视图。
                    if nx.kind.id != IrNodeId::Destructure {
                        return Some(());
                    }
                    destructures.push(x);
                    for e in &nx.links.values_out[0] {
                        queue.push_back((*e, Role::Handle));
                    }
                    for e in &nx.links.values_out[1] {
                        queue.push_back((*e, Role::Read));
                    }
                }
                // 句柄只认「直接写」:存进变量、重新组装都留给以后的内联优化。
                Role::Handle => {
                    if nx.kind.id != IrNodeId::SetLocal || link.index != 0 {
                        return Some(());
                    }
                    writes.push(x);
                }
                // 字段视图只认「直接读」(`Copy`);其它用法说不清。
                Role::Read => {
                    if nx.kind.id != IrNodeId::Copy {
                        return Some(());
                    }
                    partial_muts.push(link);
                }
            }
        }

        // 读字段:换成 owner 视图的解构。
        if !partial_muts.is_empty() {
            let d = self.graph.insert(ir::node_ir_destructure_partial_mut(owner_kind.clone(), &fields));
            self.graph.set_value_in(Link::node(d, 0), owner.clone());
            for link in partial_muts {
                self.graph.set_value_in(link, ValueIn::link(Link::node(d, idx)));
            }
        }
        // 写:换成 `Modify`(原地改 owner 的这个字段)。
        for setter in writes {
            let sn = self.graph.get_node(setter).clone();
            let value = sn.links.values_in[1].clone();
            let mut params = vec![owner_kind.clone().into(), None];
            for field in &fields {
                params.push(field.clone().into());
                params.push(IrKind::Native(NativeKind::Bool).into());
            }
            let modify = self.graph.insert(NodeKind::new(IrNodeId::Modify, 1, 1, params, vec![]));
            self.graph.set_value_in(Link::node(modify, 0), owner.clone());
            self.graph.set_value_in(Link::node(modify, 2 + idx * 2), value);
            self.graph.set_value_in(
                Link::node(modify, 3 + idx * 2),
                ValueIn::value(true.into()),
            );
            // 控制流接替原来的 setter。
            for c in &sn.links.controls_in[0] {
                self.graph
                    .relink_control(*c, Link::node(setter, 0), &[Link::node(modify, 0)]);
            }
            for c in &sn.links.controls_out[0] {
                self.graph.link_control(Link::node(modify, 0), *c);
            }
            self.graph.remove(setter);
        }
        for d in destructures {
            self.graph.remove(d);
        }
        self.graph.remove(node);
        None
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
            return Some(());
        }
        if n.links.values_in[1].link == Some(Link::node(local, 1)) {
            // 自赋值(`local = local`):删掉它不改变值。但如果它是句柄的最后一个
            // 使用者,删完 `Local` 就只剩值用户,`eliminate_local` 会把它们改接到
            // 「初始值」引脚上 —— 普通局部变量那个引脚是空的,等于断连。
            if self.handle_would_empty(local) {
                return Some(());
            }
            self.relink_controls(node, 0, 0);
            self.graph.remove(node);
            return None;
        }

        const DEFAULT_MAX: usize = 1;
        // 变量类型同样看句柄引脚里的 `LocalRef`。
        let variable = n.kind.values_in_types[0]
            .as_ref()
            .and_then(IrKind::local_ref)
            .expect("set_local 的输入 0 应当是 LocalRef");
        // 能存成局部变量的值(普通值、列表、结构体)才有改接次数上限;
        // 视图是「通过引用写」的,多处使用都是写,不受限制。
        let max = if variable.is_storable() {
            DEFAULT_MAX
        } else {
            usize::MAX
        };

        // ---- 句柄出口:只能交给这个变量自己的 setter ----
        //
        // 句柄被当成**值**存进别的变量(`let r = &mut list`)、或者交给自定义
        // 复合节点时,它可能从别处被写 —— 值来源说不清,拒绝优化。
        let mut local_setters: HashSet<NodeRef> = HashSet::new();
        for link in n_local.links.values_out[0].iter() {
            if link.index != 0 {
                return Some(());
            }
            let Some(x) = link.target.node() else {
                return Some(());
            };
            if self.graph.get_node(x).kind.id != IrNodeId::SetLocal {
                return Some(());
            }
            local_setters.insert(x);
        }
        assert!(local_setters.contains(&node));

        let Some((usages, modifiers)) = self.collect_usages(local) else {
            // 值流经自定义复合节点:说不清,放弃。
            return Some(());
        };
        let mut rewrite: Vec<Link> = vec![];
        for usage in &usages {
            match self.classify_usage(usage, node, &local_setters, &modifiers) {
                UsageVerdict::Rewrite => rewrite.push(usage.start),
                UsageVerdict::Skip => (),
                UsageVerdict::Reject => return Some(()),
            }
        }
        if rewrite.is_empty() {
            // 没有 usage 依赖当前 setter:留着它,别把变量的唯一写者删掉。
            return Some(());
        }
        if rewrite.len() > max {
            return Some(());
        }
        // 要删的是这个变量的**最后一个** setter 吗?是的话,删完 `Local` 的句柄就
        // 没人持有了,`eliminate_local` 会把剩下的值用户改接到 `Local` 的「初始值」
        // 引脚 —— 普通局部变量那个引脚是空的,改接就等于断连。所以这种情况下
        // 所有值用户都必须能跟着改接,否则别删。
        if self.handle_would_empty(local) && rewrite.len() != n_local.links.values_out[1].len() {
            return Some(());
        }
        // 被写入的值不能依赖这个 local 的旧值 —— 否则把 usage 改接到写入的
        // 值上会造环(`i += 1` 就是这种:写入值是 `i + 1`,而 `i` 正是 usage)。
        let value = n.links.values_in[1].clone();
        if let Some(v) = value.link
            && self.value_depends_on(v, Link::node(local, 1))
        {
            return Some(());
        }
        // 改接:目标引脚要的是视图,写入值是普通值 —— 中间补一层 `ToPartialMut`。
        // 读那一侧如果紧跟着 `Copy`,`eliminate_copy` 会把两层一起消掉。
        let write_kind = n.kind.values_in_types[1].as_ref().unwrap().clone();
        let pm_kind = n_local.kind.values_out_types[1].clone();
        let value = if write_kind == pm_kind {
            value
        } else {
            let pm = self.graph.insert(ir::node_ir_to_partial_mut(write_kind));
            self.graph.set_value_in(Link::node(pm, 0), value);
            ValueIn::link(Link::node(pm, 0))
        };
        self.reset_values(rewrite, value);
        self.relink_controls(node, 0, 0);
        self.graph.remove(node);
        None
    }

    /// 数据流:局部变量值出口的每条起点边,通向的 exec 是「读」还是「写」。
    ///
    /// 从值出口向后 BFS,穿过所有纯计算节点(`Copy` 会翻转 `copied`):
    /// - 带 `copied` 到达执行节点:读值 —— user;
    /// - 没带 `copied` 到达执行节点:拿到的是视图,通过引用写 —— modifier。
    ///
    /// 自定义复合节点(`Fn`)怎么处理这个值完全说不清:返回 `None`,整个优化放弃。
    fn collect_usages(&self, local: NodeRef) -> Option<(Vec<Usage>, HashSet<LinkTarget>)> {
        let n_local = self.graph.get_node(local);
        let mut modifiers: HashSet<LinkTarget> = HashSet::new();
        let mut usages: Vec<Usage> = Vec::new();
        for &start in &n_local.links.values_out[1] {
            let mut usage = Usage {
                start,
                users: vec![],
                modifiers: vec![],
            };
            let mut queue: VecDeque<(Link, bool)> = VecDeque::new();
            let mut visited: HashSet<(Link, bool)> = HashSet::new();
            queue.push_back((start, false));
            while let Some((link, copied)) = queue.pop_front() {
                if !visited.insert((link, copied)) {
                    continue;
                }
                let Some(x) = link.target.node() else {
                    // 值流出图:也当成一个使用点,回溯时会落到图入口。
                    usage.users.push(LinkTarget::Export);
                    continue;
                };
                let nx = self.graph.get_node(x);
                if matches!(nx.kind.id, IrNodeId::Fn(_)) {
                    return None;
                }
                if nx.kind.controls_in_num == 0 {
                    // 纯计算:继续向后找执行节点。
                    let copied = copied || nx.kind.id == IrNodeId::Copy;
                    for edges in &nx.links.values_out {
                        for e in edges {
                            queue.push_back((*e, copied));
                        }
                    }
                } else if copied {
                    usage.users.push(link.target);
                } else {
                    usage.modifiers.push(link.target);
                }
            }
            modifiers.extend(usage.modifiers.iter().copied());
            usages.push(usage);
        }
        Some((usages, modifiers))
    }

    /// 一处 usage 该怎么处理。
    ///
    /// 控制流回溯:沿执行点的控制输入向前走 ——
    /// - 撞上完整赋值的 setter:值由它决定,路径到此为止;
    /// - 撞上别的 modifier:记一笔,继续向前;
    /// - 走到图入口(export)还没遇到完整赋值的 setter:来源不明;
    /// - 成环:`vis` 防重入,不再展开。
    fn classify_usage(
        &self,
        usage: &Usage,
        current: NodeRef,
        setters: &HashSet<NodeRef>,
        modifiers: &HashSet<LinkTarget>,
    ) -> UsageVerdict {
        // 同一条边既被读又被写:改接会破坏写的那一侧,这一处不动。
        if !usage.users.is_empty() && !usage.modifiers.is_empty() {
            return UsageVerdict::Skip;
        }
        if usage.users.is_empty() {
            // 只被写、没有被读:不是 usage。
            return UsageVerdict::Skip;
        }
        if usage.users.contains(&LinkTarget::Node(current)) {
            // 读者就是当前 setter:改接会造环。
            return UsageVerdict::Reject;
        }
        let mut sources: HashSet<NodeRef> = HashSet::new();
        let mut saw_other = false;
        for &user in &usage.users {
            let Ok((s, o)) = self.sources_before(user, setters, modifiers) else {
                return UsageVerdict::Reject;
            };
            sources.extend(s);
            saw_other |= o;
        }
        if !saw_other && sources == HashSet::from([current]) {
            return UsageVerdict::Rewrite;
        }
        if !saw_other && !sources.is_empty() && !sources.contains(&current) {
            // 这处 usage 读的值完全来自别的 setter,与当前 setter 无关。
            return UsageVerdict::Skip;
        }
        UsageVerdict::Reject
    }

    /// `root` 执行之前,最后一个**完整赋值**本变量的 setter 是谁。
    ///
    /// 返回值里的 `bool` 表示路径上是否经过别的 modifier(部分修改)。
    /// 走到图入口还没遇到完整赋值 setter 时返回 `Err` —— 值来源不明。
    fn sources_before(
        &self,
        root: LinkTarget,
        setters: &HashSet<NodeRef>,
        modifiers: &HashSet<LinkTarget>,
    ) -> Result<(HashSet<NodeRef>, bool), ()> {
        let mut sources = HashSet::new();
        let mut saw_other = false;
        let mut vis: HashSet<LinkTarget> = HashSet::new();
        let mut queue: VecDeque<LinkTarget> = self
            .graph
            .get_links(root)
            .controls_in
            .iter()
            .flatten()
            .map(|x| x.target)
            .collect();
        while let Some(now) = queue.pop_front() {
            if !vis.insert(now) {
                // 成环:回到已经走过的点,不再展开。
                continue;
            }
            if let Some(x) = now.node()
                && setters.contains(&x)
            {
                sources.insert(x);
                continue;
            }
            if modifiers.contains(&now) {
                saw_other = true;
            }
            let preds: Vec<LinkTarget> = self
                .graph
                .get_links(now)
                .controls_in
                .iter()
                .flatten()
                .map(|x| x.target)
                .collect();
            if preds.is_empty() {
                // 走到图入口还没遇到完整赋值的 setter:这个值从哪来说不清。
                return Err(());
            }
            queue.extend(preds);
        }
        Ok((sources, saw_other))
    }

    /// 删掉这个 `Local` 的句柄使用者之后，值用户会不会被 `eliminate_local`
    /// 改接到一个空的「初始值」引脚上（也就是断连）？
    ///
    /// 条件：这是句柄的最后一个使用者，且还有值用户，且初始值没接东西。
    fn handle_would_empty(&self, local: NodeRef) -> bool {
        let n = self.graph.get_node(local);
        n.links.values_out[0].len() == 1
            && !n.links.values_out[1].is_empty()
            && n.links.values_in[0].link.is_none()
    }
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

/// 列表 IR 节点第一个引脚的元素类型。
///
/// 那个引脚可能是视图 `PartialMut<List<T>>`(`T` 非 `Freeze`),也可能是值
/// `List<T>`(`T: Freeze`)—— 节点图里两者同形,这里都认。
fn list_input_elem(kind: &IrKind) -> IrKind {
    if let IrKind::List(elem) = kind {
        return (**elem).clone();
    }
    kind.partial_mut_inner()
        .and_then(|x| x.list_elem())
        .unwrap_or_else(|| {
            panic!("列表节点的输入应当是 List<T> 或 PartialMut<List<T>>,实际是 {kind}")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::func::NodeGraphIr;
    use crate::compile::ir::{node_ir_copy, node_ir_list_get_length, node_ir_local, node_ir_set_local, node_ir_switch};
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

    /// 句柄被当成**值**存进另一个变量时（`let r = &mut list`），它逃出了这个
    /// 变量的掌控 —— 不能把当前 setter 消掉。否则另一个变量的 setter 再被删，
    /// 句柄就彻底没主了，`eliminate_local` 会把值用户改接到空的「初始值」上，
    /// 整条值链断连（`test_list_ops` 踩过）。
    #[test]
    fn setter_is_kept_when_handle_escapes_as_a_value() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "list_ops");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));

        let list = IrKind::list(int());
        let l = g.insert(node_ir_local(list.clone()));
        // `list = ...`
        let init = g.insert(node_ir_set_local(list.clone()));
        g.link_value(Link::node(l, 0), Link::node(init, 0));
        // 读列表长度
        let len = g.insert(node_ir_list_get_length(list.clone()));
        g.link_value(Link::node(l, 1), Link::node(len, 0));
        // 另一个变量，它的值是 `list` 的句柄（引脚 1，不是句柄引脚 0）
        let other = g.insert(node_ir_local(list.clone()));
        let store = g.insert(node_ir_set_local(list.clone()));
        g.link_value(Link::node(other, 0), Link::node(store, 0));
        g.link_value(Link::node(l, 0), Link::node(store, 1));

        g.link_control(Link::export(0), Link::node(init, 0));
        g.link_control(Link::node(init, 0), Link::node(store, 0));
        g.link_control(Link::node(store, 0), Link::export(0));

        let mut opt = Optimizer {
            graph: g,
            decl: FnDecl::default(),
        };
        assert_eq!(
            opt.eliminate_unnecessary_local_setter(init),
            Some(()),
            "句柄逃到别的变量里时不该优化"
        );
        assert!(opt.graph.get_nodes().contains(&init), "setter 应当保留");
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
        // 用 `i`：读值要过 `Copy`，`switch` 是个执行点
        let copy = g.insert(node_ir_copy(int()));
        g.link_value(Link::node(i, 1), Link::node(copy, 0));
        let sw = g.insert(node_ir_switch(int(), vec![1i32.into()]));
        g.link_value(Link::node(copy, 0), Link::node(sw, 0));

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
        // 读的地方改接到写入的值上：`Copy` 的输入换成 `ToPartialMut`（写入值先标成视图）
        let src = opt.graph.get_node(copy).links.values_in[0].link.unwrap();
        let src = src.target.node().unwrap();
        assert_eq!(opt.graph.get_node(src).kind.id, IrNodeId::ToPartialMut);
        assert_eq!(opt.graph.get_node(src).links.values_in[0].link, None);
    }

    /// 只拿到视图、通过引用写的执行节点（`Modify`）不是「读」，那处 usage
    /// 不算执行点；真正读值的地方会先经过 `Copy`。
    #[test]
    fn writing_through_a_reference_is_not_an_executor() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "by_ref");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));

        let adt = IrKind::Adt("Test".into());
        let s = g.insert(node_ir_local(adt.clone()));
        // `s = 0`
        let init = g.insert(node_ir_set_local(adt.clone()));
        g.link_value(Link::node(s, 0), Link::node(init, 0));
        // `s = 1`
        let set = g.insert(node_ir_set_local(adt.clone()));
        g.link_value(Link::node(s, 0), Link::node(set, 0));
        // 通过 `s` 改字段：`Modify` 的 `value in[0]` 接的是 `s` 的值
        let modify = g.insert(NodeKind::new(
            IrNodeId::Modify,
            1,
            1,
            vec![adt.clone().into(), None],
            vec![],
        ));
        g.link_value(Link::node(s, 1), Link::node(modify, 0));
        // 读 `s`：读值要过 `Copy`
        let copy = g.insert(node_ir_copy(adt.clone()));
        g.link_value(Link::node(s, 1), Link::node(copy, 0));
        let sw = g.insert(node_ir_switch(adt.clone(), vec![]));
        g.link_value(Link::node(copy, 0), Link::node(sw, 0));

        g.link_control(Link::export(0), Link::node(init, 0));
        g.link_control(Link::node(init, 0), Link::node(set, 0));
        g.link_control(Link::node(set, 0), Link::node(sw, 0));
        g.link_control(Link::node(set, 0), Link::node(modify, 0));
        g.link_control(Link::node(sw, 0), Link::node(modify, 0));
        g.link_control(Link::node(modify, 0), Link::export(0));

        let mut opt = Optimizer {
            graph: g,
            decl: FnDecl::default(),
        };
        opt.eliminate_unnecessary_local_setter(set);

        // 写那处拿的是视图，不是 usage —— 当前 setter 照样能消掉
        assert!(
            opt.graph.get_nodes().iter().all(|x| *x != set),
            "当前 setter 应该被消掉"
        );
        // 写那处不改接
        assert_eq!(
            opt.graph.get_node(modify).links.values_in[0].link,
            Some(Link::node(s, 1))
        );
        // 读的地方改接到写入的值上（值先经 `ToPartialMut` 标成视图）
        let src = opt.graph.get_node(copy).links.values_in[0].link.unwrap();
        let src = src.target.node().unwrap();
        assert_eq!(opt.graph.get_node(src).kind.id, IrNodeId::ToPartialMut);
        assert_eq!(opt.graph.get_node(src).links.values_in[0].link, None);
    }

    /// 变量本身就是视图（`PartialMut<T>`）时，下游拿视图去写，写的是它指向
    /// 的东西，不是这个变量 —— 对这处 usage 来说仍然是「读」，setter 该消。
    ///
    /// `test_list_insert` 踩过：`&mut list` 的读槽 local 一直留着。
    #[test]
    fn view_variable_is_read_by_a_write_usage() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "view_write");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));

        let adt = IrKind::Adt("Test".into());
        let pm = IrKind::PartialMut(adt.clone().into());
        let s = g.insert(node_ir_local(pm.clone()));
        let init = g.insert(node_ir_set_local(pm.clone()));
        g.link_value(Link::node(s, 0), Link::node(init, 0));
        let set = g.insert(node_ir_set_local(pm.clone()));
        g.link_value(Link::node(s, 0), Link::node(set, 0));
        // 值出口是 `PartialMut<PartialMut<Test>>`，读一层拿到变量里存的视图
        let copy = g.insert(node_ir_copy(pm.clone()));
        g.link_value(Link::node(s, 1), Link::node(copy, 0));
        // 拿视图去改字段
        let modify = g.insert(NodeKind::new(
            IrNodeId::Modify,
            1,
            1,
            vec![adt.clone().into(), None],
            vec![],
        ));
        g.link_value(Link::node(copy, 0), Link::node(modify, 0));

        g.link_control(Link::export(0), Link::node(init, 0));
        g.link_control(Link::node(init, 0), Link::node(set, 0));
        g.link_control(Link::node(set, 0), Link::node(modify, 0));
        g.link_control(Link::node(modify, 0), Link::export(0));

        let mut opt = Optimizer {
            graph: g,
            decl: FnDecl::default(),
        };
        opt.eliminate_unnecessary_local_setter(set);

        assert!(
            opt.graph.get_nodes().iter().all(|x| *x != set),
            "视图变量的 setter 应该被消掉"
        );
        // 读视图那一层改接到写入值（标回视图）
        let src = opt.graph.get_node(copy).links.values_in[0].link.unwrap();
        assert_eq!(
            opt.graph.get_node(src.target.node().unwrap()).kind.id,
            IrNodeId::ToPartialMut
        );
    }

    /// 变量存的就是句柄（`LocalRef<T>`）时，拿它的值去写，写的是句柄指向的
    /// 变量，不是这个槽 —— 对这处 usage 来说仍然是「读」。
    ///
    /// `test_set(p: &mut i32)` 踩过：写槽 local 一直留着，最后变成一个
    /// 莫名其妙的 `LocalVarRef` 参数。
    #[test]
    fn local_ref_variable_is_read_by_a_write_usage() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "ref_write");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));
        // 图参数就是一个 `&mut i32` 的句柄
        g.push_export_value_in(ExportDecl::new(
            "p".into(),
            Some(IrKind::LocalRef(int().into())),
        ));

        // 写槽：变量存的是一个 `&mut i32` 的句柄
        let holder = g.insert(node_ir_local(IrKind::LocalRef(int().into())));
        // 从图外收到句柄
        let init = g.insert(node_ir_set_local(IrKind::LocalRef(int().into())));
        g.link_value(Link::node(holder, 0), Link::node(init, 0));
        g.link_value(Link::export(0), Link::node(init, 1));
        // 值出口是 `PartialMut<LocalRef<Int>>`，读一层拿到句柄
        let copy = g.insert(node_ir_copy(IrKind::LocalRef(int().into())));
        g.link_value(Link::node(holder, 1), Link::node(copy, 0));
        // 拿这个句柄去写
        let write = g.insert(node_ir_set_local(int()));
        g.link_value(Link::node(copy, 0), Link::node(write, 0));
        g.set_value_in(Link::node(write, 1), ValueIn::value(5i32.into()));

        g.link_control(Link::export(0), Link::node(init, 0));
        g.link_control(Link::node(init, 0), Link::node(write, 0));
        g.link_control(Link::node(write, 0), Link::export(0));

        let mut opt = Optimizer {
            graph: g,
            decl: FnDecl::default(),
        };
        opt.eliminate_unnecessary_local_setter(init);

        assert!(
            opt.graph.get_nodes().iter().all(|x| *x != init),
            "句柄的中转 setter 应该被消掉"
        );
        // 读句柄那一层改接到图外传进来的句柄（标回视图）
        let src = opt.graph.get_node(copy).links.values_in[0].link.unwrap();
        let src = src.target.node().unwrap();
        assert_eq!(opt.graph.get_node(src).kind.id, IrNodeId::ToPartialMut);
        assert_eq!(
            opt.graph.get_node(src).links.values_in[0].link,
            Some(Link::export(0))
        );
    }

    /// 值流经自定义复合节点：它会不会写、怎么读完全说不清 —— 整个优化放弃。
    #[test]
    fn function_call_rejects_optimization() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "by_fn");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));

        let pm = IrKind::PartialMut(IrKind::Adt("Test".into()).into());
        let s = g.insert(node_ir_local(pm.clone()));
        let init = g.insert(node_ir_set_local(pm.clone()));
        g.link_value(Link::node(s, 0), Link::node(init, 0));
        let set = g.insert(node_ir_set_local(pm.clone()));
        g.link_value(Link::node(s, 0), Link::node(set, 0));
        let func = g.insert(NodeKind::new(
            IrNodeId::Fn("f".into()),
            1,
            1,
            vec![pm.clone().into()],
            vec![],
        ));
        g.link_value(Link::node(s, 1), Link::node(func, 0));

        g.link_control(Link::export(0), Link::node(init, 0));
        g.link_control(Link::node(init, 0), Link::node(set, 0));
        g.link_control(Link::node(set, 0), Link::node(func, 0));
        g.link_control(Link::node(func, 0), Link::export(0));

        let mut opt = Optimizer {
            graph: g,
            decl: FnDecl::default(),
        };
        assert_eq!(
            opt.eliminate_unnecessary_local_setter(set),
            Some(()),
            "值流经自定义复合节点时不该优化"
        );
        assert!(opt.graph.get_nodes().contains(&set), "setter 应当保留");
    }

    fn field_mut_graph() -> (NodeGraphIr, NodeRef, NodeRef, NodeRef, NodeRef, NodeRef) {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "field_mut");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));

        let adt = IrKind::Adt("Point".into());
        let pm = IrKind::PartialMut(adt.clone().into());
        let owner = g.insert(node_ir_local(pm.clone()));
        let fm = g.insert(NodeKind::new(
            IrNodeId::FieldMut {
                idx: 0,
                fields: vec![int(), int()],
            },
            0,
            0,
            vec![Some(pm)],
            vec![IrKind::Mut(int().into())],
        ));
        g.link_value(Link::node(owner, 1), Link::node(fm, 0));
        // 解构 `Mut<i32>`：0 号出是句柄，1 号出是字段视图
        let d = g.insert(NodeKind::new(
            IrNodeId::Destructure,
            0,
            0,
            vec![Some(IrKind::Mut(int().into()))],
            vec![IrKind::LocalRef(int().into()), IrKind::partial_mut(int())],
        ));
        g.link_value(Link::node(fm, 0), Link::node(d, 0));
        // 通过句柄写
        let write = g.insert(node_ir_set_local(int()));
        g.link_value(Link::node(d, 0), Link::node(write, 0));
        // 读字段
        let copy = g.insert(node_ir_copy(int()));
        g.link_value(Link::node(d, 1), Link::node(copy, 0));
        let sw = g.insert(node_ir_switch(int(), vec![]));
        g.link_value(Link::node(copy, 0), Link::node(sw, 0));

        g.link_control(Link::export(0), Link::node(write, 0));
        g.link_control(Link::node(write, 0), Link::node(sw, 0));
        g.link_control(Link::node(sw, 0), Link::export(0));
        (g, owner, fm, d, write, copy)
    }

    /// `&mut s.field` 的写入换成 `Modify`，读那一侧改成解构 owner 视图。
    #[test]
    fn field_mut_write_becomes_modify() {
        let (g, owner, fm, d, write, copy) = field_mut_graph();
        let mut opt = Optimizer {
            graph: g,
            decl: FnDecl::default(),
        };
        assert_eq!(opt.eliminate_field_mut(fm), None);

        assert!(opt.graph.get_nodes().iter().all(|x| ![fm, d, write].contains(x)));
        let modify = opt
            .graph
            .get_nodes()
            .into_iter()
            .find(|x| opt.graph.get_node(*x).kind.id == IrNodeId::Modify)
            .expect("应当生成 Modify");
        let m = opt.graph.get_node(modify);
        assert_eq!(m.links.values_in[0].link, Some(Link::node(owner, 1)));
        assert_eq!(m.links.values_in[3].default, Some(true.into()));
        // 控制流接替原来的 setter
        assert!(opt.graph.get_node(modify).links.controls_out[0].len() > 0);
        // 读的地方接的是新解构节点的 0 号出
        let src = opt.graph.get_node(copy).links.values_in[0].link.unwrap();
        assert_eq!(src.index, 0);
        assert_ne!(src.target.node(), Some(d));
        assert_eq!(
            opt.graph.get_node(src.target.node().unwrap()).kind.id,
            IrNodeId::Destructure
        );
    }

    /// `Copy(ToPartialMut(x))`：两层互逆，消成直连。
    #[test]
    fn copy_after_to_partial_mut_is_elided() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "round_trip");
        let pm = g.insert(crate::compile::ir::node_ir_to_partial_mut(int()));
        let copy = g.insert(node_ir_copy(int()));
        g.link_value(Link::node(pm, 0), Link::node(copy, 0));
        let sw = g.insert(node_ir_switch(int(), vec![]));
        g.link_value(Link::node(copy, 0), Link::node(sw, 0));

        let mut opt = Optimizer {
            graph: g,
            decl: FnDecl::default(),
        };
        opt.eliminate_copy(copy);

        assert!(opt.graph.get_nodes().iter().all(|x| *x != copy));
        // 读的地方直接接回 `ToPartialMut` 的输入（这里没有连线，就是默认值）
        assert_eq!(opt.graph.get_node(sw).links.values_in[0].link, None);
    }
}
