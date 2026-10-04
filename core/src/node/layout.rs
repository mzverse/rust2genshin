//! 节点图自动整理(纯计算,可单测):控制流主干 + 无控制流节点的就近锚定
//!
//! 输入是 [`NodeGraph`] 的**边**与**引脚数**,输出是每个节点的坐标,写回
//! [`Node::position`](crate::node::Node)。`.gia` 的 `x_pos` / `y_pos` 就是
//! 从这里来的(编码处读 `position`,见 `node/mod.rs`);viewer 的「自动整理」
//! 也是调用同一套 —— 两边不会漂。
//!
//! 布局只看**拓扑**(连接关系)和**节点尺寸**(由标签与 pin 数估算),
//! 不碰渲染。节点尺寸的估算必须和消费方一致(否则会互相压盖),所以
//! [`node_height`] / [`node_width`] 由渲染方直接复用,别各写一份。
//!
//! [`layout`] 的标签取 `NodeId` 的紧凑形式([`NodeLabel`]);需要更丰富的
//! 显示文本(viewer 会在后面缀上 `<类型>`)时用 [`layout_with`] 传自己的
//! 标签函数 —— 估算宽度用的是**显示出来的**那个字符串,两边必须是同一个。

use std::collections::{BTreeMap, HashMap};

use crate::compile::ir::IrNodeId;
use crate::node::{LinkTarget, NativeNodeId, NodeGraph, NodeKind, NodeRef};

/// 层间距(水平方向)
pub const LAYER_GAP: f32 = 80.0;
/// 同一层内节点间距(垂直方向)
pub const NODE_GAP: f32 = 24.0;
/// 画布边距
pub const MARGIN: f32 = 40.0;
/// 层高的下限(垂直方向总跨度)
pub const MIN_LAYER_HEIGHT: f32 = 200.0;

/// 节点宽度**下限**。
pub const NODE_W: f32 = 140.0;
/// 节点高度**下限**(0~1 个 pin 的节点用这个)。
pub const NODE_H: f32 = 36.0;
/// 侧边相邻 pin 之间的最小间距。`spread` 把 N 个 pin 均分成 N+1 段,
/// 段距 = 高度 / (N + 1),所以间距要够大,节点就得跟着长高 ——
/// 见 [`node_height`]。
pub const PIN_GAP: f32 = 16.0;
/// 缩放为 1 时的文字字号。节点标签、默认值、Export 标签共用;
/// [`node_width`] 的估算也按它算(宽度以 1 倍缩放为准,渲染时整体乘 scale)。
pub const BASE_FONT_PX: f32 = 11.0;

/// 按一侧最多的 pin 数算节点高度:pin 越多节点越高,连接点才不挤。
pub fn node_height(pins: usize) -> f32 {
    NODE_H.max((pins + 1) as f32 * PIN_GAP)
}

/// 节点宽度:按标签**估算**文字宽度,长了自动拉宽(`NODE_W` 是下限)。
///
/// 是估算不是测量 —— 精确测量要 `TextSystem`(得在拿得到 `cx` / 窗口的地方做),
/// 而宽度参与布局(层与层的横向步进按层内最宽节点累加),必须是纯数据、可单测。
/// 因此**往宽了估**:ASCII 按 6.6px/字符(11px 字号下界于大写/数字的偏高值),
/// 非 ASCII(比如中文)按 1em 记。估宽的代价只是节点多留点边距;估窄了文字会被
/// 裁掉,那正是这个函数要修的毛病。
pub fn node_width(text: &str) -> f32 {
    /// 文字两侧的留白:左右 padding(px_1)+ 描边 + 一点余量。
    const PAD: f32 = 16.0;
    let text_w: f32 = text
        .chars()
        .map(|c| if c.is_ascii() { 6.6 } else { BASE_FONT_PX })
        .sum();
    NODE_W.max(text_w + PAD)
}

/// 节点在布局里的**显示名** —— 宽度估算的依据。
///
/// `IrNodeId` / `NativeNodeId` 都按同一套紧凑形式:`Native` 带 `kind` 与 `id`
/// (`{:?}#{}`,与 `.gia` 里的 `SysCallStub#2` 一个意思),`Fn` 用函数名,
/// 其余变体 `Debug` 恰好就是要的名字。**不要**给每个变体写一段字符串常量。
///
/// 名字里的类型标注(如 `SetLocal<Int>`)不在这一层 —— 需要它的调用方用
/// [`layout_with`] 传自己的标签函数(同一个字符串必须同时喂给渲染与宽度估算)。
pub trait NodeLabel {
    fn node_label(&self) -> String;
}
impl NodeLabel for NativeNodeId {
    fn node_label(&self) -> String {
        format!("{:?}#{}", self.kind, self.id)
    }
}
impl NodeLabel for IrNodeId {
    fn node_label(&self) -> String {
        match self {
            IrNodeId::Native(x) => x.node_label(),
            IrNodeId::Fn(s) => s.clone(),
            other => format!("{other:?}"),
        }
    }
}

/// 对整张图跑一遍自动整理,把坐标写进每个节点的 [`position`](crate::node::Node)。
///
/// 标签取 [`NodeLabel`];需要更丰富的显示文本(宽度会跟着变)用 [`layout_with`]。
pub fn layout<NodeId: NodeLabel, Kind>(graph: &mut NodeGraph<NodeId, Kind>) {
    layout_with(graph, |kind| kind.id.node_label());
}

/// 同 [`layout`],但标签由调用方给 —— 宽度按这个字符串估算,
/// 所以它必须是**渲染时显示的那个字符串**,否则框和字会对不上。
pub fn layout_with<NodeId, Kind>(
    graph: &mut NodeGraph<NodeId, Kind>,
    label: impl Fn(&NodeKind<NodeId, Kind>) -> String,
) {
    // 展平成布局模型(slab 迭代序 = 下标序)。渲染方若按自己的下标序读回
    // 坐标,两边是同一套顺序 —— viewer 的展平就是这么对齐的。
    let mut w: Vec<f32> = Vec::new();
    let mut h: Vec<f32> = Vec::new();
    let mut index_of: HashMap<NodeRef, usize> = HashMap::new();
    for (key, node) in graph.nodes.iter() {
        index_of.insert(NodeRef::from(key), w.len());
        let pins = (node.kind.controls_in_num + node.kind.values_in_types.len())
            .max(node.kind.controls_out_num + node.kind.values_out_types.len());
        w.push(node_width(&label(&node.kind)));
        h.push(node_height(pins));
    }

    // 边一律**从来源节点的 out 侧**发出(`links` 存的是双向的,
    // 走 out 侧才只画一次,不会一条边出两遍)。
    let mut edges = Vec::new();
    for (key, node) in graph.nodes.iter() {
        let Some(&from) = index_of.get(&NodeRef::from(key)) else {
            continue;
        };
        for (pin, outs) in node.links.controls_out.iter().enumerate() {
            for out in outs {
                // 指向图外的边不参与布局。
                if let Some(&to) = out.target.node().and_then(|n| index_of.get(&n)) {
                    edges.push(Edge {
                        from: (from, pin),
                        to: (to, out.index),
                        ctrl: true,
                    });
                }
            }
        }
        for (pin, outs) in node.links.values_out.iter().enumerate() {
            for out in outs {
                if out.target == LinkTarget::Export {
                    continue;
                }
                if let Some(&to) = out.target.node().and_then(|n| index_of.get(&n)) {
                    edges.push(Edge {
                        from: (from, pin),
                        to: (to, out.index),
                        ctrl: false,
                    });
                }
            }
        }
    }
    // 入口块:控制流从图外流进的节点(`externals.controls_out` 的收件人,
    // 方向容易看反,见 `NodeGraph` 侧的注释)。
    let entry: Vec<usize> = graph
        .externals
        .controls_out
        .iter()
        .flatten()
        .filter_map(|out| out.target.node().and_then(|n| index_of.get(&n).copied()))
        .collect();

    let model = Model {
        w,
        h,
        edges,
        entry,
    };
    let pos = compute(&model);

    for (key, node) in graph.nodes.iter_mut() {
        if let Some(&i) = index_of.get(&NodeRef::from(key)) {
            node.position = pos[i];
        }
    }
}

/// 两条线段是否**真正相交**(端点相接、共线、平行一律不算)。
/// 垂直排序里用"节点中心连线"近似渲染的贝塞尔边来数交叉 —— 只做排序决策,
/// 不需要精确到曲线的实际控制点。
fn segments_cross(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> bool {
    let orient = |p: (f32, f32), q: (f32, f32), r: (f32, f32)| {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    };
    orient(a, b, c) * orient(a, b, d) < 0.0 && orient(c, d, a) * orient(c, d, b) < 0.0
}

struct Edge {
    /// `(节点下标, pin 序号)`
    #[allow(dead_code)] // pin 序号当前不参与计算,留着对齐渲染模型的形状
    from: (usize, usize),
    #[allow(dead_code)]
    to: (usize, usize),
    /// `true` = 控制流边
    ctrl: bool,
}

/// 展平后的布局模型:算法只看这些(与 `NodeGraph` 解耦,可单测)。
struct Model {
    /// 每节点宽度(下标同 slab 迭代序)
    w: Vec<f32>,
    /// 每节点高度
    h: Vec<f32>,
    edges: Vec<Edge>,
    /// 入口块下标(控制流 BFS 起点)
    entry: Vec<usize>,
}

/// 布局分层。两部分组成:
///
/// 1. **控制流主干**:从入口出发、只认控制流边,取最长路径 —— 目标是让每条
///    控制流边都从左往右指。值边**不影响**这些节点的层级:值流是层间的
///    数据流,混进主干松弛会把表达式节点错误地推到下一层。
/// 2. **无控制流节点**(入口走不到的值流节点、掉队的死块):就近锚定(ALAP,
///    见下面的「无控制流节点」一节)—— 放在**最早消费者**左边一列;没有
///    消费者的汇点贴生产者右侧;完全孤立的留在最右一列。以前是一律堆到
///    最右列:实测某函数 11 个值节点叠成一根竖柱、每条值边倒着往回指。
///    现在值链自消费者左侧逐段排开,**每条值边都朝右指**。
///
/// 控制流图**可能有环**(rustc 的 MIR 对 `loop` / `while` 有回边;见下),那种
/// 情况下只能做到"尽力而为" —— 边不保证向右,但保证不挂死、层级被钳在
/// `[-(n+1), n]` 内(下界服务于"值链比消费者深度更深"的负层级与值环发散
/// 时的兜底,见钳位注释)。
///
/// 返回的 Vec 与模型同序同长,类型为 `i64` —— **负层级是合法结果**
/// (在入口列左边新开列)。
///
/// # 控制流图**可能有环** —— 所以有轮数上界 + 层级钳位
///
/// 有环时约束 `depth[v] >= depth[u] + 1` 在环上自相矛盾,`changed` 永远为真,
/// `depth` 无限增长 —— 无界 `loop` **会挂死**。对一个输入是"用户随手拖进来的
/// 任意 `.ogia`"的工具来说,100% CPU 死循环比 panic 更糟:窗口无响应,
/// 用户只能杀进程,连出错现场都留不下。
///
/// 所以轮数封顶 `n`(节点数):DAG 上最长路径 ≤ n-1 条边,每轮至少传播一跳,
/// **n 轮必然够** —— 也就是说这个上界对任何无环输入都是 **no-op**,
/// 只在有环时把死循环变成一个能画出来的分层。
///
/// 光有上界还不够:跑满 n 轮后有环图的深度是 O(n²) 量级(实测 500 节点 →
/// 层级到万级 → 画布宽约 5.5e7 px,用户拖不进、滚不到、看不见任何节点)。
/// 图算出来了,但等于没算 —— 和挂死是同一类失败。所以返回前还要对层级钳位,
/// 详见钳位处的注释。
///
/// 超限时**不做任何诊断**:不告警、不标记、不计数。结果只是一个尽力而为的
/// 分层 —— 少数边可能往回指、节点可能被排在比理想值更深的一层,但一定正常
/// 返回、不 panic,且每一项都落在 `[-(n+1), n]` 内。
fn levels(m: &Model) -> Vec<i64> {
    let n = m.w.len();

    // 只从控制流边建后继表。值边一律不看。
    let mut succs: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in m.edges.iter().filter(|e| e.ctrl) {
        if e.from.0 < n && e.to.0 < n {
            succs[e.from.0].push(e.to.0);
        }
    }

    // 从入口 BFS 求可达集。BFS 自身是 visited 去重的,即使图有环也只会
    // 多跑一圈,不会死循环。
    let mut reachable = vec![false; n];
    let mut q = std::collections::VecDeque::new();
    for &i in m.entry.iter() {
        if i < n && !reachable[i] {
            reachable[i] = true;
            q.push_back(i);
        }
    }
    while let Some(u) = q.pop_front() {
        for &v in succs[u].iter() {
            if !reachable[v] {
                reachable[v] = true;
                q.push_back(v);
            }
        }
    }

    // 松弛:对每条边 u -> v 要求 depth[v] >= depth[u] + 1,取**从入口出发的最长路径**。
    //
    // 方向不能反:写成 depth[u] >= depth[v] + 1 算的是"到汇点的最长路径",
    // 那样 diamond 会得到 [3,2,1,1,0] —— 入口落在最深的第 3 层,汇合点反而在第 0 层,
    // 整张图左右颠倒。
    //
    // ⚠️ **轮数必须封顶 `n`**,理由见本函数的 doc comment(控制流图可能有环,
    // 无界 loop 会挂死)。DAG 上最长路径 ≤ n-1 条边,每轮至少传播一跳,所以 n 轮
    // 对任何无环输入都是 no-op;`n == 0` 时 `0..0` 自然不执行。
    // 删掉这个 `for` 改回无界 `loop` 之前,先看一眼 `levels_returns_on_a_cyclic_graph`。
    //
    // 不可达的节点在这一步被跳过 —— 死代码/孤立节点之间可能有边,但它们既不该
    // 影响可达节点的深度,也不该被松弛(否则不可达子图自成一个环时同样会挂死)。
    // 最坏复杂度 O(V^2 * E);单函数规模的图够用。
    let mut depth = vec![0usize; n];
    for _ in 0..n {
        let mut changed = false;
        for u in 0..n {
            if !reachable[u] {
                continue;
            }
            for &v in succs[u].iter() {
                if !reachable[v] {
                    continue;
                }
                if depth[v] < depth[u] + 1 {
                    depth[v] = depth[u] + 1;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    // ---- 无控制流节点:就近锚定(ALAP,"能多晚放多晚")----
    //
    // 这些节点实测几乎全是值流节点(Local 槽 / 表达式 / Assemble / Destructure),
    // 从入口沿控制流走不到。以前把它们**全部**塞到最右一列:实测 11 个节点
    // 叠成一根竖柱,每条值边都从右往左倒着指。
    //
    // 改成两种锚点:
    //   - 有消费者:level = min(所有消费者层级) - 1
    //     —— 放在**最早的那次使用**左边一列;值的链(Local → … → 消费者)会
    //     自然叠成一串,正好落在消费者左侧。共享槽(一个源、多个消费者)锚在
    //     最早消费者上,到更远消费者的边拉长但方向仍朝右。
    //   - 没有消费者的汇点:level = max(生产者层级) + 1,紧贴生产者。
    //   - 既无消费者也无生产者(孤立):留在初始值,即最右一列,同以前。
    //
    // 这里看**全部**边(含控制边)—— 掉队的死块也该顺着控制流贴回它还能到
    // 的地方,而不是糊到右边。
    //
    // 收敛方式与控制流松弛同一套:min 单向下调、每轮至少一跳,DAG 上 n 轮
    // 必然收敛;值流内部有环(任意输入的 .ogia 可能有)时下调发散,由轮数
    // 封顶 n 兜住,最后统一钳位。
    //
    // `init` 取**未钳位**的最大控制流深度 + 1:钳位后它仍 ≥ 任何可达节点的
    // 层级(`max_raw + 1 >= n >= 钳位上界`),孤立节点不会落在可达节点左边。
    let max_raw = depth.iter().copied().max().unwrap_or(0);
    let init = max_raw as i64 + 1;

    // 不可达节点的松弛要读任一边的对端层级,先把双向表建出来。
    let mut succs_all: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut preds_all: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in m.edges.iter() {
        if e.from.0 < n && e.to.0 < n {
            succs_all[e.from.0].push(e.to.0);
            preds_all[e.to.0].push(e.from.0);
        }
    }

    let mut level: Vec<i64> = (0..n)
        .map(|i| {
            if reachable[i] {
                depth[i].min(n) as i64
            } else {
                init
            }
        })
        .collect();

    // 轮数封顶 n,同控制流松弛(DAG 上最长链 ≤ n-1 跳,每轮至少传播一跳;
    // 有环时把发散截断成有界的"尽力而为",不挂死)。
    //
    // "只下调、从不上调"的前提:汇点锚在 `max(生产者) + 1`,而生产者只会
    // 左移(min 单向),所以锚点也单调左移 —— 不存在"降完又需要升"的振荡。
    for _ in 0..n {
        let mut changed = false;
        for u in 0..n {
            if reachable[u] {
                continue;
            }
            // 注意 `iter().min()/max()` 是对**层级值**取极值,不是对下标。
            let cand = if let Some(min_s) = succs_all[u].iter().map(|&s| level[s]).min() {
                min_s - 1
            } else if let Some(max_p) = preds_all[u].iter().map(|&p| level[p]).max() {
                max_p + 1
            } else {
                continue; // 孤立:停在 init(最右一列)
            };
            if cand < level[u] {
                level[u] = cand;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // 钳位,把有环时的层级从 O(n²) 压回 O(n)。
    //
    // 有环时约束互相矛盾,上面的轮数上界一放开深度就线性膨胀:每轮每条边 ±1,
    // 跑满 n 轮就是 O(n²) 量级(实测 500 节点 → 层级到万级 → 画布宽约 5.5e7 px)。
    // 上界 n 够用:无环图上控制流深度本来就 < n,钳位严格 no-op;有环时只需要
    // 把画布宽度压回可操作范围,不需要真的算出环语义 —— 环在画布上一眼可见。
    //
    // 下界 -(n+1) 是同一条理由在负方向的对称:值链可以合法地比消费者深度更深
    // (在入口列左边新开列),值环发散时也靠它兜住。
    //
    // 钳位必须放在**锚定之后** —— 锚定读的是钳位前的控制流深度
    // (`init` 依赖 `max_raw`),顺序反过来孤立节点就可能落在可达节点左边。
    // 返回的每一项保证落在 [-(n+1), n](测试 `cyclic_levels_stay_within_the_node_count`
    // 与 `a_value_cycle_terminates_and_stays_bounded`)。
    level
        .iter()
        .map(|&l| l.min(n as i64).max(-(n as i64 + 1)))
        .collect()
}

/// 由分层算出每个节点的左上角坐标 `(x, y)`。与模型同序同长。
fn compute(m: &Model) -> Vec<(f32, f32)> {
    let lv = levels(m);
    let n = lv.len();

    // 按层分组。`lv` 是按下标递增加序遍历出来的,
    // 所以每层内的下标天然升序,不需要再排。
    // 键用 `i64`:层级可以为负(无控制流节点的值链比消费者深度还长时,
    // 会在入口列左边新开列,见 `levels`)。
    //
    // ⚠️ 必须是 `BTreeMap` 而不是 `HashMap`:下面的重心扫描与爬山**按列序
    // 迭代**,换 `HashMap` 后迭代序随进程随机(`RandomState`)—— 爬山是
    // 路径相关的局部搜索,列序不同落点不同,**布局会变成跑一次一个样**
    // (实测:同一个测试在 8 次运行里 5 红 3 绿)。BTreeMap 按键升序,
    // 布局确定。`heights` / `layer_x` 保持 `HashMap` 无妨:只做查表。
    let mut by_layer: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for (i, &l) in lv.iter().enumerate() {
        by_layer.entry(l).or_default().push(i);
    }

    // 每层高度 = 层内各节点**真实高度**之和 + 间隔。
    // 节点高度按 pin 数不同(见 [`node_height`]),这里不能再用常量 NODE_H ——
    // 层内推进也要逐节点加,否则高节点会和下一个叠上。
    let node_h = |i: usize| m.h.get(i).copied().unwrap_or(NODE_H);
    let heights: HashMap<i64, f32> = by_layer
        .iter()
        .map(|(&l, v)| {
            let h = v.iter().map(|&i| node_h(i)).sum::<f32>();
            (l, h + NODE_GAP * v.len().saturating_sub(1) as f32)
        })
        .collect();
    // 画布纵向总跨度取「最宽的一层」与 `MIN_LAYER_HEIGHT` 的较大者;
    // 空图时 `heights` 为空,`fold` 的初值 0.0 直接被 `MIN_LAYER_HEIGHT` 兜住。
    let max_h = heights
        .values()
        .copied()
        .fold(0.0f32, f32::max)
        .max(MIN_LAYER_HEIGHT);

    // 层的横向步进按**层内最宽节点**算,逐层累加 —— 宽节点(标签长的)会把
    // 后面所有层推右,不会盖到下一层的头上。全是 `NODE_W` 宽时结果和
    // 「`MARGIN + 层号 × (NODE_W + LAYER_GAP)`」完全一样。
    let node_w = |i: usize| m.w.get(i).copied().unwrap_or(NODE_W);
    let mut levels_sorted: Vec<i64> = by_layer.keys().copied().collect();
    levels_sorted.sort_unstable();
    let mut layer_x: HashMap<i64, f32> = HashMap::with_capacity(levels_sorted.len());
    let mut x = MARGIN;
    for &l in &levels_sorted {
        layer_x.insert(l, x);
        let w = by_layer[&l].iter().map(|&i| node_w(i)).fold(0.0f32, f32::max);
        x += w + LAYER_GAP;
    }

    // ---- 垂直排序(交叉消减)----
    //
    // 列内顺序不能按下标拍脑袋:值节点各喂各的消费者,消费者在同一列里的
    // 纵向位置天差地别 —— 按 index 排会把值节点甩得离消费者很远,边斜穿
    // 整列(真实数据实测 18 处交叉就是这么来的)。
    //
    // 目标函数是**所有边的纵向总行程 + 交叉数**(排序只动 y)。求解分两步:
    //
    // 1. **barycenter 扫描**当起点:每个节点往它邻居们的纵向重心靠,几轮
    //    扫描把每列重排一遍;初始下标序也在候选里,重排永远不比不排差。
    // 2. **相邻交换爬山**收尾:在列内反复试换相邻两节点,只接受目标严格
    //    下降的交换。barycenter 是启发式,单轮可能变差 —— 实测它会把某列
    //    翻错(330 → 420),而爬山从初始序就能直接摸到最优 240。
    //    两步配合:重心给好起点,爬山修局部。
    let mut nbrs: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in m.edges.iter() {
        if e.from.0 < n && e.to.0 < n && e.from.0 != e.to.0 {
            nbrs[e.from.0].push(e.to.0);
            nbrs[e.to.0].push(e.from.0);
        }
    }
    // 打包:给定列内顺序算出每节点 y。列高与顺序无关,直接复用 `heights`;
    // 窄列在自己的纵向跨度内居中,和最宽的列对齐中线。
    let pack = |order: &BTreeMap<i64, Vec<usize>>| -> Vec<f32> {
        let mut y = vec![0.0f32; n];
        for (&l, nodes) in order.iter() {
            let mut cy = MARGIN + (max_h - heights[&l]) / 2.0;
            for &node in nodes.iter() {
                y[node] = cy;
                cy += node_h(node) + NODE_GAP;
            }
        }
        y
    };
    // 参与排序评估的边(越界/自环兜底剔除,同 `levels`)。
    let edges_all: Vec<(usize, usize)> = m
        .edges
        .iter()
        .filter(|e| e.from.0 < n && e.to.0 < n && e.from.0 != e.to.0)
        .map(|e| (e.from.0, e.to.0))
        .collect();

    // 目标 = **交叉数 × 权重 + 纵向总行程**。
    //
    // 只压行程是不够的:实测共享槽长边(横跨 8 列)制造了 5 处交叉,
    // 而把它往上挪的交换行程要涨 128px —— 纯行程目标会拒绝,交叉就永远
    // 消不掉。权重取 100(≈ 3 个节点高):宁可多绕 100px 也别多一处交叉,
    // 但不接受为少一处交叉把图拉出天际。
    //
    // 交叉用"节点中心连线"近似,每次评估 O(E²)。单函数规模的图(几十条边)
    // 毫秒级;这正是这个工具面对的量级。
    const CROSS_WEIGHT: f32 = 100.0;
    let cost = |order: &BTreeMap<i64, Vec<usize>>| -> f32 {
        let y = pack(order);
        let center = |i: usize| {
            (
                layer_x[&lv[i]] + node_w(i) / 2.0,
                y[i] + node_h(i) / 2.0,
            )
        };
        let segs: Vec<((f32, f32), (f32, f32))> = edges_all
            .iter()
            .map(|&(f, t)| (center(f), center(t)))
            .collect();
        let mut crossings = 0usize;
        for i in 0..edges_all.len() {
            for j in i + 1..edges_all.len() {
                let (f1, t1) = edges_all[i];
                let (f2, t2) = edges_all[j];
                // 共享端点的边不算交叉(它们在节点处汇拢)。
                if f1 == f2 || f1 == t2 || t1 == f2 || t1 == t2 {
                    continue;
                }
                if segments_cross(segs[i].0, segs[i].1, segs[j].0, segs[j].1) {
                    crossings += 1;
                }
            }
        }
        let travel: f32 = segs.iter().map(|(a, b)| (b.1 - a.1).abs()).sum();
        crossings as f32 * CROSS_WEIGHT + travel
    };

    let mut best = by_layer.clone();
    let mut best_cost = cost(&best);
    for _ in 0..4 {
        let y = pack(&by_layer);
        for nodes in by_layer.values_mut() {
            // 稳定排序:重心相等的节点保持原相对序;无邻居的孤立节点
            // 重心取自己的 y(留在原地,不会乱飘)。
            nodes.sort_by(|&a, &b| {
                let bc = |i: usize| -> f32 {
                    if nbrs[i].is_empty() {
                        y[i]
                    } else {
                        nbrs[i].iter().map(|&j| y[j]).sum::<f32>() / nbrs[i].len() as f32
                    }
                };
                bc(a)
                    .partial_cmp(&bc(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        let c = cost(&by_layer);
        if c < best_cost {
            best_cost = c;
            best = by_layer.clone();
        }
    }

    // 爬山。目标函数每次评估 O(n + E²),列数 × 列内对数 × 轮数都很小,
    // 单函数规模的图上毫秒级。轮数封顶 8:只接受严格下降的交换,
    // 正常几轮就停(封顶是防浮点等值边界上理论上翻来覆去的保险)。
    let keys: Vec<i64> = best.keys().copied().collect();
    for _ in 0..8 {
        let mut improved = false;
        for &l in keys.iter() {
            let cnt = best[&l].len();
            for k in 0..cnt.saturating_sub(1) {
                best.get_mut(&l).unwrap().swap(k, k + 1);
                let c = cost(&best);
                if c < best_cost {
                    best_cost = c;
                    improved = true;
                } else {
                    best.get_mut(&l).unwrap().swap(k, k + 1);
                }
            }
        }
        if !improved {
            break;
        }
    }

    let y = pack(&best);
    let mut result = vec![(0.0f32, 0.0f32); n];
    for (&l, nodes) in best.iter() {
        for &node in nodes.iter() {
            result[node] = (layer_x[&l], y[node]);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::func::NodeGraphIr;
    use crate::compile::ir::IrKind;
    use crate::node::{ExportDecl, Link, NodeGraphKind};

    /// 造一个全默认尺寸(`NODE_W` × `NODE_H`)的布局模型。
    fn model(n: usize, edges: Vec<Edge>, entry: Vec<usize>) -> Model {
        Model {
            w: vec![NODE_W; n],
            h: vec![NODE_H; n],
            edges,
            entry,
        }
    }

    fn ctrl(from: usize, to: usize) -> Edge {
        Edge {
            from: (from, 0),
            to: (to, 0),
            ctrl: true,
        }
    }

    fn val(from: usize, to: usize) -> Edge {
        Edge {
            from: (from, 0),
            to: (to, 0),
            ctrl: false,
        }
    }

    /// diamond:`0(入口) → 1(if) → (2, 3) → 4(join)`
    /// 2 和 3 是 if 的两个分支,4 是汇合点。出口边不画,故 4 无出边。
    fn diamond() -> Model {
        model(
            5,
            vec![
                ctrl(0, 1),
                ctrl(1, 2),
                ctrl(1, 3),
                ctrl(2, 4),
                ctrl(3, 4),
            ],
            vec![0],
        )
    }

    #[test]
    fn level_starts_at_the_entry_node() {
        let lv = levels(&diamond());
        assert_eq!(lv[0], 0, "入口节点必须在第 0 层");
    }

    #[test]
    fn branches_share_a_level_and_join_is_deeper() {
        let lv = levels(&diamond());
        assert_eq!(lv[2], lv[3], "if 的两个分支必须同层");
        assert!(lv[4] > lv[2], "汇合点必须比两支都深");
    }

    #[test]
    fn no_edge_points_backwards() {
        let lv = levels(&diamond());
        for e in diamond().edges.iter().filter(|e| e.ctrl) {
            assert!(
                lv[e.from.0] < lv[e.to.0],
                "边往回指了: {} -> {}",
                e.from.0,
                e.to.0
            );
        }
    }

    #[test]
    fn value_edges_do_not_push_expressions_to_the_next_level() {
        let mut m = diamond();
        // 从分支 a 指向分支 b 补一条值边。关键是**这两个节点之间没有控制流边**,
        // 且两者都是控制流可达节点:值边不得进入主干松弛,否则会多出
        // depth[3] >= depth[2] + 1 这条约束,把 b 从第 2 层推到第 3 层,两支被拆开。
        //
        // (值边只用来锚定**入口走不到**的节点 —— 见 `a_value_chain_anchors_...`
        //  一组测试。若改成补 `1 -> 2` 这种与既有控制流边重复的值边,这条测试
        //  就是空的:重复边施加的是同一条约束,`filter(|e| e.ctrl)` 写没写都过。)
        m.edges.push(val(2, 3));
        let lv = levels(&m);
        assert_eq!(lv[2], lv[3], "值边不影响控制流可达节点,两支仍由控制流边决定同层");
        assert_eq!(lv[1], lv[2] - 1, "分支层仍比 if 深一层");
    }

    /// 控制流主干 `0 → 1 → 2 → 3`(层 0..3),外加一棵值流树:
    ///
    /// ```text
    ///   4(Local) → 5 → 6 ─(v)→ 2      值链:逐段左移,链尾贴消费者
    ///   7(Local) ─(v)→ 1,─(v)→ 3      共享槽:锚在最早消费者(层 1)左侧
    ///   1 ─(v)→ 8                     汇点(无消费者):贴生产者右侧(层 2,
    ///                                  刻意 ≠ 旧行为的"最右列"值 4,否则是空测试)
    /// ```
    fn value_tree() -> Model {
        model(
            9,
            vec![
                ctrl(0, 1),
                ctrl(1, 2),
                ctrl(2, 3),
                val(4, 5),
                val(5, 6),
                val(6, 2),
                val(7, 1),
                val(7, 3),
                val(1, 8),
            ],
            vec![0],
        )
    }

    /// 值链(Local → … → 消费者)必须自消费者左侧**逐段**排开,而不是掉进最右列。
    #[test]
    fn a_value_chain_anchors_left_of_its_consumer() {
        let lv = levels(&value_tree());
        assert_eq!(lv[6], lv[2] - 1, "链尾必须紧贴消费(层 2)左侧");
        assert_eq!(lv[5], lv[6] - 1, "链逐段左移一列");
        assert_eq!(lv[4], lv[5] - 1, "链首同理 —— 链比消费者更深时允许为负");
    }

    /// 共享槽(一个源、多个消费者)锚在**最早**的消费者上,
    /// 到更远消费者的边拉长,但方向仍然朝右。
    #[test]
    fn a_shared_value_source_anchors_at_its_earliest_consumer() {
        let lv = levels(&value_tree());
        assert_eq!(lv[7], lv[1] - 1, "必须锚在最早消费者(层 1)左侧");
        assert!(lv[7] < lv[3], "到远处消费者(层 3)的边仍然正向");
    }

    /// 没有消费者的汇点(值最终没人接)紧贴生产者,而不是丢到最右一列。
    #[test]
    fn a_consumerless_value_sink_hugs_its_producer() {
        let lv = levels(&value_tree());
        assert_eq!(lv[8], lv[1] + 1, "汇点必须在生产者右侧一列");
    }

    /// 锚定之后,**每条值边都从左往右指** —— 旧行为是全部从最右一列往回指。
    #[test]
    fn every_value_edge_points_forward() {
        let m = value_tree();
        let lv = levels(&m);
        for e in m.edges.iter().filter(|e| !e.ctrl) {
            assert!(
                lv[e.from.0] < lv[e.to.0],
                "值边往回指了: {} -> {}",
                e.from.0,
                e.to.0
            );
        }
    }

    /// 列内顺序按**邻居纵向重心**重排:节点往自己消费者/生产者的纵向位置靠,
    /// 而不是按下标拍脑袋 —— 不重排时值节点会被甩到离消费者很远的位置,
    /// 边斜穿整列(真实数据 18 处交叉的来源)。
    #[test]
    fn value_nodes_are_reordered_to_sit_near_their_consumers() {
        let m = value_tree();
        let pos = compute(&m);
        // 列 0 初始下标序是 [0(ctrl), 5, 7]:5 的消费者 #6 在列 1 底部,
        // 7 的消费者(#1@列1 顶部、#3@列3 中部)整体更靠上 —— 重排后 5 沉底。
        assert!(
            pos[5].1 > pos[7].1,
            "5 的消费者最低,必须排在 7 下面:5@y{} 7@y{}",
            pos[5].1,
            pos[7].1
        );
        // 列 2 初始 [2, 8]:8 的生产者 #1 在列 1 顶部,2 的生产者 #6 在底部 —— 翻转。
        assert!(
            pos[8].1 < pos[2].1,
            "8 的生产者最高,必须排在 2 上面:8@y{} 2@y{}",
            pos[8].1,
            pos[2].1
        );
    }

    /// 坐标层面:值链渲染在消费者左侧;链深超过消费者深度时在入口列**左边**新开列。
    #[test]
    fn a_value_chain_renders_left_of_its_consumer() {
        let m = value_tree();
        let pos = compute(&m);
        assert!(
            pos[4].0 < pos[5].0 && pos[5].0 < pos[6].0 && pos[6].0 < pos[2].0,
            "值链必须自左向右排到消费者:{} {} {} {}",
            pos[4].0,
            pos[5].0,
            pos[6].0,
            pos[2].0
        );
        assert!(pos[7].0 < pos[1].0, "共享槽在最早消费者左侧");
        assert!(pos[4].0 < pos[0].0, "负层级列排在入口列左侧");
    }

    /// 值流内部有环(任意拖进来的 `.ogia` 可能有):min 松弛在环上不收敛,
    /// 靠轮数封顶 + 钳位兜住 —— 与 `levels_returns_on_a_cyclic_graph`
    /// 同一类防护。没有防护时本测试靠「跑不完」失败。
    ///
    /// ⚠️ **环的方向是下标逆序(5 → 4 → 3 → 2 → 5),这是刻意的**:
    /// 松弛按下标序扫,环上每个节点的后继下标都比自己小 —— 一轮之内一次波
    /// 就能把整圈的下调级联起来,每轮发散量是复合的(6 轮冲到 ≈ -22,远超
    /// 下界 -7),**下界钳位因此是承重的**,去掉它 `…stays_bounded` 必红
    /// (变异验证过)。反过来,下标顺序的 3 环发散慢,跑满 n 轮恰好停在
    /// -(n+1) 边界上 —— 去掉钳位测试照样过,是**空测试**。改这个夹具前
    /// 先想清楚你在测什么。
    fn value_cycle() -> Model {
        model(
            6,
            vec![
                ctrl(0, 1),
                // 值环 5 → 4 → 3 → 2 → 5(逆序),且 5 有一条值边流进控制流节点 1
                val(5, 4),
                val(4, 3),
                val(3, 2),
                val(2, 5),
                val(5, 1),
            ],
            vec![0],
        )
    }

    #[test]
    fn a_value_cycle_terminates_and_stays_bounded() {
        let m = value_cycle();
        let lv = levels(&m); // 无上界时会挂死,本测试就是靠「跑不完」失败的
        assert_eq!(lv.len(), 6, "有环也必须给每个节点一个层级");
        let n = m.w.len() as i64;
        for (i, &l) in lv.iter().enumerate() {
            assert!(
                (-(n + 1)..=n).contains(&l),
                "节点 {i} 的层级 {l} 超出界 [-{}, {n}],画布宽度会失控",
                n + 1
            );
        }
        assert_eq!(lv[0], 0, "控制流入口不受值环影响");
        assert_eq!(lv[1], 1, "控制流节点不受值环影响");
        assert_eq!(compute(&m).len(), 6, "有环时坐标也必须算得出来");
    }

    #[test]
    fn positions_increase_with_level() {
        let pos = compute(&diamond());
        assert!(pos[4].0 > pos[2].0, "汇合点必须在两支的右边");
        assert!(pos[2].1 != pos[3].1, "同层不同节点不能重叠");
    }

    /// 层的横向步进按层内**最宽**节点算:宽节点把后面所有层推右,
    /// 而不是按常量 `NODE_W` 一律加 220。
    #[test]
    fn a_wide_node_pushes_later_layers_right() {
        let mut m = diamond();
        m.w[1] = 300.0; // if 独占层 1
        let pos = compute(&m);
        assert_eq!(pos[1].0 - pos[0].0, NODE_W + LAYER_GAP, "层 0 不受影响");
        assert_eq!(pos[2].0 - pos[1].0, 300.0 + LAYER_GAP, "层 1 按最宽节点步进");
        assert_eq!(pos[2].0, pos[3].0, "同层仍然对齐");
    }

    /// 节点高度按 pin 数不同(见 [`node_height`]),同层内推进必须用**真实高度** ——
    /// 用常量 `NODE_H` 的实现在这里失败:高节点会和下一个叠上。
    ///
    /// 两个分支节点**都**加高到 64:高度相等时交换两者对纵向行程是中性,
    /// 重心/爬山都不会翻顺序,断言"2 在 3 前"才稳定。若只加高 2,实测
    /// 爬山会把 64 的节点换到下面(行程更短)—— 那是新排序的合法行为,
    /// 这个测试的假设就不成立了。
    #[test]
    fn taller_nodes_do_not_overlap_in_a_layer() {
        let mut m = diamond();
        // 2 和 3 同层(都是 if 的分支),两个都加高到 64
        m.h[2] = 64.0;
        m.h[3] = 64.0;
        let pos = compute(&m);
        assert_eq!(pos[2].0, pos[3].0, "前提:两者仍同层");
        assert!(
            pos[3].1 >= pos[2].1 + 64.0 + NODE_GAP,
            "高节点(64)与下一个之间必须留出 NODE_GAP:{} → {}",
            pos[2].1,
            pos[3].1
        );
    }

    #[test]
    fn unreachable_nodes_still_get_positions_in_the_last_column() {
        let mut m = diamond();
        m.w.push(NODE_W); // 5 孤立节点
        m.h.push(NODE_H);
        let pos = compute(&m);
        let orphan_x = pos[5].0;
        assert!(orphan_x > pos[0].0, "孤立节点必须排到最右一列");
        assert!(orphan_x > pos[4].0, "孤立节点必须在所有可达节点右边");
    }

    #[test]
    fn empty_graph_produces_empty_output() {
        let m = model(0, vec![], vec![]);
        assert!(levels(&m).is_empty());
        assert!(compute(&m).is_empty());
    }

    /// `0 → 1 → 2 → 0` 的控制流环。环上三条约束首尾相接、互相矛盾,
    /// `changed` 永远为真。
    ///
    /// ⚠️ **`entry: vec![0]` 非空是所有用到它的测试的承重前提。**
    /// `entry` 为空时,环上三个节点全被判为不可达、被松弛循环里的 `!reachable[u]`
    /// 门整个挡掉 —— 松弛压根碰不到环,于是**有环的图和有环无关的图表现完全一样,
    /// 下面两条测试都会恒真通过**。变异验证过:同时删掉轮数上界**并**把 `entry`
    /// 清空,`levels_returns_on_a_cyclic_graph` 0.00s 通过、exit 0。
    /// 改这个夹具前先想清楚你在测什么。
    fn cyclic() -> Model {
        model(3, vec![ctrl(0, 1), ctrl(1, 2), ctrl(2, 0)], vec![0])
    }

    /// **锁住 `levels` 的轮数上界。** 没有这条测试,下一个人把 `for _ in 0..n`
    /// 改回无界 `loop` 不会有任何测试变红 —— 而那等于给一个调试工具装了个
    /// 100% CPU 死循环。
    ///
    /// 断言只说「**有限时间内返回、不 panic、长度正确**」—— 具体层级不锁死:
    /// 那是尽力而为的产物,超限时节点可能被排得比理想值更深,变了也不该让本
    /// 测试变红。真正要锁的性质是**它会停下来**。
    /// 层级上界由下面的 `cyclic_levels_stay_within_the_node_count` 单独锁。
    #[test]
    fn levels_returns_on_a_cyclic_graph() {
        let m = cyclic();
        // 下面两行会挂死,若上界被删掉,本测试就是靠「跑不完」失败的。
        let lv = levels(&m);
        assert_eq!(lv.len(), 3, "有环也必须给每个节点一个层级");
        // `compute` 内部同样走 `levels`,渲染路径一并覆盖。
        assert_eq!(compute(&m).len(), 3, "有环时坐标也必须算得出来");
    }

    /// **锁住 `levels` 返回前的钳位。**
    ///
    /// 有环时约束互相矛盾,轮数上界一放开深度就线性膨胀:每轮每条边 +1,
    /// 跑满 n 轮是 **O(n²)** 量级(实测 500 节点的有环图 → 层级到万级 →
    /// 画布宽约 5.5e7 px,用户拖不进、滚不到、看不见任何节点)。
    /// 图算出来了,但等于没算 —— 和挂死是同一类失败,而且**有环恰恰是
    /// 用户最想看的那个 case**。
    ///
    /// 钳位成 O(n) 足够:无环图上 `depth` 本来就 < n(最长路径 ≤ n-1),钳位是
    /// 严格 no-op;有环图上只需要把画布宽度压回可操作范围,不需要真实的环语义 ——
    /// 环在画布上一眼可见,不需要程序替他算对。
    #[test]
    fn cyclic_levels_stay_within_the_node_count() {
        let m = cyclic();
        let lv = levels(&m);
        let n = m.w.len();
        for (i, &l) in lv.iter().enumerate() {
            assert!(
                l <= n as i64,
                "节点 {i} 的层级 {l} 超过节点数 {n},画布会宽到用户看不见东西"
            );
        }
    }

    // ---- 对 `NodeGraph` 入口(link.rs / viewer 走这条)的测试 ----

    fn int() -> IrKind {
        IrKind::Native(crate::value::NativeKind::Int)
    }

    fn set_local_kind() -> NodeKind<IrNodeId, IrKind> {
        NodeKind::new(
            IrNodeId::SetLocal,
            1,
            1,
            vec![Some(int()), Some(int())],
            vec![],
        )
    }

    /// **`Node::new` 的坐标必须是 (0,0)** —— `.ogia` 里没被布局过的图
    /// 反序列化出来就是这个值,消费方(如 viewer 的拖动初值)依赖它。
    #[test]
    fn new_nodes_start_at_the_origin() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        let key = g.insert(set_local_kind());
        assert_eq!(g.get_node(key).position, (0.0, 0.0));
    }

    /// 走 `NodeGraph` 入口:控制流链的坐标写进 `Node::position`,向右排开。
    #[test]
    fn layout_writes_positions_into_the_graph() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        // 控制流边界先声明:入口连 `Link::export(0)` 落在 `controls_out`。
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));
        let entry = g.insert(set_local_kind());
        let tail = g.insert(set_local_kind());
        g.link_control(Link::export(0), Link::node(entry, 0));
        g.link_control(Link::node(entry, 0), Link::node(tail, 0));
        g.link_control(Link::node(tail, 0), Link::export(0));
        layout(&mut g);
        let (a, b) = (g.get_node(entry).position, g.get_node(tail).position);
        assert!(b.0 > a.0, "控制流向右:入口 {a:?} 尾块 {b:?}");
        assert_eq!(a.1, b.1, "直链同一行");
    }

    /// 宽度按**调用方给的标签**估算:`layout_with` 传长标签,层步进必须
    /// 变宽 —— viewer 就是靠这个钩子把 `<类型>` 后缀算进宽度的。
    #[test]
    fn layout_with_uses_the_given_label_for_width() {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));
        let entry = g.insert(set_local_kind());
        let tail = g.insert(set_local_kind());
        g.link_control(Link::export(0), Link::node(entry, 0));
        g.link_control(Link::node(entry, 0), Link::node(tail, 0));
        g.link_control(Link::node(tail, 0), Link::export(0));
        layout_with(&mut g, |_| "x".repeat(60));
        let (a, b) = (g.get_node(entry).position, g.get_node(tail).position);
        assert!(
            b.0 - a.0 > NODE_W + LAYER_GAP,
            "长标签必须把后面的列推右:{a:?} → {b:?}"
        );
    }
}
