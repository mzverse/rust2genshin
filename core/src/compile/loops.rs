//! 把 IR 里的控制流环变成 native 的「有限循环」节点。
//!
//! IR（rustc 的 MIR 翻译过来的图）允许控制流有环；native 节点图不允许，
//! 循环得用「有限循环」节点（ID 5）表达：控制从 in[0] 进入，循环体挂在
//! out[0]，运行时每一轮重新驱动 out[0]；跳出用「跳出循环」节点（ID 6）
//! 接到有限循环的 in[1]。
//!
//! ## 区域分解
//!
//! 判定与分解用可约流图（reducible flow graph）的标准做法：DFS 找回边 →
//! 支配关系判可约 → 自然环 → 区域树。一个区域 = 一个 Rust 循环。
//!
//! ## 退出路径
//!
//! 一个区域只有一个退出目标时，什么都不用分派：跳出节点直接连到目标，
//! 不生成退出码变量、不生成 switch、不生成置码的 setter。
//!
//! 有多个退出目标时，每个区域配一个 int 局部变量（退出码）和一个紧随其
//! 后的 switch：每个退出目标一个码，跳出点先 `set(码)` 再 `break`，循环
//! 结束后由 switch 分派到真正的目标。跨层的跳出靠串联：内层 switch 的分支
//! 指向「设置外层的码并跳出外层」，一路串到最外层。
//!
//! 全部串联，没有并联：每个控制出端口至多一条连接。
//!
//! 循环体的「一轮结束」由运行时判定：链走到没有后继的地方就是一轮结束，
//! 运行时重新驱动 out[0] 进入下一轮。所以 IR 里的回边（含 `continue`）在
//! native 图里直接删掉 —— 那一轮的链到那儿就断了。

use crate::compile::func::{node_ir_native, NodeGraphIr};
use crate::compile::ir::{node_ir_local, node_ir_set_local, node_ir_switch, IrKind, IrNodeId, NodeKindIr};
use crate::node::arithmetic::{node_add, node_less_equal, node_less_than, node_subtract};
use crate::node::control::{NODE_BREAK, NODE_FOR_CLOSED};
use crate::node::{Link, LinkTarget, NodeRef, ValueIn};
use crate::value::{NativeKind, NativeValue};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// 死循环用的区间。有限循环是闭区间，每次加一，所以 i32::MAX 走不到 ——
/// 循环靠「跳出循环」结束，不会自然结束。
const DEAD_LOOP_BEGIN: i32 = i32::MIN;
const DEAD_LOOP_END: i32 = i32::MAX;

/// CFG 里的点。`Export` 是函数出口（伪点），其余是图里的节点。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Point {
    Export,
    Node(NodeRef),
}

/// 一条控制边。`out` 是源点的控制出端口号，`in_port` 是目标点的控制入端口号。
#[derive(Clone, Copy, Debug)]
struct Edge {
    from: usize,
    out: usize,
    to: usize,
    in_port: usize,
}

/// 控制流图。点编号 0 是 export，其余按节点顺序排。
struct Cfg {
    points: Vec<Point>,
    succ: Vec<Vec<usize>>,
    pred: Vec<Vec<usize>>,
    edges: Vec<Edge>,
    /// 函数入口：externals 的控制出端口 0 的目标（点号 + 目标点的控制入端口号）。
    /// 函数被调用时从这儿进。
    entry: Vec<(usize, usize)>,
    /// 节点 → 点号。
    node_index: HashMap<NodeRef, usize>,
}

impl Cfg {
    fn build(graph: &NodeGraphIr) -> Cfg {
        let mut points = vec![Point::Export];
        let mut index: HashMap<Point, usize> = HashMap::new();
        index.insert(Point::Export, 0);
        for n in graph.get_nodes() {
            index.insert(Point::Node(n), points.len());
            points.push(Point::Node(n));
        }
        let mut edges = Vec::new();
        for n in graph.get_nodes() {
            let from = index[&Point::Node(n)];
            let node = graph.get_node(n);
            for (out, targets) in node.links.controls_out.iter().enumerate() {
                for t in targets {
                    let to = match t.target {
                        LinkTarget::Export => 0,
                        LinkTarget::Node(m) => index[&Point::Node(m)],
                    };
                    edges.push(Edge {
                        from,
                        out,
                        to,
                        in_port: t.index,
                    });
                }
            }
        }
        let mut succ = vec![Vec::new(); points.len()];
        let mut pred = vec![Vec::new(); points.len()];
        for e in &edges {
            succ[e.from].push(e.to);
            pred[e.to].push(e.from);
        }
        // externals 的出边是「函数被调用」那一次，不属于 CFG 的内部边，
        // 只当根区域的入口。
        let mut entry = Vec::new();
        if let Some(targets) = graph.externals.controls_out.first() {
            for t in targets {
                let to = match t.target {
                    LinkTarget::Export => 0,
                    LinkTarget::Node(m) => index[&Point::Node(m)],
                };
                entry.push((to, t.index));
            }
        }
        Cfg {
            points,
            succ,
            pred,
            edges,
            entry,
            node_index: index
                .iter()
                .filter_map(|(p, i)| match p {
                    Point::Node(n) => Some((*n, *i)),
                    Point::Export => None,
                })
                .collect(),
        }
    }
}

/// DFS 树。`disc` 是发现序，`parent` 是树上的父亲。
struct Dfs {
    disc: Vec<usize>,
    parent: Vec<usize>,
    order: Vec<usize>,
}

impl Dfs {
    fn run(cfg: &Cfg, root: usize) -> Dfs {
        let mut dfs = Dfs {
            disc: vec![usize::MAX; cfg.points.len()],
            parent: vec![usize::MAX; cfg.points.len()],
            order: Vec::new(),
        };
        // 显式栈，深图也不会爆栈。
        let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
        dfs.disc[root] = 0;
        dfs.order.push(root);
        while !stack.is_empty() {
            let (v, i) = *stack.last().unwrap();
            if i < cfg.succ[v].len() {
                stack.last_mut().unwrap().1 += 1;
                let w = cfg.succ[v][i];
                if dfs.disc[w] == usize::MAX {
                    dfs.disc[w] = dfs.order.len();
                    dfs.order.push(w);
                    dfs.parent[w] = v;
                    stack.push((w, 0));
                }
            } else {
                stack.pop();
            }
        }
        dfs
    }

    /// `a` 是不是 `b` 的祖先（含 `a == b`）。
    fn is_ancestor(&self, a: usize, b: usize) -> bool {
        let mut x = b;
        while x != usize::MAX {
            if x == a {
                return true;
            }
            x = self.parent[x];
        }
        false
    }
}

/// 支配集合。函数级图很小，直接用集合迭代法，不上 Lengauer–Tarjan。
fn dominators(cfg: &Cfg, order: &[usize]) -> HashMap<usize, HashSet<usize>> {
    let all: HashSet<usize> = order.iter().copied().collect();
    let mut dom: HashMap<usize, HashSet<usize>> = HashMap::new();
    for &n in order {
        dom.insert(
            n,
            if n == order[0] {
                HashSet::from([n])
            } else {
                all.clone()
            },
        );
    }
    loop {
        let mut changed = false;
        for &n in order.iter().skip(1) {
            let preds: Vec<usize> = cfg.pred[n]
                .iter()
                .copied()
                .filter(|p| dom.contains_key(p))
                .collect();
            let new: HashSet<usize> = match preds.split_first() {
                None => HashSet::from([n]),
                Some((first, rest)) => {
                    let mut acc = dom[first].clone();
                    for p in rest {
                        acc = acc.intersection(&dom[p]).copied().collect();
                    }
                    acc.insert(n);
                    acc
                }
            };
            if new != dom[&n] {
                dom.insert(n, new);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    dom
}

/// 自然环：回边 `tail -> head` 圈住的点。`head` 自己算在里面。
fn natural_loop(cfg: &Cfg, head: usize, tail: usize) -> HashSet<usize> {
    // tail 自己也在环里（长度为 0 的路径也算）。
    let mut set: HashSet<usize> = HashSet::from([head, tail]);
    let mut seen: HashSet<usize> = HashSet::from([tail]);
    let mut stack = vec![tail];
    while let Some(x) = stack.pop() {
        for &p in &cfg.pred[x] {
            if p == head {
                continue;
            }
            if seen.insert(p) {
                set.insert(p);
                stack.push(p);
            }
        }
    }
    set
}

/// 一个区域 = 一个 Rust 循环。
struct Region {
    /// 区域里的点。
    nodes: HashSet<usize>,
    /// 区域的头（循环入口）。
    head: usize,
    parent: Option<usize>,
    children: Vec<usize>,
    /// 有没有回边指向自己的头 —— 没有就不是循环（只有根区域可能是这样）。
    is_loop: bool,
}

/// 区域分解的结果。
struct Regions {
    regions: Vec<Region>,
    /// 每个点所属的最内层区域。
    owner: Vec<usize>,
    /// 头 → 区域。根区域的头是函数入口。
    head_region: HashMap<usize, usize>,
    /// 支配集合：点 → 支配它的点。
    dom: HashMap<usize, HashSet<usize>>,
}

impl Regions {
    /// 点所属的最内层区域。
    fn owner_of(&self, p: usize) -> usize {
        self.owner[p]
    }
}

/// 区域分解。可约性判定 + 自然环 + 区域树。
fn analyze(cfg: &Cfg) -> Result<Regions, String> {
    let n = cfg.points.len();
    let empty = Regions {
        regions: Vec::new(),
        owner: vec![0; n],
        head_region: HashMap::new(),
        dom: HashMap::new(),
    };
    if cfg.entry.is_empty() {
        return Ok(empty);
    }
    if cfg.entry.len() > 1 {
        return Err("函数入口有多个目标，区域分解要求单入口".into());
    }
    let root_point = cfg.entry[0].0;
    let dfs = Dfs::run(cfg, root_point);
    let dom = dominators(cfg, &dfs.order);

    // 回边 + 可约性判定：回边的头必须支配尾，否则是多入口的环，native 表达不了。
    let mut back_edges = Vec::new();
    for e in &cfg.edges {
        // 不可达的点不参与区域分解（MIR 里可能有没有前驱的块）。
        let Some(from_dom) = dom.get(&e.from) else {
            continue;
        };
        if dfs.is_ancestor(e.to, e.from) {
            if !from_dom.contains(&e.to) {
                return Err(format!(
                    "不可约的控制流：点 {} 到点 {} 的回边，头不支配尾",
                    e.from, e.to
                ));
            }
            back_edges.push(*e);
        }
    }

    // 自然环，按头合并 —— 同一个头的所有回边合成一个区域。
    let mut loops: HashMap<usize, HashSet<usize>> = HashMap::new();
    for e in &back_edges {
        loops
            .entry(e.to)
            .or_default()
            .extend(natural_loop(cfg, e.to, e.from));
    }

    // 区域列表：0 是根区域（全部可达点），其余是自然环。
    let mut regions: Vec<Region> = vec![Region {
        nodes: dfs.order.iter().copied().collect(),
        head: root_point,
        parent: None,
        children: Vec::new(),
        is_loop: loops.contains_key(&root_point),
    }];
    let mut heads: Vec<usize> = loops.keys().copied().filter(|h| *h != root_point).collect();
    heads.sort();
    for h in heads {
        regions.push(Region {
            nodes: loops[&h].clone(),
            head: h,
            parent: None,
            children: Vec::new(),
            is_loop: true,
        });
    }

    // 区域必须两两不交或互相包含（可约图的自然环天然满足，检查一下别让它悄悄错）。
    for i in 0..regions.len() {
        for j in i + 1..regions.len() {
            let (a, b) = (&regions[i].nodes, &regions[j].nodes);
            if a.intersection(b).count() == 0 {
                continue;
            }
            if !(a.is_subset(b) || b.is_subset(a)) {
                return Err(format!("区域 {} 与 {} 相交但互不包含", i, j));
            }
        }
    }

    // 父子关系：父区域是严格包含它、且包含得最紧的那个。
    for i in 1..regions.len() {
        let mut best: Option<usize> = None;
        for j in 0..regions.len() {
            if i == j {
                continue;
            }
            if regions[i].nodes.is_subset(&regions[j].nodes)
                && regions[i].nodes.len() < regions[j].nodes.len()
                && best.is_none_or(|b| regions[j].nodes.len() < regions[b].nodes.len())
            {
                best = Some(j);
            }
        }
        regions[i].parent = best;
    }
    for i in 1..regions.len() {
        if let Some(p) = regions[i].parent {
            regions[p].children.push(i);
        }
    }

    // 每个点归最内层的区域。
    let mut owner = vec![usize::MAX; n];
    for &p in &dfs.order {
        let mut best = 0usize;
        for (i, r) in regions.iter().enumerate() {
            if r.nodes.contains(&p) && r.nodes.len() < regions[best].nodes.len() {
                best = i;
            }
        }
        owner[p] = best;
    }

    // 单入口检查：区域外的边只能落在区域的头上。
    for (i, r) in regions.iter().enumerate() {
        if i == 0 {
            continue;
        }
        for e in &cfg.edges {
            if r.nodes.contains(&e.to) && !r.nodes.contains(&e.from) && e.to != r.head {
                return Err(format!(
                    "区域 {} 有多个入口：点 {} 也能进到区域里（头是 {}）",
                    i, e.from, r.head
                ));
            }
        }
    }

    let mut head_region = HashMap::new();
    for (i, r) in regions.iter().enumerate() {
        head_region.insert(r.head, i);
    }
    Ok(Regions {
        regions,
        owner,
        head_region,
        dom,
    })
}

/// 一个区域的改写计划：循环怎么表达、退出往哪走。
///
/// 建节点时一次填好，后面接线、改边、摘节点都只读它 —— 不再有
/// 「半成品状态槽」。
struct RegionPlan {
    /// 循环节点怎么驱动。
    mode: LoopMode,
    /// 退出目标（点号）→ 退出码。按目标点号排序，码从 1 开始。
    exits: BTreeMap<usize, i32>,
    /// 有限循环节点。
    loop_node: NodeRef,
    /// 多出口时的分派 switch。
    dispatch: Option<NodeRef>,
    /// 退出目标 →「置码 + 跳出」链的入口。自然结束不在表里。
    chain: BTreeMap<usize, NodeRef>,
    /// 进循环前把退出码设成「正常结束」那个码的节点。
    pre_set: Option<NodeRef>,
    /// 特化时折叠掉的拷贝写入（`tmp = 当前值` 那种），以及它们写的
    /// 局部变量。值已经改接到有限循环的出参，这些节点稍后统一删。
    folded: Vec<(NodeRef, NodeRef)>,
}

impl RegionPlan {
    /// 计数循环自然结束的目标（点号）。
    fn natural_exit(&self) -> Option<usize> {
        match &self.mode {
            LoopMode::Counting(c) => Some(c.natural_exit),
            LoopMode::Conditional => None,
        }
    }

    /// 这个退出目标要不要一条「置码 + 跳出」链。自然结束不用：
    /// 它靠区间耗尽，出口是有限循环的 out[1]。
    fn needs_chain(&self, target: usize) -> bool {
        self.natural_exit() != Some(target)
    }

    /// 进循环的控制入口。有预置退出码的节点时先经过它。
    fn entry(&self) -> Link {
        match self.pre_set {
            Some(set) => Link::node(set, 0),
            None => Link::node(self.loop_node, 0),
        }
    }
}

/// 循环节点怎么驱动。
enum LoopMode {
    /// 认不出循环规律：区间取满，靠条件跳出。循环体入口就是区域的头。
    Conditional,
    /// 计数循环：区间是真的，循环自己数到头。
    Counting(CountingLoop),
}

/// 认出来的计数循环。
struct CountingLoop {
    /// 循环头：那个 bool 的 switch，改写完要摘掉。
    head: NodeRef,
    /// 比较节点，改写完要摘掉。
    cmp: NodeRef,
    /// 循环体的起点（原来比较的 true 分支）。
    body: NodeRef,
    /// 正常结束的目标（原来比较的 false 分支，点号）。
    natural_exit: usize,
    /// 区间起点。
    begin: ValueIn,
    /// 区间终点怎么算。
    end: EndSpec,
    /// 循环变量的局部变量节点。
    var: NodeRef,
    /// 循环变量的初值写入（循环外）。
    init: NodeRef,
    /// 循环内的加一写入。
    inc: NodeRef,
    /// 加一的 `+` 节点。
    add: NodeRef,
    /// 循环体实际读的那个局部变量。`for ... in range` 会多一层拷贝：
    /// 加一作用在原变量上，体里读的是拷出来的那个。
    read: NodeRef,
    /// 拷贝的写入节点（有拷贝时才有），连同 `read` 一起要摘掉。
    copy: Option<NodeRef>,
}

/// 区间终点的算法。检测阶段只看图，不建节点；建节点时再落成值。
#[derive(Clone)]
enum EndSpec {
    /// 常量终点。
    Const(i32),
    /// 直接引用那个值（`i <= bound`）。
    Link(Link),
    /// 引用值减一（`i < bound` 对应 `[begin, bound - 1]`）。
    MinusOne(Link),
}

/// 边在改造后的分类。
#[derive(Debug)]
enum EdgeAction {
    /// 区域内部的边，不动。
    Keep,
    /// 回边 / `continue`：删掉，这一轮的链到这儿就断。
    Drop,
    /// 区域入口：改接到有限循环的 in[0]。
    Entry { region: usize },
    /// 退出：改接到「置码 + 跳出」链。
    Exit { region: usize, target: usize },
}

/// 把图里的控制流环改写成有限循环节点。
///
/// 分四段：认出每个区域是什么循环 → 给边分类、收集退出目标 →
/// 由外向内建节点 → 全局接线（分派、入口、改边、摘节点）。
pub fn lower_loops(graph: &mut NodeGraphIr) -> Result<(), String> {
    check_control_outs(graph)?;
    let cfg = Cfg::build(graph);
    let regions = analyze(&cfg)?;
    if regions.regions.iter().all(|r| !r.is_loop) {
        // 没有环，native 图本来就合法。
        return Ok(());
    }

    // 1. 认出每个循环区域是什么循环。只读图，不改图。
    let mut modes: Vec<Option<LoopMode>> = (0..regions.regions.len())
        .map(|_| None)
        .collect();
    for (i, r) in regions.regions.iter().enumerate() {
        if r.is_loop {
            modes[i] = Some(match detect_counting_loop(graph, &cfg, &regions, i) {
                Some(c) => LoopMode::Counting(c),
                None => LoopMode::Conditional,
            });
        }
    }

    // 2. 给每条边定分类。
    let actions: Vec<EdgeAction> = cfg.edges.iter().map(|e| classify(&regions, e)).collect();

    // 3. 收集每个区域的退出目标，分配退出码。
    let mut exits = collect_exits(&cfg, &regions, &actions);

    // 4. 建节点。由外向内，外层先把「跳出外层」的链接备好。
    let mut order: Vec<usize> = (0..regions.regions.len()).collect();
    order.sort_by_key(|i| std::cmp::Reverse(depth(&regions, *i)));
    let mut plans: Vec<Option<RegionPlan>> = (0..regions.regions.len())
        .map(|_| None)
        .collect();
    for i in order {
        let Some(mode) = modes[i].take() else {
            continue;
        };
        let exits = std::mem::take(&mut exits[i]);
        plans[i] = Some(build_region(graph, &cfg, &regions, i, mode, exits)?);
    }

    // 5. 接线：循环的 out[1] 接退出目标 —— 单出口直接接，多出口经 switch。
    //    `continue 外层` 的目标是个祖先区域的头，那种分支要接到「链尾」——
    //    链在那里断掉，外层就进入下一轮。链尾节点只在真需要时建。
    let need_dead_end = plans.iter().enumerate().any(|(i, plan)| {
        plan.as_ref().is_some_and(|plan| {
            plan.exits.keys().any(|v| {
                regions
                    .head_region
                    .get(v)
                    .is_some_and(|a| *a != i && regions.regions[i].parent == Some(*a))
            })
        })
    });
    let dead_end = if need_dead_end {
        Some(graph.insert(crate::compile::ir::node_ir_unreachable()))
    } else {
        None
    };
    for (i, plan) in plans.iter().enumerate() {
        let Some(plan) = plan else {
            continue;
        };
        if plan.exits.is_empty() {
            continue;
        }
        match plan.dispatch {
            // 多个退出目标：switch 的第 k 支对第 k 个码，最后一支是 default。
            Some(sw) => {
                for (k, target) in plan.exits.keys().enumerate() {
                    let to = case_target(&cfg, &regions, &plans, i, *target, dead_end)?;
                    graph.link_control(Link::node(sw, k), to);
                }
            }
            // 只有一个退出目标：没有 switch，out[1] 直接接目标。
            None => {
                let target = *plan.exits.keys().next().unwrap();
                let to = case_target(&cfg, &regions, &plans, i, target, dead_end)?;
                graph.link_control(Link::node(plan.loop_node, 1), to);
            }
        }
    }

    // 6. 改边。
    for (e, a) in cfg.edges.iter().zip(&actions) {
        let from = Link::node(point_node(&cfg, e.from), e.out);
        let to = match cfg.points[e.to] {
            Point::Export => Link::export(e.in_port),
            Point::Node(n) => Link::node(n, e.in_port),
        };
        match a {
            EdgeAction::Keep => (),
            EdgeAction::Drop => graph.unlink_control(from, to),
            EdgeAction::Entry { region } => {
                graph.unlink_control(from, to);
                let entry = plans[*region].as_ref().unwrap().entry();
                graph.link_control(from, entry);
            }
            EdgeAction::Exit { region, target } => {
                graph.unlink_control(from, to);
                let plan = plans[*region].as_ref().unwrap();
                match plan.chain.get(target) {
                    Some(chain) => graph.link_control(from, Link::node(*chain, 0)),
                    // 自然结束：靠区间耗尽，这条边随循环头的分支一起消失。
                    None => assert_eq!(
                        plan.natural_exit(),
                        Some(*target),
                        "区域 {region} 缺退出目标 {target} 的链，它又不是正常结束"
                    ),
                }
            }
        }
    }

    // 7. 根区域的入口边（函数被调用时的那一条）改接到根的有限循环。
    if let Some(plan) = &plans[0] {
        let head = plan.entry();
        for (to, in_port) in &cfg.entry {
            let from = Link::export(0);
            let to = match cfg.points[*to] {
                Point::Export => Link::export(*in_port),
                Point::Node(n) => Link::node(n, *in_port),
            };
            graph.unlink_control(from, to);
            graph.link_control(from, head);
        }
    }

    // 8. 摘掉计数循环里多余的东西：
    //    - 比较和分支：区间已经说明了循环从哪数到哪，条件恒真；
    //    - 循环变量那套：初值写入、每轮的 `i += 1`（含 `+` 节点）、
    //      `for ... in range` 的拷贝写入 —— 循环体现在直接读有限循环的
    //      出参，这些都不再有消费者。
    for plan in plans.iter().flatten() {
        let LoopMode::Counting(c) = &plan.mode else {
            continue;
        };
        // 折叠掉的拷贝：值已经改接到出参，这里把节点摘掉。
        for (set, local) in &plan.folded {
            bypass_control(graph, *set);
            if graph.get_node(*local).links.is_empty() {
                graph.remove(*local);
            }
        }
        // SetLocal 在控制流上，要用旁路的方式摘（前驱直接接后继）。
        bypass_control(graph, c.inc);
        if let Some(set) = c.copy {
            bypass_control(graph, set);
        }
        bypass_control(graph, c.init);
        // 加一是纯计算节点，没有控制流。
        graph.remove(c.add);
        graph.remove(c.head);
        graph.remove(c.cmp);
        // 变量本身没有别的消费者就一起删；还有（参数、导出）就留着。
        for n in [Some(c.var), c.copy.map(|_| c.read)].into_iter().flatten() {
            if graph.get_node(n).links.is_empty() {
                graph.remove(n);
            }
        }
    }

    check_control_outs(graph)?;
    check_no_control_cycle(graph)?;
    Ok(())
}

/// 区域 `i` 的深度（根是 0）。
fn depth(regions: &Regions, i: usize) -> usize {
    let mut d = 0;
    let mut cur = regions.regions[i].parent;
    while let Some(p) = cur {
        d += 1;
        cur = regions.regions[p].parent;
    }
    d
}

/// 边该归哪一类。
fn classify(regions: &Regions, e: &Edge) -> EdgeAction {
    let owner = regions.owner_of(e.from);
    // 不可达的点不归任何区域，它的边原样保留。
    if owner == usize::MAX {
        return EdgeAction::Keep;
    }
    // 目标是某个区域的头？那看是入口、回边还是退出。
    if let Some(r) = regions.head_region.get(&e.to) {
        let r = *r;
        if regions.regions[r].nodes.contains(&e.from) {
            // 区域内部的边指向自己的头：回边或 continue。
            if owner == r {
                EdgeAction::Drop
            } else {
                // continue 外层：跳出内层，落到外层的头上。
                EdgeAction::Exit { region: owner, target: e.to }
            }
        } else {
            EdgeAction::Entry { region: r }
        }
    } else if e.to == 0 {
        // 函数出口是伪点，不属于任何区域。从循环体里出去（return）才算
        // 退出；根区域（不是循环）本来就该直接接出口。
        if regions.regions[owner].is_loop {
            EdgeAction::Exit { region: owner, target: e.to }
        } else {
            EdgeAction::Keep
        }
    } else {
        let owner_v = regions.owner_of(e.to);
        if owner_v == owner {
            EdgeAction::Keep
        } else {
            EdgeAction::Exit { region: owner, target: e.to }
        }
    }
}

/// 每个区域的退出目标 → 退出码。
///
/// 一条边可能同时跨好几层（比如内层循环里 return 到了最外层之外），
/// 中间每一层都要记着这个目标：它得为这个目标建一条「置码 + 跳出」
/// 链，内层的分派才能一层层往上抛。只记最内层的话，中间层不会
/// 被 break，循环会接着跑下一轮。
///
/// 码从 1 开始：局部变量的初值 0 不与任何 case 相等，走 default
/// （default 不连任何东西）。
fn collect_exits(
    cfg: &Cfg,
    regions: &Regions,
    actions: &[EdgeAction],
) -> Vec<BTreeMap<usize, i32>> {
    let mut targets: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); regions.regions.len()];
    for (_e, a) in cfg.edges.iter().zip(actions) {
        let EdgeAction::Exit { region, target } = a else {
            continue;
        };
        let mut cur = *region;
        loop {
            targets[cur].insert(*target);
            match regions.regions[cur].parent {
                Some(p) if !regions.regions[p].nodes.contains(target) => cur = p,
                _ => break,
            }
        }
    }
    targets
        .into_iter()
        .map(|set| {
            set.into_iter()
                .enumerate()
                .map(|(k, v)| (v, k as i32 + 1))
                .collect()
        })
        .collect()
}

/// 建一个区域的节点：有限循环、退出码局部变量、switch、「置码 + 跳出」链。
///
/// `mode` 说循环怎么驱动，`exits` 是退出目标 → 退出码。返回的计划
/// 是后续接线、改边唯一的依据。
fn build_region(
    graph: &mut NodeGraphIr,
    cfg: &Cfg,
    regions: &Regions,
    i: usize,
    mode: LoopMode,
    exits: BTreeMap<usize, i32>,
) -> Result<RegionPlan, String> {
    let head_node = point_node(cfg, regions.regions[i].head);
    let head_links = graph.get_node(head_node).links.controls_in.len();
    if head_links != 1 {
        return Err(format!(
            "区域 {} 的头有 {} 个控制入口，native 节点只支持一个",
            i, head_links
        ));
    }

    // 有限循环。认不出循环规律时区间取满，只靠条件跳出。
    let lp = graph.insert(node_ir_native(NODE_FOR_CLOSED.clone()));
    // 计数循环特化时折叠掉的拷贝（见 `fold_copies`），建完 plan 再挂上去。
    let mut folded = Vec::new();
    let body = match &mode {
        LoopMode::Conditional => {
            graph.set_default(Link::node(lp, 0), Some(DEAD_LOOP_BEGIN.into()));
            graph.set_default(Link::node(lp, 1), Some(DEAD_LOOP_END.into()));
            head_node
        }
        LoopMode::Counting(c) => {
            let begin = c.begin.clone();
            let end = materialize_end(graph, &c.end);
            graph.set_value_in(Link::node(lp, 0), begin);
            graph.set_value_in(Link::node(lp, 1), end);
            // 有限循环节点自己就是循环变量：体里读它的出参，原来那套
            // 「局部变量 + 每轮加一」整个摘掉。
            replace_value_source(graph, Link::node(c.read, 1), Link::node(lp, 0));

            folded = fold_copies(graph, cfg, regions, i, lp);

            // 循环体入口可能正好是要摘的节点（体里只有 `i += 1`，或者
            // `for` 的拷贝就在体首）：体从它的控制后继开始。
            let mut body = c.body;
            let mut hops = 0;
            while hops < 4 && (body == c.inc || c.copy == Some(body)) {
                let next = graph.get_node(body).links.controls_out[0]
                    .first()
                    .and_then(|l| l.target.node())
                    .filter(|n| *n != c.head && *n != c.cmp);
                match next {
                    Some(n) => {
                        body = n;
                        hops += 1;
                    }
                    None => break,
                }
            }
            if body == c.inc || c.copy == Some(body) {
                // 体里只有这些节点：摘掉之后循环体是空的。挂个 nop 兜住
                // out[0] —— 每轮仍要被驱动一下，然后一轮结束。
                body = graph.insert(crate::compile::ir::node_ir_nop());
            }
            // 真正删节点放到改边之后（第 8 步）：改边要遍历原始 CFG 的边，
            // 那些边还连着这些节点，提前删会让 unlink 撞上不存在的 key。
            // 比较和分支摘掉之后，循环体从比较的 true 那支进。
            body
        }
    };
    graph.link_control(Link::node(lp, 0), Link::node(body, 0));

    let mut plan = RegionPlan {
        mode,
        exits,
        loop_node: lp,
        dispatch: None,
        chain: BTreeMap::new(),
        pre_set: None,
        folded,
    };

    // 没有退出目标（`loop {}`）：out[1] 悬空，不需要退出码和分派。
    if plan.exits.is_empty() {
        return Ok(plan);
    }
    // native switch 的分支数上限。
    if plan.exits.len() > 10 {
        return Err(format!(
            "区域 {} 有 {} 条退出路径，超过 switch 上限（10）",
            i,
            plan.exits.len()
        ));
    }

    if plan.exits.len() > 1 {
        // 多个退出目标：一个退出码局部变量 + 循环之后的 switch 按码分派。
        let local = graph.insert(node_ir_local(IrKind::Native(NativeKind::Int)));
        let values: Vec<Box<dyn NativeValue>> =
            plan.exits.values().map(|code| (*code).into()).collect();
        let sw = graph.insert(node_ir_switch(IrKind::Native(NativeKind::Int), values));
        graph.set_value_in(Link::node(sw, 0), ValueIn::link(Link::node(local, 1)));
        graph.link_control(Link::node(lp, 1), Link::node(sw, 0));
        plan.dispatch = Some(sw);

        // 每个需要「置码 + 跳出」的退出目标一条链。
        for (&target, &code) in &plan.exits {
            if !plan.needs_chain(target) {
                continue;
            }
            let set = graph.insert(node_ir_set_local(IrKind::Native(NativeKind::Int)));
            graph.set_value_in(Link::node(set, 0), ValueIn::link(Link::node(local, 0)));
            graph.set_value_in(Link::node(set, 1), ValueIn::value(code.into()));
            let brk = graph.insert(node_ir_native(NODE_BREAK.clone()));
            graph.link_control(Link::node(set, 0), Link::node(brk, 0));
            graph.link_control(Link::node(brk, 0), Link::node(lp, 1));
            plan.chain.insert(target, set);
        }

        // 进循环前把码设成「正常结束」那个。循环自然跑完时码没变，
        // 分派 switch 就落到正常结束那支，不落到 default。
        if let Some(code) = plan.natural_exit().and_then(|natural| plan.exits.get(&natural).copied()) {
            let set = graph.insert(node_ir_set_local(IrKind::Native(NativeKind::Int)));
            graph.set_value_in(Link::node(set, 0), ValueIn::link(Link::node(local, 0)));
            graph.set_value_in(Link::node(set, 1), ValueIn::value(code.into()));
            graph.link_control(Link::node(set, 0), Link::node(lp, 0));
            plan.pre_set = Some(set);
        }
    } else {
        // 只有一处退出目标：不用分派，也不生成退出码变量和 switch。
        // 跳出节点直接接在有限循环的 in[1] 上，out[1] 就是那个目标。
        let target = *plan.exits.keys().next().unwrap();
        if plan.needs_chain(target) {
            let brk = graph.insert(node_ir_native(NODE_BREAK.clone()));
            graph.link_control(Link::node(brk, 0), Link::node(lp, 1));
            plan.chain.insert(target, brk);
        }
    }
    Ok(plan)
}

/// 从 `node` 的值出口顺着值流看，有没有消费者落在区域外。
///
/// 控制节点判断它是否属于区域；纯值节点（没有控制流）不在区域集合里，
/// 递归看它自己的消费者。图外的导出也算逃逸。
fn value_escapes_region(
    graph: &NodeGraphIr,
    cfg: &Cfg,
    region: &Region,
    node: NodeRef,
    seen: &mut HashSet<NodeRef>,
) -> bool {
    if !seen.insert(node) {
        return false;
    }
    let n = graph.get_node(node);
    // `Local` 有两个值出口：0 号是写句柄、1 号才是读值。只看读值 ——
    // 写句柄会喂给 SetLocal，那不是「谁读了这个值」。
    let outs: Vec<&Vec<Link>> = if n.kind.id == IrNodeId::Local {
        vec![&n.links.values_out[1]]
    } else {
        n.links.values_out.iter().collect()
    };
    for outs in outs {
        for link in outs {
            let Some(m) = link.target.node() else {
                return true; // 导出到图外
            };
            let inside = cfg
                .node_index
                .get(&m)
                .is_some_and(|idx| region.nodes.contains(idx));
            if inside {
                continue;
            }
            let m_node = graph.get_node(m);
            if m_node.kind.is_calc() {
                if value_escapes_region(graph, cfg, region, m, seen) {
                    return true;
                }
            } else {
                return true;
            }
        }
    }
    false
}

/// 把「每轮把有限循环的当前值写进一个临时局部变量」这种拷贝折叠掉。
///
/// `for r in a..b` 的迭代变量在 MIR 里常不止一层：循环变量加一，体里再
/// 拷一份出来给业务代码读。特化之后这些拷贝的值就是有限循环的出参，
/// 留着只是每轮多一次写入。折叠的条件：
/// - 写入的值就是 `lp` 的出参；
/// - 该局部变量全图只被这一处写；
/// - 它的读取者都在本区域内（区域外读到的是循环结束后的值，语义不同）。
///
/// 返回 (拷贝写入, 局部变量)，由调用方在改边之后统一摘。
fn fold_copies(
    graph: &mut NodeGraphIr,
    cfg: &Cfg,
    regions: &Regions,
    i: usize,
    lp: NodeRef,
) -> Vec<(NodeRef, NodeRef)> {
    let region = &regions.regions[i];
    let mut folded = Vec::new();
    // 折叠可能级联（拷贝再拷贝），跑到不动点。
    loop {
        let mut changed = false;
        for n in graph.get_nodes() {
            let node = graph.get_node(n);
            if node.kind.id != IrNodeId::SetLocal {
                continue;
            }
            if node.links.values_in[1].link != Some(Link::node(lp, 0)) {
                continue;
            }
            let Some(LinkTarget::Node(local)) = node.links.values_in[0].link.map(|l| l.target)
            else {
                continue;
            };

            // 只被这一处写，否则别处写的值会被我们替换掉。
            if local_writers(graph, local) != [n] {
                continue;
            }
            // 已经没有读取者了：说明这一轮已经折过它（读值都改接到出参
            // 了），再折就是不动点循环 —— 必须在这里停。
            if graph.get_node(local).links.values_out[1].is_empty() {
                continue;
            }
            // 读取者不能逃出本区域：区域外读到的是循环结束后的值。
            // 纯值节点（乘法、参数计算）不在控制流区域里，要顺着值流
            // 继续往下看它最终喂给了谁。
            if value_escapes_region(graph, cfg, region, local, &mut HashSet::new()) {
                continue;
            }
            replace_value_source(graph, Link::node(local, 1), Link::node(lp, 0));
            folded.push((n, local));
            changed = true;
        }
        if !changed {
            return folded;
        }
    }
}

/// 删掉一个控制流上的节点，把它旁路掉：每个前驱直接接到它的后继。
///
/// 计数循环里被摘的 `SetLocal`（初值 / 加一 / 拷贝）都是 1 进 1 出的，
/// 删它们不能只是 `remove` —— 那样控制流在中间就断了（初值写入正好在
/// 循环入口前，断掉之后有限循环没有入口）。
fn bypass_control(graph: &mut NodeGraphIr, n: NodeRef) {
    let node = graph.get_node(n);
    let ins = node.links.controls_in[0].clone();
    let outs = node.links.controls_out[0].clone();
    graph.remove(n);
    for from in ins {
        for to in &outs {
            graph.link_control(from, *to);
        }
    }
}

/// 把 `from` 的所有值消费者改接到 `to`。
///
/// 值边在两端各记一次：先解开旧的那条，再接新的。
fn replace_value_source(graph: &mut NodeGraphIr, from: Link, to: Link) {
    let LinkTarget::Node(n) = from.target else {
        return;
    };
    let consumers = graph.get_node(n).links.values_out[from.index].clone();
    for c in consumers {
        graph.unlink_value(from, c);
        graph.link_value(to, c);
    }
}

/// 把区间终点落成值：常量直接用，`<` 的引用值插一个减法节点算 `- 1`。
fn materialize_end(graph: &mut NodeGraphIr, end: &EndSpec) -> ValueIn {
    match end {
        EndSpec::Const(v) => ValueIn::value((*v).into()),
        EndSpec::Link(l) => ValueIn::link(*l),
        EndSpec::MinusOne(l) => {
            let sub = graph.insert(node_ir_native(node_subtract(NativeKind::Int)));
            graph.set_value_in(Link::node(sub, 0), ValueIn::link(*l));
            graph.set_value_in(Link::node(sub, 1), ValueIn::value(1i32.into()));
            ValueIn::link(Link::node(sub, 0))
        }
    }
}

/// 写 `local` 的那些节点。`local` 是局部变量节点，写它的入口是它的
/// 第 0 个值出口（`LocalRef::setter`）。
fn local_writers(graph: &NodeGraphIr, local: NodeRef) -> Vec<NodeRef> {
    graph
        .get_nodes()
        .into_iter()
        .filter(|n| {
            let node = graph.get_node(*n);
            node.kind.id == IrNodeId::SetLocal
                && node.links.values_in[0].link == Some(Link::node(local, 0))
        })
        .collect()
}

/// `a` 支配 `b` 吗 —— 从入口到 `b` 的每条路径都经过 `a`。
fn dominates(regions: &Regions, cfg: &Cfg, a: NodeRef, b: NodeRef) -> bool {
    let Some(d) = regions.dom.get(&cfg.node_index[&b]) else {
        return false;
    };
    d.contains(&cfg.node_index[&a])
}

/// 这个值在循环里不变吗：不依赖循环变量，也不依赖循环里被改的局部变量。
fn is_invariant(
    graph: &NodeGraphIr,
    cfg: &Cfg,
    regions: &Regions,
    i: usize,
    var: NodeRef,
    node: NodeRef,
    seen: &mut HashSet<NodeRef>,
) -> bool {
    if !seen.insert(node) {
        // 值流里有环，追不出底。
        return false;
    }
    let n = graph.get_node(node);
    match &n.kind.id {
        IrNodeId::Local => {
            if node == var {
                return false;
            }
            // 循环里没被改过就是不变的。
            !local_writers(graph, node)
                .iter()
                .any(|w| regions.regions[i].nodes.contains(&cfg.node_index[w]))
        }
        _ if n.kind.is_calc() => n.links.values_in.iter().all(|v| match &v.link {
            Some(l) => match l.target.node() {
                Some(m) => is_invariant(graph, cfg, regions, i, var, m, seen),
                // 函数入口的值在循环里不变。
                None => true,
            },
            None => true,
        }),
        _ => false,
    }
}

/// 认出「`i` 从 begin 数到 bound，每轮加一」这种计数循环。
///
/// 条件（都在 IR 图上看），每条都对应一个不能错的不变量：
///
/// - 区域的头是 bool 的 switch，它的值来自 `<` / `<=` 比较节点；
///   —— 比较的结果决定走哪支，摘掉它就得保证那支真的是「循环体」。
/// - 比较的第一个操作数是循环变量（局部变量），第二个是上界；
///   —— 循环变量是 native 区间要数的那个数，上界是区间的终点。
/// - 比较节点只被这个头用；
///   —— 摘掉它才干净；被别人用就摘不掉，那就不特化。
/// - 循环变量全图只被写两次：循环外一次（初值，且支配循环头）、循环内一次（加一）；
///   —— 只有这两处写，才能说清每轮它怎么变、初值是多少。
/// - 循环内那次写的是「循环变量 + 1」（可以经一层拷贝，拷贝得在加一之前）；
///   —— 每轮恰好加一，native 的计数才跟得上循环变量。
/// - 每一条回到循环头的边都被加一支配；
///   —— 「连回循环开始的边都在 `i += 1` 之后」。`for` 的 `continue`
///     执行更新部分（rustc 把自增提到体首），`while` 的 `continue`
///     跳过更新部分 —— 后者 `i` 不增长，native 的计数会多跑，不能特化。
/// - 上界在循环里不变；
///   —— 区间的终点得是固定的值。
/// - 循环体在循环里，正常结束在循环外；
///   —— 循环体跑出循环、或者区间耗尽后跳回循环体内部，语义就全错了。
///
/// 认出来就把区间填进有限循环节点：循环于是自己数到头，不用靠条件跳出。
///
/// 任何一条不满足都退回通用改写（区间取满、靠条件跳出）—— 那条路是
/// 改动之前就有的，行为一定没变。特化只在这里「能证明等价」时才发生。
fn detect_counting_loop(
    graph: &mut NodeGraphIr,
    cfg: &Cfg,
    regions: &Regions,
    i: usize,
) -> Option<CountingLoop> {
    let region = &regions.regions[i];
    let head_node = point_node(cfg, region.head);
    let head = graph.get_node(head_node);

    // 头是 bool 的 switch：true 那支是循环体，false 那支是跳出。
    let IrNodeId::Switch(values) = &head.kind.id else {
        return None;
    };
    if !matches!(
        head.kind.values_in_types[0],
        Some(IrKind::Native(NativeKind::Bool))
    ) {
        return None;
    }
    if values.len() != 1 && values.len() != 2 {
        return None;
    }
    // case 列表里可能有 false、可能有 true，也可能只有其中一个 ——
    // 缺的那个走 default 支。两种来源的约定相反：
    // - MIR 的 `switchInt(bool)`：case 是 [false]，所以 false 在第 0 支；
    // - `if` 语句来的：case 是 [true]，所以 true 在第 0 支。
    // 两种都得认，否则一半的循环头会被漏掉。
    let mut false_port = None;
    let mut true_port = None;
    for (k, v) in values.iter().enumerate() {
        match v.downcast_ref::<bool>().ok()? {
            false => false_port = Some(k),
            true => true_port = Some(k),
        }
    }
    // 缺的那个走 default 支（在 case 列表之后）。
    let false_port = false_port.unwrap_or(values.len());
    let true_port = true_port.unwrap_or(values.len());
    let body = head.links.controls_out[true_port].first()?.target.node()?;
    // 「正常结束」是区间耗尽时自然落到的位置 —— 多半是循环之后的代码，
    // 也可能就是函数出口（循环是函数体的最后一句）。
    let natural_exit = match head.links.controls_out[false_port].first()?.target {
        LinkTarget::Node(n) => cfg.node_index[&n],
        LinkTarget::Export => 0,
    };
    // 循环体得在循环里，正常结束得在循环外。
    //
    // 这两条是防误优化的底线：循环体跑出循环、或者区间耗尽后跳回循环
    // 体内部，语义就全错了。正常结束可以是别的区域的头（`continue 外层`
    // 的形状），那由 `case_target` 接到链尾，不算错。
    if !region.nodes.contains(&cfg.node_index[&body]) {
        return None;
    }
    if region.nodes.contains(&natural_exit) {
        return None;
    }

    // 头的值是比较的结果。
    let cmp_link = head.links.values_in[0].link?;
    let cmp_node = cmp_link.target.node()?;
    if cmp_link.index != 0 {
        return None;
    }
    // 比较只被这个头用，摘掉它才干净。
    if graph.get_node(cmp_node).links.values_out[0].len() != 1 {
        return None;
    }
    let cmp = graph.get_node(cmp_node);
    let cmp_id = cmp.kind.id.as_native()?;
    let inclusive = if *cmp_id == node_less_equal(NativeKind::Int).id {
        true
    } else if *cmp_id == node_less_than(NativeKind::Int).id {
        false
    } else {
        return None;
    };

    // 比较的第一个操作数是循环变量，第二个是上界。
    let var_link = cmp.links.values_in[0].link?;
    let var = var_link.target.node()?;
    if var_link.index != 1 || graph.get_node(var).kind.id != IrNodeId::Local {
        return None;
    }
    let bound = cmp.links.values_in[1].clone();

    // 循环变量全图只被写两次：循环外的初值、循环内的加一。
    let writers = local_writers(graph, var);
    if writers.len() != 2 {
        return None;
    }
    let in_region = |w: &NodeRef| region.nodes.contains(&cfg.node_index[w]);
    let inside: Vec<NodeRef> = writers.iter().copied().filter(in_region).collect();
    let outside: Vec<NodeRef> = writers.iter().copied().filter(|w| !in_region(w)).collect();
    if inside.len() != 1 || outside.len() != 1 {
        return None;
    }
    let inc = inside[0];
    let init = outside[0];
    // 加一得是本区域自己的：落在内层循环里的话，一轮里会加好几次。
    if regions.owner_of(cfg.node_index[&inc]) != i {
        return None;
    }
    // 初值得在进循环之前执行。
    if !dominates(regions, cfg, init, head_node) {
        return None;
    }

    // 循环内那次写：「循环变量 + 1」，可以经一层拷贝。
    let inc_node = graph.get_node(inc);
    let add_link = inc_node.links.values_in[1].link?;
    let add_node = add_link.target.node()?;
    if add_link.index != 0 {
        return None;
    }
    if graph.get_node(add_node).kind.id.as_native()? != &node_add(NativeKind::Int).id {
        return None;
    }
    let add = graph.get_node(add_node);
    let is_one = |v: &ValueIn| {
        v.link.is_none() && v.default.as_ref().and_then(|d| d.downcast_ref::<i32>().ok()) == Some(&1)
    };
    let other = if is_one(&add.links.values_in[0]) {
        &add.links.values_in[1]
    } else if is_one(&add.links.values_in[1]) {
        &add.links.values_in[0]
    } else {
        return None;
    };
    let other_link = other.link?;
    let other_local = other_link.target.node()?;
    if other_link.index != 1 || graph.get_node(other_local).kind.id != IrNodeId::Local {
        return None;
    }
    // 循环体实际读的是谁。`for ... in range` 会多一层拷贝：加一作用在
    // 原变量上，体里读的是拷出来的那个；`while` 形状则直接读原变量。
    let (read, copy) = if other_local == var {
        (var, None)
    } else {
        // 一层拷贝：这个局部变量在区域里只被写一次，写的是循环变量的读值。
        let copies: Vec<NodeRef> = local_writers(graph, other_local)
            .into_iter()
            .filter(in_region)
            .collect();
        if copies.len() != 1 {
            return None;
        }
        if regions.owner_of(cfg.node_index[&copies[0]]) != i {
            return None;
        }
        if graph.get_node(copies[0]).links.values_in[1].link != Some(Link::node(var, 1)) {
            return None;
        }
        // 拷贝得在加一之前。
        if !dominates(regions, cfg, copies[0], inc) {
            return None;
        }
        (other_local, Some(copies[0]))
    };

    // 回到头的边：可以有多条（`continue` 和正常路径各一条），但每一条
    // 都得被加一支配 —— 也就是「连回循环开始的边都在 i+=1 之后」。
    // 这样每轮都恰好加一次，native 的计数才跟得上循环变量。
    //
    // 加一在体首还是体尾都行：rustc 常把 `for` 的自增提到体首，
    // `continue` 跳的是旧值的副本。
    let back: Vec<usize> = cfg
        .edges
        .iter()
        .filter(|e| e.to == region.head && region.nodes.contains(&e.from))
        .map(|e| e.from)
        .collect();
    if back.is_empty() {
        return None;
    }
    for b in &back {
        if !dominates(regions, cfg, inc, point_node(cfg, *b)) {
            return None;
        }
    }

    // 初值。
    let init_node = graph.get_node(init);
    let begin = init_node.links.values_in[1].clone();
    match &begin.link {
        Some(l) => {
            let src = l.target.node()?;
            if !is_invariant(graph, cfg, regions, i, var, src, &mut HashSet::new()) {
                return None;
            }
        }
        // 常量初值：得是 int。
        None => {
            begin.default.as_ref()?.downcast_ref::<i32>().ok()?;
        }
    }

    // 终点：`i < bound` 数到 bound - 1，`i <= bound` 数到 bound。
    // 这里只描述怎么算，不建节点 —— 检测阶段不该改图。
    let end = match (&bound.link, &bound.default) {
        (None, Some(d)) => {
            let b = *d.downcast_ref::<i32>().ok()?;
            EndSpec::Const(if inclusive { b } else { b.checked_sub(1)? })
        }
        (Some(l), _) => {
            let src = l.target.node()?;
            if !is_invariant(graph, cfg, regions, i, var, src, &mut HashSet::new()) {
                return None;
            }
            if inclusive {
                EndSpec::Link(*l)
            } else {
                EndSpec::MinusOne(*l)
            }
        }
        _ => return None,
    };

    Some(CountingLoop {
        head: head_node,
        cmp: cmp_node,
        body,
        natural_exit,
        begin,
        end,
        var,
        init,
        inc,
        add: add_node,
        read,
        copy,
    })
}

/// 区域 `i` 的 switch 里，退出目标 `v` 对应的分支该连到哪。
///
/// 目标还在父区域里 —— 直接连目标。目标已经出了父区域 —— 连父区域的
/// 「置码 + 跳出」链，由父区域再往上抛一层。
fn case_target(
    cfg: &Cfg,
    regions: &Regions,
    plans: &[Option<RegionPlan>],
    i: usize,
    v: usize,
    dead_end: Option<NodeRef>,
) -> Result<Link, String> {
    // 目标是某个祖先区域的头 —— 这是 `continue 祖先`：跳出中间的循环，
    // 让祖先进入下一轮。不能直接跳回那个头（路径会绕回去，图里就有环了），
    // 而是把链断在那里：祖先看到自己的循环体走到头，就进入下一轮。
    //
    // 但只有「目标的区域就是本层的父层」时才能直接断链 —— 再往下的层
    // 得先把中间的循环逐层 break 掉（走父层的链），否则中间那层会自己
    // 再驱动一轮，continue 就传不上去了。
    if let Some(a) = regions.head_region.get(&v) {
        let a = *a;
        if a != i {
            if !is_ancestor_region(regions, i, a) {
                return Err(format!(
                    "区域 {} 想跳进区域 {} 的循环体，native 表达不了",
                    i, a
                ));
            }
            if regions.regions[i].parent == Some(a) {
                let Some(dead) = dead_end else {
                    return Err("缺链尾节点".into());
                };
                return Ok(Link::node(dead, 0));
            }
            // 更下面的层：走父层的链，一层层往上抛（见下面的分支）。
        }
    }
    if let Some(p) = regions.regions[i].parent
        && regions.regions[p].is_loop
        && !regions.regions[p].nodes.contains(&v)
        && let Some(chain) = plans[p].as_ref().and_then(|plan| plan.chain.get(&v))
    {
        return Ok(Link::node(*chain, 0));
    }
    Ok(match cfg.points[v] {
        Point::Export => Link::export(0),
        Point::Node(n) => Link::node(n, 0),
    })
}

/// 区域 `a` 是不是区域 `i` 的祖先。
fn is_ancestor_region(regions: &Regions, i: usize, a: usize) -> bool {
    let mut cur = regions.regions[i].parent;
    while let Some(p) = cur {
        if p == a {
            return true;
        }
        cur = regions.regions[p].parent;
    }
    false
}

/// 点号 → 节点。
fn point_node(cfg: &Cfg, p: usize) -> NodeRef {
    match cfg.points[p] {
        Point::Export => unreachable!("export 不是节点"),
        Point::Node(n) => n,
    }
}

/// 检查每个控制出端口至多一条连接。整套改造靠这个前提保持「不并联」。
pub fn check_control_outs(graph: &NodeGraphIr) -> Result<(), String> {
    for n in graph.get_nodes() {
        let node = graph.get_node(n);
        for (i, targets) in node.links.controls_out.iter().enumerate() {
            if targets.len() > 1 {
                return Err(format!(
                    "节点 {:?} 的第 {} 个控制出端口有 {} 条连接（至多一条）",
                    n,
                    i,
                    targets.len()
                ));
            }
        }
    }
    Ok(())
}

/// native 节点图不允许控制流有环 —— 唯一的例外是有限循环的「跳出」入口：
/// 跳出节点接在 in[1] 上，它不是控制流的入口，只是给循环打一个「本轮结束」
/// 的标记，循环随即走 out[1]。所以检查时把 in[1] 当终点。
pub fn check_no_control_cycle(graph: &NodeGraphIr) -> Result<(), String> {
    let mut state: HashMap<NodeRef, u8> = HashMap::new();
    fn visit(graph: &NodeGraphIr, n: NodeRef, state: &mut HashMap<NodeRef, u8>) -> Option<NodeRef> {
        match state.get(&n) {
            Some(1) => return Some(n),
            Some(2) => return None,
            _ => (),
        }
        state.insert(n, 1);
        let node = graph.get_node(n);
        for outs in &node.links.controls_out {
            for t in outs {
                let LinkTarget::Node(m) = t.target else {
                    continue;
                };
                // 跳出入口不是控制流入口。
                if is_loop_node_kind(&graph.get_node(m).kind) && t.index == 1 {
                    continue;
                }
                if let Some(c) = visit(graph, m, state) {
                    return Some(c);
                }
            }
        }
        state.insert(n, 2);
        None
    }
    for n in graph.get_nodes() {
        if let Some(c) = visit(graph, n, &mut state) {
            return Err(format!("控制流里有环，涉及节点 {c:?}"));
        }
    }
    Ok(())
}

/// 是不是有限循环节点。
fn is_loop_node_kind(kind: &NodeKindIr) -> bool {
    matches!(kind.id, IrNodeId::Native(ref n) if *n == NODE_FOR_CLOSED.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::ir::node_ir_nop;
    use crate::node::ExportDecl;
    use crate::node::NodeGraphKind;

    /// 造一张空图，带好函数的进出口。
    fn new_graph() -> NodeGraphIr {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "test");
        g.push_export_control_in(ExportDecl::new(String::new(), None));
        g.push_export_control_out(ExportDecl::new(String::new(), None));
        g
    }

    /// 一个带一个分支的 switch，当条件判断用。
    fn branch(g: &mut NodeGraphIr) -> NodeRef {
        g.insert(node_ir_switch(
            IrKind::Native(NativeKind::Int),
            vec![Box::new(1i32) as Box<dyn NativeValue>],
        ))
    }

    /// 控制图里有没有环。有限循环的「跳出」入口算终点 —— 那不是控制流入口。
    fn has_cycle(g: &NodeGraphIr) -> bool {
        check_no_control_cycle(g).is_err()
    }

    /// 从某个节点出发能到哪些节点。
    fn reachable(g: &NodeGraphIr, start: NodeRef) -> HashSet<NodeRef> {
        let mut seen = HashSet::new();
        let mut stack = vec![start];
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            let node = g.get_node(n);
            for outs in &node.links.controls_out {
                for t in outs {
                    if let LinkTarget::Node(m) = t.target {
                        stack.push(m);
                    }
                }
            }
        }
        seen
    }

    fn find_node(g: &NodeGraphIr, pred: impl Fn(&NodeKindIr) -> bool) -> Option<NodeRef> {
        g.get_nodes().into_iter().find(|n| pred(&g.get_node(*n).kind))
    }

    fn is_loop_node(k: &NodeKindIr) -> bool {
        matches!(k.id, IrNodeId::Native(ref n) if *n == NODE_FOR_CLOSED.id)
    }
    fn is_break_node(k: &NodeKindIr) -> bool {
        matches!(k.id, IrNodeId::Native(ref n) if *n == NODE_BREAK.id)
    }
    fn is_switch(k: &NodeKindIr) -> bool {
        matches!(k.id, IrNodeId::Switch(_))
    }

    /// 所有有限循环节点。
    fn loops(g: &NodeGraphIr) -> Vec<NodeRef> {
        g.get_nodes()
            .into_iter()
            .filter(|n| is_loop_node(&g.get_node(*n).kind))
            .collect()
    }

    /// 循环之后那个分派 switch —— 有限循环的 out[1] 接的就是它。
    fn dispatch_of(g: &NodeGraphIr, lp: NodeRef) -> NodeRef {
        let t = g.get_node(lp).links.controls_out[1][0];
        match t.target {
            LinkTarget::Node(m) => m,
            LinkTarget::Export => unreachable!(),
        }
    }

    /// 最基本的循环：入口 → 头 → 体 → 头，头的另一支出去。
    #[test]
    fn simple_loop_becomes_finite_loop() {
        let mut g = new_graph();
        let n0 = g.insert(node_ir_nop());
        let head = branch(&mut g);
        let body = g.insert(node_ir_nop());
        let after = g.insert(node_ir_nop());

        g.link_control(Link::export(0), Link::node(n0, 0));
        g.link_control(Link::node(n0, 0), Link::node(head, 0));
        g.link_control(Link::node(head, 1), Link::node(body, 0)); // 循环体
        g.link_control(Link::node(head, 0), Link::node(after, 0)); // 退出
        g.link_control(Link::node(body, 0), Link::node(head, 0)); // 回边

        lower_loops(&mut g).unwrap();

        // 有了一个有限循环节点、一个跳出循环节点。
        let lp = find_node(&g, is_loop_node).expect("没有有限循环节点");
        assert!(find_node(&g, is_break_node).is_some());
        // 只有一处退出目标：不用分派，有限循环的 out[1] 直接接循环之后。
        let target = dispatch_of(&g, lp);
        assert_eq!(target, after, "只有一处退出目标时不该有分派 switch");
        // 图里不该再有环。
        assert!(!has_cycle(&g), "改造后控制流还有环");
        // 回边删掉了：循环体的出端口空了。
        assert!(g.get_node(body).links.controls_out[0].is_empty());
        // 入口接到有限循环的 in[0]，循环体挂在 out[0]。
        assert!(g.get_node(lp).links.controls_in[0].contains(&Link::node(n0, 0)));
        assert!(g.get_node(lp).links.controls_out[0].contains(&Link::node(head, 0)));
        // 退出路径：有限循环的 out[1] → 循环之后。
        assert!(reachable(&g, target).contains(&after));
    }

    /// 一个循环有两条退出路径：靠 switch 分派。第 k 个 case 得接第 k 支，
    /// 最后一支是 default —— 局部变量的初值 0 不与任何 case 相等，default
    /// 不接任何东西。
    #[test]
    fn multi_exit_loop_dispatches_cases_in_order() {
        let mut g = new_graph();
        let n0 = g.insert(node_ir_nop());
        let head = branch(&mut g);
        let body = branch(&mut g);
        let after1 = g.insert(node_ir_nop());
        let after2 = g.insert(node_ir_nop());

        g.link_control(Link::export(0), Link::node(n0, 0));
        g.link_control(Link::node(n0, 0), Link::node(head, 0));
        g.link_control(Link::node(head, 1), Link::node(body, 0)); // 循环体
        g.link_control(Link::node(head, 0), Link::node(after1, 0)); // 退出 1
        g.link_control(Link::node(body, 0), Link::node(head, 0)); // 回边
        g.link_control(Link::node(body, 1), Link::node(after2, 0)); // 退出 2

        lower_loops(&mut g).unwrap();

        let lp = find_node(&g, is_loop_node).expect("没有有限循环节点");
        let sw = dispatch_of(&g, lp);
        assert!(is_switch(&g.get_node(sw).kind), "两个退出目标该有分派 switch");
        let outs = g.get_node(sw).links.controls_out.clone();
        // 两个 case + 一支 default。
        assert_eq!(outs.len(), 3);
        assert!(outs[2].is_empty(), "default 分支不该接东西");
        // 两个 case 各自接一个退出目标，互不重复。
        let case = |i: usize| outs[i].first().and_then(|l| l.target.node());
        let (c0, c1) = (case(0), case(1));
        assert!(c0.is_some() && c1.is_some(), "两个 case 都得接目标");
        assert_ne!(c0, c1, "两个 case 接到同一个目标了");
        assert!([after1, after2].contains(&c0.unwrap()));
        assert!([after1, after2].contains(&c1.unwrap()));
    }

    /// 循环里 return：退出目标就是函数出口。
    #[test]
    fn return_inside_loop_dispatches_to_the_exit() {
        let mut g = new_graph();
        let n0 = g.insert(node_ir_nop());
        let head = branch(&mut g);
        let body = g.insert(node_ir_nop());

        g.link_control(Link::export(0), Link::node(n0, 0));
        g.link_control(Link::node(n0, 0), Link::node(head, 0));
        g.link_control(Link::node(head, 1), Link::node(body, 0)); // 循环体
        g.link_control(Link::node(head, 0), Link::export(0)); // return
        g.link_control(Link::node(body, 0), Link::node(head, 0)); // 回边

        lower_loops(&mut g).unwrap();
        assert!(!has_cycle(&g));
        let lp = find_node(&g, is_loop_node).unwrap();
        // 只有一处退出目标（函数出口）：不用分派，有限循环的 out[1] 直接接出口。
        assert!(
            g.get_node(lp)
                .links
                .controls_out[1]
                .iter()
                .any(|t| t.target.is_export()),
            "有限循环的 out[1] 应该直接接函数出口"
        );
    }

    /// 嵌套循环：内层的退出要一路抛到外层，continue 外层要落回外层的下一轮。
    #[test]
    fn nested_loops_dispatch_through_switches() {
        let mut g = new_graph();
        let n0 = g.insert(node_ir_nop());
        // 外层循环的头：out[1] 进循环体，out[0] 退出。
        let outer = branch(&mut g);
        // 内层循环的头。
        let inner = branch(&mut g);
        // 循环体末尾：out[0] 回内层（回边），out[1] continue 外层。
        let cont = branch(&mut g);
        let after = g.insert(node_ir_nop());

        g.link_control(Link::export(0), Link::node(n0, 0));
        g.link_control(Link::node(n0, 0), Link::node(outer, 0));
        g.link_control(Link::node(outer, 1), Link::node(inner, 0));
        g.link_control(Link::node(inner, 1), Link::node(cont, 0));
        g.link_control(Link::node(inner, 0), Link::node(after, 0)); // 内层退出
        g.link_control(Link::node(cont, 0), Link::node(inner, 0)); // 内层回边
        g.link_control(Link::node(cont, 1), Link::node(outer, 0)); // continue 外层
        g.link_control(Link::node(outer, 0), Link::node(after, 0)); // 外层退出

        lower_loops(&mut g).unwrap();

        assert!(!has_cycle(&g), "改造后控制流还有环");
        // 两个有限循环。外层只有一处退出目标，不用分派；内层有两处（退出、
        // continue 外层），靠 switch 分派。
        let lps = loops(&g);
        assert_eq!(lps.len(), 2);
        let outer_lp = lps
            .iter()
            .copied()
            .find(|lp| {
                g.get_node(*lp)
                    .links
                    .controls_out[0]
                    .contains(&Link::node(outer, 0))
            })
            .expect("外层的有限循环");
        assert_eq!(dispatch_of(&g, outer_lp), after, "外层不该有分派 switch");
        // `continue 外层` 不能直接跳回外层的头（那会让图里出现环），
        // 而是把链断在链尾 —— 外层看到循环体走到头，就进入下一轮。
        let inner_lp = lps
            .iter()
            .copied()
            .find(|lp| {
                g.get_node(*lp)
                    .links
                    .controls_out[0]
                    .contains(&Link::node(inner, 0))
            })
            .expect("内层的有限循环");
        let inner_sw = dispatch_of(&g, inner_lp);
        assert!(is_switch(&g.get_node(inner_sw).kind), "内层该有分派 switch");
        // 分派 switch 要能到循环之后。
        assert!(
            reachable(&g, inner_sw).contains(&after),
            "switch 到不了循环之后"
        );
        let dead_end_branch = g
            .get_node(inner_sw)
            .links
            .controls_out
            .iter()
            .flatten()
            .find_map(|t| match t.target {
                LinkTarget::Node(m) => Some(m),
                LinkTarget::Export => None,
            })
            .filter(|m| {
                g.get_node(*m)
                    .links
                    .controls_out
                    .iter()
                    .all(|o| o.is_empty())
            })
            .expect("continue 外层应该接到链尾");
        assert!(!reachable(&g, dead_end_branch).contains(&outer));
    }

    /// 跨两层的退出：中间层自己没有直接的退出边，退出目标也得记到它名下，
    /// 否则内层抛不上去 —— 中间层不会被 break，循环会继续下一轮。
    #[test]
    fn multi_level_exit_registers_the_target_on_every_level() {
        let mut g = new_graph();
        let n0 = g.insert(node_ir_nop());
        // 外层循环的头：两个分支都进内层 —— 外层自己没有直接的退出边。
        let outer = branch(&mut g);
        // 内层循环的头。
        let inner = branch(&mut g);
        // 内层循环体末尾：out[0] 回内层，out[1] continue 外层。
        let body = branch(&mut g);
        // 退出目标：在两层循环之外。
        let out = g.insert(node_ir_nop());

        g.link_control(Link::export(0), Link::node(n0, 0));
        g.link_control(Link::node(n0, 0), Link::node(outer, 0));
        g.link_control(Link::node(outer, 0), Link::node(inner, 0));
        g.link_control(Link::node(outer, 1), Link::node(inner, 0));
        g.link_control(Link::node(inner, 1), Link::node(body, 0));
        g.link_control(Link::node(inner, 0), Link::node(out, 0)); // 内层退出
        g.link_control(Link::node(body, 0), Link::node(inner, 0)); // 内层回边
        g.link_control(Link::node(body, 1), Link::node(outer, 0)); // continue 外层

        lower_loops(&mut g).unwrap();

        assert!(!has_cycle(&g), "改造后控制流还有环");
        // 两层循环。
        let lps = loops(&g);
        assert_eq!(lps.len(), 2);
        // 两层都只有这一个退出目标：不用分派，但要能到退出目标。
        for lp in &lps {
            let to = dispatch_of(&g, *lp);
            assert!(
                reachable(&g, to).contains(&out),
                "有一层到不了退出目标"
            );
        }
        // 中间层（外层）也得记着这个目标：内层抛上去时，中间层要跟着 break。
        let outer_lp = lps
            .iter()
            .copied()
            .find(|lp| {
                g.get_node(*lp)
                    .links
                    .controls_out[0]
                    .contains(&Link::node(outer, 0))
            })
            .expect("外层的有限循环");
        assert_eq!(
            dispatch_of(&g, outer_lp),
            out,
            "外层该直接接退出目标"
        );
    }

    /// 三层循环，最内层 `continue` 到最外层：中间的循环也要被 break 掉，
    /// 否则它会自己再驱动一轮，continue 就传不上去。
    #[test]
    fn continue_to_grandparent_breaks_the_middle_loop() {
        let mut g = new_graph();
        let n0 = g.insert(node_ir_nop());
        // 最外层的头：两个分支都进中层。
        let l1 = branch(&mut g);
        // 中层的头：两个分支都进内层。
        let l2 = branch(&mut g);
        // 内层的头。
        let l3 = branch(&mut g);
        // 内层循环体末尾：out[0] 回内层，out[1] continue 最外层，
        // out[2] 落到中层体末尾。
        let body = g.insert(node_ir_switch(
            IrKind::Native(NativeKind::Int),
            vec![
                Box::new(1i32) as Box<dyn NativeValue>,
                Box::new(2i32) as Box<dyn NativeValue>,
            ],
        ));
        // 中层体末尾：回到中层的头。
        let mid_tail = g.insert(node_ir_nop());
        // 退出目标：在三层之外。
        let out = g.insert(node_ir_nop());

        g.link_control(Link::export(0), Link::node(n0, 0));
        g.link_control(Link::node(n0, 0), Link::node(l1, 0));
        g.link_control(Link::node(l1, 0), Link::node(l2, 0));
        g.link_control(Link::node(l1, 1), Link::node(l2, 0));
        g.link_control(Link::node(l2, 0), Link::node(l3, 0));
        g.link_control(Link::node(l2, 1), Link::node(l3, 0));
        g.link_control(Link::node(l3, 1), Link::node(body, 0));
        g.link_control(Link::node(l3, 0), Link::node(out, 0)); // 内层退出
        g.link_control(Link::node(body, 0), Link::node(l3, 0)); // 内层回边
        g.link_control(Link::node(body, 1), Link::node(l1, 0)); // continue 最外层
        g.link_control(Link::node(body, 2), Link::node(mid_tail, 0)); // 内层正常走完
        g.link_control(Link::node(mid_tail, 0), Link::node(l2, 0)); // 中层回边

        lower_loops(&mut g).unwrap();

        assert!(!has_cycle(&g), "改造后控制流还有环");
        // 三层循环。
        let lps = loops(&g);
        assert_eq!(lps.len(), 3);

        // 最内层那个：它的循环体挂在 l3 上。
        let inner_lp = lps
            .iter()
            .copied()
            .find(|lp| {
                g.get_node(*lp)
                    .links
                    .controls_out[0]
                    .contains(&Link::node(l3, 0))
            })
            .expect("内层的有限循环");
        let inner_sw = dispatch_of(&g, inner_lp);

        // 最外层那个：它的循环体挂在 l1 上。
        let outer_lp = lps
            .iter()
            .copied()
            .find(|lp| {
                g.get_node(*lp)
                    .links
                    .controls_out[0]
                    .contains(&Link::node(l1, 0))
            })
            .expect("最外层的有限循环");
        // 最外层也有退出目标（内层退出到 `out` 时会穿过最外层），
        // 但只有这一个目标，不用分派，直接接。
        assert_eq!(dispatch_of(&g, outer_lp), out, "最外层不该有分派 switch");

        // 中层那个：它的循环体挂在 l2 上。
        let mid_lp = lps
            .iter()
            .copied()
            .find(|lp| {
                g.get_node(*lp)
                    .links
                    .controls_out[0]
                    .contains(&Link::node(l2, 0))
            })
            .expect("中层的有限循环");
        let mid_sw = dispatch_of(&g, mid_lp);

        // 内层的 continue 分支不能直接接链尾 —— 得先跳出中层。
        // 直接接链尾的话，中层会自己再驱动一轮。
        let inner_targets: Vec<NodeRef> = g
            .get_node(inner_sw)
            .links
            .controls_out
            .iter()
            .flatten()
            .filter_map(|t| match t.target {
                LinkTarget::Node(m) => Some(m),
                LinkTarget::Export => None,
            })
            .collect();
        // 链尾节点（continue 断链用的那个 unreachable 节点）。
        let is_dead_end = |m: NodeRef| {
            matches!(g.get_node(m).kind.id, IrNodeId::Unreachable)
        };
        assert!(
            !inner_targets.iter().any(|m| is_dead_end(*m)),
            "内层的 continue 直接接了链尾，中层没被 break"
        );
        // 中层的那一支才接链尾。
        let mid_targets: Vec<NodeRef> = g
            .get_node(mid_sw)
            .links
            .controls_out
            .iter()
            .flatten()
            .filter_map(|t| match t.target {
                LinkTarget::Node(m) => Some(m),
                LinkTarget::Export => None,
            })
            .collect();
        assert!(
            mid_targets.iter().any(|m| is_dead_end(*m)),
            "中层的 continue 分支应该接链尾"
        );
    }

    /// 不可约的图（多入口的环）要报错，而不是硬转。
    #[test]
    fn irreducible_graph_is_rejected() {
        let mut g = new_graph();
        // s 的两支分别进 a 和 b，两支都汇到 c，c 再回到 a —— a 不支配 c。
        let s = branch(&mut g);
        let a = g.insert(node_ir_nop());
        let b = g.insert(node_ir_nop());
        let c = g.insert(node_ir_nop());

        g.link_control(Link::export(0), Link::node(s, 0));
        g.link_control(Link::node(s, 1), Link::node(a, 0));
        g.link_control(Link::node(s, 0), Link::node(b, 0));
        g.link_control(Link::node(a, 0), Link::node(c, 0));
        g.link_control(Link::node(b, 0), Link::node(c, 0));
        g.link_control(Link::node(c, 0), Link::node(a, 0));

        let e = lower_loops(&mut g).unwrap_err();
        assert!(e.contains("不可约"), "错误信息不对：{e}");
    }

    /// 一个「`i` 从 0 数到 bound」的计数循环。
    ///
    /// 形状跟 rustc 编译 `for i in 0..bound` 出来的一样：初值 → 头
    /// （比较 + 分支）→ 体（`i += 1`）→ 回到头。
    struct CountLoop {
        /// 局部变量 `i`。
        var: NodeRef,
        /// 初值那个节点（`i = 0`）。
        init: NodeRef,
        /// 比较节点 `i < bound`。
        cmp: NodeRef,
        /// 循环头（分支节点）。
        head: NodeRef,
        /// 循环体（`i += 1` 那个节点）。
        body: NodeRef,
        /// 正常结束之后。
        after: NodeRef,
    }

    fn counting_loop(g: &mut NodeGraphIr, bound: i32) -> CountLoop {
        let var = g.insert(node_ir_local(IrKind::Native(NativeKind::Int)));
        // 初值：i = 0
        let init = g.insert(node_ir_set_local(IrKind::Native(NativeKind::Int)));
        g.set_value_in(Link::node(init, 0), ValueIn::link(Link::node(var, 0)));
        g.set_value_in(Link::node(init, 1), ValueIn::value(0i32.into()));
        // 比较：i < bound
        let cmp = g.insert(node_ir_native(node_less_than(NativeKind::Int)));
        g.set_value_in(Link::node(cmp, 0), ValueIn::link(Link::node(var, 1)));
        g.set_value_in(Link::node(cmp, 1), ValueIn::value(bound.into()));
        // 头：bool 的分支。第 0 支是 false（跳出），第 1 支是 true（体）。
        let head = g.insert(node_ir_switch(
            IrKind::Native(NativeKind::Bool),
            vec![Box::new(false) as Box<dyn NativeValue>],
        ));
        g.set_value_in(Link::node(head, 0), ValueIn::link(Link::node(cmp, 0)));
        // 体：i += 1
        let add = g.insert(node_ir_native(node_add(NativeKind::Int)));
        g.set_value_in(Link::node(add, 0), ValueIn::link(Link::node(var, 1)));
        g.set_value_in(Link::node(add, 1), ValueIn::value(1i32.into()));
        let body = g.insert(node_ir_set_local(IrKind::Native(NativeKind::Int)));
        g.set_value_in(Link::node(body, 0), ValueIn::link(Link::node(var, 0)));
        g.set_value_in(Link::node(body, 1), ValueIn::link(Link::node(add, 0)));
        let after = g.insert(node_ir_nop());

        g.link_control(Link::export(0), Link::node(init, 0));
        g.link_control(Link::node(init, 0), Link::node(head, 0));
        g.link_control(Link::node(head, 1), Link::node(body, 0)); // 真 → 体
        g.link_control(Link::node(head, 0), Link::node(after, 0)); // 假 → 跳出
        g.link_control(Link::node(body, 0), Link::node(head, 0)); // 回边
        CountLoop {
            var,
            init,
            cmp,
            head,
            body,
            after,
        }
    }

    /// 节点某个值入脚上的整数常量。
    fn int_of(g: &NodeGraphIr, n: NodeRef, port: usize) -> Option<i32> {
        g.get_node(n).links.values_in[port]
            .default
            .as_ref()
            .and_then(|d| d.downcast_ref::<i32>().ok())
            .copied()
    }

    /// 计数循环：有限循环节点带上真的区间，那个恒真的比较摘掉。
    #[test]
    fn counting_loop_gets_a_real_range() {
        let mut g = new_graph();
        let c = counting_loop(&mut g, 10);
        lower_loops(&mut g).unwrap();

        let lp = find_node(&g, is_loop_node).expect("没有有限循环节点");
        // 区间 [0, 9]：闭区间，`i < 10` 数到 9。
        assert_eq!(int_of(&g, lp, 0), Some(0));
        assert_eq!(int_of(&g, lp, 1), Some(9));
        // 比较的分支摘了：图里不该再有分支节点。
        assert!(
            find_node(&g, is_switch).is_none(),
            "比较的分支不该留着"
        );
        // 体里只有 `i += 1`：摘掉之后循环体是空的，out[0] 挂一个 nop。
        // 退出挂 out[1]。
        let body = g.get_node(lp).links.controls_out[0]
            .first()
            .and_then(|t| t.target.node())
            .expect("循环体入口");
        assert_ne!(body, c.body, "加一节点该被摘掉");
        assert!(
            g.get_node(lp)
                .links
                .controls_out[1]
                .iter()
                .any(|t| t.target.node() == Some(c.after))
        );
        // 初值写入被摘掉，入口直接从函数出口进来（区间自己带着起点）。
        assert!(!g.get_nodes().contains(&c.init), "初值节点该被摘掉");
        assert!(
            g.get_node(lp)
                .links
                .controls_in[0]
                .iter()
                .any(|t| t.target.is_export())
        );
    }

    /// 特化之后循环体读的是有限循环节点的出参，原来那个局部变量和
    /// 「每轮加一」整条链都不在了。
    #[test]
    fn counting_loop_reads_from_the_finite_loop_node() {
        let mut g = new_graph();
        let c = counting_loop(&mut g, 10);
        // 体里加一个「读循环变量」的消费者（模拟 sum += i）。
        let use_var = g.insert(node_ir_native(node_add(NativeKind::Int)));
        g.set_value_in(Link::node(use_var, 0), ValueIn::link(Link::node(c.var, 1)));
        g.set_value_in(Link::node(use_var, 1), ValueIn::value(1i32.into()));

        lower_loops(&mut g).unwrap();

        let lp = find_node(&g, is_loop_node).expect("没有有限循环节点");
        // 读值改接到有限循环的出参。
        assert_eq!(
            g.get_node(use_var).links.values_in[0].link,
            Some(Link::node(lp, 0)),
            "循环体该读有限循环的出参"
        );
        // 初值写入、加一写入、`+` 节点、局部变量都摘掉了。
        let alive = g.get_nodes();
        for n in [c.init, c.body] {
            assert!(!alive.contains(&n), "{n:?} 该被摘掉");
        }
        assert!(!alive.contains(&c.var), "循环变量该被摘掉");
        assert!(
            !alive.iter().any(|n| {
                g.get_node(*n).kind.id.as_native() == Some(&node_add(NativeKind::Int).id)
                    && g.get_node(*n).links.values_out[0]
                        .iter()
                        .any(|l| l.target.node() == Some(c.body))
            }),
            "加一的 `+` 节点该被摘掉"
        );
    }

    /// 循环体里还有别的出口（break）时，「正常结束」也算一个分支：
    /// 进循环前就把退出码设成正常结束那个码，循环自然结束时码没变，
    /// 分派 switch 就落到正常结束那支，而不是 default。
    #[test]
    fn counting_loop_with_extra_break_presets_the_normal_exit_code() {
        let mut g = new_graph();
        let c = counting_loop(&mut g, 10);
        // 体里加一条分支：一支走到 i += 1，一支跳出循环。
        let br = branch(&mut g);
        let brk_to = g.insert(node_ir_nop());
        g.unlink_control(Link::node(c.head, 1), Link::node(c.body, 0));
        g.link_control(Link::node(c.head, 1), Link::node(br, 0));
        g.link_control(Link::node(br, 1), Link::node(c.body, 0));
        g.link_control(Link::node(br, 0), Link::node(brk_to, 0));

        lower_loops(&mut g).unwrap();

        let lp = find_node(&g, is_loop_node).expect("没有有限循环节点");
        // 循环的 out[1] 接分派 switch。
        let sw = g
            .get_node(lp)
            .links
            .controls_out[1]
            .first()
            .and_then(|l| l.target.node())
            .expect("out[1] 该接分派 switch");
        let IrNodeId::Switch(values) = &g.get_node(sw).kind.id else {
            panic!("out[1] 该接分派 switch");
        };
        assert_eq!(values.len(), 2, "两个退出目标该有两个 case");
        // 进循环前那个设码的节点。
        let pre = g
            .get_node(lp)
            .links
            .controls_in[0]
            .first()
            .and_then(|l| l.target.node())
            .expect("进循环前该有个设码的节点");
        assert!(
            matches!(g.get_node(pre).kind.id, IrNodeId::SetLocal),
            "进循环前那个节点该是设码"
        );
        let pre_code = int_of(&g, pre, 1).expect("设码节点得写一个整数");
        // 正常结束那支的码就是循环前设的码。
        let mut found = false;
        for (k, v) in values.iter().enumerate() {
            let code = *v.downcast_ref::<i32>().ok().unwrap();
            let target = g
                .get_node(sw)
                .links
                .controls_out[k]
                .first()
                .and_then(|l| l.target.node());
            if target == Some(c.after) {
                assert_eq!(code, pre_code, "正常结束那支的码该是循环前设的那个");
                found = true;
            }
        }
        assert!(found, "分派 switch 里该有正常结束那支");
    }

    /// `continue` 的形状：循环体里有两条路回到头（`continue` 一条、
    /// 正常路径一条），但两条都在 `i += 1` 之后 —— 每轮都恰好加一次，
    /// 所以照样能特化。
    ///
    /// 跳出分支是函数出口（循环是函数体的最后一句）也算正常结束。
    #[test]
    fn counting_loop_with_continue_is_still_counted() {
        let mut g = new_graph();
        let c = counting_loop(&mut g, 10);
        // 体里加一条分支：一支直接回到头（continue），一支先做点事再回。
        let br = branch(&mut g);
        let work = g.insert(node_ir_nop());
        g.unlink_control(Link::node(c.body, 0), Link::node(c.head, 0));
        g.link_control(Link::node(c.body, 0), Link::node(br, 0));
        g.link_control(Link::node(br, 0), Link::node(c.head, 0)); // continue
        g.link_control(Link::node(br, 1), Link::node(work, 0)); // 干活
        g.link_control(Link::node(work, 0), Link::node(c.head, 0));

        lower_loops(&mut g).unwrap();

        let lp = find_node(&g, is_loop_node).expect("没有有限循环节点");
        // 区间还是 [0, 9]。
        assert_eq!(int_of(&g, lp, 0), Some(0));
        assert_eq!(int_of(&g, lp, 1), Some(9));
        // 循环头的分支摘了，只剩体里那个分支。
        assert_eq!(find_node(&g, is_switch), Some(br), "比较的分支不该留着");
        // 两条回到头的边都删掉了（链断 = 一轮结束）。
        assert!(g.get_node(br).links.controls_out[0].is_empty());
        assert!(g.get_node(work).links.controls_out[0].is_empty());
        // 循环体挂在 out[0]（体首是那个分支）。
        assert!(
            g.get_node(lp)
                .links
                .controls_out[0]
                .iter()
                .any(|t| t.target.node() == Some(br))
        );
    }

    /// `while` 形状的 `continue`：它跳过更新部分，`i` 不增长。
    /// native 的计数跟 Rust 的迭代次数对不上，不能特化。
    ///
    /// `for` 形状的 `continue` 相反 —— 它执行更新部分（rustc 把自增提到
    /// 体首），所以 `i` 照常增长，能特化（见上一个测试）。
    #[test]
    fn counting_loop_with_continue_before_the_increment_is_not_counted() {
        let mut g = new_graph();
        let c = counting_loop(&mut g, 10);
        // 体里：先分支，`continue` 直接回头（跳过加一），另一支才加一。
        let br = branch(&mut g);
        g.unlink_control(Link::node(c.head, 1), Link::node(c.body, 0));
        g.link_control(Link::node(c.head, 1), Link::node(br, 0));
        g.link_control(Link::node(br, 0), Link::node(c.head, 0)); // continue：跳过加一
        g.link_control(Link::node(br, 1), Link::node(c.body, 0)); // 另一支才加一

        lower_loops(&mut g).unwrap();

        let lp = find_node(&g, is_loop_node).expect("没有有限循环节点");
        // 区间还是满的：没特化。
        assert_eq!(int_of(&g, lp, 0), Some(i32::MIN));
        assert_eq!(int_of(&g, lp, 1), Some(i32::MAX));
        // 比较的分支也还在。
        assert!(find_node(&g, is_switch).is_some(), "比较的分支不该被摘掉");
    }

    /// 计数循环里有 `return`：两个出口（函数出口 + 正常结束），
    /// 正常结束那个码在进循环前就设好，循环自然跑完时码没变。
    #[test]
    fn counting_loop_with_return_in_the_body_presets_the_normal_exit_code() {
        let mut g = new_graph();
        let c = counting_loop(&mut g, 10);
        // 体里加一条分支：一支走到 i += 1，一支直接 return。
        let br = branch(&mut g);
        g.unlink_control(Link::node(c.head, 1), Link::node(c.body, 0));
        g.link_control(Link::node(c.head, 1), Link::node(br, 0));
        g.link_control(Link::node(br, 1), Link::node(c.body, 0));
        g.link_control(Link::node(br, 0), Link::export(0)); // return

        lower_loops(&mut g).unwrap();

        let lp = find_node(&g, is_loop_node).expect("没有有限循环节点");
        // 区间还是 [0, 9]。
        assert_eq!(int_of(&g, lp, 0), Some(0));
        assert_eq!(int_of(&g, lp, 1), Some(9));
        // 两个出口 → 有分派 switch，循环的 out[1] 接它。
        let sw = g
            .get_node(lp)
            .links
            .controls_out[1]
            .first()
            .and_then(|l| l.target.node())
            .expect("out[1] 该接分派 switch");
        let IrNodeId::Switch(values) = &g.get_node(sw).kind.id else {
            panic!("out[1] 该接分派 switch");
        };
        assert_eq!(values.len(), 2, "两个退出目标该有两个 case");
        // 正常结束那支的码，就是进循环前设的码。
        let pre = g
            .get_node(lp)
            .links
            .controls_in[0]
            .first()
            .and_then(|l| l.target.node())
            .expect("进循环前该有个设码的节点");
        let pre_code = int_of(&g, pre, 1).expect("设码节点得写一个整数");
        let mut found = false;
        for (k, v) in values.iter().enumerate() {
            let code = *v.downcast_ref::<i32>().ok().unwrap();
            let target = g
                .get_node(sw)
                .links
                .controls_out[k]
                .first()
                .and_then(|l| l.target.node());
            if target == Some(c.after) {
                assert_eq!(code, pre_code, "正常结束那支的码该是循环前设的那个");
                found = true;
            }
        }
        assert!(found, "分派 switch 里该有正常结束那支");
    }

    /// 「正常结束」指回循环体内部时不能特化：区间耗尽后 native 会跳进
    /// 循环体，语义就错了。
    #[test]
    fn counting_loop_whose_normal_exit_stays_inside_is_not_counted() {
        let mut g = new_graph();
        let c = counting_loop(&mut g, 10);
        // 头为假时跳到体尾（而不是循环之后）。
        let y = g.insert(node_ir_nop());
        g.unlink_control(Link::node(c.body, 0), Link::node(c.head, 0));
        g.link_control(Link::node(c.body, 0), Link::node(y, 0));
        g.link_control(Link::node(y, 0), Link::node(c.head, 0));
        g.unlink_control(Link::node(c.head, 0), Link::node(c.after, 0));
        g.link_control(Link::node(c.head, 0), Link::node(y, 0));

        lower_loops(&mut g).unwrap();

        let lp = find_node(&g, is_loop_node).expect("没有有限循环节点");
        // 区间还是满的：没特化。
        assert_eq!(int_of(&g, lp, 0), Some(i32::MIN));
        assert_eq!(int_of(&g, lp, 1), Some(i32::MAX));
        assert!(find_node(&g, is_switch).is_some(), "比较的分支不该被摘掉");
    }

    /// 循环头的分支约定有两种：`if` 语句来的 case 列表是 [true]，
    /// MIR 的 `switchInt(bool)` 来的是 [false]。两种都得认。
    #[test]
    fn counting_loop_head_with_true_case_list_is_recognized() {        let mut g = new_graph();
        let var = g.insert(node_ir_local(IrKind::Native(NativeKind::Int)));
        let init = g.insert(node_ir_set_local(IrKind::Native(NativeKind::Int)));
        g.set_value_in(Link::node(init, 0), ValueIn::link(Link::node(var, 0)));
        g.set_value_in(Link::node(init, 1), ValueIn::value(0i32.into()));
        let cmp = g.insert(node_ir_native(node_less_than(NativeKind::Int)));
        g.set_value_in(Link::node(cmp, 0), ValueIn::link(Link::node(var, 1)));
        g.set_value_in(Link::node(cmp, 1), ValueIn::value(10.into()));
        // case 列表是 [true]：true 在第 0 支，false 在 default 支。
        let head = g.insert(node_ir_switch(
            IrKind::Native(NativeKind::Bool),
            vec![Box::new(true) as Box<dyn NativeValue>],
        ));
        g.set_value_in(Link::node(head, 0), ValueIn::link(Link::node(cmp, 0)));
        let add = g.insert(node_ir_native(node_add(NativeKind::Int)));
        g.set_value_in(Link::node(add, 0), ValueIn::link(Link::node(var, 1)));
        g.set_value_in(Link::node(add, 1), ValueIn::value(1i32.into()));
        let body = g.insert(node_ir_set_local(IrKind::Native(NativeKind::Int)));
        g.set_value_in(Link::node(body, 0), ValueIn::link(Link::node(var, 0)));
        g.set_value_in(Link::node(body, 1), ValueIn::link(Link::node(add, 0)));
        let after = g.insert(node_ir_nop());

        g.link_control(Link::export(0), Link::node(init, 0));
        g.link_control(Link::node(init, 0), Link::node(head, 0));
        g.link_control(Link::node(head, 0), Link::node(body, 0)); // true → 体
        g.link_control(Link::node(head, 1), Link::node(after, 0)); // false → 跳出
        g.link_control(Link::node(body, 0), Link::node(head, 0)); // 回边

        lower_loops(&mut g).unwrap();

        let lp = find_node(&g, is_loop_node).expect("没有有限循环节点");
        assert_eq!(int_of(&g, lp, 0), Some(0));
        assert_eq!(int_of(&g, lp, 1), Some(9));
        assert!(find_node(&g, is_switch).is_none(), "比较的分支不该留着");
        // 体里只有加一：摘掉后循环体是空的，out[0] 挂 nop。
        assert!(
            g.get_node(lp)
                .links
                .controls_out[0]
                .first()
                .and_then(|t| t.target.node())
                .is_some(),
            "循环体入口不该悬空"
        );
    }
}
