//! 主画布的**纯计算**层 —— 拍出这一帧要画什么、命中了什么。
//!
//! 这里的东西**不产生 GPUI 元素**,只产出数据:
//!
//! - [`NodeView`] / [`EdgeView`] / [`BoundaryView`] / [`PinView`] /
//!   [`DefaultView`] / [`CtrlLabelView`] —— 这一帧要画的每样东西的**位置和
//!   文案**;
//! - [`hit_test`] —— 光标下是什么。
//!
//! 渲染层(`view.rs`)只负责把这些数据画出来。分开的理由:
//!
//! - 命中测试和几何是**最容易算错**的部分(用户报过"看到的和命中的不是
//!   两套几何"),它们必须只有**一份**实现;
//! - 它们不依赖 GPUI 的元素系统,所以能直接单测 —— `view.rs` 里那 30 多个
//!   几何测试搬到这里,不用绕开 `Viewer` 才能测(它构造不出来:`FocusHandle`
//!   没有 `Default`)。
//!
//! # 坐标
//!
//! 所有坐标都是**容器局部**坐标(容器左上角为原点),已经乘过缩放。窗口
//! 坐标由 `view.rs` 在 canvas 里加上 `bounds.origin` 换算。

use gpui::{Point, Size, point, size};

use rust2genshin::compile::ir::IrNodeId;

use crate::doc::{
    Edge, GraphEntry, NodeEntry, PinRows, comment_height, comment_width, pin_rows,
};
use crate::theme::node_color;

// ------------------------------------------------------------------ 视图数据

/// 这一帧要画的一个节点。
pub struct NodeView {
    pub index: usize,
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub border: gpui::Rgba,
    /// 这是不是复合节点(`Fn`)。Ctrl+点击 / Ctrl+悬停只对它有意义。
    pub is_fn: bool,
    /// 这个节点当前是不是被选中(单选 / 框选)。选中态用另一种描边色。
    pub selected: bool,
    /// 图边界节点(`entry` / `exit`)描粗一档。
    ///
    /// ⚠️ 这**只是**"这里是图边界"的视觉标记,**不是**可达性结论,
    /// 更不能读成"这里断了" —— 本工具不做任何 CF 分析。
    pub boundary: bool,
    /// 这一帧的缩放。字号要跟着它走 —— `text_xs()` 那种固定字号不随缩放变。
    pub scale: f32,
    /// 节点上方的注释:文本 + 估算出来的宽高(世界坐标,未乘缩放)。
    pub comment: Option<(String, f32, f32)>,
}

/// 一条边已经算好的三次贝塞尔(四个控制点,容器局部坐标)。
pub struct EdgeView {
    pub ctrl: bool,
    pub from: Point<f32>,
    pub a: Point<f32>,
    pub b: Point<f32>,
    pub to: Point<f32>,
    /// 悬停气泡的文案(`edge_tip` 算好)。控制流边是 `None`。
    pub tip: Option<String>,
    /// 边在 `g.edges` 里的下标 —— 命中测试找到边之后要靠它反查文案,
    /// 也让同一条边的两个端点共享一个身份。
    pub index: usize,
}

/// 一个图边界 pin 的画布表现:从 pin 位往图外的一小段直线(桩),
/// 末端挂一个写边界号的「Export N」标签框。
/// 控制流用实线蓝、值流用虚线黄,和对应边的样式一致。
#[derive(Clone)]
pub struct BoundaryView {
    pub ctrl: bool,
    /// 桩的两个端点(容器局部坐标)。
    pub from: Point<f32>,
    pub to: Point<f32>,
    /// 图外边界号(`ExportPin::export`)。
    pub export: usize,
}

/// 桩的长度(1 倍缩放时;跟着画布缩放走)。
pub const STUB_LEN: f32 = 18.0;
/// 「Export N」标签框的尺寸(1 倍缩放时)。
pub const TAG_W: f32 = 64.0;
pub const TAG_H: f32 = 18.0;
/// 桩末端与标签框之间的缝。
pub const TAG_GAP: f32 = 4.0;

/// 引脚圆点的直径(1 倍缩放时)。
pub const PIN_D: f32 = 7.0;
/// 默认值文本的框宽(1 倍缩放时)。框**右对齐**到引脚左侧 ——
/// 文字长度不确定,固定右缘才能和引脚严丝合缝。
pub const DEF_W: f32 = 48.0;

/// 节点标签两侧的留白合计(世界坐标),和渲染层 `node_box_el` 的
/// `.px_1()`(左右各 4px)+ `.border_1()`(左右各 1px)对齐,再留一点余量。
///
/// **调这个值就能调"节点有多松"**:调大 → 字和边框之间更空;调小 → 更紧。
/// 现在取 18(左右各 9px),比原来的 `.px_1()` 只宽一点点 —— 用户报的是
/// "空白很大",所以偏向紧凑。
pub const NODE_TEXT_PAD: f32 = 18.0;

/// 节点渲染宽度的**下限**(世界坐标)。
///
/// 只保证"再短的名字也有个像样的盒子",不是布局用的宽度:
///
/// - **布局**(`core::node::layout`)按 [`NODE_W`](rust2genshin::node::layout::NODE_W)
///   这个名义列宽排位置;
/// - **渲染**按"实测文字宽 + padding"决定盒子,下限是这个值。
///
/// 两者故意不同:名义列宽比典型节点宽,所以节点之间自然留出空隙;
/// 而盒子贴着文字,不再有"左右各空一大截"的观感(用户报的问题)。
pub const NODE_MIN_W: f32 = 56.0;

/// 引脚 / 边的命中半径(容器局部坐标,**不随缩放变**)。
///
/// 圆点本身只有 `PIN_D * scale` —— 缩到 0.2 倍时直径不到 1.5px,真按图形
/// 面积去命中几乎点不中。所以命中半径给一个**固定**的屏幕像素数(缩放无关),
/// 让"小图也能指着"这件事成立。边同理:1px 宽的虚线不能用线宽当命中区。
pub const PIN_HIT_R: f32 = 7.0;
pub const EDGE_HIT_R: f32 = 5.0;

/// 引脚 / 边的悬停文案里,"这个 pin 没有类型"的固定说法。
pub const NONE_TIP: &str = "None";

/// 拍出这一帧的图边界(桩线 + 末端标签的数据)。
///
/// 入边一侧(左)的桩往左伸、出边一侧(右)的桩往右伸;纵向位置用和连线
/// 完全一样的 [`pin_y`],所以桩和线的端点严丝合缝 —— 就是把「连到图外」
/// 的那半条线画出来,再由标签框说明连到**哪一号**边界。
pub fn build_boundaries(g: &GraphEntry, origin: Point<f32>, scale: f32) -> Vec<BoundaryView> {
    let len = STUB_LEN * scale;
    let mut out = Vec::new();
    for (i, n) in g.nodes.iter().enumerate() {
        let Some((p, s)) = node_box(&g.nodes, origin, scale, i) else {
            continue;
        };
        let (left_rows, right_rows) = pin_rows(
            n.controls_in_num,
            n.values_in_num,
            n.controls_out_num,
            n.values_out_num,
        );
        let mut stub = |ctrl: bool, left: bool, pin: usize, export: usize| {
            let (ctrl_num, value_num, rows) = if left {
                (n.controls_in_num, n.values_in_num, left_rows)
            } else {
                (n.controls_out_num, n.values_out_num, right_rows)
            };
            let y = pin_y(
                p.y,
                scale,
                ctrl_num,
                value_num,
                rows.value_start,
                ctrl,
                pin,
            );
            let x = if left { p.x } else { p.x + s.width };
            out.push(BoundaryView {
                ctrl,
                from: point(x, y),
                to: point(if left { x - len } else { x + len }, y),
                export,
            });
        };
        for e in &n.ctrl_in_exports {
            stub(true, true, e.pin, e.export);
        }
        for e in &n.ctrl_out_exports {
            stub(true, false, e.pin, e.export);
        }
        for e in &n.value_in_exports {
            stub(false, true, e.pin, e.export);
        }
        for e in &n.value_out_exports {
            stub(false, false, e.pin, e.export);
        }
    }
    out
}

/// 「Export N」标签框的位置与尺寸(容器局部坐标,已乘 `scale`):
/// 贴在桩末端**外侧**,纵向以桩线为中心。
pub fn tag_box(b: &BoundaryView, scale: f32) -> (Point<f32>, Size<f32>) {
    let (w, h) = (TAG_W * scale, TAG_H * scale);
    let x = if b.to.x >= b.from.x {
        b.to.x + TAG_GAP * scale // 右侧出:框在末端右边
    } else {
        b.to.x - w - TAG_GAP * scale // 左侧出:框在末端左边
    };
    (point(x, b.to.y - h / 2.0), size(w, h))
}

/// 节点侧边上的一个引脚标记。
pub struct PinView {
    pub x: f32,
    pub y: f32,
    pub ctrl: bool,
    /// 悬停气泡的文案。控制流 pin 是 `None` —— 控制流不分类型,给不出
    /// 有意义的话。值流 pin 是该 pin 的类型标签,空串(`""`)表示"这个 pin
    /// 没有类型",气泡里显示 `None`。
    pub tip: Option<String>,
}

/// 值入 pin 的默认值文本(容器局部坐标)。
pub struct DefaultView {
    /// 文本框的**右缘** x(框向左展开,文字右对齐)。
    pub right: f32,
    pub y: f32,
    pub text: String,
}

/// 控制出 pin 的分支名文本(容器局部坐标),画在**节点盒子内侧**。
pub struct CtrlLabelView {
    /// 文本框的**右缘** x(框向左展开,文字右对齐)。
    pub right: f32,
    pub y: f32,
    pub text: String,
}

/// 命中测试的结果:找到的东西 + 气泡落在哪儿。
pub struct Hover {
    /// 气泡文案(已算好,渲染直接用)。
    pub text: String,
    /// 气泡锚点,**容器局部**坐标 —— 跟着光标走,不是钉在被命中的元素上
    /// (钉在 pin 上会让气泡盖住圆点本身,反而看不清)。
    pub at: Point<f32>,
}

// ------------------------------------------------------------------ 几何

/// 侧边上第 `pin` 个 pin 的纵向位置(容器局部坐标,已含 `scale`)。
///
/// 几何**全部由 core 提供**(`node::layout`):引脚靠上排(`pin_row_y`),
/// 值流的起始行跨两侧取最大(`pin_rows`)—— 见那边的注释。
/// **不要**在这里自己算一份:画布、「调用约定」视图、`.gia` 的自动整理
/// 用的必须是同一套,否则它们会互相漂开。
///
/// `top` 是节点顶部(容器局部 y),`ctrl_num` / `value_num` / `value_start`
/// 来自 [`pin_rows`](rust2genshin::node::layout::pin_rows)。
pub fn pin_y(
    top: f32,
    scale: f32,
    ctrl_num: usize,
    value_num: usize,
    value_start: usize,
    is_ctrl: bool,
    pin: usize,
) -> f32 {
    use rust2genshin::node::layout::{pin_row_y, side_pin_ys};
    let (ctrl, value) = side_pin_ys(ctrl_num, value_num, value_start);
    let row = if is_ctrl {
        ctrl.get(pin).copied()
    } else {
        value.get(pin).copied()
    };
    // 拿不到(畸形数据 / 越界)时退回首行 —— 不 panic。
    top + row.unwrap_or_else(|| pin_row_y(0)) * scale
}

/// 节点 `i` 的左上角(容器局部坐标)与尺寸。
///
/// 坐标直接读节点的 `position` —— 它是**唯一**的一份(见 `doc::NodeEntry`
/// 的字段注释),这里不再另收一个 `pos` 参数。以前那个平行数组是同一份
/// 数据的两份拷贝,靠"同序同长"的不成文约定维系,拖动时要手工同步。
pub fn node_box(
    nodes: &[NodeEntry],
    origin: Point<f32>,
    scale: f32,
    i: usize,
) -> Option<(Point<f32>, Size<f32>)> {
    let n = nodes.get(i)?;
    let p = n.position;
    Some((
        point(
            origin.x + p.x * scale,
            origin.y + p.y * scale,
        ),
        size_scaled(n.size, scale),
    ))
}

fn size_scaled(s: Size<f32>, scale: f32) -> Size<f32> {
    Size {
        width: s.width * scale,
        height: s.height * scale,
    }
}

// ------------------------------------------------------------------ 拍帧

/// 拍出这一帧要画的所有节点。
pub fn build_nodes(
    g: &GraphEntry,
    origin: Point<f32>,
    scale: f32,
    selected: &[usize],
) -> Vec<NodeView> {
    g.nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            // 标签就是节点本身,不再拼 `[in]` / `[out]` 后缀 —— 图边界由侧边的
            // 桩线标,比后缀多一层信息:是**哪一个引脚**。
            let (p, s) = node_box(&g.nodes, origin, scale, i).unwrap_or_default();
            NodeView {
                index: i,
                label: n.display.clone(),
                x: p.x,
                y: p.y,
                w: s.width,
                h: s.height,
                border: node_color(&n.kind),
                is_fn: matches!(n.kind, IrNodeId::Fn(_)),
                selected: selected.contains(&i),
                boundary: g.entry.contains(&i) || g.exit.contains(&i),
                scale,
                comment: n.comment.as_ref().map(|c| {
                    (
                        c.clone(),
                        comment_width(c),
                        comment_height(c),
                    )
                }),
            }
        })
        .collect()
}

/// 拍出这一帧要画的边(三次贝塞尔)。
pub fn build_edges(g: &GraphEntry, origin: Point<f32>, scale: f32) -> Vec<EdgeView> {
    g.edges
        .iter()
        .enumerate()
        .filter_map(|(index, e): (usize, &Edge)| {
            let (from, fs) = node_box(&g.nodes, origin, scale, e.from.0)?;
            let (to, _ts) = node_box(&g.nodes, origin, scale, e.to.0)?;
            // 两侧各自的 pin 行数(值流起点跨两侧取最大 —— 见 core 的
            // `pin_rows`)。拿不到节点时按"没有引脚"兜底,不 unwrap。
            let (from_rows, to_rows) = match (g.nodes.get(e.from.0), g.nodes.get(e.to.0)) {
                (Some(a), Some(b)) => (
                    pin_rows(
                        a.controls_in_num,
                        a.values_in_num,
                        a.controls_out_num,
                        a.values_out_num,
                    )
                    .1,
                    pin_rows(
                        b.controls_in_num,
                        b.values_in_num,
                        b.controls_out_num,
                        b.values_out_num,
                    )
                    .0,
                ),
                _ => (
                    PinRows {
                        value_start: 0,
                        rows: 0,
                    },
                    PinRows {
                        value_start: 0,
                        rows: 0,
                    },
                ),
            };
            let (out_c, out_v) = g
                .nodes
                .get(e.from.0)
                .map_or((0, 0), |n| (n.controls_out_num, n.values_out_num));
            let (in_c, in_v) = g
                .nodes
                .get(e.to.0)
                .map_or((0, 0), |n| (n.controls_in_num, n.values_in_num));
            let y0 = pin_y(
                from.y,
                scale,
                out_c,
                out_v,
                from_rows.value_start,
                e.ctrl,
                e.from.1,
            );
            let y1 = pin_y(
                to.y,
                scale,
                in_c,
                in_v,
                to_rows.value_start,
                e.ctrl,
                e.to.1,
            );

            // 目标在源左边时(有回边的控制流图;值流的源常在最右一列、
            // 往左指)`to.x - from.x` 是负的,取绝对值再夹一个下限,
            // 避免控制点挤在一起看不出走向。
            let dx = ((to.x - (from.x + fs.width)).abs() * 0.5).max(4.0);
            Some(EdgeView {
                ctrl: e.ctrl,
                from: point(from.x + fs.width, y0),
                a: point(from.x + fs.width + dx, y0),
                b: point(to.x - dx, y1),
                to: point(to.x, y1),
                tip: edge_tip(g, e),
                index,
            })
        })
        .collect()
}

/// 节点侧边上的**全部**引脚标记 —— 无论有没有连线都画,
/// 「这个节点有几个脚、哪个脚空着」才一眼可见。
pub fn build_pins(g: &GraphEntry, origin: Point<f32>, scale: f32) -> Vec<PinView> {
    let mut out = Vec::new();
    for (i, n) in g.nodes.iter().enumerate() {
        let Some((p, s)) = node_box(&g.nodes, origin, scale, i) else {
            continue;
        };
        // 入边一侧在左边缘、出边一侧在右边缘。纵向位置由 core 的
        // `pin_rows` / `pin_row_y` 决定(引脚靠上排,值流起点跨两侧取最大)。
        let (left_rows, right_rows) = pin_rows(
            n.controls_in_num,
            n.values_in_num,
            n.controls_out_num,
            n.values_out_num,
        );
        for (ctrl_num, value_num, rows, is_out, x) in [
            (n.controls_in_num, n.values_in_num, left_rows, false, p.x),
            (
                n.controls_out_num,
                n.values_out_num,
                right_rows,
                true,
                p.x + s.width,
            ),
        ] {
            for k in 0..(ctrl_num + value_num) {
                let ctrl = k < ctrl_num;
                out.push(PinView {
                    x,
                    y: pin_y(
                        p.y,
                        scale,
                        ctrl_num,
                        value_num,
                        rows.value_start,
                        ctrl,
                        if ctrl { k } else { k - ctrl_num },
                    ),
                    ctrl,
                    // 控制流 pin 一般没有类型可显示 —— 唯一的例外是
                    // `Switch` 的分支名(`ctrl_out_labels`),出边才标。
                    tip: if ctrl {
                        is_out
                            .then(|| n.ctrl_out_labels.get(k).cloned())
                            .flatten()
                    } else {
                        // 值流 pin 的 pin 序号是 `k - ctrl_num`(值流排在控制流下方)。
                        Some(pin_tip(n, is_out, k - ctrl_num))
                    },
                });
            }
        }
    }
    out
}

/// 拍出这一帧要显示的默认值。位置:紧贴值入引脚圆点左侧、纵向居中对齐。
pub fn build_defaults(g: &GraphEntry, origin: Point<f32>, scale: f32) -> Vec<DefaultView> {
    let mut out = Vec::new();
    for (i, n) in g.nodes.iter().enumerate() {
        let Some((p, _)) = node_box(&g.nodes, origin, scale, i) else {
            continue;
        };
        let (left_rows, _) = pin_rows(
            n.controls_in_num,
            n.values_in_num,
            n.controls_out_num,
            n.values_out_num,
        );
        for (pin, default) in n.value_in_defaults.iter().enumerate() {
            let Some(text) = default else {
                continue;
            };
            out.push(DefaultView {
                right: p.x - (PIN_D / 2.0 + 3.0) * scale,
                y: pin_y(
                    p.y,
                    scale,
                    n.controls_in_num,
                    n.values_in_num,
                    left_rows.value_start,
                    false,
                    pin,
                ),
                text: text.clone(),
            });
        }
    }
    out
}

/// 拍出这一帧要显示的控制出分支名。位置:节点盒内、紧贴控制出引脚左侧。
///
/// 画在盒内而不是盒外 —— 盒子外面就是连线,名字压在线上两边都看不清。
/// 宽度固定(`DEF_W`)且**不裁剪**:分支名长度不定(`default` 短、`"MyStruct"`
/// 长),宁可伸出去也别只剩尾巴。
pub fn build_ctrl_labels(g: &GraphEntry, origin: Point<f32>, scale: f32) -> Vec<CtrlLabelView> {
    let mut out = Vec::new();
    for (i, n) in g.nodes.iter().enumerate() {
        let Some((p, s)) = node_box(&g.nodes, origin, scale, i) else {
            continue;
        };
        let (_, right_rows) = pin_rows(
            n.controls_in_num,
            n.values_in_num,
            n.controls_out_num,
            n.values_out_num,
        );
        for (pin, text) in n.ctrl_out_labels.iter().enumerate() {
            out.push(CtrlLabelView {
                right: p.x + s.width - (PIN_D / 2.0 + 3.0) * scale,
                y: pin_y(
                    p.y,
                    scale,
                    n.controls_out_num,
                    n.values_out_num,
                    right_rows.value_start,
                    true,
                    pin,
                ),
                text: text.clone(),
            });
        }
    }
    out
}

// ------------------------------------------------------------------ 悬停文案

/// 值流 pin 的类型文案。`is_out` 决定读出参还是入参的表,`pin` 是**值流里**
/// 的序号(控制流 pin 不带类型,不走这里)。
///
/// 两张表都可能越界 / 为空(展平层不保证),拿不到就回 [`NONE_TIP`] 而不是
/// panic —— 悬停是纯交互增强,不能因为某个畸形 `.ogia` 把窗口带走。
pub fn pin_tip(n: &NodeEntry, is_out: bool, pin: usize) -> String {
    let raw = if is_out {
        n.values_out_types.get(pin)
    } else {
        n.values_in_types.get(pin)
    };
    match raw {
        Some(s) if !s.is_empty() => s.clone(),
        _ => NONE_TIP.to_string(),
    }
}

/// 值流边的悬停文案:两端类型**一致**就只显示那一个,**不一致**就两个都显示
/// (`A → B`)。控制流边没有类型,回 `None`。
///
/// 分隔符用 `→` 而不是 `-` / `,`:连线上本来就有方向感,箭头读起来就是
/// "从哪个类型流到哪个类型",不会和负号、连字符混淆。
pub fn edge_tip(g: &GraphEntry, e: &Edge) -> Option<String> {
    if e.ctrl {
        return None;
    }
    let from = g.nodes.get(e.from.0).map(|n| pin_tip(n, true, e.from.1));
    let to = g.nodes.get(e.to.0).map(|n| pin_tip(n, false, e.to.1));
    match (from, to) {
        // 节点下标越界(展平层不保证,理论上不该发生)—— 两端都拿不到就
        // 当作没类型,别 unwrap。
        (None, None) | (None, _) | (_, None) => Some(NONE_TIP.to_string()),
        (Some(a), Some(b)) if a == b => Some(a),
        (Some(a), Some(b)) => Some(format!("{a} → {b}")),
    }
}

// ------------------------------------------------------------------ 命中

/// 两点距离。
pub fn dist(a: Point<f32>, b: Point<f32>) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// 命中测试:离光标最近的**有气泡的** pin / 边。
///
/// pin 优先于边 —— 引脚是小目标、又压在边上面,两者都够近时选 pin 才是
/// 用户想要的(他要的是那个引脚的类型,不是那条线的)。同类里取最近的;
/// 完全不在半径内回 `None`。
///
/// 边用**折线采样**近似贝塞尔:三次贝塞尔没有"点到曲线距离"的闭式解,
/// 采样足够密(24 段)时误差远小于命中半径,不值得为此引一个几何库。
pub fn hit_test(pins: &[PinView], edges: &[EdgeView], at: Point<f32>) -> Option<Hover> {
    // pin 优先:有 tip 的 pin 里找最近的。
    let pin = pins
        .iter()
        .filter_map(|p| p.tip.as_ref().map(|t| (dist(point(p.x, p.y), at), t)))
        .filter(|(d, _)| *d <= PIN_HIT_R)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, text)) = pin {
        return Some(Hover {
            text: text.clone(),
            at,
        });
    }
    // 没有 pin 命中,再找边。
    let edge = edges
        .iter()
        .filter_map(|e| e.tip.as_ref().map(|t| (bezier_dist(e, at), t)))
        .filter(|(d, _)| *d <= EDGE_HIT_R)
        .min_by(|a, b| a.0.total_cmp(&b.0))?;
    Some(Hover {
        text: edge.1.clone(),
        at,
    })
}

/// 光标到三次贝塞尔的最短距离(折线采样近似)。
///
/// 采样点之间的连线段用**点到线段**的距离算,不是点到采样点的距离 ——
/// 后者在采样稀疏时会明显偏大,曲线弧度大的地方直接漏判。
pub fn bezier_dist(e: &EdgeView, at: Point<f32>) -> f32 {
    const SAMPLES: usize = 24;
    let mut prev = e.from;
    let mut best = f32::MAX;
    for i in 1..=SAMPLES {
        let t = i as f32 / SAMPLES as f32;
        let cur = bezier_at(e, t);
        best = best.min(seg_dist(at, prev, cur));
        prev = cur;
    }
    best
}

/// 三次贝塞尔在参数 `t` 处的点。控制点是 `from` / `a` / `b` / `to`
/// (与 [`EdgeView`] 字段同名,顺序也和 `PathBuilder::cubic_bezier_to` 一致)。
pub fn bezier_at(e: &EdgeView, t: f32) -> Point<f32> {
    let u = 1.0 - t;
    let (uu, uuu) = (u * u, u * u * u);
    let (tt, ttt) = (t * t, t * t * t);
    // 标准三次贝塞尔:B(t) = (1-t)³P₀ + 3(1-t)²t·P₁ + 3(1-t)t²·P₂ + t³P₃
    let w = [uuu, 3.0 * uu * t, 3.0 * u * tt, ttt];
    let ps = [e.from, e.a, e.b, e.to];
    point(
        w.iter().zip(ps.iter()).map(|(a, b)| a * b.x).sum(),
        w.iter().zip(ps.iter()).map(|(a, b)| a * b.y).sum(),
    )
}

/// 点到线段 `a`-`b` 的最短距离。`a == b` 时退化成点到点距离(除零保护)。
pub fn seg_dist(p: Point<f32>, a: Point<f32>, b: Point<f32>) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 <= f32::EPSILON {
        return dist(p, a);
    }
    // 投影参数夹到 [0, 1] —— 落在线段延长线上的点要夹回端点。
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    dist(p, point(a.x + dx * t, a.y + dy * t))
}

/// 控制流引脚的三角形顶点(包在直径 `d` 的方框里、**尖端向右**)。
///
/// 形状即方向:控制流从左往右走,输入侧的引脚也朝右指 —— 两端一致的箭头
/// 读起来就是"往这边流",比圆点多一层信息。值流不分方向(数据可回指),
/// 保持圆点。
///
/// div 画不出三角形(gpui 的边框只有一个颜色,拼不出 CSS 那套 border 三角),
/// 渲染时用 canvas 填色画;顶点在做几何这里算,**可单测**。
pub fn pin_triangle(cx: f32, cy: f32, d: f32) -> [Point<f32>; 3] {
    [
        point(cx - d / 2.0, cy - d / 2.0),
        point(cx - d / 2.0, cy + d / 2.0),
        point(cx + d / 2.0, cy),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use gpui::Size;

    use rust2genshin::compile::ir::IrNodeId;
    use rust2genshin::node::NodeRef;

    use crate::doc::{NODE_H, NODE_W, Edge, GraphEntry, NodeEntry, pin_rows};

    #[test]
    fn value_pins_sit_below_control_pins() {
        let top = 100.0;
        let (c, v) = (2, 2);
        let (c0, c1, v0, v1) = (
            pin_y(top, 1.0, c, v, c, true, 0),
            pin_y(top, 1.0, c, v, c, true, 1),
            pin_y(top, 1.0, c, v, c, false, 0),
            pin_y(top, 1.0, c, v, c, false, 1),
        );
        assert!(c0 < c1 && c1 < v0 && v0 < v1, "顺序:{c0} {c1} {v0} {v1}");
        assert!(v0 > c1, "值流最靠上的 pin 也要在控制流最靠下的 pin 之下");
        // **靠上**:第一个引脚就在顶部附近,不是浮在中间。
        assert!(c0 < top + 20.0, "第一个引脚应当靠上:{c0}");
    }

    /// 纯值节点(GetLocal / 表达式,控制 pin 为 0):值 pin 从顶部开始排。
    #[test]
    fn value_only_node_starts_at_the_top() {
        let (y0, y1, y2) = (
            pin_y(0.0, 1.0, 0, 3, 0, false, 0),
            pin_y(0.0, 1.0, 0, 3, 0, false, 1),
            pin_y(0.0, 1.0, 0, 3, 0, false, 2),
        );
        assert!(y0 < y1 && y1 < y2, "依次排开:{y0} {y1} {y2}");
        assert!(y0 < 20.0, "第一个引脚靠上:{y0}");
    }

    /// **左右控制流数量不同时,第一个数据流引脚仍然对齐**(用户要求)。
    ///
    /// 值流的起始行跨两侧取最大 —— 各算各的话会错开一格。
    #[test]
    fn the_first_value_pin_lines_up_when_control_counts_differ() {
        // 左边 1 个控制流,右边 3 个。
        let (l, r) = pin_rows(1, 2, 3, 2);
        assert_eq!(l.value_start, 3, "值流起点取两侧最大(3)");
        assert_eq!(r.value_start, 3);
        // 两侧第一个值流引脚的 y 必须**完全相等**。
        let left_first = pin_y(0.0, 1.0, 1, 2, l.value_start, false, 0);
        let right_first = pin_y(0.0, 1.0, 3, 2, r.value_start, false, 0);
        assert_eq!(left_first, right_first, "首个数据流引脚要对齐");
        // 控制流各自从顶部开始,数量不同不影响它们自己的位置。
        assert_eq!(pin_y(0.0, 1.0, 1, 2, l.value_start, true, 0), pin_y(0.0, 1.0, 3, 2, r.value_start, true, 0));
    }

    fn node(controls_in: usize, controls_out: usize, values_in: usize, values_out: usize) -> NodeEntry {
        node_with_types(controls_in, controls_out, values_in, values_out, &[], &[])
    }

    /// 给 `g` 里的节点**按序**设定坐标(坐标只存在节点上,没有平行的数组了)。
    fn placed(mut g: GraphEntry, pos: &[(f32, f32)]) -> GraphEntry {
        for (n, (x, y)) in g.nodes.iter_mut().zip(pos.iter()) {
            n.position = point(*x, *y);
        }
        g
    }

    /// [`node`] 加上逐 pin 类型(悬停气泡用)。类型表给**空串**当"无类型"。
    fn node_with_types(
        controls_in: usize,
        controls_out: usize,
        values_in: usize,
        values_out: usize,
        in_types: &[&str],
        out_types: &[&str],
    ) -> NodeEntry {
        NodeEntry {
            node_ref: NodeRef::from(0),
            // 渲染测试自带坐标,不走布局。
            position: point(0.0, 0.0),
            kind: IrNodeId::SetLocal,
            display: "SetLocal".to_string(),
            type_label: String::new(),
            controls_in_num: controls_in,
            controls_out_num: controls_out,
            values_in_num: values_in,
            values_out_num: values_out,
            values_in_types: in_types.iter().map(|s| s.to_string()).collect(),
            values_out_types: out_types.iter().map(|s| s.to_string()).collect(),
            ctrl_out_labels: vec![],
            ctrl_in_exports: vec![],
            ctrl_out_exports: vec![],
            value_in_exports: vec![],
            value_out_exports: vec![],
            value_in_defaults: vec![],
            comment: None,
            size: Size {
                width: NODE_W,
                height: NODE_H,
            },
        }
    }

    /// 节点宽度由「实测文字宽 + `NODE_TEXT_PAD`」决定,渲染盒子用 `.min_w()`。
    ///
    /// ⚠️ **这两个值必须对得上**,否则右侧引脚会错位:盒子的真实宽度是
    /// "文字宽 + `.px_1()` 的左右 8px + 边框 2px",而几何按
    /// `实测宽 + NODE_TEXT_PAD` 算右边缘。只要 `NODE_TEXT_PAD` **不小于**
    /// 8 + 2,盒子就由文字撑开、宽度正好等于几何用的值;小于的话盒子会比
    /// 几何算的宽,右侧引脚就跑到盒子里面去了。
    ///
    /// 这条测试锁的就是"不许把 padding 调到比 `.px_1()` + 边框还小"。
    #[test]
    fn the_width_padding_covers_the_box_padding_and_border() {
        // `.px_1()` = 左右各 4px;`.border_1()` = 左右各 1px。
        let box_padding_and_border = 4.0 * 2.0 + 1.0 * 2.0;
        assert!(
            NODE_TEXT_PAD >= box_padding_and_border,
            "NODE_TEXT_PAD({NODE_TEXT_PAD}) 不能小于盒子的 padding + 边框\
             ({box_padding_and_border}),否则右侧引脚会跑进盒子内部"
        );
        // 同时别太松 —— 用户报的是"空白很大"。
        assert!(
            NODE_TEXT_PAD <= 28.0,
            "NODE_TEXT_PAD({NODE_TEXT_PAD}) 太大,节点内部又会空出一大截"
        );
    }

    /// 注释要跟着节点进 `NodeView`,并带上 core 那套尺寸估算 ——
    /// 渲染时按它把文本框摆在节点上方。
    #[test]
    fn comments_reach_the_node_view_with_their_estimated_size() {
        let mut n = node(1, 1, 0, 0);
        n.comment = Some("第一行
第二行".into());
        let g = placed(
            GraphEntry {
                name: "t".to_string(),
                index_of: HashMap::new(),
                entry: vec![],
                exit: vec![],
                nodes: vec![n],
                edges: vec![],
                interface: Default::default(),
            },
            &[(40.0, 80.0)],
        );
        let ns = build_nodes(&g, point(0.0, 0.0), 2.0, &[]);
        let (text, w, h) = ns[0].comment.as_ref().expect("注释没进 NodeView");
        assert_eq!(text, "第一行
第二行");
        assert_eq!(*w, comment_width(text));
        assert_eq!(*h, comment_height(text));
    }

    /// 值流边也从**侧边的 pin 位**进出(右出、左进),不再从节点中心的上下缘。
    /// 纵向位置必须落在值流那一档,而不是节点中心(`NODE_H / 2`)。
    #[test]
    fn value_edges_leave_from_the_side_pins() {
        let g = placed(
            GraphEntry {
                name: "t".to_string(),
                index_of: HashMap::new(),
                entry: vec![0],
                exit: vec![],
                // 0:1 控制出 + 2 值出;1:1 控制入 + 2 值入
                nodes: vec![node(0, 1, 0, 2), node(1, 0, 2, 0)],
                interface: Default::default(),
                edges: vec![
                    // 控制边:0 控制出 0 -> 1 控制入 0
                    Edge {
                        from: (0, 0),
                        to: (1, 0),
                        ctrl: true,
                    },
                    // 值边:0 值出 1 -> 1 值入 1
                    Edge {
                        from: (0, 1),
                        to: (1, 1),
                        ctrl: false,
                    },
                ],
            },
            &[(0.0, 0.0), (300.0, 0.0)],
        );
        let es = build_edges(&g, point(0.0, 0.0), 1.0);
        assert_eq!(es.len(), 2);

        let ctrl = &es[0];
        assert!(ctrl.ctrl);
        assert_eq!(ctrl.from.x, NODE_W, "从来源右边缘出");
        assert_eq!(ctrl.to.x, 300.0, "进目标左边缘");
        // 引脚**靠上**排:控制流 pin 0 在首行(`PIN_TOP`),不是节点中心。
        assert_eq!(ctrl.from.y, crate::doc::PIN_TOP);
        assert_eq!(ctrl.to.y, crate::doc::PIN_TOP);

        let value = &es[1];
        assert!(!value.ctrl, "仍是值流样式(虚线)");
        assert_eq!(value.from.x, NODE_W);
        assert_eq!(value.to.x, 300.0);
        // 值流 pin **1**(第 2 个值引脚):值流从第 1 行开始,所以它落在第 2 行。
        let expect = crate::doc::pin_row_y(2);
        assert_eq!(value.from.y, expect, "落在值流那一行");
        assert_eq!(value.to.y, expect);
        assert!(value.from.y > ctrl.from.y, "值流的连接点在控制流下方");
    }

    /// 图边界:导出的 pin 往图外伸一段(入边一侧向左、出边一侧向右),
    /// 纵向用同一套 `pin_y` 与连线严丝合缝;**普通连接的 pin 一根桩都不该有**。
    /// 末端标签框要带上**具体**的边界号。
    #[test]
    fn export_pins_get_stubs_and_plain_pins_do_not() {
        use crate::doc::ExportPin;

        let mut n0 = node(1, 1, 2, 2);
        n0.ctrl_in_exports = vec![ExportPin { pin: 0, export: 0 }];
        n0.value_out_exports = vec![ExportPin { pin: 1, export: 7 }];
        let g = placed(
            GraphEntry {
                name: "t".to_string(),
                index_of: HashMap::new(),
                entry: vec![0],
                exit: vec![],
                nodes: vec![n0],
                edges: vec![],
                interface: Default::default(),
            },
            &[(10.0, 20.0)],
        );
        let bs = build_boundaries(&g, point(0.0, 0.0), 1.0);
        assert_eq!(bs.len(), 2, "只有一个控制入桩和一个值出桩,别的 pin 不给桩");

        let ctrl = bs.iter().find(|b| b.ctrl).expect("有控制流桩");
        assert_eq!(ctrl.from.x, 10.0, "从节点左边缘起");
        assert_eq!(ctrl.to.x, 10.0 - STUB_LEN, "往左伸");
        assert_eq!(ctrl.from.y, ctrl.to.y, "桩是水平的");
        // 入边侧控制流 pin 0 在**首行**。
        assert_eq!(ctrl.from.y, 20.0 + crate::doc::PIN_TOP);
        assert_eq!(ctrl.export, 0, "控制入连的是边界 0");
        let (tp, tsz) = tag_box(ctrl, 1.0);
        assert_eq!(tp.x, ctrl.to.x - TAG_W - TAG_GAP, "左侧出:框在末端左边");
        assert_eq!(tp.y + tsz.height / 2.0, ctrl.to.y, "框纵向以桩线为中心");

        let value = bs.iter().find(|b| !b.ctrl).expect("有值流桩");
        assert_eq!(value.from.x, 10.0 + NODE_W, "从节点右边缘起");
        assert_eq!(value.to.x, 10.0 + NODE_W + STUB_LEN, "往右伸");
        // 出边侧值流 pin 1 紧接在 1 个控制流之后 —— 第 3 行(0 基)。
        assert_eq!(value.from.y, 20.0 + crate::doc::pin_row_y(2));
        assert_eq!(value.export, 7, "边界号要逐 pin 对上,不是清一色 0");
        let (vp, _) = tag_box(value, 1.0);
        assert_eq!(vp.x, value.to.x + TAG_GAP, "右侧出:框在末端右边");
    }

    /// 控制流引脚的形状是**向右指的三角形**(形状即方向,输入端也朝右);
    /// 值流保持圆点(渲染层按 `ctrl` 分流,见 `render`)。
    #[test]
    fn control_pins_are_right_pointing_triangles() {
        let t = pin_triangle(100.0, 50.0, 7.0);
        assert_eq!(t[0].x, t[1].x, "底边两个顶点必须同 x(竖直边在左)");
        assert!(t[2].x > t[0].x, "尖端必须在底边**右侧** —— 这就是\"向右\"");
        assert_eq!(t[2].y, (t[0].y + t[1].y) / 2.0, "尖端在底边中点高度,纵向对称");
        assert_eq!(t[0].x, 100.0 - 3.5, "包在直径 7 的框里,和圆点一样重");
        assert_eq!(t[2].x, 100.0 + 3.5);
        assert_eq!(t[2].y, 50.0, "中心即给定坐标");
    }

    /// 每个连接点都有标记,**零连线也画**:1 控制 + 2 值 → 一侧 3 个。
    #[test]
    fn every_pin_gets_a_dot_even_without_edges() {
        let g = placed(
            GraphEntry {
                name: "t".to_string(),
                index_of: HashMap::new(),
                entry: vec![0],
                exit: vec![],
                nodes: vec![node(1, 1, 2, 2)],
                edges: vec![],
                interface: Default::default(),
            },
            &[(10.0, 20.0)],
        );
        let pins = build_pins(&g, point(0.0, 0.0), 1.0);
        assert_eq!(pins.len(), 6, "入 3 + 出 3,哪怕一条边都没有");

        let in_side: Vec<_> = pins.iter().filter(|p| p.x == 10.0).collect();
        assert_eq!(in_side.len(), 3);
        // 档位顺序:控制流在前(蓝),值流依次在后(黄)
        assert!(in_side[0].ctrl, "第 1 档是控制流");
        assert!(!in_side[1].ctrl && !in_side[2].ctrl, "后两档是值流");
        assert!(in_side[0].y < in_side[1].y && in_side[1].y < in_side[2].y);
        // 控制流在**首行**(靠上,不是居中)。
        assert_eq!(in_side[0].y, 20.0 + crate::doc::PIN_TOP);

        let out_side: Vec<_> = pins.iter().filter(|p| p.x == 10.0 + NODE_W).collect();
        assert_eq!(out_side.len(), 3, "出边一侧也要画");
        // 靠上:第一个引脚就在顶部附近,整排不越过节点高度。
        assert!(out_side[0].y < 20.0 + 20.0, "第一个引脚靠上");
        assert!(out_side.iter().all(|p| p.y < 20.0 + crate::doc::node_height(3)));
    }

    /// 值入默认值:只有 `Some` 的才显示,右缘贴引脚圆点左侧、纵向对齐该 pin。
    #[test]
    fn defaults_hang_left_of_their_pin() {
        let mut n0 = node(1, 0, 2, 0);
        n0.value_in_defaults = vec![None, Some("5".to_string())];
        let g = placed(
            GraphEntry {
                name: "t".to_string(),
                index_of: HashMap::new(),
                entry: vec![0],
                exit: vec![],
                nodes: vec![n0],
                edges: vec![],
                interface: Default::default(),
            },
            &[(10.0, 20.0)],
        );
        let ds = build_defaults(&g, point(0.0, 0.0), 1.0);
        assert_eq!(ds.len(), 1, "只有 pin 1 有默认值");
        assert_eq!(ds[0].text, "5");
        assert_eq!(ds[0].right, 10.0 - (PIN_D / 2.0 + 3.0), "右缘贴引脚左侧");
        // 值入 pin **1**(第 2 个值引脚):值流从第 1 行开始,所以落在第 2 行。
        assert_eq!(
            ds[0].y,
            20.0 + crate::doc::pin_row_y(2),
            "纵向对齐值入 pin 1"
        );
    }

    // ------------------------------------------------------- 悬停:气泡文案

    /// 引脚气泡 = 该 pin 的类型;**没有类型就显示 `None`**,不是空串。
    ///
    /// 空串看起来像"功能坏了",`None` 才是信息:这个 pin 确实没类型。
    #[test]
    fn pin_tooltip_shows_the_type_or_none() {
        let n = node_with_types(0, 0, 2, 1, &["", "Int"], &["Bool"]);
        assert_eq!(pin_tip(&n, false, 0), "None", "空串类型 → None");
        assert_eq!(pin_tip(&n, false, 1), "Int", "有类型就显示它");
        assert_eq!(pin_tip(&n, true, 0), "Bool", "出参类型");
        // 越界不能 panic —— 悬停是交互增强,畸形 .ogia 不能把窗口带走。
        assert_eq!(pin_tip(&n, false, 99), "None");
        assert_eq!(pin_tip(&n, true, 99), "None");
    }

    /// 边气泡:两端**一致**只显示一个;**不一致**显示两个(`A → B`)。
    /// 控制流边没有类型,不给气泡。
    #[test]
    fn edge_tooltip_shows_one_type_when_equal_and_two_otherwise() {
        let mut g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![],
            exit: vec![],
            nodes: vec![
                node_with_types(0, 0, 0, 2, &[], &["Int", "Int"]),
                node_with_types(0, 0, 2, 0, &["Int", "Bool"], &[]),
            ],
            edges: vec![],
            interface: Default::default(),
        };
        let edge = |from: usize, from_pin: usize, to: usize, to_pin: usize, ctrl: bool| Edge {
            from: (from, from_pin),
            to: (to, to_pin),
            ctrl,
        };

        // Int -> Int:两端一致,只显示一个
        g.edges = vec![edge(0, 0, 1, 0, false)];
        assert_eq!(edge_tip(&g, &g.edges[0]).as_deref(), Some("Int"));

        // Int -> Bool:不一致,两个都显示
        g.edges = vec![edge(0, 0, 1, 1, false)];
        assert_eq!(edge_tip(&g, &g.edges[0]).as_deref(), Some("Int → Bool"));

        // 控制流边:没有类型,没有气泡
        g.edges = vec![edge(0, 0, 1, 0, true)];
        assert_eq!(edge_tip(&g, &g.edges[0]), None);
    }

    /// 一端没类型(`None`)时不能 panic,也不能静默少显示一端 ——
    /// `None → Bool` 才是完整信息。
    #[test]
    fn edge_tooltip_shows_none_for_an_untyped_end() {
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![],
            exit: vec![],
            nodes: vec![
                node_with_types(0, 0, 0, 1, &[], &[""]),
                node_with_types(0, 0, 1, 0, &["Bool"], &[]),
            ],
            edges: vec![Edge {
                from: (0, 0),
                to: (1, 0),
                ctrl: false,
            }],
            interface: Default::default(),
        };
        assert_eq!(edge_tip(&g, &g.edges[0]).as_deref(), Some("None → Bool"));
    }

    // ------------------------------------------------------- 悬停:命中测试

    /// 命中半径**不随缩放变** —— 缩到 0.2 倍时圆点直径不到 1.5px,按图形
    /// 面积命中几乎点不中。这里锁住"固定屏幕像素"这个性质。
    #[test]
    fn hover_hit_radius_is_independent_of_zoom() {
        assert!(PIN_HIT_R >= 5.0, "命中半径至少要够指着一个 7px 的点");
        assert!(EDGE_HIT_R >= 3.0, "1px 宽的虚线不能按线宽命中");
    }

    /// 光标压在引脚圆点上 → 命中那个引脚的气泡。偏 2px(仍在半径内)也要中。
    #[test]
    fn hovering_a_pin_hits_that_pin() {
        let g = placed(
            GraphEntry {
                name: "t".to_string(),
                index_of: HashMap::new(),
                entry: vec![],
                exit: vec![],
                nodes: vec![node_with_types(1, 0, 1, 0, &["Int"], &[])],
                edges: vec![],
                interface: Default::default(),
            },
            &[(0.0, 0.0)],
        );
        let pins = build_pins(&g, point(0.0, 0.0), 1.0);
        let value_pin = pins
            .iter()
            .find(|p| p.tip.is_some())
            .expect("有一个值流 pin");

        let hit = hit_test(&pins, &[], point(value_pin.x, value_pin.y))
            .expect("压在引脚上应当命中");
        assert_eq!(hit.text, "Int");
        // 偏 2px 仍在半径内
        assert_eq!(
            hit_test(&pins, &[], point(value_pin.x + 2.0, value_pin.y))
                .map(|h| h.text),
            Some("Int".to_string()),
            "半径内偏一点也要命中"
        );
        // 偏到半径外就不该命中了
        assert!(
            hit_test(&pins, &[], point(value_pin.x + PIN_HIT_R * 4.0, value_pin.y)).is_none(),
            "半径外不该命中"
        );
    }

    /// **引脚优先于边**:两者都够近时选引脚 —— 引脚是小目标、又压在边上面,
    /// 用户指着那个点要的是它的类型,不是那条线的。只查边的实现会在这里
    /// 返回边的文案。
    #[test]
    fn a_pin_wins_over_an_edge_at_the_same_spot() {
        let g = placed(
            GraphEntry {
                name: "t".to_string(),
                index_of: HashMap::new(),
                entry: vec![],
                exit: vec![],
                nodes: vec![
                    node_with_types(0, 0, 0, 1, &[], &["Int"]),
                    node_with_types(0, 0, 1, 0, &["Float"], &[]),
                ],
                edges: vec![Edge {
                    from: (0, 0),
                    to: (1, 0),
                    ctrl: false,
                }],
                interface: Default::default(),
            },
            &[(0.0, 0.0), (300.0, 0.0)],
        );
        let pins = build_pins(&g, point(0.0, 0.0), 1.0);
        let edges = build_edges(&g, point(0.0, 0.0), 1.0);
        // 边的**起点**正好是源节点的出 pin —— 两边都在命中半径内。
        let start = edges[0].from;
        let hit = hit_test(&pins, &edges, start).expect("起点上应当命中");
        assert_eq!(hit.text, "Int", "引脚优先,不是边的 `Int → Float`");
    }

    /// 光标压在边的**中段**(不在任何引脚附近)→ 命中那条边的气泡。
    /// 这条锁住"贝塞尔曲线上的点要能命中" —— 用端点近似(只测两端连线)
    /// 的实现会在弧度大的边上完全测不到中段。
    #[test]
    fn hovering_the_middle_of_an_edge_hits_the_edge() {
        let g = placed(
            GraphEntry {
                name: "t".to_string(),
                index_of: HashMap::new(),
                entry: vec![],
                exit: vec![],
                nodes: vec![
                    node_with_types(0, 0, 0, 1, &[], &["Int"]),
                    node_with_types(0, 0, 1, 0, &["Float"], &[]),
                ],
                edges: vec![Edge {
                    from: (0, 0),
                    to: (1, 0),
                    ctrl: false,
                }],
                interface: Default::default(),
            },
            &[(0.0, 0.0), (300.0, 0.0)],
        );
        let edges = build_edges(&g, point(0.0, 0.0), 1.0);
        // 端点上下 40px,弧度最大处;离两端引脚都远。
        let mid = bezier_at(&edges[0], 0.5);
        assert!(
            dist(mid, edges[0].from) > PIN_HIT_R,
            "中段离起点引脚够远,测的确实是曲线不是端点"
        );
        let hit = hit_test(&[], &edges, mid).expect("曲线上应当命中边");
        assert_eq!(hit.text, "Int → Float", "两端不一致显示两个");
    }

    /// 空白处不该命中任何东西(不给气泡)。
    #[test]
    fn empty_space_hits_nothing() {
        let g = placed(
            GraphEntry {
                name: "t".to_string(),
                index_of: HashMap::new(),
                entry: vec![],
                exit: vec![],
                nodes: vec![node_with_types(0, 0, 0, 1, &[], &["Int"])],
                edges: vec![],
                interface: Default::default(),
            },
            &[(0.0, 0.0)],
        );
        let pins = build_pins(&g, point(0.0, 0.0), 1.0);
        assert!(hit_test(&pins, &[], point(5000.0, 5000.0)).is_none());
    }

    /// 控制流引脚**不给**气泡:控制流不分类型,显示 `None` 只会误导
    /// (读起来像"这个引脚没连上")。
    #[test]
    fn control_pins_have_no_tooltip() {
        let g = placed(
            GraphEntry {
                name: "t".to_string(),
                index_of: HashMap::new(),
                entry: vec![],
                exit: vec![],
                nodes: vec![node(1, 1, 0, 0)],
                edges: vec![],
                interface: Default::default(),
            },
            &[(0.0, 0.0)],
        );
        let pins = build_pins(&g, point(0.0, 0.0), 1.0);
        assert!(pins.iter().all(|p| p.tip.is_none()), "控制流 pin 不带 tip");
        for p in &pins {
            assert!(
                hit_test(&pins, &[], point(p.x, p.y)).is_none(),
                "控制流引脚上不该有气泡"
            );
        }
    }

    /// 点到线段的距离:落在线段**延长线**上的点要夹回端点,不能算投影。
    /// 投影不夹的话,`seg_dist` 会给一个远小于实际的值,命中范围会顺着
    /// 线段方向无限延伸出去。
    #[test]
    fn seg_dist_clamps_to_the_endpoints() {
        let a = point(0.0, 0.0);
        let b = point(10.0, 0.0);
        // 正上方中点:距离 5
        assert!((seg_dist(point(5.0, 5.0), a, b) - 5.0).abs() < 1e-4);
        // 延长线上、b 右边 100px:夹回 b,距离 100(不是 0)
        assert!(
            (seg_dist(point(110.0, 0.0), a, b) - 100.0).abs() < 1e-4,
            "延长线上的点要夹回端点"
        );
        // 退化线段 a == b:退化成点到点,不能除零
        assert!((seg_dist(point(3.0, 4.0), a, a) - 5.0).abs() < 1e-4);
    }

    /// `bezier_dist` 采样的是**点到线段**,不是点到采样点。
    ///
    /// 用一条**直线**贝塞尔(四个控制点共线 = 曲线就是那条直线)当基准:
    /// 已知解析解,和采样近似对比。采样点之间的空隙里,"到最近采样点"会
    /// **偏大**(24 段时最坏差 ~L/48),超过命中半径时就会漏判;
    /// "到线段"没有这个误差。这条测试把查询点放在两段采样点的**正中间**
    /// —— 那正是点到点会露馅的位置(压在线上的点测不出来,必须偏开)。
    #[test]
    fn bezier_dist_measures_to_the_segment_not_the_sample_point() {
        // 水平直线 0 → 200,四个控制点共线。
        let straight = EdgeView {
            ctrl: false,
            from: point(0.0, 0.0),
            a: point(200.0 / 3.0, 0.0),
            b: point(400.0 / 3.0, 0.0),
            to: point(200.0, 0.0),
            tip: None,
            index: 0,
        };
        // 曲线是直线,所以"到曲线的最短距离"有解析解 = 到那条线段的距离。
        // 折线采样**到线段**时两者应当几乎相等(折线就是那条直线本身);
        // 采样**到点**时会偏大 —— 偏大量约等于采样步长的一半。
        // 查询点故意取在两个采样点的**正中间**,那是最能暴露差别的地方。
        let step = 200.0 / 24.0;
        for (k, x) in [step * 0.5, step * 12.5, step * 23.5]
            .into_iter()
            .enumerate()
        {
            let query = point(x, 3.0);
            let exact = seg_dist(query, straight.from, straight.to);
            let approx = bezier_dist(&straight, query);
            assert!(
                (approx - exact).abs() < 1e-3,
                "查询点 {k}(x={x}):实测 {approx},解析解 {exact} —— \
                 偏大说明用的是点到采样点而不是点到线段"
            );
        }
        // 顺带钉住"压在线上"这个基准:距离就是 0。
        assert!(bezier_dist(&straight, point(100.0, 0.0)) < 1e-3);
    }

    /// 贝塞尔端点:t=0 落在 `from`,t=1 落在 `to` —— 采样距离的边界条件,
    /// 错了会让端点附近的命中偏掉。
    #[test]
    fn bezier_endpoints_are_exact() {
        let e = EdgeView {
            ctrl: false,
            from: point(0.0, 0.0),
            a: point(50.0, 100.0),
            b: point(150.0, -100.0),
            to: point(200.0, 0.0),
            tip: None,
            index: 0,
        };
        assert_eq!(bezier_at(&e, 0.0), e.from);
        assert_eq!(bezier_at(&e, 1.0), e.to);
        // 中点必须在控制点凸包内(不会跑到曲线外面去)
        let mid = bezier_at(&e, 0.5);
        assert!(mid.x > 0.0 && mid.x < 200.0);
    }
}
