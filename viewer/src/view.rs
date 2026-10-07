//! GPUI 渲染层 —— `canvas` 画边 + 绝对定位 `div` 画节点,外加三个交互。
//!
//! # 拖拽路线:**路 A**(把 `on_mouse_move` 挂在覆盖整个画布的容器上,节点只挂 `on_mouse_down`)
//!
//! brief 给的两条路里选路 A,依据是读了 gpui 0.2.2 的命中测试实现:
//!
//! - `Window::hit_test`(`window.rs:775-797`)从**最后插入的 hitbox 往前**遍历,
//!   把所有"包含光标"的 hitbox 全部收进 `ids`;只有遇到
//!   `HitboxBehavior::BlockMouse` 的那个才 `break`。
//! - `BlockMouse` **只有** `InteractiveElement::occlude()` 会设(`div.rs:576`),
//!   默认是 `HitboxBehavior::Normal`(`window.rs:570-573`)。本文件不调 `occlude`,
//!   所以循环不会提前 `break`,`hover_hitbox_count` 最终等于 `ids.len()`。
//! - 于是**容器的 hitbox 在"指针压在某个节点上"时同样算 hovered**,
//!   `on_mouse_move` 的 `hitbox.is_hovered(window)` 门(`div.rs:268`)过得去。
//! - 顺带确认了 move 事件是**无条件派发**的:`dispatch_mouse_event`(`window.rs:3677`)
//!   对每个鼠标事件都跑一遍 capture + bubble,不因为"元素没被 hover"而跳过。
//!
//! 路 B(`on_drag` + `on_drag_move`)确实更"语义正确",但 `on_drag` 的 constructor
//! 要返回一个 `Entity<W> where W: Render` 拖拽负载(`div.rs:499`),为了拖一个节点
//! 建一整个 Render 实体不划算;路 A 已经够用。
//!
//! 事件分工:节点的 `on_mouse_down` 只记 `drag = Some(Drag { node: Some(i) })` 并
//! `stop_propagation()`(否则同一次按下会既拖节点又平移画布);
//! 容器的 `on_mouse_down` 看到冒上来的事件就记 `node: None`(平移);
//! 移动、缩放全部在容器上。
//!
//! # 坐标
//!
//! `positions[图下标]` 是那张图的节点坐标,与 `doc.graphs[图下标].nodes`
//! **同序同长**(来源:`Doc::push_target` / `Doc::relayout` 里 core 布局写进
//! IR 的 `Node::position`,展平时照抄),所以渲染和拖动里到处是
//! `nodes[i]` ↔ `pos[i]` 的直接配对。**不要**用 `HashMap<NodeRef, Point<f32>>`
//! 当单张图内的坐标系:那需要反查 `NodeRef`,而 `NodeRef` 与 Vec 下标不恒等
//! (Slab 删过节点后 key 有空洞)。
//!
//! 布局只在**加载时**算一次(`Viewer::add_target`),之后靠拖动累积;
//! **切图不重算**,拖过的位置切走再切回来还在。要重算只有一条路:
//! 点右栏的「自动整理」(`Viewer::arrange`)。
//!
//! 渲染期一律 `pos.get(i).copied().unwrap_or_default()`,**从不直接索引** ——
//! 长度短暂不齐(优化删完节点、坐标还没跟着重排)是可能的。

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

// `gpui::util` 是私有模块,`FluentBuilder`(提供 `.when(..)`)只能从 prelude 拿。
use gpui::prelude::FluentBuilder;
use gpui::{
    Bounds, Context, ExternalPaths, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, PathBuilder, Pixels, Point, Render, ScrollDelta,
    ScrollWheelEvent, Size, StatefulInteractiveElement, Styled, Window, canvas, div, hsla, point,
    px, rgba, size,
};
use rust2genshin::compile::ir::IrNodeId;
use rust2genshin::compile::link::Target;
use rust2genshin::node::NodeRef;

use crate::doc::{BASE_FONT_PX, Doc, Edge, GraphEntry, NodeEntry, label};
use crate::optimize::{self, RULES, Snapshot};

// Ctrl+Z → 撤销。`actions!` 展开出一个单元结构体 + `gpui::Action` 实现;
// 键位在 `main.rs` 绑(`AppContext::bind_keys`),处理也挂在 App 级
// (`AppContext::on_action`)。**不依赖焦点** —— 画布里的节点 div 都没焦点,
// 走元素级 `on_key_down` 得先搭一套 focus 机制,不值。
gpui::actions!([Undo]);

/// 缩放下限。再小节点上就是一个点,看不清。
const MIN_SCALE: f32 = 0.2;
/// 缩放上限。再大单个节点糊满整屏,也看不清边。
const MAX_SCALE: f32 = 4.0;
/// 一格滚轮的缩放倍率。
const ZOOM_STEP: f32 = 1.1;

// ------------------------------------------------------------------ 配色

// `rgb` / `rgba` / `hsla` 都不是 `const fn`,只能包一层函数,没法写成常量。
fn canvas_bg() -> gpui::Rgba {
    rgba(0x11141aff)
}
/// 左栏底色。比画布**亮一档**,既和画布分开、也让字有足够对比度。
/// 之前用 0x161a22 太暗,字和底色几乎糊在一起。
fn panel_bg() -> gpui::Rgba {
    rgba(0x232a36ff)
}
fn edge_ctrl() -> gpui::Rgba {
    rgba(0x7aa2f7c0)
}
/// 控制流引脚(三角形)的填充色。比 [`edge_ctrl`](边)**更深、不透明** ——
/// 三角形就压在节点的边缘和浅蓝的控制流线上,用同一个色会和线糊在一起,
/// 分不出哪是端点哪是线。
fn pin_ctrl() -> gpui::Rgba {
    rgba(0x3b5bdbff)
}
fn edge_value() -> gpui::Rgba {
    rgba(0xe0af6888)
}
fn node_bg() -> gpui::Rgba {
    rgba(0x1b2130ff)
}
fn text_hi() -> gpui::Rgba {
    rgba(0xe6e9f0ff)
}
fn text_dim() -> gpui::Rgba {
    rgba(0x8a93a6ff)
}

/// 节点类别色(描边用)。
///
/// 配色**必须**按类别区分 —— 这是 spec 的可读性要求,和标签是两回事:
/// 标签只特判两个带载荷的变体(见 [`label`]),这里必须穷尽八个。
fn node_color(kind: &IrNodeId) -> gpui::Rgba {
    match kind {
        IrNodeId::Local => rgba(0x9aa3b2ff),                              // 灰
        IrNodeId::SetLocal => rgba(0x4f8ef7ff),                           // 蓝
        IrNodeId::Unreachable => rgba(0xb03434ff),                        // 暗红
        IrNodeId::Assemble | IrNodeId::Destructure | IrNodeId::Modify => {
            rgba(0x3fa45bff) // 绿
        }
        IrNodeId::Fn(_) => rgba(0xd98c2bff), // 橙
        // 原生节点按 `id` 哈希取色:不同的原生节点天然不同色,同一个节点
        // 在图里每次出现都是同一个色,读图时能对得上。
        IrNodeId::Native(n) => {
            let h = (n.id.wrapping_mul(0x9E37_79B1) as u32) as f32 / u32::MAX as f32;
            hsla(h, 0.45, 0.55, 1.0).into()
        }
        _ => rgba(0xb03434ff),
    }
}

// ------------------------------------------------------------------ 渲染数据

/// 一个节点画在画布上所需的全部信息。
///
/// 渲染前先拍成 owned 值:事件闭包要在 `'static` 里活过这一帧,带不走
/// `&mut Viewer`,只能带走拷贝。
struct NodeView {
    /// 在 `pos` 里的下标 —— 按下鼠标时靠它找回"是哪个节点"。
    index: usize,
    label: String,
    /// **容器局部**坐标(已经乘过 `scale`),直接喂给 `left()` / `top()`。
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    border: gpui::Rgba,
    /// 图边界节点(`entry` / `exit`)描粗一档。
    ///
    /// ⚠️ 这**只是**"这里是图边界"的视觉标记,**不是**可达性结论,
    /// 更不能读成"这里断了" —— 本工具不做任何 CF 分析(spec 的 Out of scope)。
    boundary: bool,
    /// 这一帧的缩放。字号要跟着它走 —— `text_xs()` 那种固定字号不随缩放变。
    scale: f32,
}

/// 一条边已经算好的三次贝塞尔(四个控制点,容器局部坐标)。
struct EdgeView {
    ctrl: bool,
    from: Point<f32>,
    a: Point<f32>,
    b: Point<f32>,
    to: Point<f32>,
    /// 悬停气泡的文案(`edge_tip` 算好)。控制流边是 `None`。
    tip: Option<String>,
    /// 边在 `g.edges` 里的下标 —— 命中测试找到边之后要靠它反查文案,
    /// 也让同一条边的两个端点共享一个身份。
    index: usize,
}

// ------------------------------------------------------------------ 几何

/// 窗口坐标(`Point<Pixels>`)→ 内部用的 `Point<f32>`。
///
/// `Pixels` 的字段是私有的(`geometry.rs:2573`),拿不到裸 f32,只能走 `From`。
fn from_px(p: Point<Pixels>) -> Point<f32> {
    point(p.x.into(), p.y.into())
}

/// 容器局部坐标 + 窗口偏移 → `Point<Pixels>`。给 canvas 用。
fn add_px(offset: Point<Pixels>, p: Point<f32>) -> Point<Pixels> {
    point(px(f32::from(offset.x) + p.x), px(f32::from(offset.y) + p.y))
}

/// 把第 `pin` 个 pin 均分在 `[lo, lo + len]` 上,首尾各留一格。
///
/// 用 `N+1` 段而不是 `N-1` 段:`N == 1` 时后者要除零,`N == 0` 时没有意义。
fn spread(lo: f32, len: f32, pin: usize, total: usize) -> f32 {
    if total == 0 {
        return lo + len / 2.0;
    }
    lo + len * (pin as f32 + 1.0) / (total as f32 + 1.0)
}

/// 侧边上第 `pin` 个 pin 的纵向位置(容器局部坐标)。
///
/// 一条侧边上,控制流 pin 在前、值流 pin 在后 —— **值流的连接点依次排在
/// 控制流下方**(和游戏内节点编辑器的排法一致)。`ctrl_num` / `value_num`
/// 是这条侧边上两类 pin 的总数,`pin` 是 `is_ctrl` 那一类里的序号。
fn pin_y(lo: f32, len: f32, ctrl_num: usize, value_num: usize, is_ctrl: bool, pin: usize) -> f32 {
    let k = if is_ctrl { pin } else { ctrl_num + pin };
    spread(lo, len, k, ctrl_num + value_num)
}

/// 节点 `i` 的左上角(容器局部坐标)与尺寸。
fn node_box(
    pos: &[Point<f32>],
    nodes: &[NodeEntry],
    origin: Point<f32>,
    scale: f32,
    i: usize,
) -> Option<(Point<f32>, Size<f32>)> {
    let n = nodes.get(i)?;
    // 一律 `get` + `unwrap_or_default`,不索引 —— `pos` 与 `nodes` 长度
    // 短暂不等长时(优化删完节点、坐标还没跟着重排)直接索引就是 panic。
    let p = pos.get(i).copied().unwrap_or_default();
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

/// 拍出这一帧要画的所有节点。
fn build_nodes(
    g: &GraphEntry,
    pos: &[Point<f32>],
    origin: Point<f32>,
    scale: f32,
) -> Vec<NodeView> {
    g.nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            // 标签就是节点本身,不再拼 `[in]` / `[out]` 后缀 —— 图边界由侧边的
            // 桩线标(`build_stubs`),比后缀多一层信息:是**哪一个引脚**。
            let (p, s) = node_box(pos, &g.nodes, origin, scale, i).unwrap_or_default();
            NodeView {
                index: i,
                label: label(&n.kind, &n.type_label),
                x: p.x,
                y: p.y,
                w: s.width,
                h: s.height,
                border: node_color(&n.kind),
                boundary: g.entry.contains(&i) || g.exit.contains(&i),
                scale,
            }
        })
        .collect()
}

/// 拍出这一帧要画的所有边(三次贝塞尔控制点)。
///
/// 控制流和值流**同一条排法**:都从来源右侧的 pin 位出、进目标左侧的 pin 位,
/// 侧边上控制流在前、值流在后(见 [`pin_y`])。值流不再从节点中心的上下缘进出。
///
/// 端点下标越界直接 `filter_map` 掉 —— 展平层(`doc.rs`)已经用 `index_of`
/// 过滤过一次,这里只是兜底,不能 unwrap。
fn build_edges(g: &GraphEntry, pos: &[Point<f32>], origin: Point<f32>, scale: f32) -> Vec<EdgeView> {
    g.edges
        .iter()
        .enumerate()
        .filter_map(|(index, e): (usize, &Edge)| {
            let (from, fs) = node_box(pos, &g.nodes, origin, scale, e.from.0)?;
            let (to, ts) = node_box(pos, &g.nodes, origin, scale, e.to.0)?;
            // 两侧各自的 pin 总数:控制流 + 值流,值流排在下方。
            let (out_c, out_v) = g
                .nodes
                .get(e.from.0)
                .map_or((0, 0), |n| (n.controls_out_num, n.values_out_num));
            let (in_c, in_v) = g
                .nodes
                .get(e.to.0)
                .map_or((0, 0), |n| (n.controls_in_num, n.values_in_num));
            let y0 = pin_y(from.y, fs.height, out_c, out_v, e.ctrl, e.from.1);
            let y1 = pin_y(to.y, ts.height, in_c, in_v, e.ctrl, e.to.1);
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

/// 一个图边界 pin 的画布表现:从 pin 位往图外的一小段直线(桩),
/// 末端挂一个写边界号的「Export N」标签框。
/// 控制流用实线蓝、值流用虚线黄,和对应边的样式一致。
#[derive(Clone)]
struct BoundaryView {
    ctrl: bool,
    /// 桩的两个端点(容器局部坐标)。
    from: Point<f32>,
    to: Point<f32>,
    /// 图外边界号(`ExportPin::export`)。
    export: usize,
}

/// 桩的长度(1 倍缩放时;跟着画布缩放走)。
const STUB_LEN: f32 = 18.0;
/// 「Export N」标签框的尺寸(1 倍缩放时)。
const TAG_W: f32 = 64.0;
const TAG_H: f32 = 18.0;
/// 桩末端与标签框之间的缝。
const TAG_GAP: f32 = 4.0;

/// 拍出这一帧的图边界(桩线 + 末端标签的数据)。
///
/// 入边一侧(左)的桩往左伸、出边一侧(右)的桩往右伸;纵向位置用和连线
/// 完全一样的 [`pin_y`],所以桩和线的端点严丝合缝 —— 就是把「连到图外」
/// 的那半条线画出来,再由标签框说明连到**哪一号**边界。
fn build_boundaries(g: &GraphEntry, pos: &[Point<f32>], origin: Point<f32>, scale: f32) -> Vec<BoundaryView> {
    let len = STUB_LEN * scale;
    let mut out = Vec::new();
    for (i, n) in g.nodes.iter().enumerate() {
        let Some((p, s)) = node_box(pos, &g.nodes, origin, scale, i) else {
            continue;
        };
        let mut stub = |ctrl: bool, left: bool, pin: usize, export: usize| {
            let (ctrl_num, value_num) = if left {
                (n.controls_in_num, n.values_in_num)
            } else {
                (n.controls_out_num, n.values_out_num)
            };
            let y = pin_y(p.y, s.height, ctrl_num, value_num, ctrl, pin);
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
fn tag_box(b: &BoundaryView, scale: f32) -> (Point<f32>, Size<f32>) {
    let (w, h) = (TAG_W * scale, TAG_H * scale);
    let x = if b.to.x >= b.from.x {
        b.to.x + TAG_GAP * scale // 右侧出:框在末端右边
    } else {
        b.to.x - w - TAG_GAP * scale // 左侧出:框在末端左边
    };
    (point(x, b.to.y - h / 2.0), size(w, h))
}

/// 引脚圆点的直径(1 倍缩放时)。
const PIN_D: f32 = 7.0;
/// 默认值文本的框宽(1 倍缩放时)。框**右对齐**到引脚左侧 ——
/// 文字长度不确定,固定右缘才能和引脚严丝合缝。
const DEF_W: f32 = 48.0;

/// 一个引脚标记(容器局部中心坐标)。`ctrl` 决定**形状与颜色**:控制流画
/// 向右的三角形(见 [`pin_triangle`]),值流画圆点。
struct PinView {
    x: f32,
    y: f32,
    ctrl: bool,
    /// 悬停气泡的文案。控制流 pin 是 `None` —— 控制流不分类型,给不出
    /// 有意义的话。值流 pin 是该 pin 的类型标签,空串(`""`)表示"这个 pin
    /// 没有类型",气泡里显示 `None`。
    tip: Option<String>,
}

/// 引脚 / 边的悬停文案(纯逻辑,可单测)。
///
/// `NONE_TIP` 是"这个 pin 没有类型"的固定说法:类型标签为空串时用它,
/// 而不是把空串直接显示出去(那看起来像 bug,不像信息)。
const NONE_TIP: &str = "None";

/// 值流 pin 的类型文案。`is_out` 决定读出参还是入参的表,`pin` 是**值流里**
/// 的序号(控制流 pin 不带类型,不走这里)。
///
/// 两张表都可能越界 / 为空(展平层不保证),拿不到就回 `NONE_TIP` 而不是
/// panic —— 悬停是纯交互增强,不能因为某个畸形 `.ogia` 把窗口带走。
fn pin_tip(n: &NodeEntry, is_out: bool, pin: usize) -> String {
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
fn edge_tip(g: &GraphEntry, e: &Edge) -> Option<String> {
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

/// 控制流引脚的三角形顶点(包在直径 `d` 的方框里、**尖端向右**)。
///
/// 形状即方向:控制流从左往右走,输入侧的引脚也朝右指 —— 两端一致的箭头
/// 读起来就是"往这边流",比圆点多一层信息。值流不分方向(数据可回指),
/// 保持圆点。
///
/// div 画不出三角形(gpui 的边框只有一个颜色,拼不出 CSS 那套 border 三角),
/// 渲染时用 canvas 填色画;顶点在做几何这里算,**可单测**。
fn pin_triangle(cx: f32, cy: f32, d: f32) -> [Point<f32>; 3] {
    [
        point(cx - d / 2.0, cy - d / 2.0),
        point(cx - d / 2.0, cy + d / 2.0),
        point(cx + d / 2.0, cy),
    ]
}

/// 节点侧边上的**全部**引脚标记 —— 无论有没有连线都画,
/// 「这个节点有几个脚、哪个脚空着」才一眼可见。
fn build_pins(g: &GraphEntry, pos: &[Point<f32>], origin: Point<f32>, scale: f32) -> Vec<PinView> {
    let mut out = Vec::new();
    for (i, n) in g.nodes.iter().enumerate() {
        let Some((p, s)) = node_box(pos, &g.nodes, origin, scale, i) else {
            continue;
        };
        // 入边一侧在左边缘、出边一侧在右边缘;纵向档位就是 `0..(控制+值)`,
        // 与控制流/值流的先后无关 —— 那个顺序只决定颜色。
        for (ctrl_num, value_num, is_out, x) in [
            (n.controls_in_num, n.values_in_num, false, p.x),
            (n.controls_out_num, n.values_out_num, true, p.x + s.width),
        ] {
            for k in 0..(ctrl_num + value_num) {
                let ctrl = k < ctrl_num;
                out.push(PinView {
                    x,
                    y: spread(p.y, s.height, k, ctrl_num + value_num),
                    ctrl,
                    // 控制流 pin 没有类型可显示;值流 pin 的 pin 序号是
                    // `k - ctrl_num`(值流排在控制流下方)。
                    tip: (!ctrl).then(|| pin_tip(n, is_out, k - ctrl_num)),
                });
            }
        }
    }
    out
}

/// 值入 pin 的默认值文本(容器局部坐标)。
struct DefaultView {
    /// 文本框的**右缘** x(框向左展开,文字右对齐)。
    right: f32,
    y: f32,
    text: String,
}

/// 拍出这一帧要显示的默认值。位置:紧贴值入引脚圆点左侧、纵向居中对齐。
fn build_defaults(g: &GraphEntry, pos: &[Point<f32>], origin: Point<f32>, scale: f32) -> Vec<DefaultView> {
    let mut out = Vec::new();
    for (i, n) in g.nodes.iter().enumerate() {
        let Some((p, s)) = node_box(pos, &g.nodes, origin, scale, i) else {
            continue;
        };
        for (pin, default) in n.value_in_defaults.iter().enumerate() {
            let Some(text) = default else {
                continue;
            };
            out.push(DefaultView {
                right: p.x - (PIN_D / 2.0 + 3.0) * scale,
                y: pin_y(p.y, s.height, n.controls_in_num, n.values_in_num, false, pin),
                text: text.clone(),
            });
        }
    }
    out
}

// ------------------------------------------------------------------ 悬停命中

/// 引脚 / 边的命中半径(容器局部坐标,**不随缩放变**)。
///
/// 圆点本身只有 `PIN_D * scale` —— 缩到 0.2 倍时直径不到 1.5px,真按图形
/// 面积去命中几乎点不中。所以命中半径给一个**固定**的屏幕像素数(缩放无关),
/// 让"小图也能指着"这件事成立。边同理:1px 宽的虚线不能用线宽当命中区。
const PIN_HIT_R: f32 = 7.0;
const EDGE_HIT_R: f32 = 5.0;

/// 悬停气泡相对光标的偏移(容器局部 = 屏幕像素)和字号。偏右下是为了不挡住
/// 光标底下那个引脚 / 连线。气泡字号**固定**、不跟画布缩放 —— 它是 UI 提示,
/// 不是画布内容,缩到 6px 就读不了了。
const HOVER_DX: f32 = 12.0;
const HOVER_DY: f32 = 16.0;
const HOVER_FONT_PX: f32 = 12.0;

/// 命中测试的结果:找到的东西 + 气泡落在哪儿。
pub struct Hover {
    /// 气泡文案(已算好,渲染直接用)。
    pub text: String,
    /// 气泡锚点,**容器局部**坐标 —— 跟着光标走,不是钉在被命中的元素上
    /// (钉在 pin 上会让气泡盖住圆点本身,反而看不清)。
    pub at: Point<f32>,
}

/// 两点距离。
fn dist(a: Point<f32>, b: Point<f32>) -> f32 {
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
fn hit_test(pins: &[PinView], edges: &[EdgeView], at: Point<f32>) -> Option<Hover> {
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
fn bezier_dist(e: &EdgeView, at: Point<f32>) -> f32 {
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
/// (与 `EdgeView` 字段同名,顺序也和 `PathBuilder::cubic_bezier_to` 一致)。
fn bezier_at(e: &EdgeView, t: f32) -> Point<f32> {
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
fn seg_dist(p: Point<f32>, a: Point<f32>, b: Point<f32>) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 <= f32::EPSILON {
        return dist(p, a);
    }
    // 投影参数夹到 [0, 1] —— 落在线段延长线上的点要夹回端点。
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    dist(p, point(a.x + dx * t, a.y + dy * t))
}

// ------------------------------------------------------------------ 状态

/// 节点右键菜单(现在只有 `Fn` 节点的「link」一项)。
/// `x` / `y` 是**容器局部**坐标(和节点几何一个坐标系)。
struct NodeMenu {
    /// 右键时那个节点在展平顺序里的下标。
    node_index: usize,
    x: f32,
    y: f32,
}

struct Drag {
    /// `Some(i)` = 拖第 `i` 个节点;`None` = 平移画布。
    node: Option<usize>,
    /// 上一次事件的鼠标位置,**窗口**坐标。
    ///
    /// 存窗口坐标而不是容器局部坐标是刻意的:拖动和平移都只用**增量**
    /// (`event.position - last`),平移量与坐标系原点无关,所以两条路径
    /// 都不需要知道画布容器在哪。只有缩放要换算(见 `on_scroll_wheel`)。
    last: Point<f32>,
}

pub struct Viewer {
    pub doc: Doc,
    /// 当前显示第几张图。越界时按空图处理,渲染不索引。
    pub current: usize,
    /// 解析错误。`Some` 时居中盖住画布显示它。
    pub error: Option<String>,
    /// 每张图的节点坐标,与 `doc.graphs` 平行(下标即图下标,只增不改)。
    ///
    /// 加载时算一次([`Viewer::add_target`]),拖动就地改;切图**不**重算,
    /// 只有点「自动整理」才用 core 的布局重来(`Doc::relayout`)。
    pub positions: Vec<Vec<Point<f32>>>,
    /// 画布平移量(**容器局部**坐标):世界原点在画布里的落点。
    pub origin: Point<f32>,
    pub scale: f32,
    drag: Option<Drag>,
    /// 画布容器在**窗口**坐标系里的左上角,由 `canvas` 的 prepaint 写、事件回调读。
    ///
    /// 用 `Rc<Cell<..>>` 而不是 `&mut Viewer`:prepaint 拿得到 `self` 也拿不到
    /// 事件回调要用的 `&mut Viewer`;而 prepaint 里 `entity.update(.., cx.notify())`
    /// 会每帧把视图标脏(每帧重绘),`Cell` 只写不重绘。
    ///
    /// 值是"上一帧画完时的布局",而事件在那之后才到达,所以读到的就是当帧的。
    canvas_origin: Rc<Cell<Point<f32>>>,
    /// 每张图一个撤销栈。图下标只增不改(`Doc::graphs` 只会 append),
    /// 所以栈不会串到别的图上;切图不清栈,切回来还能接着撤。
    undo_stack: HashMap<usize, Vec<Snapshot>>,
    /// 右栏底部的一行状态(最近一次优化 / 撤销的结果)。
    status: Option<String>,
    /// 节点右键菜单,`None` = 没弹。
    menu: Option<NodeMenu>,
    /// 当前悬停命中的引脚 / 边,`None` = 鼠标不在任何带气泡的东西上。
    hover: Option<Hover>,
}

impl Viewer {
    pub fn new() -> Self {
        Self {
            doc: Doc::new(),
            current: 0,
            error: None,
            positions: Vec::new(),
            origin: point(0.0, 0.0),
            scale: 1.0,
            drag: None,
            canvas_origin: Rc::new(Cell::new(point(0.0, 0.0))),
            undo_stack: HashMap::new(),
            status: None,
            menu: None,
            hover: None,
        }
    }

    /// 追加一个已解析的 `Target`,并切到它的第一张图。拖放回调与命令行入口都走这里。
    ///
    /// **追加而不是替换** —— spec:"拖入第二个文件不清空已有内容"。
    /// 新图在这里算好坐标;已有图的坐标(含拖动)原样保留。
    pub fn add_target(&mut self, t: Target, cx: &mut Context<Self>) {
        if let Some(first) = self.doc.push_target(t) {
            // 布局只在**加载时**算一次(`push_target` 里由 core 的
            // `node::layout` 写进 IR 节点),这里只是把坐标读进视图状态。
            // 之后切图不重算 —— 拖过的位置能留住,想重排得点「自动整理」。
            for g in &self.doc.graphs[first..] {
                self.positions
                    .push(g.nodes.iter().map(|n| n.position).collect());
            }
            self.error = None;
            self.select(first, cx);
        } else {
            cx.notify();
        }
    }

    /// 切到第 `i` 张图,把视图(平移/缩放)复位。**不重算坐标**。
    pub fn select(&mut self, i: usize, cx: &mut Context<Self>) {
        // 越界直接返回:**不索引**。切图是交互驱动的,双击列表/拖入文件
        // 交叠时给一个越界下标是完全可能的。
        if self.doc.graphs.get(i).is_none() {
            return;
        }
        self.current = i;
        self.origin = point(0.0, 0.0);
        self.scale = 1.0;
        self.drag = None;
        // 状态行说的是"上一张图的操作",切过来还挂着会误导。
        self.status = None;
        // 菜单记的节点下标属于上一张图,不能留着。
        self.menu = None;
        cx.notify();
    }

    /// 右键节点:能给出菜单项(`Fn` 节点 → link)才弹;别的节点只关旧菜单。
    /// `at_window` 是事件的**窗口**坐标,换算成容器局部坐标存下来。
    fn open_menu(&mut self, node_index: usize, at_window: Point<f32>, cx: &mut Context<Self>) {
        let is_fn = self
            .doc
            .graphs
            .get(self.current)
            .and_then(|g| g.nodes.get(node_index))
            .is_some_and(|n| matches!(n.kind, IrNodeId::Fn(_)));
        self.menu = is_fn.then(|| {
            let o = self.canvas_origin.get();
            NodeMenu {
                node_index,
                x: at_window.x - o.x,
                y: at_window.y - o.y,
            }
        });
        cx.notify();
    }

    /// 右键菜单里点了「link」:对菜单记下的那个 `Fn` 节点执行
    /// `Optimizer::link_node`(展开成内联的复合节点)。
    pub fn link_menu_node(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else {
            return;
        };
        let refs = self.current_refs();
        let pos = self.positions.get(self.current).cloned().unwrap_or_default();
        let Some(node) = refs.get(menu.node_index).copied() else {
            self.status = Some("这个节点已经不在了".to_string());
            cx.notify();
            return;
        };
        let r = optimize::link_fn_node(&mut self.doc, self.current, &refs, &pos, node);
        if let Some(bytes) = r.undo {
            self.undo_stack.entry(self.current).or_default().push(Snapshot {
                bytes,
                refs,
                pos,
                kind: optimize::SnapshotKind::Optimize,
            });
        }
        self.set_pos(r.pos);
        self.status = Some(r.status);
        cx.notify();
    }

    /// 「拖入文件」的落点逻辑。**两条渲染分支都绑它**(占位分支与正常分支,
    /// 各自 `.on_drop` 到这个函数)—— 启动界面收不到拖放是个真实的坑,
    /// 见 `render` 里占位分支的注释。
    fn handle_drop(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        // 逐个处理:某个文件解析失败不影响其他(spec 错误处理章)。
        for p in paths.paths() {
            match crate::doc::load(p) {
                Ok(t) => self.add_target(t, cx),
                Err(e) => {
                    self.error = Some(format!("{}: {e}", p.display()));
                    cx.notify();
                }
            }
        }
    }

    /// 换掉当前图的坐标(下标与 `doc.graphs` 平行,图在 ⇒ 下标一定在;
    /// 还是 `get_mut` 兜底,渲染期的越界访问一律不 panic)。
    fn set_pos(&mut self, pos: Vec<Point<f32>>) {
        if let Some(slot) = self.positions.get_mut(self.current) {
            *slot = pos;
        }
    }

    /// 右栏「自动整理」:重算当前图的坐标。拖动结果会丢,Ctrl+Z 能撤回。
    pub fn arrange(&mut self, cx: &mut Context<Self>) {
        let i = self.current;
        let Some(g) = self.doc.graphs.get(i) else {
            return;
        };
        let refs: Vec<NodeRef> = g.nodes.iter().map(|n| n.node_ref).collect();
        let old_pos = self.positions.get(i).cloned().unwrap_or_default();
        // 存档里带上图本体:撤销走的是和优化同一条 `optimize::undo`
        // (解码 + 重展平 + 按 kind 选坐标来源)。存档必须在重新布局**之前**。
        let bytes = match self.doc.encode(i) {
            Ok(b) => b,
            Err(e) => {
                self.status = Some(e);
                cx.notify();
                return;
            }
        };
        self.undo_stack.entry(i).or_default().push(Snapshot {
            bytes,
            refs,
            pos: old_pos,
            kind: optimize::SnapshotKind::Arrange,
        });
        // 布局在 core(`Doc::relayout`):对 IR 图跑自动整理 + 重展平,
        // 再把新坐标读进视图状态。
        self.doc.relayout(i);
        let new_pos: Vec<Point<f32>> = self.doc.graphs[i]
            .nodes
            .iter()
            .map(|n| n.position)
            .collect();
        self.set_pos(new_pos);
        self.status = Some("已自动整理(Ctrl+Z 可撤销)".to_string());
        cx.notify();
    }

    /// 右栏「一键优化」:Fn 节点全部展开 + 消除规则跑到不动点 ——
    /// core `Optimizer::lower` 在 IR 段的整条流水线(右键「link」的批量版)。
    /// 整次点击只留一个存档:一次 Ctrl+Z 回到点击前。
    pub fn optimize_all(&mut self, cx: &mut Context<Self>) {
        let refs = self.current_refs();
        let pos = self.positions.get(self.current).cloned().unwrap_or_default();
        let r = optimize::optimize_all(&mut self.doc, self.current, &refs, &pos);
        if let Some(bytes) = r.undo {
            self.undo_stack.entry(self.current).or_default().push(Snapshot {
                bytes,
                refs,
                pos,
                kind: optimize::SnapshotKind::Optimize,
            });
        }
        self.set_pos(r.pos);
        self.status = Some(r.status);
        cx.notify();
    }

    /// 当前图的展平顺序(`NodeRef` 序列),与 `positions[current]` 同序。
    fn current_refs(&self) -> Vec<NodeRef> {
        self.doc
            .graphs
            .get(self.current)
            .map(|g| g.nodes.iter().map(|n| n.node_ref).collect())
            .unwrap_or_default()
    }

    /// 右栏点了一条优化规则:在**当前图**上把这条规则过一遍。
    ///
    /// 坐标、panic 回滚、撤销存档的来龙去脉全在 [`crate::optimize::apply_rule`],
    /// 这里只负责接上视图状态。
    pub fn apply_rule(&mut self, rule_index: usize, cx: &mut Context<Self>) {
        let Some(rule) = RULES.get(rule_index) else {
            return;
        };
        let refs = self.current_refs();
        let pos = self.positions.get(self.current).cloned().unwrap_or_default();
        let r = optimize::apply_rule(&mut self.doc, self.current, &refs, &pos, rule);
        if let Some(bytes) = r.undo {
            self.undo_stack.entry(self.current).or_default().push(Snapshot {
                bytes,
                refs,
                pos,
                kind: optimize::SnapshotKind::Optimize,
            });
        }
        self.set_pos(r.pos);
        self.status = Some(r.status);
        cx.notify();
    }

    /// Ctrl+Z / 右栏「撤销」按钮:撤掉当前图最近一次优化或整理。
    pub fn undo_step(&mut self, cx: &mut Context<Self>) {
        let Some(entry) = self.undo_stack.get_mut(&self.current).and_then(|s| s.pop()) else {
            self.status = Some("没有可撤销的操作".to_string());
            cx.notify();
            return;
        };
        let refs = self.current_refs();
        let pos = self.positions.get(self.current).cloned().unwrap_or_default();
        let (new_pos, status) = optimize::undo(&mut self.doc, self.current, &refs, &pos, &entry);
        self.set_pos(new_pos);
        self.status = Some(status);
        cx.notify();
    }

    /// 鼠标移动后重算悬停命中。
    ///
    /// `at_window` 是事件的**窗口**坐标;减掉画布容器左上角换成容器局部
    /// 坐标,和 [`build_pins`] / [`build_edges`] 算几何用的是同一个坐标系。
    /// 画布左上角由 canvas 的 prepaint 每帧写进 [`Viewer::canvas_origin`]。
    ///
    /// **只有气泡内容变了才 `notify`** —— 光标在同一个引脚上移动时文案不变,
    /// 每帧重绘纯属浪费(气泡锚点会跟着光标动,所以位置变了也要重绘)。
    fn update_hover(&mut self, at_window: Point<f32>, cx: &mut Context<Self>) {
        let o = self.canvas_origin.get();
        let at = point(at_window.x - o.x, at_window.y - o.y);
        let hit = self.pick(at);
        let changed = match (&self.hover, &hit) {
            (None, None) => false,
            (Some(h), Some(n)) => h.text != n.text || h.at != n.at,
            _ => true,
        };
        if changed {
            self.hover = hit;
            cx.notify();
        }
    }

    /// 在容器局部坐标 `at` 上做一次命中测试。悬停和渲染共用这一条路径,
    /// 免得"看到的"和"命中的"是两套几何。
    fn pick(&self, at: Point<f32>) -> Option<Hover> {
        let g = self.doc.graphs.get(self.current)?;
        let pos = self
            .positions
            .get(self.current)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let pins = build_pins(g, pos, self.origin, self.scale);
        let edges = build_edges(g, pos, self.origin, self.scale);
        hit_test(&pins, &edges, at)
    }

    /// 无内容可画时的占位文案。`Some` 表示这一帧不画画布,居中显示一行字。
    fn placeholder(&self) -> Option<String> {
        if let Some(e) = &self.error {
            return Some(format!("解析失败:{e}"));
        }
        if self.doc.graphs.is_empty() {
            return Some("拖入 .ogia 或 .rlib 文件".to_string());
        }
        // `current` 越界(切图瞬间)按"没有可显示的图"处理,不索引、不 panic。
        let Some(g) = self.doc.graphs.get(self.current) else {
            return Some("没有可显示的图".to_string());
        };
        // 注意:**节点为 0 的图是正常数据,不是错误**。真实 Target 里完全可能有
        // 一张空图(函数被优化没了、或只有声明没有语句)。所以这里不拦 ——
        // 交给渲染层画一张空画布。只有解析失败才走 `error` 分支。
        let _ = g;
        None
    }
}

impl Render for Viewer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ---- 占位分支:错误 / 没有文档 / 当前图是空的 ----
        if let Some(msg) = self.placeholder() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(canvas_bg())
                .text_color(text_dim())
                .text_sm()
                // ⚠️ **这一支也必须收拖放** —— 启动时没有文档,占位界面
                // ("拖入 .ogia 或 .rlib 文件")恰恰是**最该接住拖放的那一刻**。
                // 只在正常分支挂的话,启动后往窗口里拖文件会毫无反应、界面
                // 停在原地(用户实测踩到)。
                .on_drop::<ExternalPaths>(cx.listener(|v, paths: &ExternalPaths, _w, cx| {
                    v.handle_drop(paths, cx);
                }))
                .child(msg)
                .into_any_element();
        }

        // `placeholder()` 已经保证图存在且非空,这里还是用 `get`:
        // 渲染期不做任何未检查的下标访问。
        let Some(g) = self.doc.graphs.get(self.current) else {
            return div().size_full().into_any_element();
        };

        // ---- 拍成 owned 值 ----
        // 下面所有闭包都要求 `'static`,带不走 `&self` 借用。
        let origin = self.origin;
        let scale = self.scale;
        let canvas_origin = self.canvas_origin.clone();
        // 当前图的坐标。缺(理论上只有坐标还没跟上时)按空切片处理 ——
        // `build_nodes` / `build_edges` 对每个节点都是 `get` + 默认值。
        let pos = self
            .positions
            .get(self.current)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let nodes = build_nodes(g, pos, origin, scale);
        let edges = build_edges(g, pos, origin, scale);
        let boundaries = build_boundaries(g, pos, origin, scale);
        let pins = build_pins(g, pos, origin, scale);
        let defaults = build_defaults(g, pos, origin, scale);

        // ---- 边和图边界桩:canvas 画在最下面 ----
        let record_origin = canvas_origin.clone();
        let boundaries_for_canvas = boundaries.clone();
        let edges_el = canvas(
            move |bounds: Bounds<Pixels>, _window, _cx| {
                // 画布容器和它的 `absolute().inset_0()` 子元素同源同大小,
                // 所以 canvas 的 bounds.origin 就是容器在窗口坐标系里的左上角。
                record_origin.set(from_px(bounds.origin));
                (edges, boundaries_for_canvas)
            },
            move |bounds, (edges, boundaries), window, _cx| {
                // 边是**容器局部**坐标,canvas 自己的 bounds 是窗口坐标,加个偏移。
                let o = bounds.origin;
                let at = |p: Point<f32>| add_px(o, p);
                for e in &edges {
                    let mut pb = PathBuilder::stroke(px(if e.ctrl { 2.0 } else { 1.0 }));
                    if !e.ctrl {
                        // 值流边虚线。线宽 + 线型 + 颜色三重区分,
                        // 不靠单一手段 —— 缩放小到 1px 时线型是最后的可读线索。
                        pb = pb.dash_array(&[px(4.0), px(3.0)]);
                    }
                    pb.move_to(at(e.from));
                    pb.cubic_bezier_to(at(e.to), at(e.a), at(e.b));
                    // `build()` 失败就跳过这条边 —— 不能因为一条画不出来的
                    // 边把整个程序带崩,输入是用户拖进来的任意 `.ogia`。
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, if e.ctrl { edge_ctrl() } else { edge_value() });
                    }
                }
                // 图边界桩:从 exported pin 往图外引的一小段直线,样式与对应边一致。
                // 末端的「Export N」标签框是上面那层 div(文字画不进 canvas)。
                for b in &boundaries {
                    let mut pb = PathBuilder::stroke(px(if b.ctrl { 2.0 } else { 1.0 }));
                    if !b.ctrl {
                        pb = pb.dash_array(&[px(4.0), px(3.0)]);
                    }
                    pb.move_to(at(b.from));
                    pb.line_to(at(b.to));
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, if b.ctrl { edge_ctrl() } else { edge_value() });
                    }
                }
            },
        )
        .absolute()
        .inset_0();

        // ---- 节点:一层 `absolute()` 的 div 盖在边上 ----
        // 用 div 而不是自绘,是为了白拿 GPUI 的命中测试 / hover / 事件系统。
        let mut layer = div().absolute().inset_0();
        for nv in nodes {
            let mut d = div()
                .absolute()
                .left(px(nv.x))
                .top(px(nv.y))
                .w(px(nv.w))
                .h(px(nv.h))
                .flex()
                .items_center()
                .justify_center()
                .px_1()
                .rounded_md()
                .bg(node_bg())
                .border_color(nv.border)
                .text_color(text_hi())
                // 字号**跟随缩放**:`text_xs()` 是固定 rem(≈12px),缩放时节点框
                // 会跟着变大变小而字不变 —— 放大后字挤成一小撮、缩小后又溢出被裁。
                // 下限 6px:再小就是糊点,不如让布局自己裁。
                .text_size(px((BASE_FONT_PX * nv.scale).max(6.0)))
                .overflow_hidden()
                .cursor_grab();
            d = if nv.boundary { d.border_2() } else { d.border_1() };
            layer = layer.child(
                // 节点拖拽用**左键**;画布平移才用中键。
                d.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |v, e: &MouseDownEvent, _w, cx| {
                        // 左键按在节点上顺手关掉已经弹出的菜单。
                        v.menu = None;
                        v.begin_drag(Some(nv.index), from_px(e.position));
                        // 不拦住的话同一次左键按下会**又拖节点又平移画布**:
                        // 事件继续冒泡到容器的 `on_mouse_down`。容器只监听中键,
                        // 所以左键本来也不会走到那儿 —— 这个 stop_propagation 是双保险。
                        cx.stop_propagation();
                    }),
                )
                // 右键弹菜单(只有 Fn 节点有项,见 `open_menu`)。
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |v, e: &MouseDownEvent, _w, cx| {
                        v.open_menu(nv.index, from_px(e.position), cx);
                        // 不拦住的话同一次右键会冒到容器,把刚弹的菜单又关掉。
                        cx.stop_propagation();
                    }),
                )
                .child(nv.label),
            );
        }

        // ---- 引脚:每个连接点都画,有没有连线都画 ----
        // 画在节点之后:引脚压在节点边缘上,一半在内一半在外。
        // 值流 = 圆点(div 白拿圆角)。
        for pin in pins.iter().filter(|p| !p.ctrl) {
            let d = PIN_D * scale;
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(pin.x - d / 2.0))
                    .top(px(pin.y - d / 2.0))
                    .w(px(d))
                    .h(px(d))
                    .rounded_full()
                    .bg(edge_value()),
            );
        }
        // 控制流 = **向右的三角形**(形状即方向,见 `pin_triangle`)。
        // div 画不出三角形,用 canvas 填色;这层 canvas 加在节点与圆点之后,
        // 和圆点同一视觉层(盖在节点边缘上)。
        let ctrl_pins: Vec<(f32, f32)> = pins
            .iter()
            .filter(|p| p.ctrl)
            .map(|p| (p.x, p.y))
            .collect();
        layer = layer.child(
            canvas(
                move |_bounds, _window, _cx| (),
                move |bounds, (), window, _cx| {
                    // 引脚是**容器局部**坐标,canvas 自己的 bounds 是窗口坐标。
                    let o = bounds.origin;
                    let d = PIN_D * scale;
                    for &(x, y) in ctrl_pins.iter() {
                        let t = pin_triangle(x, y, d);
                        let mut pb = PathBuilder::fill();
                        pb.move_to(add_px(o, t[0]));
                        pb.line_to(add_px(o, t[1]));
                        pb.line_to(add_px(o, t[2]));
                        pb.close();
                        // 失败就跳过 —— 不能因为一个画不出来的引脚把程序带崩。
                        if let Ok(path) = pb.build() {
                            window.paint_path(path, pin_ctrl());
                        }
                    }
                },
            )
            .absolute()
            .inset_0(),
        );

        // ---- 值入 pin 的默认值:紧贴引脚左侧,文字右对齐 ----
        // 框宽固定、右缘贴引脚 —— 文本长度不确定,只有固定右缘才能和引脚对齐;
        // **不裁剪**(不给 overflow_hidden):长默认值宁可伸出去,也不能被截得只剩尾巴。
        for dv in &defaults {
            let (w, h) = (DEF_W * scale, (BASE_FONT_PX + 4.0) * scale);
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(dv.right - w))
                    .top(px(dv.y - h / 2.0))
                    .w(px(w))
                    .h(px(h))
                    .flex()
                    .items_center()
                    .justify_end()
                    .text_size(px((BASE_FONT_PX * scale).max(6.0)))
                    .text_color(text_dim())
                    .child(dv.text.clone()),
            );
        }

        // ---- 图边界标签:桩末端的小框,写图外边界号 ----
        // 用 div 画(和节点同一层、同一套缩放):canvas 画不了文字。
        for b in &boundaries {
            let (p, sz) = tag_box(b, scale);
            let color = if b.ctrl { edge_ctrl() } else { edge_value() };
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(p.x))
                    .top(px(p.y))
                    .w(px(sz.width))
                    .h(px(sz.height))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .bg(node_bg())
                    .border_1()
                    .border_color(color)
                    .text_color(color)
                    .text_size(px((BASE_FONT_PX * scale).max(6.0)))
                    .overflow_hidden()
                    .child(format!("Export {}", b.export)),
            );
        }

        // ---- 悬停气泡(最上层,压在右键菜单之上)----
        // 跟着光标走、偏右下 —— 偏右下是为了**不挡住**光标底下那个引脚/
        // 连线本身。字号**不跟缩放**:这是 UI 提示,不是画布内容,跟着画布
        // 缩到 6px 就没法读了。
        if let Some(h) = &self.hover {
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(h.at.x + HOVER_DX))
                    .top(px(h.at.y + HOVER_DY))
                    .px_2()
                    .rounded_sm()
                    .bg(panel_bg())
                    .border_1()
                    .border_color(hsla(0.0, 0.0, 1.0, 0.35))
                    .text_color(text_hi())
                    .text_size(px(HOVER_FONT_PX))
                    // **不裁剪**:类型名可能很长(`rust2genshin_demo::MyStruct`),
                    // 宁可伸出去也不能只剩尾巴。
                    .child(h.text.clone()),
            );
        }

        // ---- 节点右键菜单(最上层)----
        if let Some(menu) = &self.menu {
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(menu.x))
                    .top(px(menu.y))
                    .w(px(120.0))
                    .p_1()
                    .rounded_md()
                    .bg(panel_bg())
                    .border_1()
                    .border_color(hsla(0.0, 0.0, 1.0, 0.25))
                    .text_color(text_hi())
                    // 按在菜单上的左键**绝不能**冒到容器:容器的「左键关菜单」
                    // 会在按下的一瞬间就把菜单关掉(并标脏重绘),菜单项的
                    // click —— 需要按下+抬起都落在自己身上 —— 就永远不成立。
                    // 「点了 link 没反应」的根因就在这。
                    .on_mouse_down(MouseButton::Left, cx.listener(|_v, _e, _w, cx| {
                        cx.stop_propagation();
                    }))
                    .child(
                        div()
                            .id("menu-link")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .overflow_hidden()
                            .text_xs()
                            .child("link")
                            .on_click(cx.listener(|v, _e, _w, cx| v.link_menu_node(cx))),
                    ),
            );
        }

        // ---- 容器:三个交互里"移动"和"缩放"挂在这里,"按下"也挂在这里 ----
        let canvas_el = div()
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(canvas_bg())
            // 中键拖 = 平移画布,和 AutoCAD / Blender 的约定一致,用 move 光标。
            .cursor_move()
            // 拖空白处平移。按在节点上时节点的 handler 已经 stop_propagation,
            // 所以冒到这里的事件一定是落在空白处的。
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|v, e: &MouseDownEvent, _w, _cx| {
                    v.begin_drag(None, from_px(e.position));
                }),
            )
            // 左键 / 右键落在空白处:关掉菜单(节点上的按下已经 stop_propagation,
            // 冒到这里的一定是空白处)。空白左键没有别的语义 —— 平移是中间拖。
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|v, _e: &MouseDownEvent, _w, cx| {
                    if v.menu.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|v, _e: &MouseDownEvent, _w, cx| {
                    if v.menu.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            // 拖节点 / 拖空白,都在这里推进。**必须挂在容器上而不是节点上**:
            // 拖动时指针早离开了起手那个节点,节点的 hitbox 已经不是 hovered。
            // 详见模块头「拖拽路线」。
            .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _w, cx| {
                // 悬停命中**先**算:它跟"是不是在拖"无关,静止时也要跟手。
                v.update_hover(from_px(e.position), cx);
                // `take` 出来再放回去:下面有一支要把 `drag` 直接清成 None,
                // 拿着 `&mut Drag` 就没法同时改 `v.drag`。
                let Some(mut d) = v.drag.take() else {
                    return;
                };
                // 左键=拖节点,中键=平移,两种都在推进,别把一种挡掉。
                if !matches!(
                    e.pressed_button,
                    Some(MouseButton::Left) | Some(MouseButton::Middle)
                ) {
                    // 指针移出窗口再回来时已经没有按键了(win32 的 wparam 不带
                    // MK_LBUTTON),而 `mouse_up` 那时收不到 —— 靠这里兜底,
                    // 不然画布会一直停在"拖拽中"直到下一次按下。
                    return;
                }
                let now = from_px(e.position);
                let delta = point(now.x - d.last.x, now.y - d.last.y);
                d.last = now;
                match d.node {
                    Some(i) => {
                        // 位移要除以 `scale`,否则缩放越大节点跑得越快。
                        // `scale` 被夹在 [MIN_SCALE, MAX_SCALE],恒 > 0。
                        // 坐标按图存:改的是当前图里第 `i` 个节点。
                        if let Some(p) = v
                            .positions
                            .get_mut(v.current)
                            .and_then(|ps| ps.get_mut(i))
                        {
                            p.x += delta.x / v.scale;
                            p.y += delta.y / v.scale;
                        }
                    }
                    None => {
                        // 平移量与坐标系原点无关,所以直接用窗口坐标的增量。
                        v.origin.x += delta.x;
                        v.origin.y += delta.y;
                    }
                }
                v.drag = Some(d);
                // `pos` / `origin` 变了。边每帧都从 `pos` 重算,所以节点一动,
                // 连线下一帧自动跟上 —— 这就是"连线实时跟随"。
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|v, _e: &MouseUpEvent, _w, _cx| {
                    // 拖拽状态不参与渲染,不用 notify。
                    v.drag = None;
                }),
            )
            // 拖到窗口边缘外再松开时上面那个收不到(capture 阶段,要求容器**没被**
            // hover,正好补上另一半)。两个 handler 合起来覆盖"松手时指针在/
            // 不在画布内"两种情况。
            .on_mouse_up_out(
                MouseButton::Middle,
                cx.listener(|v, _e: &MouseUpEvent, _w, _cx| {
                    v.drag = None;
                }),
            )
            .on_scroll_wheel(cx.listener(|v, e: &ScrollWheelEvent, _w, cx| {
                // `ScrollDelta` 是元组变体,没有 `.y` 字段,得按变体解构。
                let dy = match &e.delta {
                    ScrollDelta::Pixels(p) => f32::from(p.y),
                    ScrollDelta::Lines(p) => p.y,
                };
                if dy == 0.0 {
                    return;
                }
                // Windows 的 `WM_MOUSEWHEEL` 里 `wparam` 的 HIWORD **向上滚为正**
                // (`platform/windows/events.rs` 的 `handle_mouse_wheel_msg`),
                // 所以"向上滚 = 放大"对应 `dy > 0`。
                let f = if dy > 0.0 { ZOOM_STEP } else { 1.0 / ZOOM_STEP };
                let new_scale = (v.scale * f).clamp(MIN_SCALE, MAX_SCALE);
                let ratio = new_scale / v.scale;
                // 事件坐标是**窗口**坐标,`origin` 是**容器局部**坐标,两者差一个
                // 画布原点。整个文件里只有缩放需要这个换算 —— 拖动和平移只用
                // 增量,与原点无关。
                let o = v.canvas_origin.get();
                let cur = point(
                    f32::from(e.position.x) - f32::from(o.x),
                    f32::from(e.position.y) - f32::from(o.y),
                );
                // 保持光标下的图元不动:图元上的一点 `p` 满足
                // `p * scale + origin == cur`,换 scale 后让等式仍成立即得下式。
                v.origin.x = cur.x - (cur.x - v.origin.x) * ratio;
                v.origin.y = cur.y - (cur.y - v.origin.y) * ratio;
                v.scale = new_scale;
                cx.notify();
            }))
            .child(edges_el)
            .child(layer);

        // ---- 左栏:图列表 ----
        // `overflow_y_scroll` 和 `on_click` 都挂在 `StatefulInteractiveElement` 上
        // 而不是 `Styled` / `InteractiveElement` 上,所以每一层都得先 `.id(..)`
        // 变成 stateful 元素。**`.id()` 必须在调用它们之前**,链式调用是从左往右
        // 推导接收者的,写反了就还是普通 `Div`。
        let sidebar = div()
            .id("sidebar")
            .w(px(240.0))
            .h_full()
            .flex_shrink_0()
            .overflow_y_scroll()
            .bg(panel_bg())
            .border_r_1()
            .border_color(hsla(0.0, 0.0, 1.0, 0.12))
            .p_2()
            .text_color(text_hi())
            .children(self.doc.graphs.iter().enumerate().map(|(i, entry)| {
                let active = i == self.current;
                div()
                    .id(("graph", i))
                    .px_2()
                    .py_1()
                    .mb_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .overflow_hidden()
                    .text_xs()
                    .when(active, |s| s.bg(hsla(0.0, 0.0, 1.0, 0.20)))
                    .child(format!("{} ({})", entry.name, entry.nodes.len()))
                    .on_click(cx.listener(move |v, _e, _w, cx| v.select(i, cx)))
            }));

        // ---- 右栏:优化 ----
        // 按钮 = `eliminate_solo`(`core/src/compile/ir.rs:402`)调用链上的每条规则,
        // 点一次 = 该规则在当前图的所有节点上各跑一次(**一遍**,不循环到不动点)。
        // 标签省掉 `eliminate_` 前缀:栏名已经说了这是优化,240px 宽放不下全名。
        let rules_panel = div()
            .id("rules")
            .w(px(240.0))
            .h_full()
            .flex_shrink_0()
            .overflow_y_scroll()
            .bg(panel_bg())
            .border_l_1()
            .border_color(hsla(0.0, 0.0, 1.0, 0.12))
            .p_2()
            .text_color(text_hi())
            .child(
                div()
                    .id("arrange")
                    .px_2()
                    .py_1()
                    .mb_3()
                    .rounded_sm()
                    .cursor_pointer()
                    .overflow_hidden()
                    .text_xs()
                    .bg(hsla(0.0, 0.0, 1.0, 0.10))
                    .child("自动整理")
                    .on_click(cx.listener(|v, _e, _w, cx| v.arrange(cx))),
            )
            .child(
                // 一键优化:和右键「link」同一套执行方式(护栏 + 存档 + 回滚),
                // 但跑的是整条流程 —— Fn 全展开 + 消除规则到不动点。
                div()
                    .id("optimize_all")
                    .px_2()
                    .py_1()
                    .mb_3()
                    .rounded_sm()
                    .cursor_pointer()
                    .overflow_hidden()
                    .text_xs()
                    .bg(hsla(0.0, 0.0, 1.0, 0.10))
                    .child("一键优化")
                    .on_click(cx.listener(|v, _e, _w, cx| v.optimize_all(cx))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(text_dim())
                    .mb_2()
                    .child("优化 —— 点一次 = 该规则过一遍"),
            )
            .children(RULES.iter().enumerate().map(|(i, r)| {
                div()
                    .id(("rule", i))
                    .px_2()
                    .py_1()
                    .mb_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .overflow_hidden()
                    .text_xs()
                    .bg(hsla(0.0, 0.0, 1.0, 0.10))
                    .child(r.name.strip_prefix("eliminate_").unwrap_or(r.name))
                    .on_click(cx.listener(move |v, _e, _w, cx| v.apply_rule(i, cx)))
            }))
            .child(
                div()
                    .id("undo")
                    .px_2()
                    .py_1()
                    .mt_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .overflow_hidden()
                    .text_xs()
                    .bg(hsla(0.0, 0.0, 1.0, 0.10))
                    .child("撤销 (Ctrl+Z)")
                    .on_click(cx.listener(|v, _e, _w, cx| v.undo_step(cx))),
            )
            .when_some(self.status.clone(), |d, s| {
                d.child(div().mt_2().text_xs().text_color(text_dim()).child(s))
            });

        // ---- 整窗:接收文件拖放,左中右分栏 ----
        div()
            .flex()
            .size_full()
            .on_drop::<ExternalPaths>(cx.listener(|v, paths: &ExternalPaths, _w, cx| {
                v.handle_drop(paths, cx);
            }))
            .child(sidebar)
            .child(canvas_el)
            .child(rules_panel)
            .into_any_element()
    }
}

impl Viewer {
    fn begin_drag(&mut self, node: Option<usize>, last: Point<f32>) {
        self.drag = Some(Drag { node, last });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use crate::doc::{NODE_H, NODE_W};

    /// 侧边 pin 排布:**控制流在前、值流在后,值流依次排在控制流下方**。
    #[test]
    fn value_pins_sit_below_control_pins() {
        let (lo, len) = (100.0, 60.0);
        let (c, v) = (2, 2);
        let c0 = pin_y(lo, len, c, v, true, 0);
        let c1 = pin_y(lo, len, c, v, true, 1);
        let v0 = pin_y(lo, len, c, v, false, 0);
        let v1 = pin_y(lo, len, c, v, false, 1);
        assert!(c0 < c1 && c1 < v0 && v0 < v1, "顺序:{c0} {c1} {v0} {v1}");
        assert!(v0 > c1, "值流最靠上的 pin 也要在控制流最靠下的 pin 之下");
        assert!(c0 > lo && v1 < lo + len, "pin 不贴边");
    }

    /// 纯值节点(GetLocal / 表达式,控制 pin 为 0):值 pin 均分整条侧边。
    #[test]
    fn value_only_node_spreads_across_the_whole_side() {
        let (y0, y1, y2) = (
            pin_y(0.0, 30.0, 0, 3, false, 0),
            pin_y(0.0, 30.0, 0, 3, false, 1),
            pin_y(0.0, 30.0, 0, 3, false, 2),
        );
        assert!(y0 < y1 && y1 < y2, "依次排开:{y0} {y1} {y2}");
        assert!(y0 > 0.0 && y2 < 30.0);
    }

    fn node(controls_in: usize, controls_out: usize, values_in: usize, values_out: usize) -> NodeEntry {
        node_with_types(controls_in, controls_out, values_in, values_out, &[], &[])
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
            type_label: String::new(),
            controls_in_num: controls_in,
            controls_out_num: controls_out,
            values_in_num: values_in,
            values_out_num: values_out,
            values_in_types: in_types.iter().map(|s| s.to_string()).collect(),
            values_out_types: out_types.iter().map(|s| s.to_string()).collect(),
            ctrl_in_exports: vec![],
            ctrl_out_exports: vec![],
            value_in_exports: vec![],
            value_out_exports: vec![],
            value_in_defaults: vec![],
            size: Size {
                width: NODE_W,
                height: NODE_H,
            },
        }
    }

    /// 值流边也从**侧边的 pin 位**进出(右出、左进),不再从节点中心的上下缘。
    /// 纵向位置必须落在值流那一档,而不是节点中心(`NODE_H / 2`)。
    #[test]
    fn value_edges_leave_from_the_side_pins() {
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![0],
            exit: vec![],
            // 0:1 控制出 + 2 值出;1:1 控制入 + 2 值入
            nodes: vec![node(0, 1, 0, 2), node(1, 0, 2, 0)],
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
        };
        let pos = vec![point(0.0, 0.0), point(300.0, 0.0)];
        let es = build_edges(&g, &pos, point(0.0, 0.0), 1.0);
        assert_eq!(es.len(), 2);

        let ctrl = &es[0];
        assert!(ctrl.ctrl);
        assert_eq!(ctrl.from.x, NODE_W, "从来源右边缘出");
        assert_eq!(ctrl.to.x, 300.0, "进目标左边缘");
        // 「1 控制 + 2 值」共 3 档:控制流 pin 0 在最上面(36 * 1/4 = 9)。
        assert_eq!(ctrl.from.y, NODE_H / 4.0);
        assert_eq!(ctrl.to.y, NODE_H / 4.0);

        let value = &es[1];
        assert!(!value.ctrl, "仍是值流样式(虚线)");
        assert_eq!(value.from.x, NODE_W);
        assert_eq!(value.to.x, 300.0);
        // 值流 pin 1 在「1 控制 + 2 值」里的第 2 位(0 基):36 * 3/4 = 27 ——
        // 落在控制流下方,不是节点中心(18)。
        assert_eq!(value.from.y, NODE_H * 3.0 / 4.0, "落在值流档位,不是节点中心");
        assert_eq!(value.to.y, NODE_H * 3.0 / 4.0);
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
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![0],
            exit: vec![],
            nodes: vec![n0],
            edges: vec![],
        };
        let pos = vec![point(10.0, 20.0)];
        let bs = build_boundaries(&g, &pos, point(0.0, 0.0), 1.0);
        assert_eq!(bs.len(), 2, "只有一个控制入桩和一个值出桩,别的 pin 不给桩");

        let ctrl = bs.iter().find(|b| b.ctrl).expect("有控制流桩");
        assert_eq!(ctrl.from.x, 10.0, "从节点左边缘起");
        assert_eq!(ctrl.to.x, 10.0 - STUB_LEN, "往左伸");
        assert_eq!(ctrl.from.y, ctrl.to.y, "桩是水平的");
        // 入边侧「1 控制 + 2 值」共 3 档,控制流 pin 0 在最上面。
        assert_eq!(ctrl.from.y, 20.0 + NODE_H / 4.0);
        assert_eq!(ctrl.export, 0, "控制入连的是边界 0");
        let (tp, tsz) = tag_box(ctrl, 1.0);
        assert_eq!(tp.x, ctrl.to.x - TAG_W - TAG_GAP, "左侧出:框在末端左边");
        assert_eq!(tp.y + tsz.height / 2.0, ctrl.to.y, "框纵向以桩线为中心");

        let value = bs.iter().find(|b| !b.ctrl).expect("有值流桩");
        assert_eq!(value.from.x, 10.0 + NODE_W, "从节点右边缘起");
        assert_eq!(value.to.x, 10.0 + NODE_W + STUB_LEN, "往右伸");
        // 出边侧「1 控制 + 2 值」共 3 档,值流 pin 1 在第 2 位。
        assert_eq!(value.from.y, 20.0 + NODE_H * 3.0 / 4.0);
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
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![0],
            exit: vec![],
            nodes: vec![node(1, 1, 2, 2)],
            edges: vec![],
        };
        let pos = vec![point(10.0, 20.0)];
        let pins = build_pins(&g, &pos, point(0.0, 0.0), 1.0);
        assert_eq!(pins.len(), 6, "入 3 + 出 3,哪怕一条边都没有");

        let in_side: Vec<_> = pins.iter().filter(|p| p.x == 10.0).collect();
        assert_eq!(in_side.len(), 3);
        // 档位顺序:控制流在前(蓝),值流依次在后(黄)
        assert!(in_side[0].ctrl, "第 1 档是控制流");
        assert!(!in_side[1].ctrl && !in_side[2].ctrl, "后两档是值流");
        assert!(in_side[0].y < in_side[1].y && in_side[1].y < in_side[2].y);
        assert_eq!(in_side[0].y, 20.0 + NODE_H / 4.0);

        let out_side: Vec<_> = pins.iter().filter(|p| p.x == 10.0 + NODE_W).collect();
        assert_eq!(out_side.len(), 3, "出边一侧也要画");
        assert!(out_side.iter().all(|p| p.y > 20.0 && p.y < 20.0 + NODE_H));
    }

    /// 值入默认值:只有 `Some` 的才显示,右缘贴引脚圆点左侧、纵向对齐该 pin。
    #[test]
    fn defaults_hang_left_of_their_pin() {
        let mut n0 = node(1, 0, 2, 0);
        n0.value_in_defaults = vec![None, Some("5".to_string())];
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![0],
            exit: vec![],
            nodes: vec![n0],
            edges: vec![],
        };
        let pos = vec![point(10.0, 20.0)];
        let ds = build_defaults(&g, &pos, point(0.0, 0.0), 1.0);
        assert_eq!(ds.len(), 1, "只有 pin 1 有默认值");
        assert_eq!(ds[0].text, "5");
        assert_eq!(ds[0].right, 10.0 - (PIN_D / 2.0 + 3.0), "右缘贴引脚左侧");
        // 入边侧「1 控制 + 2 值」共 3 档,值入 pin 1 在第 2 位。
        assert_eq!(ds[0].y, 20.0 + NODE_H * 3.0 / 4.0, "纵向对齐值入 pin 1");
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
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![],
            exit: vec![],
            nodes: vec![node_with_types(1, 0, 1, 0, &["Int"], &[])],
            edges: vec![],
        };
        let pos = vec![point(0.0, 0.0)];
        let pins = build_pins(&g, &pos, point(0.0, 0.0), 1.0);
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
        let g = GraphEntry {
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
        };
        let pos = vec![point(0.0, 0.0), point(300.0, 0.0)];
        let pins = build_pins(&g, &pos, point(0.0, 0.0), 1.0);
        let edges = build_edges(&g, &pos, point(0.0, 0.0), 1.0);
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
        let g = GraphEntry {
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
        };
        let pos = vec![point(0.0, 0.0), point(300.0, 0.0)];
        let edges = build_edges(&g, &pos, point(0.0, 0.0), 1.0);
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
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![],
            exit: vec![],
            nodes: vec![node_with_types(0, 0, 0, 1, &[], &["Int"])],
            edges: vec![],
        };
        let pos = vec![point(0.0, 0.0)];
        let pins = build_pins(&g, &pos, point(0.0, 0.0), 1.0);
        assert!(hit_test(&pins, &[], point(5000.0, 5000.0)).is_none());
    }

    /// 控制流引脚**不给**气泡:控制流不分类型,显示 `None` 只会误导
    /// (读起来像"这个引脚没连上")。
    #[test]
    fn control_pins_have_no_tooltip() {
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![],
            exit: vec![],
            nodes: vec![node(1, 1, 0, 0)],
            edges: vec![],
        };
        let pos = vec![point(0.0, 0.0)];
        let pins = build_pins(&g, &pos, point(0.0, 0.0), 1.0);
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
