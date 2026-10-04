//! 右栏「优化」的纯逻辑层 —— 规则表、一遍扫描、一键优化、坐标重建、撤销。
//!
//! 规则就是 `core/src/compile/ir.rs` 里 `Optimizer::eliminate_solo`(`ir.rs:402`)
//! 调用链上的每一条,顺序与它一致。单条规则的按钮**不做** `eliminate_solos`
//! 那种「循环到不动点」—— 一次点击 = 一条规则在整张图上过一遍,
//! 便于逐步观察是哪一步把图改坏的;要一步到位就点「一键优化」
//! ([`optimize_all`]:Fn 全展开 + `Optimizer::optimize()` 到不动点)。
//!
//! 这里不碰 GPUI:全部输入输出是 `Doc` / `NodeRef` / `Point<f32>`,
//! 所以能在 `cargo test` 里直接验证(右栏按钮只是这些函数的薄壳)。

use std::any::Any;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use gpui::{Point, point};
use rust2genshin::compile::ir::{IrNodeId, Optimizer};
use rust2genshin::node::NodeRef;

use crate::doc::{Doc, NODE_H, NODE_W};

/// 一条消除规则。`run` 与 `Optimizer` 的同名方法一一对应:
/// `None` = 命中(删了或改写了节点),`Some` = 这个节点没它的事。
pub struct Rule {
    pub name: &'static str,
    pub run: fn(&mut Optimizer, NodeRef) -> Option<()>,
}

/// `eliminate_solo` 调用链上的全部规则,顺序与 core 一致(`ir.rs:403-411`)。
///
/// 与 core 保持逐条镜像:core 增删了某条规则,这里跟着动
/// (测试 `rules_mirror_eliminate_solo` 会盯着这份名单)。
pub const RULES: &[Rule] = &[
    Rule {
        name: "eliminate_if",
        run: Optimizer::eliminate_if,
    },
    Rule {
        name: "eliminate_set_local",
        run: Optimizer::eliminate_set_local,
    },
    Rule {
        name: "eliminate_local",
        run: Optimizer::eliminate_local,
    },
    Rule {
        name: "eliminate_calc",
        run: Optimizer::eliminate_calc,
    },
    Rule {
        name: "eliminate_unnecessary_local_setter",
        run: Optimizer::eliminate_unnecessary_local_setter,
    },
    Rule {
        name: "eliminate_assemble",
        run: Optimizer::eliminate_assemble,
    },
    Rule {
        name: "eliminate_destructure",
        run: Optimizer::eliminate_destructure,
    },
    Rule {
        name: "eliminate_bool_eq_int",
        run: Optimizer::eliminate_bool_eq_int,
    },
    Rule {
        name: "eliminate_not_not",
        run: Optimizer::eliminate_not_not,
    },
];

/// 一遍扫描的结果。
pub struct SweepResult {
    /// 规则报告「命中」的次数(`Option::None` 的返回值个数)。
    pub hits: usize,
    pub before: usize,
    pub after: usize,
}

impl SweepResult {
    pub fn describe(&self) -> String {
        format!("命中 {} 次,节点 {} → {}", self.hits, self.before, self.after)
    }
}

/// 把一条规则在整张图的**每个节点上各跑一次**(一遍)。
///
/// 先取一份节点表再逐个跑:规则只删自己收到的那个节点(`remove(node)`),
/// 但删过之后 slab 里会留洞,后面轮到的 key 仍然有效 —— 只需跳过键已不存在的。
pub fn sweep(opt: &mut Optimizer, run: fn(&mut Optimizer, NodeRef) -> Option<()>) -> SweepResult {
    let before = opt.graph.nodes.len();
    let mut hits = 0;
    for node in opt.graph.get_nodes() {
        if !opt.graph.nodes.contains(node.into()) {
            continue;
        }
        if run(opt, node).is_none() {
            hits += 1;
        }
    }
    SweepResult {
        hits,
        before,
        after: opt.graph.nodes.len(),
    }
}

/// [`sweep`] 的 panic 护栏。
///
/// 规则里有大量 `unwrap`(如 `eliminate_if` 撞上没有条件、也没有默认值的 IF
/// 节点)。viewer 的输入是用户随手拖进来的任意 `.ogia`,不能让一次点击把
/// 整个窗口带走 —— 拦成 `Err(消息)`,由调用方回滚。
pub fn sweep_guarded(
    opt: &mut Optimizer,
    run: fn(&mut Optimizer, NodeRef) -> Option<()>,
) -> Result<SweepResult, String> {
    catch_unwind(AssertUnwindSafe(|| sweep(opt, run))).map_err(|p| panic_message(p.as_ref()))
}

/// `catch_unwind` 的负载 → 一行文案。
pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else {
        "非字符串 panic 负载".to_string()
    }
}

/// 重新展平后,给每个节点找回坐标。
///
/// `sources` 是 `(节点序列, 坐标序列)` 的对照表,**按优先级从高到低**:
/// 同一个 `NodeRef` 出现在多张表里时取第一张。撤销时当前状态在前
/// (保住优化之后用户拖过的位置),存档在后(补回被优化删掉的节点)。
///
/// 两张表里都没有的节点落到 `anchor`:
/// - `Some(p)`:落在 `p` 附近(多个新节点沿右下方向依次错开)—— link 用,
///   展开出来的节点该待在**被 link 那个节点原来的位置**;
/// - `None`:停到最右侧空列 —— 优化时新插的节点(只有 `eliminate_bool_eq_int`
///   会插一个 NOT)没有更好的落点。
pub fn rebuild_pos(
    new_refs: &[NodeRef],
    sources: &[(&[NodeRef], &[Point<f32>])],
    anchor: Option<Point<f32>>,
) -> Vec<Point<f32>> {
    let maps: Vec<HashMap<NodeRef, Point<f32>>> = sources
        .iter()
        .map(|(refs, pos)| refs.iter().copied().zip(pos.iter().copied()).collect())
        .collect();
    let max_x = maps
        .iter()
        .flat_map(|m| m.values())
        .map(|p| p.x)
        .fold(0.0_f32, f32::max);
    let mut parked = 0usize;
    new_refs
        .iter()
        .map(|r| match maps.iter().find_map(|m| m.get(r).copied()) {
            Some(p) => p,
            None => {
                let p = match anchor {
                    Some(a) => point(a.x + parked as f32 * 28.0, a.y + parked as f32 * 28.0),
                    None => point(max_x + NODE_W + 40.0, parked as f32 * (NODE_H + 16.0)),
                };
                parked += 1;
                p
            }
        })
        .collect()
}

/// 存档是哪一类操作留下的 —— 决定撤销时坐标先看哪一边。
pub enum SnapshotKind {
    /// 优化:改的是图本身。坐标以**当前**为准(保住优化之后用户拖过的位置),
    /// 存档只补回被优化删掉的节点。
    Optimize,
    /// 整理:改的就是坐标。以**存档**为准,把整理前的坐标还回来。
    Arrange,
}

/// 一次操作的存档:图的字节 + 当时的展平顺序与坐标。
/// 撤销栈里存这个;`bytes` 同时也是 panic 回滚的恢复源。
pub struct Snapshot {
    pub bytes: Vec<u8>,
    pub refs: Vec<NodeRef>,
    pub pos: Vec<Point<f32>>,
    pub kind: SnapshotKind,
}

pub struct ApplyResult {
    /// 只有当这一跳成功返回时才 `Some` —— panic 回滚的一跳不进撤销栈。
    pub undo: Option<Vec<u8>>,
    pub pos: Vec<Point<f32>>,
    pub status: String,
}

/// 点一次规则的完整数据流:
/// 存档 → 取图 → 过一遍 → 放回或回滚 → 重展平 → 按 `NodeRef` 重建坐标。
///
/// `refs` / `pos` 是点击前这一帧的展平顺序与坐标(`pos` 与 `refs` 同序)。
pub fn apply_rule(
    doc: &mut Doc,
    i: usize,
    refs: &[NodeRef],
    pos: &[Point<f32>],
    rule: &Rule,
) -> ApplyResult {
    let noop = |status: String| ApplyResult {
        undo: None,
        pos: pos.to_vec(),
        status,
    };
    let bytes = match doc.encode(i) {
        Ok(b) => b,
        Err(e) => return noop(e),
    };
    let Some(mut opt) = doc.take(i) else {
        return noop("这张图取不出来(它背后的 Target 里没有)".to_string());
    };
    match sweep_guarded(&mut opt, rule.run) {
        Ok(r) => {
            doc.put(i, opt);
            doc.reflatten(i);
            ApplyResult {
                undo: Some(bytes),
                pos: reposition(doc, i, refs, pos, None),
                status: format!("{}: {}", rule.name, r.describe()),
            }
        }
        Err(p) => {
            // 图可能停在半改状态 —— 用点击前的字节整体换回来。
            drop(opt);
            let status = match doc.decode_into(i, &bytes) {
                Ok(()) => {
                    doc.reflatten(i);
                    format!("{}: panic 已拦截,已回滚 —— {p}", rule.name)
                }
                Err(e) => format!("{}: panic 后回滚也失败了:{e}", rule.name),
            };
            ApplyResult {
                undo: None,
                pos: reposition(doc, i, refs, pos, None),
                status,
            }
        }
    }
}

/// 对一个 `IrNodeId::Fn` 节点执行 `Optimizer::link_node`:把它展开成
/// 内联的复合节点。数据流与 [`apply_rule`] 相同:存档 → 取图 → 防 panic →
/// 放回 / 回滚 → 重展平 → 重建坐标;展开出来的新节点落在**被 link 那个
/// 节点原来的位置**(`anchor`)。
///
/// 和优化规则不同的一点:`link_node` 还动 `Linker` 的**持久状态** ——
/// 被 link 的函数从 `target.functions` 移进展开缓存、复合节点进资产。
/// panic 回滚能把图换回来,但 linker 的半成品没法回滚(函数已经被搬走了)。
/// 这条路径就是 core 自己在跑的展开逻辑,panic 意味着 core 出错;
/// 真发生的话状态栏会说明,再点一次 link 会命中缓存、通常还能继续用。
pub fn link_fn_node(
    doc: &mut Doc,
    i: usize,
    refs: &[NodeRef],
    pos: &[Point<f32>],
    node: NodeRef,
) -> ApplyResult {
    let noop = |status: String| ApplyResult {
        undo: None,
        pos: pos.to_vec(),
        status,
    };
    let bytes = match doc.encode(i) {
        Ok(b) => b,
        Err(e) => return noop(e),
    };
    let Some(mut opt) = doc.take(i) else {
        return noop("这张图取不出来(它背后的 Target 里没有)".to_string());
    };
    // 从这里起,任何提前返回都必须先把图放回去。
    if !opt.graph.nodes.contains(node.into()) {
        doc.put(i, opt);
        return noop("这个节点已经不在了".to_string());
    }
    let key = match &opt.graph.get_node(node).kind.id {
        IrNodeId::Fn(key) => key.clone(),
        _ => {
            doc.put(i, opt);
            return noop("只有 Fn 节点能 link".to_string());
        }
    };
    let anchor = refs
        .iter()
        .position(|r| *r == node)
        .and_then(|k| pos.get(k).copied());
    let Some(linker) = doc.linker_mut(i) else {
        doc.put(i, opt);
        return noop("这张图背后的 Linker 不见了".to_string());
    };
    match catch_unwind(AssertUnwindSafe(|| opt.link_node(linker, node))) {
        Ok(()) => {
            doc.put(i, opt);
            doc.reflatten(i);
            ApplyResult {
                undo: Some(bytes),
                pos: reposition(doc, i, refs, pos, anchor),
                status: format!("link:{key} 已展开"),
            }
        }
        Err(p) => {
            drop(opt);
            let status = match doc.decode_into(i, &bytes) {
                Ok(()) => {
                    doc.reflatten(i);
                    format!("link:panic 已拦截,已回滚 —— {}", panic_message(p.as_ref()))
                }
                Err(e) => format!("link:panic 后回滚也失败了:{e}"),
            };
            ApplyResult {
                undo: None,
                pos: reposition(doc, i, refs, pos, None),
                status,
            }
        }
    }
}

/// 一键优化:`Optimizer::lower` 里 IR 段的**整条流水线** —— 先对**每个**
/// `IrNodeId::Fn` 节点执行 `link_node`(右键「link」的批量版),再 `optimize()`
/// (9 条消除规则跑到**不动点** + 控制边界收尾规整;右栏的单条规则按钮只过
/// "一遍",这里到不动点)。
///
/// 数据流与 [`apply_rule`] 相同:存档 → 取图 → 防 panic → 放回 / 回滚 →
/// 重展平 → 重建坐标;**整次点击只留一个存档**(一次 Ctrl+Z 全撤)。
/// 展开 / 消除会冒出很多新节点、没有单一锚点,统一落到最右列停放位
/// (`anchor = None`),和规则按钮一致;要排布再点「自动整理」。
/// 没有任何变化时**不压撤销栈**(空档点了也白点),状态栏会说明。
///
/// 和 [`link_fn_node`] 同一处注意:`link_node` 还动 `Linker` 的持久状态
/// (展开缓存 / 资产),panic 回滚换不回它 —— 这条路径就是 core 自己在跑的
/// 流程,panic 意味着 core 出错,状态栏会说明。
pub fn optimize_all(
    doc: &mut Doc,
    i: usize,
    refs: &[NodeRef],
    pos: &[Point<f32>],
) -> ApplyResult {
    let noop = |status: String| ApplyResult {
        undo: None,
        pos: pos.to_vec(),
        status,
    };
    let bytes = match doc.encode(i) {
        Ok(b) => b,
        Err(e) => return noop(e),
    };
    let Some(mut opt) = doc.take(i) else {
        return noop("这张图取不出来(它背后的 Target 里没有)".to_string());
    };
    let Some(linker) = doc.linker_mut(i) else {
        doc.put(i, opt);
        return noop("这张图背后的 Linker 不见了".to_string());
    };
    let fns = opt
        .graph
        .get_nodes()
        .iter()
        .filter(|x| matches!(opt.graph.get_node(**x).kind.id, IrNodeId::Fn(_)))
        .count();
    // 展开 + 规则链整段都在护栏里 —— 中途任何一步 panic 都用点击前的字节
    // 整体回滚(`optimize()` 尾部对 `controls_in[0]` 的索引是可能 panic 的
    // 点之一,见 core 里的注释)。
    let result = catch_unwind(AssertUnwindSafe(|| {
        for x in opt.graph.get_nodes() {
            opt.link_node(linker, x);
        }
        opt.optimize();
    }));
    match result {
        Ok(()) => {
            doc.put(i, opt);
            doc.reflatten(i);
            // 没变化就别骗用户"优化了" —— 第二次点击通常就是这种情形
            // (规则已到不动点、没有 Fn 可展开)。
            let changed = matches!(doc.encode(i), Ok(b) if b != bytes);
            let undo = if changed { Some(bytes) } else { None };
            ApplyResult {
                undo,
                pos: reposition(doc, i, refs, pos, None),
                status: if changed {
                    format!("一键优化:展开 {fns} 个 Fn 节点 + 消除规则到不动点")
                } else {
                    "一键优化:无变化(已是优化后的形态)".to_string()
                },
            }
        }
        Err(p) => {
            drop(opt);
            let status = match doc.decode_into(i, &bytes) {
                Ok(()) => {
                    doc.reflatten(i);
                    format!(
                        "一键优化:panic 已拦截,已回滚 —— {}",
                        panic_message(p.as_ref())
                    )
                }
                Err(e) => format!("一键优化:panic 后回滚也失败了:{e}"),
            };
            ApplyResult {
                undo: None,
                pos: reposition(doc, i, refs, pos, None),
                status,
            }
        }
    }
}

/// 撤销一步:把存档换回去,再看 [`SnapshotKind`] 决定坐标以哪边为准。
pub fn undo(
    doc: &mut Doc,
    i: usize,
    refs: &[NodeRef],
    pos: &[Point<f32>],
    entry: &Snapshot,
) -> (Vec<Point<f32>>, String) {
    if let Err(e) = doc.decode_into(i, &entry.bytes) {
        return (pos.to_vec(), format!("撤销失败:{e}"));
    }
    doc.reflatten(i);
    let Some(g) = doc.graphs.get(i) else {
        return (Vec::new(), "撤销失败:图不见了".to_string());
    };
    let new_refs: Vec<NodeRef> = g.nodes.iter().map(|n| n.node_ref).collect();
    let current = (refs, pos);
    let saved = (entry.refs.as_slice(), entry.pos.as_slice());
    let sources = match entry.kind {
        SnapshotKind::Optimize => [current, saved],
        SnapshotKind::Arrange => [saved, current],
    };
    (rebuild_pos(&new_refs, &sources, None), "已撤销".to_string())
}

fn reposition(
    doc: &Doc,
    i: usize,
    refs: &[NodeRef],
    pos: &[Point<f32>],
    anchor: Option<Point<f32>>,
) -> Vec<Point<f32>> {
    let Some(g) = doc.graphs.get(i) else {
        return Vec::new();
    };
    let new_refs: Vec<NodeRef> = g.nodes.iter().map(|n| n.node_ref).collect();
    rebuild_pos(&new_refs, &[(refs, pos)], anchor)
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust2genshin::compile::func::{NodeGraphIr, node_ir_native};
    use rust2genshin::compile::ir::{FnInfo, IrKind, node_ir_local, node_ir_set_local};
    use rust2genshin::compile::link::Target;
    use rust2genshin::node::control::NODE_IF;
    use rust2genshin::node::{ExportDecl, Link, NodeGraphKind, ValueIn};
    use rust2genshin::value::NativeKind;

    use crate::doc::flatten;

    fn int() -> IrKind {
        IrKind::Native(NativeKind::Int)
    }

    fn target_with(g: NodeGraphIr) -> Target {
        let mut t = Target::default();
        t.functions.insert(
            "f".into(),
            FnInfo {
                description: String::new(),
                graph: g,
                decl: Default::default(),
                exported: false,
            },
        );
        t
    }

    fn rule_named(name: &str) -> &'static Rule {
        RULES
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("规则 {name} 不存在"))
    }

    /// 视图坐标:直接从展平快照读(`push_target` 已让 core 的布局写过 IR 坐标,
    /// viewer 不再自己算)。
    fn positions(doc: &crate::doc::Doc, gi: usize) -> Vec<Point<f32>> {
        doc.graphs[gi].nodes.iter().map(|n| n.position).collect()
    }

    fn refs_of(doc: &Doc, i: usize) -> Vec<NodeRef> {
        doc.graphs[i].nodes.iter().map(|n| n.node_ref).collect()
    }

    /// 两个独立、**都该被消掉**的 SetLocal 链:
    ///
    /// ```text
    /// export -> s1 -> s3 -> export
    /// l0[0] -> s1[0]      l2[0] -> s3[0]
    /// ```
    ///
    /// `l0` / `l2` 的值出口(`values_out[1]`)没人连 —— 变量没人读,
    /// 写它就毫无意义,`eliminate_set_local` 的定义正是消这种 setter。
    fn two_setters() -> (NodeGraphIr, NodeRef, NodeRef, NodeRef, NodeRef) {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "two_setters");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));
        let l0 = g.insert(node_ir_local(&int()));
        let s1 = g.insert(node_ir_set_local(&int()));
        let l2 = g.insert(node_ir_local(&int()));
        let s3 = g.insert(node_ir_set_local(&int()));
        g.link_value(Link::node(l0, 0), Link::node(s1, 0));
        g.link_value(Link::node(l2, 0), Link::node(s3, 0));
        g.link_control(Link::export(0), Link::node(s1, 0));
        g.link_control(Link::node(s1, 0), Link::node(s3, 0));
        g.link_control(Link::node(s3, 0), Link::export(0));
        (g, l0, s1, l2, s3)
    }

    /// 按钮 = `eliminate_solo` 调用链上的方法,一条不能少、顺序一致。
    /// core 改了那条链,这份名单和上面的 `RULES` 都得跟着动。
    #[test]
    fn rules_mirror_eliminate_solo() {
        let names: Vec<&str> = RULES.iter().map(|r| r.name).collect();
        assert_eq!(
            names,
            vec![
                "eliminate_if",
                "eliminate_set_local",
                "eliminate_local",
                "eliminate_calc",
                "eliminate_unnecessary_local_setter",
                "eliminate_assemble",
                "eliminate_destructure",
                "eliminate_bool_eq_int",
                "eliminate_not_not",
            ]
        );
    }

    /// 一键 = 一 **遍**:同一条规则要在这一遍里把**所有**候选都处理掉。
    /// 只处理第一个就收手的实现在这里失败。
    #[test]
    fn sweep_eliminates_every_candidate_in_one_pass() {
        let (g, l0, s1, l2, s3) = two_setters();
        let mut doc = flatten(target_with(g));
        let mut opt = doc.take(0).expect("取出图");
        let r = sweep(&mut opt, rule_named("eliminate_set_local").run);
        doc.put(0, opt);
        doc.reflatten(0);

        assert_eq!(r.hits, 2, "两个 SetLocal 都该消掉:{}", r.describe());
        assert_eq!(r.after, r.before - 2, "节点数应恰好少 2");
        let refs = refs_of(&doc, 0);
        assert!(!refs.contains(&s1) && !refs.contains(&s3), "s1 / s3 应当被删");
        assert!(refs.contains(&l0) && refs.contains(&l2), "Local 不是这条规则的菜");
    }

    /// 前提不满足时必须放过:变量的值出口还有人读(`values_out[1]` 非空)时,
    /// 这个 setter 不是死写,不能消。
    #[test]
    fn sweep_leaves_non_candidates_alone() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "read_after_write");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));
        let l0 = g.insert(node_ir_local(&int()));
        let s1 = g.insert(node_ir_set_local(&int()));
        // l0 的值出口连到 s1 的值入口 => l0.values_out[1] 非空 => s1 不可消
        g.link_value(Link::node(l0, 0), Link::node(s1, 0));
        g.link_value(Link::node(l0, 1), Link::node(s1, 1));
        g.link_control(Link::export(0), Link::node(s1, 0));
        g.link_control(Link::node(s1, 0), Link::export(0));

        let mut doc = flatten(target_with(g));
        let mut opt = doc.take(0).expect("取出图");
        let r = sweep(&mut opt, rule_named("eliminate_set_local").run);
        doc.put(0, opt);
        doc.reflatten(0);

        assert_eq!(r.hits, 0, "s1 的值出口被读,不该被消:{}", r.describe());
        assert_eq!(r.after, r.before);
        assert!(refs_of(&doc, 0).contains(&l0));
    }

    /// 用户要求:**优化后已有的节点坐标不变**。存活节点的坐标必须逐一相等
    /// (不是"差不多")—— 重算布局的实现会在这里失败。
    #[test]
    fn apply_rule_keeps_existing_node_coordinates() {
        let (g, l0, _s1, l2, _s3) = two_setters();
        let mut doc = flatten(target_with(g));
        let pos0 = positions(&doc, 0);
        let refs0 = refs_of(&doc, 0);

        let r = apply_rule(&mut doc, 0, &refs0, &pos0, rule_named("eliminate_set_local"));
        assert!(r.undo.is_some(), "成功的这一跳应当留下撤销存档:{}", r.status);
        assert!(r.status.contains("命中 2 次"), "状态栏:{}", r.status);

        let g1 = &doc.graphs[0];
        assert_eq!(g1.nodes.len(), refs0.len() - 2);
        for (k, r0) in refs0.iter().enumerate() {
            if let Some(&j) = g1.index_of.get(r0) {
                assert_eq!(r.pos[j], pos0[k], "节点 {r0:?} 的坐标不该变");
            }
        }
        assert!(g1.index_of.contains_key(&l0) && g1.index_of.contains_key(&l2));
    }

    /// 撤销:被优化删掉的节点带着**原来的**坐标回来;存活节点保留优化之后
    /// 用户拖到的位置。只认存档的实现会丢拖动,只认当前的实现会让回来的
    /// 节点丢坐标 —— 两边都得看。
    #[test]
    fn undo_brings_back_removed_nodes_at_their_old_positions() {
        let (g, l0, s1, _l2, s3) = two_setters();
        let mut doc = flatten(target_with(g));
        let pos0 = positions(&doc, 0);
        let refs0 = refs_of(&doc, 0);

        let r = apply_rule(&mut doc, 0, &refs0, &pos0, rule_named("eliminate_set_local"));
        let entry = Snapshot {
            bytes: r.undo.expect("成功的跳有存档"),
            refs: refs0.clone(),
            pos: pos0.clone(),
            kind: SnapshotKind::Optimize,
        };
        let mut pos1 = r.pos;
        let refs1 = refs_of(&doc, 0);
        // 优化之后拖一下 l0:撤销不该把这次拖动一起撤掉
        let l0_idx = refs1.iter().position(|x| *x == l0).expect("l0 还在");
        pos1[l0_idx] = point(1234.0, 567.0);

        let (pos2, status) = undo(&mut doc, 0, &refs1, &pos1, &entry);
        assert_eq!(status, "已撤销");

        let g2 = &doc.graphs[0];
        let refs2 = refs_of(&doc, 0);
        assert_eq!(refs2, refs0, "撤销后节点集合(含顺序)应当与优化前一致");
        for (k, r0) in refs0.iter().enumerate() {
            let expect = if *r0 == l0 {
                point(1234.0, 567.0)
            } else {
                pos0[k]
            };
            let j = g2.index_of[r0];
            assert_eq!(pos2[j], expect, "节点 {r0:?} 的坐标");
        }
        assert!(g2.index_of.contains_key(&s1) && g2.index_of.contains_key(&s3));
    }

    /// 诊断:把 `R2G_OPT=<路径>` 指向的文件逐图、逐规则各跑一遍,打印结果。
    /// 不开窗口就能看见 9 条规则在**真实**图上的命中 / panic。跑法:
    ///
    /// ```text
    /// R2G_OPT=target/genshin-unknown-server/release/librust2genshin_demo.rlib \
    ///   cargo test -p rust2genshin-viewer --lib -- --nocapture optimize_a_file
    /// ```
    ///
    /// 每条规则都在**新解析的一份**文件上跑(规则会改图,不能共用);
    /// 图名只从第一份里取一次。
    #[test]
    fn optimize_a_file_from_env() {
        let Ok(path) = std::env::var("R2G_OPT") else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        let names: Vec<String> = match crate::doc::load(&path) {
            Ok(t) => crate::doc::flatten(t)
                .graphs
                .into_iter()
                .map(|g| g.name)
                .collect(),
            Err(e) => {
                println!("解析失败: {e}");
                return;
            }
        };
        for gi in 0..names.len() {
            for rule in RULES {
                let Ok(t) = crate::doc::load(&path) else {
                    return;
                };
                let mut doc = crate::doc::flatten(t);
                let refs: Vec<NodeRef> = doc.graphs[gi].nodes.iter().map(|n| n.node_ref).collect();
                let pos = positions(&doc, gi);
                let r = apply_rule(&mut doc, gi, &refs, &pos, rule);
                println!("{:>24} / {:<38} {}", names[gi], rule.name, r.status);
            }
        }
    }

    /// 撤销「自动整理」:坐标要回到整理**前**(存档优先),而不是保留整理后的
    /// —— 只认当前的实现会在这里失败。
    #[test]
    fn undoing_an_arrange_restores_the_old_coordinates() {
        let (g, _l0, _s1, _l2, _s3) = two_setters();
        let mut doc = flatten(target_with(g));
        let refs = refs_of(&doc, 0);
        let old_pos = positions(&doc, 0);
        // 模拟点完「自动整理」之后的坐标:整张图被挪到了别处
        let arranged: Vec<Point<f32>> = old_pos
            .iter()
            .map(|p| point(p.x + 500.0, p.y + 300.0))
            .collect();
        let entry = Snapshot {
            bytes: doc.encode(0).expect("编码"),
            refs: refs.clone(),
            pos: old_pos.clone(),
            kind: SnapshotKind::Arrange,
        };

        let (pos, status) = undo(&mut doc, 0, &refs, &arranged, &entry);
        assert_eq!(status, "已撤销");
        assert_eq!(pos, old_pos, "撤销整理应把坐标还给整理前");
    }

    /// 规则里的 `unwrap` 撞上不满足前提的节点会 panic —— 必须拦下、回滚到
    /// 点击前,且**不进撤销栈**(这一跳没有成功)。
    ///
    /// 图中特意让 panic 发生在一个成功的消除**之后**:不靠存档回滚的实现
    /// 会留下那一半改动,这里就会对不上。
    #[test]
    fn panicking_sweep_is_caught_and_rolled_back() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "boom");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));
        // 0:IF,条件有默认值 => eliminate_if 会把它消掉
        let if_ok = g.insert(node_ir_native(NODE_IF.clone()));
        g.get_node_mut(if_ok).links.values_in[0] = ValueIn::value(true.into());
        // 1:SetLocal 垫在中间
        let s = g.insert(node_ir_set_local(&int()));
        // 2:IF,条件既没连线也没默认值 => eliminate_if 在这里 unwrap 到 None
        let if_bad = g.insert(node_ir_native(NODE_IF.clone()));
        g.link_control(Link::export(0), Link::node(if_ok, 0));
        g.link_control(Link::node(if_ok, 0), Link::node(s, 0));
        g.link_control(Link::node(s, 0), Link::node(if_bad, 0));
        g.link_control(Link::node(if_bad, 0), Link::export(0));

        let mut doc = flatten(target_with(g));
        let pos0 = positions(&doc, 0);
        let refs0 = refs_of(&doc, 0);
        let edges0 = doc.graphs[0].edges.len();

        let r = apply_rule(&mut doc, 0, &refs0, &pos0, rule_named("eliminate_if"));
        assert!(r.undo.is_none(), "panic 的一跳不该进撤销栈:{}", r.status);
        assert!(r.status.contains("panic"), "状态栏应说明 panic:{}", r.status);
        assert_eq!(refs_of(&doc, 0), refs0, "图必须整体回滚 —— if_ok 也不能少");
        assert_eq!(doc.graphs[0].edges.len(), edges0, "边也应回到点击前");
        assert_eq!(r.pos, pos0, "坐标不该动");
    }

    /// link:`Fn` 节点被展开成低层的复合原生节点 —— Fn 没了、多出一个
    /// Native,数量不变,而且新节点落在**被 link 节点的原位**(不是最右列)。
    #[test]
    fn link_expands_a_fn_node_in_place() {
        // 测试图提到 `doc::fixture` 了(view 的右键菜单测试也用同一份)。
        let t = crate::doc::fixture::linkable_target();

        let mut doc = flatten(t);
        let gi = doc
            .graphs
            .iter()
            .position(|g| g.name == "caller")
            .expect("caller 图存在");
        let refs = refs_of(&doc, gi);
        let pos = positions(&doc, gi);
        let fnn_ref = doc.graphs[gi]
            .nodes
            .iter()
            .find(|n| matches!(n.kind, IrNodeId::Fn(_)))
            .expect("有 Fn 节点")
            .node_ref;
        let old_pos = pos[refs.iter().position(|r| *r == fnn_ref).unwrap()];

        let r = link_fn_node(&mut doc, gi, &refs, &pos, fnn_ref);
        assert!(r.undo.is_some(), "成功的 link 应留下撤销存档:{}", r.status);
        assert!(r.status.contains("已展开"), "状态栏:{}", r.status);

        let g = &doc.graphs[gi];
        assert!(!g.index_of.contains_key(&fnn_ref), "Fn 节点应被替换");
        assert_eq!(g.nodes.len(), refs.len(), "换成低层节点,数量不变");
        let new_node = g
            .nodes
            .iter()
            .find(|n| matches!(n.kind, IrNodeId::Native(_)))
            .expect("多出一个复合原生节点");
        let new_pos = r.pos[g.index_of[&new_node.node_ref]];
        assert_eq!(new_pos, old_pos, "展开出来的节点应落在被 link 节点的原位");
    }

    /// 一键优化:Fn 全部展开 + 规则到不动点,**整次点击一个存档** ——
    /// 一次撤销回到点击前(Fn 节点带着原图回来)。
    #[test]
    fn optimize_all_expands_all_fn_nodes_and_undoes_in_one_step() {
        let t = crate::doc::fixture::linkable_target();

        let mut doc = flatten(t);
        let gi = doc
            .graphs
            .iter()
            .position(|g| g.name == "caller")
            .expect("caller 图存在");
        let refs = refs_of(&doc, gi);
        let pos = positions(&doc, gi);
        assert!(
            doc.graphs[gi]
                .nodes
                .iter()
                .any(|n| matches!(n.kind, IrNodeId::Fn(_))),
            "前提:点击前有 Fn 节点"
        );

        let r = optimize_all(&mut doc, gi, &refs, &pos);
        let bytes = r.undo.expect("有变化就必须留下撤销存档");
        assert!(
            !doc.graphs[gi]
                .nodes
                .iter()
                .any(|n| matches!(n.kind, IrNodeId::Fn(_))),
            "Fn 节点应当已展开:{}",
            r.status
        );
        assert!(r.status.contains("展开 1 个"), "状态栏:{}", r.status);

        // 一次 Ctrl+Z 全撤 —— 存档是点击前那一份;节点集合(含 Fn)逐一回来。
        // (节点**数量**不锁:规则链同时在跑,展开加节点、消除减节点都合法。)
        doc.decode_into(gi, &bytes).expect("回滚");
        doc.reflatten(gi);
        assert!(
            doc.graphs[gi]
                .nodes
                .iter()
                .any(|n| matches!(n.kind, IrNodeId::Fn(_))),
            "撤销后 Fn 节点必须回来"
        );
        let mut before = refs.clone();
        let mut after = refs_of(&doc, gi);
        before.sort_unstable_by_key(|r| usize::from(*r));
        after.sort_unstable_by_key(|r| usize::from(*r));
        assert_eq!(before, after, "撤销后节点集合必须与点击前一致");
    }

    /// 一键优化**到不动点**:第二次点击没有变化 —— 不压撤销栈、状态栏说明。
    /// (没有这条测试,把 `optimize()` 换成单遍 `eliminate_solos()` 不会有
    /// 任何测试变红;而 core 的完整流程就是到不动点。)
    #[test]
    fn optimize_all_is_a_noop_the_second_time() {
        let t = crate::doc::fixture::linkable_target();
        let mut doc = flatten(t);
        let gi = doc
            .graphs
            .iter()
            .position(|g| g.name == "caller")
            .expect("caller 图存在");

        let (refs1, pos1) = (refs_of(&doc, gi), positions(&doc, gi));
        let r1 = optimize_all(&mut doc, gi, &refs1, &pos1);
        assert!(r1.undo.is_some(), "第一次应当有变化:{}", r1.status);

        let (refs2, pos2) = (refs_of(&doc, gi), positions(&doc, gi));
        let r2 = optimize_all(&mut doc, gi, &refs2, &pos2);
        assert!(r2.undo.is_none(), "第二次不该进撤销栈:{}", r2.status);
        assert!(r2.status.contains("无变化"), "状态栏:{}", r2.status);
    }

    /// 一键优化对**没有 Fn 的图**同样是有效的规则流程:纯规则链被消掉。
    /// (只有 link 的实现在这里会"无变化"。)
    #[test]
    fn optimize_all_runs_rules_even_without_fn_nodes() {
        let (g, _l0, s1, _l2, s3) = two_setters();
        let mut doc = flatten(target_with(g));
        let refs = refs_of(&doc, 0);
        let pos = positions(&doc, 0);
        assert!(!doc.graphs[0].nodes.iter().any(|n| matches!(n.kind, IrNodeId::Fn(_))));

        let r = optimize_all(&mut doc, 0, &refs, &pos);
        assert!(r.undo.is_some(), "两个 setter 应被消掉:{}", r.status);
        let remaining: Vec<NodeRef> = refs_of(&doc, 0);
        assert!(!remaining.contains(&s1) && !remaining.contains(&s3), "setter 们该没了");
        assert!(r.status.contains("展开 0 个"), "状态栏:{}", r.status);
    }
}

