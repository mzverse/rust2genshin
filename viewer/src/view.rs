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
//! 事件分工:节点的左键 `on_mouse_down` 算出这次要拖的一组节点、记进
//! [`Drag::Nodes`] 并 `stop_propagation()`(否则同一次按下会既拖节点又框选 /
//! 平移画布);冒到容器的左键按下 = 空白处框选([`Drag::Marquee`]),中键 = 平移
//! ([`Drag::Pan`]);移动、缩放、松手全部在容器上。
//!
//! # 焦点
//!
//! 画布持有焦点([`Viewer::canvas_focus`]),左栏持有另一个焦点
//! ([`Viewer::search_focus`])。这不是为了输入字符 —— 是为了让
//! `ModifiersChanged` 事件能到达根 `div`:gpui 只沿"根 → 已聚焦元素"这条路径
//! 分发它,焦点为空时路径上只剩根 View 的 dispatch 节点。
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
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

// `gpui::util` 是私有模块,`FluentBuilder`(提供 `.when(..)`)只能从 prelude 拿。
use gpui::prelude::FluentBuilder;
use gpui::{
    Bounds, Context, ExternalPaths, InteractiveElement, IntoElement, ModifiersChangedEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, NavigationDirection,
    ParentElement, PathBuilder, Pixels, Point, Render, ScrollDelta, ScrollWheelEvent, Size,
    StatefulInteractiveElement, Styled, Window, canvas, div, hsla, point, px, rgba, size,
};
use rust2genshin::compile::ir::IrNodeId;
use rust2genshin::compile::link::Target;
use rust2genshin::node::NodeRef;

use crate::conv;
use crate::doc::{
    BASE_FONT_PX, COMMENT_FONT_PX, COMMENT_GAP, COMMENT_LINE_H, COMMENT_PAD_X, COMMENT_PAD_Y, Doc,
    Edge, FnInterface, GraphEntry, NodeEntry, PinRows, comment_height, comment_width, pin_rows,
};
use crate::optimize::{self, RULES, Snapshot};

/// 右栏(优化按钮 + 调用约定)的宽度(px)。
const RULES_WIDTH: f32 = 360.0;
/// 「调用约定」面板的高度**下限**(px)。实际高度由外层盒子的引脚数决定 ——
/// 真实产物里有 20+ 个参数的函数,固定高度会把盒子裁掉。
const CONV_HEIGHT: f32 = 300.0;
/// 「调用约定」面板的内边距。
const CONV_PAD: f32 = 8.0;

// Ctrl+Z → 撤销、Ctrl+Y → 重做、Alt+←/→ → 后退 / 前进。
// `actions!` 展开出一个单元结构体 + `gpui::Action` 实现;键位在 `main.rs` 绑
// (`AppContext::bind_keys`),处理也挂在 App 级(`AppContext::on_action`)。
// **不依赖焦点** —— 焦点在左栏时也该能撤销,挂在 App 级就不用每个持有焦点的
// 元素各绑一遍。
gpui::actions!([Undo, Redo, NavBack, NavForward]);

/// 缩放下限。再小节点上就是一个点,看不清。
const MIN_SCALE: f32 = 0.2;
/// 缩放上限。再大单个节点糊满整屏,也看不清边。
const MAX_SCALE: f32 = 4.0;
/// 一格滚轮的缩放倍率。
const ZOOM_STEP: f32 = 1.1;
/// 左栏默认宽度(px)。
const TOC_WIDTH: f32 = 240.0;
/// 左栏宽度下限:再窄连函数名都放不下。
const TOC_MIN_WIDTH: f32 = 160.0;
/// 左栏宽度上限。
const TOC_MAX_WIDTH: f32 = 720.0;
/// 左栏右侧分隔条的宽度(px)。
const TOC_DIVIDER_WIDTH: f32 = 5.0;

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
/// 左栏分隔条的底色。**不透明** —— 之前没设背景,空隙里透出的是窗口底色,
/// 看起来像左栏裂了一道缝。
fn divider_bg() -> gpui::Rgba {
    rgba(0x2c3442ff)
}
/// 分隔条悬停时的底色(和按下拖动的反馈)。
fn divider_hover_bg() -> gpui::Rgba {
    rgba(0x3d4a5eff)
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
/// 选中节点的描边色。亮黄 —— 和各类节点自己的颜色都区分得开。
fn selected_border() -> gpui::Rgba {
    rgba(0xffd866ff)
}
/// 按住 Ctrl 悬停复合节点时的描边色。青色 —— 提示"点一下能跳进定义"。
fn link_hint_border() -> gpui::Rgba {
    rgba(0x4ec9b0ff)
}
/// 框选矩形的边框 / 底色。
fn marquee_border() -> gpui::Rgba {
    rgba(0x7aa2f7ff)
}
fn marquee_bg() -> gpui::Rgba {
    rgba(0x7aa2f733)
}
/// 注释框底色。比节点淡一档,读起来是「备注」而不是另一个节点。
fn comment_bg() -> gpui::Rgba {
    rgba(0x1b2130cc)
}
fn comment_border() -> gpui::Rgba {
    rgba(0x8a93a655)
}

/// 右栏按钮的**禁用**底色:比可用的暗一档,读起来像"按不动"。
fn button_disabled_bg() -> gpui::Rgba {
    rgba(0xffffff08)
}
/// 右栏按钮**禁用**时的文字色。
fn button_disabled_text() -> gpui::Rgba {
    rgba(0x8a93a680)
}

/// 右栏一个按钮的外壳(尺寸 / 位置一样,只有可用与否不同)。
///
/// **禁用态不挂 `on_click`** —— 这是最省事也最不会出错的做法:没有回调就没有
/// 任何路径能触发那个操作。不用 `cursor_pointer`,再压暗一档。
///
/// `enabled` 为 `true` 时**仍然要调用方自己挂 `on_click`**(见 `render` 里的
/// `.when(editable, |d| d.on_click(..))`):这里拿不到 `cx`,硬塞一个回调进去
/// 反而把"谁负责触发"这件事藏起来。
///
/// 返回 `Stateful<Div>` 而不是 `Div` —— `.id(..)` 的产物就是它,`on_click`
/// 也只在这一层(`StatefulInteractiveElement`)。
fn panel_button(id: &'static str, label: &'static str, enabled: bool) -> gpui::Stateful<gpui::Div> {
    let base = div()
        .id(id)
        .px_2()
        .py_1()
        .mb_1()
        .rounded_sm()
        .overflow_hidden()
        .text_xs();
    if enabled {
        base.cursor_pointer()
            .bg(hsla(0.0, 0.0, 1.0, 0.10))
            .child(label)
    } else {
        base.cursor_default()
            .bg(button_disabled_bg())
            .text_color(button_disabled_text())
            .child(label)
    }
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
    /// 这是不是复合节点(`Fn`)。Ctrl+点击 / Ctrl+悬停只对它有意义。
    is_fn: bool,
    /// 这个节点当前是不是被选中(单选 / 框选)。选中态用另一种描边色。
    selected: bool,
    /// 图边界节点(`entry` / `exit`)描粗一档。
    ///
    /// ⚠️ 这**只是**"这里是图边界"的视觉标记,**不是**可达性结论,
    /// 更不能读成"这里断了" —— 本工具不做任何 CF 分析(spec 的 Out of scope)。
    boundary: bool,
    /// 这一帧的缩放。字号要跟着它走 —— `text_xs()` 那种固定字号不随缩放变。
    scale: f32,
    /// 节点上方的注释：文本 + 估算出来的宽高（世界坐标，未乘缩放）。
    comment: Option<(String, f32, f32)>,
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
///
/// ⚠️ **引脚不再用这个分布** —— 用户要求引脚**靠上**排,不用整条边均分。
/// 那个是 [`pin_row_y`](rust2genshin::node::layout::pin_row_y) 的活。
/// 这个函数只剩 `spread` 的旧用法(如果有的话)。
pub fn spread(lo: f32, len: f32, pin: usize, total: usize) -> f32 {
    if total == 0 {
        return lo + len / 2.0;
    }
    lo + len * (pin as f32 + 1.0) / (total as f32 + 1.0)
}

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
    selected: &[usize],
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

/// 一个节点盒子的**唯一渲染实现**。
///
/// 主画布和「调用约定」视图的内层节点都走这里。返回 `Div`(不是
/// `AnyElement`)—— 调用方要往上挂交互 / 选中描边,必须还是 `Div`。
///
/// 尺寸约定:调用方给的是**节点自己的**盒子(不含任何额外内边距)——
/// 引脚位置来自 core 的 `pin_row_y`,它假定引脚从节点顶部的 `PIN_TOP`
/// 开始。谁在这里偷偷加 `PAD`,引脚就会跑到盒子外面去。
fn node_box_el(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    label: &str,
    border: gpui::Rgba,
) -> gpui::Div {
    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(w))
        .h(px(h))
        .flex()
        .items_center()
        .justify_center()
        .px_1()
        .rounded_md()
        .bg(node_bg())
        .border_1()
        .border_color(border)
        .text_color(text_hi())
        // 字号**跟随缩放**:`text_xs()` 是固定 rem(≈12px),缩放时节点框
        // 会跟着变大变小而字不变 —— 放大后字挤成一撮、缩小后溢出被裁。
        .text_size(px((BASE_FONT_PX * scale).max(6.0)))
        .overflow_hidden()
        .child(label.to_string())
}

/// 一组引脚的**唯一渲染实现**:值流画圆点、控制流画右指的三角形。
///
/// 主画布和「调用约定」视图共用。引脚位置由调用方按 core 的 `pin_row_y`
/// 算好 —— 这里只负责画。
///
/// 每一项是 `(x, y, 是否控制流)`。
fn pin_els(pins: &[(f32, f32, bool)], scale: f32) -> Vec<gpui::AnyElement> {
    let mut out = Vec::new();
    let d = PIN_D * scale;
    // 值流圆点(div 白拿圆角)。
    for &(x, y, _) in pins.iter().filter(|p| !p.2) {
        out.push(
            div()
                .absolute()
                .left(px(x - d / 2.0))
                .top(px(y - d / 2.0))
                .w(px(d))
                .h(px(d))
                .rounded_full()
                .bg(edge_value())
                .into_any_element(),
        );
    }
    // 控制流三角形:div 画不出三角形,用 canvas 填色。
    let tri: Vec<(f32, f32)> = pins.iter().filter(|p| p.2).map(|p| (p.0, p.1)).collect();
    if !tri.is_empty() {
        out.push(
            canvas(
                move |_bounds, _window, _cx| (),
                move |bounds, (), window, _cx| {
                    let o = bounds.origin;
                    for &(cx, cy) in tri.iter() {
                        let t = pin_triangle(cx, cy, d);
                        let mut pb = PathBuilder::fill();
                        pb.move_to(add_px(o, t[0]));
                        pb.line_to(add_px(o, t[1]));
                        pb.line_to(add_px(o, t[2]));
                        pb.close();
                        if let Ok(path) = pb.build() {
                            window.paint_path(path, pin_ctrl());
                        }
                    }
                },
            )
            .absolute()
            .inset_0()
            .into_any_element(),
        );
    }
    out
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

/// 控制出 pin 的分支名文本(容器局部坐标),画在**节点盒子内侧**。
struct CtrlLabelView {
    /// 文本框的**右缘** x(框向左展开,文字右对齐)。
    right: f32,
    y: f32,
    text: String,
}

/// 拍出这一帧要显示的控制出分支名。位置:节点盒内、紧贴控制出引脚左侧。
///
/// 画在盒内而不是盒外 —— 盒子外面就是连线,名字压在线上两边都看不清。
/// 宽度固定(`DEF_W`)且**不裁剪**:分支名长度不定(`default` 短、`"MyStruct"`
/// 长),宁可伸出去也别只剩尾巴。
fn build_ctrl_labels(g: &GraphEntry, pos: &[Point<f32>], origin: Point<f32>, scale: f32) -> Vec<CtrlLabelView> {
    let mut out = Vec::new();
    for (i, n) in g.nodes.iter().enumerate() {
        let Some((p, s)) = node_box(pos, &g.nodes, origin, scale, i) else {
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

/// 一个标签页:打开着哪张图,以及它自己的视图状态。
///
/// 标签页存的是**图下标**而不是图本身 —— `Doc::graphs` 只 append,下标是稳定的,
/// 同一张图在两个标签页里打开时也天然共享同一份数据(改了都能看见)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tab {
    /// 这张标签页显示 `Doc::graphs` 里的第几张图。
    pub graph: usize,
    /// 平移量(**容器局部**坐标)。每个标签页各记一份 —— 切走再切回来,
    /// 视野还停在原处,不会被复位。
    pub origin: Point<f32>,
    /// 缩放。
    pub scale: f32,
}

/// 前进 / 后退历史里的一条记录。
///
/// 记的是"当时在哪张图、视野在哪",所以后退不只是切回上一张图,而是连
/// 平移缩放一起还回来 —— 这两个动作对用户来说是同一件"我刚才在看哪儿"。
#[derive(Clone, Copy, Debug, PartialEq)]
struct NavEntry {
    graph: usize,
    origin: Point<f32>,
    scale: f32,
}

/// 前进 / 后退历史。
///
/// 语义照浏览器:`back` 把当前状态压进 `forward`,自己退一格;`forward` 反过来。
/// 任何**新的**导航(切图、跳转、点左栏)都会清空 `forward`。
#[derive(Default)]
struct NavHistory {
    back: Vec<NavEntry>,
    forward: Vec<NavEntry>,
}

/// 标签页 + 前进/后退历史。
///
/// 单独成一个结构是为了**能测**:它不碰 GPUI,也不碰 `Doc`(下标的合法性由
/// 调用方 `Viewer` 判断)。`Viewer` 持有一个,所有标签页 / 历史的规则都在
/// 这里,视图层只负责把结果搬到 `current` / `origin` / `scale` 上。
struct TabHost {
    tabs: Vec<Tab>,
    active: usize,
    nav: NavHistory,
}

impl TabHost {
    fn new() -> Self {
        Self {
            tabs: vec![Tab {
                graph: 0,
                origin: point(0.0, 0.0),
                scale: 1.0,
            }],
            active: 0,
            nav: NavHistory::default(),
        }
    }

    /// 把"当前看过的位置"压进后退历史,清空前进历史。
    ///
    /// 连续两次记同一个位置**不重复记** —— 否则来回点同一张图会把后退栈
    /// 塞满同一条,按十次后退还在原地。
    fn record(&mut self, entry: NavEntry) {
        if self.nav.back.last() == Some(&entry) {
            return;
        }
        self.nav.back.push(entry);
        self.nav.forward.clear();
    }

    /// 后退一步:返回要跳到的位置,并把当前位置压进前进栈。
    fn back(&mut self, current: NavEntry) -> Option<NavEntry> {
        let prev = self.nav.back.pop()?;
        self.nav.forward.push(current);
        Some(prev)
    }

    /// 前进一步:返回要跳到的位置,并把当前位置压回后退栈。
    fn forward(&mut self, current: NavEntry) -> Option<NavEntry> {
        let next = self.nav.forward.pop()?;
        self.nav.back.push(current);
        Some(next)
    }

    /// 新开一个标签页并激活。返回新标签页的副本。
    ///
    /// **不校验图下标** —— 那要问 `Doc`,由调用方(`Viewer`)负责。
    fn open(&mut self, graph: usize) -> Tab {
        let tab = Tab {
            graph,
            origin: point(0.0, 0.0),
            scale: 1.0,
        };
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        tab
    }

    /// 切到已经开着这张图的页签(没有则 `None`)。**不新建**。
    ///
    /// 这是"按图定位页签"的唯一实现:左栏点图、历史导航都靠它决定
    /// "是切过去还是新开一页"。
    fn activate_graph(&mut self, graph: usize) -> Option<Tab> {
        let idx = self.find_graph(graph)?;
        self.active = idx;
        self.tabs.get(idx).copied()
    }

    /// 切到第 `idx` 个标签页。返回它的副本。下标越界或已是当前页时 `None`。
    fn switch(&mut self, idx: usize) -> Option<Tab> {
        if idx >= self.tabs.len() || idx == self.active {
            return None;
        }
        self.active = idx;
        self.tabs.get(idx).copied()
    }

    /// 关掉第 `idx` 个标签页,返回新的活动标签页。
    ///
    /// 只剩一个时**不真的关**(窗口总得有东西可看),只把视野复位。
    fn close(&mut self, idx: usize) -> Option<Tab> {
        if idx >= self.tabs.len() {
            return None;
        }
        if self.tabs.len() == 1 {
            let tab = self.tabs.first_mut()?;
            tab.origin = point(0.0, 0.0);
            tab.scale = 1.0;
            return Some(*tab);
        }
        self.tabs.remove(idx);
        // 活动标签的落点:关掉的在它左边就左移一位;关掉的正是它就取右邻。
        if idx < self.active {
            self.active -= 1;
        } else if idx == self.active {
            self.active = self.active.min(self.tabs.len() - 1);
        }
        self.tabs.get(self.active).copied()
    }

    /// 已经开着这张图的页签下标(没有则 `None`)。
    fn find_graph(&self, graph: usize) -> Option<usize> {
        self.tabs.iter().position(|t| t.graph == graph)
    }

    /// 把当前标签页的视野记账更新(平移 / 缩放 / 换图之后调)。
    fn store_view(&mut self, graph: usize, origin: Point<f32>, scale: f32) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.graph = graph;
            tab.origin = origin;
            tab.scale = scale;
        }
    }
}

/// 拖「调用约定」视图里的内层节点时记的东西。
///
/// 按下时记一份**锚点**:鼠标在哪、内层节点在哪。之后的位置一律按
/// `start + (现在 - anchor)` 算。
///
/// ⚠️ **不能累加每次的移动量** —— 指针移出窗口再回来时,补上的那次
/// `mouse_move` 带着整段位移(win32 的 wparam 在指针离开时不带按键信息,
/// `mouse_up` 收不到),累加实现会把这一整段一次性甩到位置上。
/// 相对按下点算就没有这个问题:中间丢多少事件都不影响。
#[derive(Clone, Copy, Debug)]
struct ConvDrag {
    /// 拖的是哪张图。
    graph: usize,
    /// 按下时的鼠标位置(窗口坐标)。
    anchor: Point<f32>,
    /// 按下时内层节点的位置(面板局部坐标)。
    start: (f32, f32),
}

/// 拖拽的种类。
enum Drag {
    /// 拖动**一组**节点(可能只有一个)。`last` 是上一次鼠标位置(窗口坐标)。
    Nodes {
        /// 这次要一起移动的节点(展平下标)。
        indices: Vec<usize>,
        /// 按下的那个节点 —— 松手时用它判断"是单击还是拖动"。
        primary: usize,
        /// 有没有真的动过。没动过 = 单击 = 选区收成一个节点。
        moved: bool,
        last: Point<f32>,
    },
    /// 平移画布。`last` 是上一次鼠标位置(窗口坐标)。
    Pan { last: Point<f32> },
    /// 框选。`start` / `cur` 是**容器局部**坐标(和节点框同一个坐标系,
    /// 判交才不用来回换算)。
    Marquee { start: Point<f32>, cur: Point<f32> },
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
    /// 每张图一个重做栈,与 [`Viewer::undo_stack`] 同构。
    ///
    /// 任何**新的**操作都会把它清空(标准撤销语义:改过的分支就不能再重做了);
    /// 撤销时把"撤之前的这一帧"压进来,重做时再弹出去。
    redo_stack: HashMap<usize, Vec<Snapshot>>,
    /// 哪些图跑过「lower」。图下标只增不改,所以这个集合不会串台。
    ///
    /// **必须显式记下来,不能从图上推**:lower 之后节点全成了 `IrNodeId::Native`,
    /// 但反过来推不成立 —— 点「一键优化」也可能把一张小图消到只剩一两个原生节点
    /// (`add_n` / `borrow_read` / `pair_sum` 这些在真实产物里就是这样),
    /// 那种图**没有**被 lower,改写按钮必须还能点。
    ///
    /// 这个标志跟着**存档**走([`Snapshot::lowered`]):撤销 / 重做恢复图的同时
    /// 也恢复它,所以 lower 之后 Ctrl+Z 会把按钮放回来。
    lowered: HashSet<usize>,
    /// 打开的标签页 + 前进/后退历史(规则都在 [`TabHost`] 里,可单独测)。
    tabs: TabHost,
    /// 正在执行历史导航中 —— 这期间切图**不该**再往后退历史里压一条,
    /// 否则后退会把自己也算成一次新导航,历史立刻变成"后退/前进/后退/前进"。
    navigating: bool,
    /// 右栏底部的一行状态(最近一次优化 / 撤销的结果)。
    status: Option<String>,
    /// 节点右键菜单,`None` = 没弹。
    menu: Option<NodeMenu>,
    /// 当前悬停命中的引脚 / 边,`None` = 鼠标不在任何带气泡的东西上。
    hover: Option<Hover>,
    /// 左栏宽度(px),拖左栏右边那条分隔条改。
    toc_width: f32,
    /// 正在拖分隔条。放在 `Viewer` 上而不是分隔条自己的状态里:
    /// 指针拖出去以后分隔条的 hitbox 不再 hover,移动事件收不到。
    resizing_toc: bool,
    /// 上一次移动事件的指针 x(**窗口**坐标),算宽度增量用。
    toc_drag_x: f32,
    /// 左栏搜索框里的文字。空串 = 不过滤。
    search: String,
    /// 搜索框的焦点。点击搜索框时 `window.focus(..)`,键盘输入才进得来。
    search_focus: gpui::FocusHandle,
    /// 画布的焦点。点画布 / 节点时焦点落在这里(而不是 `window.blur()`)。
    ///
    /// **为什么非得有个焦点**:gpui 的 `ModifiersChanged` 事件只沿"根 → 已聚焦
    /// 元素"这条路径分发(`window.rs` 的 `dispatch_modifiers_changed_event`)。
    /// 焦点是 `None` 时路径上只有根 View 的那一个 dispatch 节点,我们挂在根 `div`
    /// 上的 `on_modifiers_changed` 收不到 —— 表现就是"按住 Ctrl 不动鼠标时
    /// 节点不变色"。让画布始终持有焦点,根 `div` 就总在路径上。
    canvas_focus: gpui::FocusHandle,
    /// 当前选中的节点(展平后的**下标**,不是 `NodeRef`)。
    ///
    /// 跟着当前图走:切图时清空(`select` 里做)。下标只在**这一张图**里
    /// 有意义,换图就换一套。
    selected: Vec<usize>,
    /// 框选的实时矩形(容器局部坐标)。`None` = 没在框选。
    ///
    /// 只在拖动过程中存在,松手后清掉 —— 选中的结果记在 [`Viewer::selected`]。
    marquee: Option<(Point<f32>, Point<f32>)>,
    /// Ctrl 是否按着。悬停复合节点时用它决定要不要变色(提示"点一下能跳进去")。
    ctrl_held: bool,
    /// 鼠标当前压在哪个节点上(展平下标)。`None` = 没压在任何节点上。
    ///
    /// 和 [`Viewer::hover`](引脚/边的气泡)是两回事:这个只用来判断
    /// "Ctrl+悬停"要不要给节点换色。
    hover_node: Option<usize>,
    /// 「调用约定」视图里**内层节点**的位置(右栏面板局部坐标)。
    ///
    /// 每个图各记一份 —— 换图时位置复原,免得串台。
    conv_pos: HashMap<usize, (f32, f32)>,
    /// 正在拖「调用约定」视图里的内层节点。
    ///
    /// `anchor` 是**按下时**的鼠标位置(窗口坐标),`start` 是按下时内层节点的
    /// 位置(面板局部坐标)。位置按 `start + (现在 - anchor)` 算,不是累加
    /// 每次的移动量 —— 理由见 [`Viewer::drag_conv_inner`]。
    conv_drag: Option<ConvDrag>,
    /// 「调用约定」视图里当前悬停命中的东西(`None` = 没悬在任何东西上)。
    ///
    /// 悬停气泡显示**类型** —— 引脚旁边只写名字(用户要求)。
    conv_hover: Option<conv::Hit>,
    /// 「调用约定」面板在**窗口**坐标系里的左上角。
    ///
    /// 由面板的 canvas prepaint 每帧写 —— 悬停命中要把窗口坐标换算成
    /// 面板局部坐标,靠它。
    conv_origin: Rc<Cell<Point<f32>>>,
}

impl Viewer {
    /// `cx` 只用来拿 [`gpui::FocusHandle`],没有别的初始化要上下文。
    pub fn new(cx: &mut Context<Self>) -> Self {
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
            redo_stack: HashMap::new(),
            lowered: HashSet::new(),
            tabs: TabHost::new(),
            navigating: false,
            status: None,
            menu: None,
            hover: None,
            toc_width: TOC_WIDTH,
            resizing_toc: false,
            toc_drag_x: 0.0,
            search: String::new(),
            search_focus: cx.focus_handle(),
            canvas_focus: cx.focus_handle(),
            selected: Vec::new(),
            marquee: None,
            ctrl_held: false,
            hover_node: None,
            conv_pos: HashMap::new(),
            conv_drag: None,
            conv_hover: None,
            conv_origin: Rc::new(Cell::new(point(0.0, 0.0))),
        }
    }

    /// 开窗时该把焦点交给谁:画布。
    ///
    /// 见 [`Viewer::canvas_focus`]:焦点为 `None` 时 modifier 事件到不了根
    /// `div`,按住 Ctrl 悬停复合节点就不变色。
    pub fn initial_focus(&self) -> gpui::FocusHandle {
        self.canvas_focus.clone()
    }

    /// 图名是否命中搜索词。空搜索词一律命中;比较前两边都转小写。
    fn matches_search(name: &str, query: &str) -> bool {
        query.is_empty() || name.to_lowercase().contains(&query.to_lowercase())
    }

    /// 追加一个已解析的 `Target`,并在一张**新标签页**里打开它的第一张图。
    /// 拖放回调与命令行入口都走这里。
    ///
    /// **追加而不是替换** —— spec:"拖入第二个文件不清空已有内容"。
    /// 新图在这里算好坐标;已有图的坐标(含拖动)原样保留。
    ///
    /// 第一个文件复用那张初始空标签页,之后每个文件各开一张新标签页
    /// —— 和浏览器"新开一个文件就是新开一页"一致。
    pub fn add_target(&mut self, t: Target, cx: &mut Context<Self>) {
        let had_graphs = !self.doc.graphs.is_empty();
        if let Some(first) = self.doc.push_target(t) {
            // 布局只在**加载时**算一次(`push_target` 里由 core 的
            // `node::layout` 写进 IR 节点),这里只是把坐标读进视图状态。
            // 之后切图不重算 —— 拖过的位置能留住,想重排得点「自动整理」。
            for g in &self.doc.graphs[first..] {
                self.positions
                    .push(g.nodes.iter().map(|n| n.position).collect());
            }
            self.error = None;
            if had_graphs {
                self.open_tab(first, cx);
            } else {
                self.select(first, cx);
            }
        } else {
            cx.notify();
        }
    }

    /// 在当前标签页里切到第 `i` 张图。
    ///
    /// 左栏点图 / 跳转到定义都走这里。**会记一条后退历史**
    /// (除非正在执行历史导航本身,见 [`Viewer::navigating`])。
    ///
    /// 视野复位到原点 —— 这是"换了一张图"的语义,不是"回到刚才的位置"。
    /// 想保住视野请用标签页(每个标签页各记一份 origin/scale)。
    pub fn select(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.select_graph(i) {
            cx.notify();
        }
    }

    /// [`Viewer::select`] 的纯逻辑部分。返回是否真的改了。
    fn select_graph(&mut self, i: usize) -> bool {
        // 越界直接返回:**不索引**。切图是交互驱动的,双击列表/拖入文件
        // 交叠时给一个越界下标是完全可能的。
        if self.doc.graphs.get(i).is_none() {
            return false;
        }
        if !self.navigating {
            self.record_nav();
        }
        // 换图 = 视野复位(不是"回到刚才的位置")。
        self.show(i, point(0.0, 0.0), 1.0);
        true
    }

    // -------------------------------------------------- 视野 / 导航 / 标签页

    /// 当前的"看过的位置"。
    fn here(&self) -> NavEntry {
        NavEntry {
            graph: self.current,
            origin: self.origin,
            scale: self.scale,
        }
    }

    /// 把"当前在看哪儿"压进后退历史,并清掉前进历史(新导航 = 前进分支作废)。
    fn record_nav(&mut self) {
        let now = self.here();
        self.tabs.record(now);
    }

    /// 把视图切到"第 `graph` 张图 + 这个视野",并同步进当前标签页。
    ///
    /// 所有"换视野"的路径最后都落到这里 —— 标签页切换、历史导航、左栏点图的
    /// 差别只在归一化(视野复位 / 保留)和历史记账上,搬运是同一份。
    fn show(&mut self, graph: usize, origin: Point<f32>, scale: f32) {
        self.current = graph;
        self.origin = origin;
        self.scale = scale;
        self.tabs.store_view(graph, origin, scale);
        self.drag = None;
        // 状态行说的是"上一张图的操作",切过来还挂着会误导。
        self.status = None;
        // 菜单记的节点下标属于上一张图,不能留着。
        self.menu = None;
        // 选区也是上一张图的下标,换图就得清空。
        self.clear_selection();
    }

    /// 后退:回到上一个"看过的位置"(图 + 视野)。
    pub fn nav_back(&mut self, cx: &mut Context<Self>) {
        if self.nav_back_step() {
            cx.notify();
        }
    }

    /// 前进:抵消上一次后退。
    pub fn nav_forward(&mut self, cx: &mut Context<Self>) {
        if self.nav_forward_step() {
            cx.notify();
        }
    }

    /// [`Viewer::nav_back`] 的纯逻辑部分。返回是否真的动了。
    fn nav_back_step(&mut self) -> bool {
        let now = self.here();
        let Some(prev) = self.tabs.back(now) else {
            self.status = Some("没有可后退的位置".to_string());
            return false;
        };
        self.apply_nav(prev)
    }

    /// [`Viewer::nav_forward`] 的纯逻辑部分。返回是否真的动了。
    fn nav_forward_step(&mut self) -> bool {
        let now = self.here();
        let Some(next) = self.tabs.forward(now) else {
            self.status = Some("没有可前进的位置".to_string());
            return false;
        };
        self.apply_nav(next)
    }

    /// 把当前的 `origin` / `scale` 回写进**当前标签页**。
    ///
    /// 平移和缩放是直接改 `self.origin` / `self.scale` 的,页签自己不会知道。
    /// 不调这个,切走再切回来就会看到**打开时那一刻**的视野 —— 用户实测的
    /// "切回来缩放和位置被重置"就是这个。
    ///
    /// 视野一动就调一次(平移、缩放、滚轮)。开销只是一个 `Vec` 里的字段赋值。
    fn sync_view(&mut self) {
        self.tabs
            .store_view(self.current, self.origin, self.scale);
    }

    /// 跳到历史里记的那个位置。**不碰历史本身**(调用方已经维护好了)。
    ///
    /// 关键是**不要改写当前页签的内容** —— 那是"替换",用户明确否掉了。
    /// 正确的做法和左栏点图完全一样:那张图已经开在某个页签里就切过去,
    /// 否则新开一页。历史记的页签可能早就被关掉了,所以这里不能按页签找,
    /// 只能按**图**找。
    fn apply_nav(&mut self, entry: NavEntry) -> bool {
        if self.doc.graphs.get(entry.graph).is_none() {
            self.status = Some("那个位置已经不在了".to_string());
            return false;
        }
        // 期间若再走 `select` 也不会重复记历史。
        self.navigating = true;
        match self.tabs.activate_graph(entry.graph) {
            // 这张图已经开着:切过去,并按历史还原视野。
            Some(_) => self.show(entry.graph, entry.origin, entry.scale),
            // 原来那页已经关掉了 —— 按"打开这张图"的规则新开一页,
            // 而不是把当前页签改掉。
            None => {
                let tab = self.tabs.open(entry.graph);
                self.show(tab.graph, entry.origin, entry.scale);
            }
        }
        self.navigating = false;
        true
    }

    /// 新开一个标签页显示第 `i` 张图,并切过去。
    ///
    /// 这是**导航**的一种(用户换了视野),所以会记一条后退历史。
    pub fn open_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.open_tab_step(i) {
            cx.notify();
        }
    }

    /// 切到第 `idx` 个标签页。**记一条后退历史**(用户要求:切标签页可后退)。
    pub fn switch_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if self.switch_tab_step(idx) {
            cx.notify();
        }
    }

    /// 关掉第 `idx` 个标签页。中键、标签上的关闭按钮都走这里。
    pub fn close_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if self.close_tab_step(idx) {
            cx.notify();
        }
    }

    /// [`Viewer::open_tab`] 的纯逻辑部分。
    fn open_tab_step(&mut self, i: usize) -> bool {
        if self.doc.graphs.get(i).is_none() {
            return false;
        }
        self.record_nav();
        let tab = self.tabs.open(i);
        self.show(tab.graph, tab.origin, tab.scale);
        true
    }

    /// 在标签页里打开第 `i` 张图(左栏点图走这里)。
    ///
    /// 已经开在某个页签里就**切过去**,否则新开一页。这样反复点同一项不会
    /// 堆出一排指向同一张图的重复页签 —— 编辑器和浏览器都是这个规则。
    ///
    /// 后退 / 前进走的是同一条规则(见 [`Viewer::apply_nav`]):那张图开着就
    /// 切过去,原来那页已经关掉才新开 —— **绝不改写当前页签的内容**。
    pub fn open_graph_in_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.open_graph_step(i) {
            cx.notify();
        }
    }

    /// [`Viewer::open_graph_in_tab`] 的纯逻辑部分。
    fn open_graph_step(&mut self, i: usize) -> bool {
        if self.doc.graphs.get(i).is_none() {
            return false;
        }
        self.record_nav();
        match self.tabs.activate_graph(i) {
            Some(tab) => self.show(tab.graph, tab.origin, tab.scale),
            None => {
                let tab = self.tabs.open(i);
                self.show(tab.graph, tab.origin, tab.scale);
            }
        }
        true
    }

    /// [`Viewer::switch_tab`] 的纯逻辑部分。
    fn switch_tab_step(&mut self, idx: usize) -> bool {
        // 先记历史再切 —— `record_nav` 记的是"切之前"的位置。
        if idx >= self.tabs.tabs.len() || idx == self.tabs.active {
            return false;
        }
        self.record_nav();
        let Some(tab) = self.tabs.switch(idx) else {
            return false;
        };
        self.show(tab.graph, tab.origin, tab.scale);
        true
    }

    /// [`Viewer::close_tab`] 的纯逻辑部分。
    ///
    /// 关掉最后一个时**不真的关**(窗口得有东西可看):退回"只剩这一张图"的
    /// 单标签状态。
    fn close_tab_step(&mut self, idx: usize) -> bool {
        if idx >= self.tabs.tabs.len() {
            return false;
        }
        self.record_nav();
        let Some(tab) = self.tabs.close(idx) else {
            return false;
        };
        self.show(tab.graph, tab.origin, tab.scale);
        true
    }

    /// 当前标签页的标题(图名)。没有图时给个占位。
    fn tab_title(&self, tab: &Tab) -> String {
        self.doc
            .graphs
            .get(tab.graph)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "?".to_string())
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
            let snap = self.snapshot(self.current, bytes, refs, pos, optimize::SnapshotKind::Optimize);
            self.push_undo(self.current, snap);
        }
        self.set_pos(r.pos);
        self.clear_selection();
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

    /// 压一条新的撤销存档,并**清空当前图的重做栈**。
    ///
    /// 所有会改图的入口都必须走这里(而不是直接 `undo_stack.push`):
    /// 一旦用户做了新操作,原来那条"重做"分支就不成立了 —— 标准撤销语义。
    fn push_undo(&mut self, i: usize, snapshot: Snapshot) {
        self.undo_stack.entry(i).or_default().push(snapshot);
        self.redo_stack.remove(&i);
    }

    /// 给某一帧拍一个存档(还没决定压到哪个栈)。
    ///
    /// `lowered` 在这里统一从 [`Viewer::lowered`] 读 —— 调用方不必各自记得填。
    fn snapshot(
        &self,
        i: usize,
        bytes: Vec<u8>,
        refs: Vec<NodeRef>,
        pos: Vec<Point<f32>>,
        kind: optimize::SnapshotKind,
    ) -> Snapshot {
        Snapshot {
            bytes,
            refs,
            pos,
            kind,
            lowered: self.lowered.contains(&i),
        }
    }

    /// 设置某张图的「已 lower」标记。
    ///
    /// 撤销 / 重做恢复图之后调它 —— 存档里记着那一帧的状态,所以按钮的
    /// 禁用态跟着一起回滚。
    fn set_lowered(&mut self, i: usize, lowered: bool) {
        if lowered {
            self.lowered.insert(i);
        } else {
            self.lowered.remove(&i);
        }
    }

    /// 当前图是不是已经跑过「lower」(决定右栏那几个按钮是否禁用)。
    ///
    /// 直接查 [`Viewer::lowered`],**不从图上推导** —— 理由见那个字段的注释。
    fn is_lowered(&self) -> bool {
        self.lowered.contains(&self.current)
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
        self.push_undo(i, Snapshot {
            bytes,
            refs,
            pos: old_pos,
            kind: optimize::SnapshotKind::Arrange,
            lowered: self.lowered.contains(&i),
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

    /// 右栏「一键优化」:消除规则跑到不动点。
    /// **不含**展开 Fn 节点 —— 那是单独的「链接全部」按钮。
    /// 整次点击只留一个存档:一次 Ctrl+Z 回到点击前。
    pub fn optimize_all(&mut self, cx: &mut Context<Self>) {
        self.batch_op(optimize::optimize_all, false, cx);
    }

    /// 右栏「链接全部」:把当前图里所有 Fn 节点展开成内联的复合节点
    /// (右键「link」的批量版)。
    pub fn link_all(&mut self, cx: &mut Context<Self>) {
        self.batch_op(optimize::link_all, false, cx);
    }

    /// 右栏「循环改写」:把控制流里的环换成 native 的「有限循环」节点。
    pub fn lower_loops(&mut self, cx: &mut Context<Self>) {
        self.batch_op(optimize::lower_loops_step, false, cx);
    }

    /// 右栏「lower」:跑 core 的**完整**降级流程(`Optimizer::lower_ir`)。
    ///
    /// 跑完之后图里不再有 Fn / 循环 / 列表 IR 节点,右栏那几个改写按钮就没有
    /// 对象了 —— 禁用态由 [`Viewer::is_lowered`] 决定,判据是显式记下来的
    /// [`Viewer::lowered`],**不是**从图上推(理由见那个字段的注释)。
    pub fn lower(&mut self, cx: &mut Context<Self>) {
        self.batch_op(optimize::lower_full, true, cx);
    }

    /// 批量操作(优化 / 链接全部 / 循环改写 / lower)的公共外壳:
    /// 取当前图的展平顺序与坐标 → 调 `f` → 存撤销档 → 换坐标 → 写状态栏。
    ///
    /// `marks_lowered` 只有「lower」传 `true`:存档**先**拍(那时还没标记),
    /// 再把这张图记进 [`Viewer::lowered`] —— 于是撤销回到"没 lower 过"的那一帧,
    /// 按钮跟着复活。
    fn batch_op(
        &mut self,
        f: fn(&mut Doc, usize, &[NodeRef], &[Point<f32>]) -> optimize::ApplyResult,
        marks_lowered: bool,
        cx: &mut Context<Self>,
    ) {
        let i = self.current;
        let refs = self.current_refs();
        let pos = self.positions.get(i).cloned().unwrap_or_default();
        let r = f(&mut self.doc, i, &refs, &pos);
        // 先看有没有变化 —— `r.undo` 下面要移进存档里。
        let changed = r.undo.is_some();
        if let Some(bytes) = r.undo {
            let snap = self.snapshot(i, bytes, refs, pos, optimize::SnapshotKind::Optimize);
            self.push_undo(i, snap);
        }
        // 只有真的改了图才记标记:lower 报"无变化"时不该禁用按钮。
        if marks_lowered && changed {
            self.lowered.insert(i);
        }
        self.set_pos(r.pos);
        // 优化会删 / 换节点,展平下标跟着变 —— 旧选区不能再留着指别人。
        self.clear_selection();
        self.status = Some(r.status);
        cx.notify();
    }

    /// 清空选区与框选矩形。凡是有可能改动"展平下标 → 节点"对应关系的操作
    /// 都要调它,否则高亮会落到别的节点上、拖动还会拖错。
    fn clear_selection(&mut self) {
        self.selected.clear();
        self.marquee = None;
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
            let snap = self.snapshot(self.current, bytes, refs, pos, optimize::SnapshotKind::Optimize);
            self.push_undo(self.current, snap);
        }
        self.set_pos(r.pos);
        self.clear_selection();
        self.status = Some(r.status);
        cx.notify();
    }

    /// Ctrl+Z:撤掉当前图最近一次操作。
    ///
    /// **右栏没有撤销按钮** —— 只留快捷键(用户要求)。撤之前先把"当前这一帧"
    /// 压进重做栈,所以撤销随时可以被 Ctrl+Y 反向抵掉。
    pub fn undo_step(&mut self, cx: &mut Context<Self>) {
        let i = self.current;
        let Some(entry) = self.undo_stack.get_mut(&i).and_then(|s| s.pop()) else {
            self.status = Some("没有可撤销的操作".to_string());
            cx.notify();
            return;
        };
        let refs = self.current_refs();
        let pos = self.positions.get(i).cloned().unwrap_or_default();
        // 先给"撤之前"这一帧拍个快照 —— 它就是重做要回到的状态。
        let current = self
            .doc
            .encode(i)
            .ok()
            .map(|bytes| self.snapshot(i, bytes, refs.clone(), pos.clone(), optimize::SnapshotKind::Redo));
        let (new_pos, status) = optimize::undo(&mut self.doc, i, &refs, &pos, &entry);
        if let Some(s) = current {
            self.redo_stack.entry(i).or_default().push(s);
        }
        // 存档里记着那一帧的 lower 状态 —— 恢复图的同时也把按钮恢复回来。
        self.set_lowered(i, entry.lowered);
        self.set_pos(new_pos);
        // 撤销会把节点列表换回旧版本,下标对应关系跟着变。
        self.clear_selection();
        self.status = Some(status);
        cx.notify();
    }

    /// Ctrl+Y:重做刚被撤销的那一步。
    pub fn redo_step(&mut self, cx: &mut Context<Self>) {
        let i = self.current;
        let Some(entry) = self.redo_stack.get_mut(&i).and_then(|s| s.pop()) else {
            self.status = Some("没有可重做的操作".to_string());
            cx.notify();
            return;
        };
        let refs = self.current_refs();
        let pos = self.positions.get(i).cloned().unwrap_or_default();
        // 这一帧的反向存档:重做之后还能再撤销回来,且要**整份**回到这一帧
        // (所以 kind 用 `Redo` = 坐标也照存档,而不是优化那种"当前优先")。
        // 走 `push_undo` 会清空重做栈 —— 所以这里**直接**压,不能经过那个入口。
        let current = self
            .doc
            .encode(i)
            .ok()
            .map(|bytes| self.snapshot(i, bytes, refs.clone(), pos.clone(), optimize::SnapshotKind::Redo));
        let (new_pos, status) = optimize::redo(&mut self.doc, i, &refs, &pos, &entry);
        if let Some(s) = current {
            self.undo_stack.entry(i).or_default().push(s);
        }
        self.set_lowered(i, entry.lowered);
        self.set_pos(new_pos);
        self.clear_selection();
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
    fn update_hover(&mut self, at_window: Point<f32>, ctrl: bool, cx: &mut Context<Self>) {
        let o = self.canvas_origin.get();
        let at = point(at_window.x - o.x, at_window.y - o.y);
        let hit = self.pick(at);
        let changed = match (&self.hover, &hit) {
            (None, None) => false,
            (Some(h), Some(n)) => h.text != n.text || h.at != n.at,
            _ => true,
        };
        // Ctrl 按下/松开也要重绘:它决定复合节点要不要换成"可跳转"的青色。
        // 不动鼠标、只按 Ctrl 的那条路走根 `div` 的 `on_modifiers_changed`,
        // 这里只是让鼠标事件也顺手同步一次(焦点万一丢了也能跟上)。
        let ctrl_changed = self.set_ctrl_held(ctrl);
        let node = self.node_at(at);
        let node_changed = self.hover_node != node;
        if changed {
            self.hover = hit;
        }
        if node_changed {
            self.hover_node = node;
        }
        if changed || ctrl_changed || node_changed {
            cx.notify();
        }
    }

    /// 记录 Ctrl 的按下状态;真变了返回 `true`(调用方据此决定要不要重绘)。
    fn set_ctrl_held(&mut self, ctrl: bool) -> bool {
        if self.ctrl_held == ctrl {
            return false;
        }
        self.ctrl_held = ctrl;
        true
    }

    /// 容器局部坐标 `at` 处的节点(展平下标)。
    ///
    /// 命中多个时取**最后一个**(后画的在上层)。节点框用和渲染一样的
    /// [`node_box`],所以"看到的"和"命中的"是同一套几何。
    fn node_at(&self, at: Point<f32>) -> Option<usize> {
        let g = self.doc.graphs.get(self.current)?;
        let pos = self
            .positions
            .get(self.current)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut hit = None;
        for i in 0..g.nodes.len() {
            let Some((p, s)) = node_box(pos, &g.nodes, self.origin, self.scale, i) else {
                continue;
            };
            if at.x >= p.x && at.x <= p.x + s.width && at.y >= p.y && at.y <= p.y + s.height {
                hit = Some(i);
            }
        }
        hit
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let canvas_focus = self.canvas_focus.clone();
        // Ctrl 悬停复合节点 → 换青色描边(提示"点一下能跳进定义")。
        let ctrl_held = self.ctrl_held;
        let hover_node = self.hover_node;
        // 当前图的坐标。缺(理论上只有坐标还没跟上时)按空切片处理 ——
        // `build_nodes` / `build_edges` 对每个节点都是 `get` + 默认值。
        let pos = self
            .positions
            .get(self.current)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let nodes = build_nodes(g, pos, origin, scale, &self.selected);
        let edges = build_edges(g, pos, origin, scale);
        let boundaries = build_boundaries(g, pos, origin, scale);
        let pins = build_pins(g, pos, origin, scale);
        let defaults = build_defaults(g, pos, origin, scale);
        let ctrl_labels = build_ctrl_labels(g, pos, origin, scale);

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
            // 注释：贴在节点上方的小文本框,多行。布局已经为它留过位置
            // (`node::layout` 的 `ch`),这里按同一个估算高度往上顶。
            if let Some((text, _cw, ch)) = &nv.comment {
                let font = (COMMENT_FONT_PX * nv.scale).max(5.0);
                // 不给宽高:让它按内容自然撑开 —— 和 web 里一样由文本决定
                // 尺寸。`ch` 只用来定位(布局为注释预留的高度,core 估算),
                // 不用来限制渲染 —— 估窄了会把字折行、裁掉,"sum" 只剩
                // "su" 就是这么来的。
                let mut lines = div()
                    .absolute()
                    .left(px(nv.x))
                    .top(px(nv.y - (COMMENT_GAP + *ch) * nv.scale))
                    .flex()
                    .flex_col()
                    .px(px(COMMENT_PAD_X / 2.0 * nv.scale))
                    .py(px(COMMENT_PAD_Y / 2.0 * nv.scale))
                    .rounded(px(4.0 * nv.scale))
                    .bg(comment_bg())
                    .border_1()
                    .border_color(comment_border())
                    .text_color(text_dim())
                    .text_size(px(font));
                // 不裁剪 —— 和值默认值的做法一致:宽度是按字宽估的,估窄
                // 一点点也不该把字截掉半截(宁可让它伸出框外)。
                for line in text.split('\n') {
                    // 每一行固定高度 —— 空行也要占位,不然多行注释会塌成一行。
                    //
                    // `whitespace_nowrap` 是必须的:默认会按容器宽折行,而
                    // 宽度是按字宽**估**的 —— 差一两个像素时最后一个字母就被
                    // 折到第二行,第二行又落在按估算行数给的框高外面(实测
                    // "sum" 只剩 "su")。换行由注释里的 `\n` 决定,不该再折。
                    lines = lines.child(
                        div()
                            .h(px(COMMENT_LINE_H * nv.scale))
                            .whitespace_nowrap()
                            .child(line.to_string()),
                    );
                }
                layer = layer.child(lines);
            }
            // 盒子走**唯一的**渲染实现(`node_box_el`);选中 / 图边界只把
            // 描边加粗一档。引脚由下面那段统一画(在盒子之后,才不会被盖住)。
            let border = if nv.selected {
                selected_border()
            } else if nv.is_fn && ctrl_held && hover_node == Some(nv.index) {
                // 按住 Ctrl 悬停复合节点:提示"点一下能跳进定义"。
                link_hint_border()
            } else {
                nv.border
            };
            let d = if nv.boundary || nv.selected {
                node_box_el(nv.x, nv.y, nv.w, nv.h, nv.scale, &nv.label, border).border_2()
            } else {
                node_box_el(nv.x, nv.y, nv.w, nv.h, nv.scale, &nv.label, border)
            };
            layer = layer.child(
                // 节点拖拽用**左键**;画布平移才用中键。
                d.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |v, e: &MouseDownEvent, window, cx| {
                        // 左键按在节点上顺手关掉已经弹出的菜单。
                        v.menu = None;
                        // 焦点从左侧列表挪到画布 —— 之后打字不该再进搜索框。
                        window.focus(&v.canvas_focus);
                        // Ctrl+左键:跳到复合节点的定义。
                        if e.modifiers.control && nv.is_fn {
                            v.jump_to_definition(nv.index, cx);
                            cx.stop_propagation();
                            return;
                        }
                        // Shift+左键:加选 / 取消选这一个节点(不动选区其余部分)。
                        if e.modifiers.shift {
                            if let Some(p) = v.selected.iter().position(|x| *x == nv.index) {
                                v.selected.remove(p);
                            } else {
                                v.selected.push(nv.index);
                            }
                            v.selected.sort_unstable();
                            cx.notify();
                            cx.stop_propagation();
                            return;
                        }
                        v.begin_node_drag(nv.index, from_px(e.position));
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
                ),
            );
        }

        // ---- 引脚:每个连接点都画,有没有连线都画 ----
        //
        // 画在节点之后:引脚压在节点边缘上,一半在内一半在外。走**唯一的**
        // 渲染实现(`pin_els`),位置来自 `build_pins`(即 core 的 `pin_row_y`)。
        let flat: Vec<(f32, f32, bool)> = pins.iter().map(|p| (p.x, p.y, p.ctrl)).collect();
        for el in pin_els(&flat, scale) {
            layer = layer.child(el);
        }

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

        // ---- 框选矩形:半透明蓝框,画在节点之上 ----
        // 坐标是**容器局部**(和节点框同一套),直接用 left/top。
        if let Some((a, b)) = self.marquee {
            let (x0, x1) = if a.x <= b.x { (a.x, b.x) } else { (b.x, a.x) };
            let (y0, y1) = if a.y <= b.y { (a.y, b.y) } else { (b.y, a.y) };
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(x0))
                    .top(px(y0))
                    .w(px(x1 - x0))
                    .h(px(y1 - y0))
                    .border_1()
                    .border_color(marquee_border())
                    .bg(marquee_bg()),
            );
        }

        // ---- 控制出 pin 的分支名(Switch):节点盒内、紧贴引脚,文字右对齐 ----
        // 画在节点 div **之后**:名字要压在自己的盒子上,不是浮在连线中间。
        for cv in &ctrl_labels {
            let (w, h) = (DEF_W * scale, (BASE_FONT_PX + 4.0) * scale);
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(cv.right - w))
                    .top(px(cv.y - h / 2.0))
                    .w(px(w))
                    .h(px(h))
                    .flex()
                    .items_center()
                    .justify_end()
                    .text_size(px((BASE_FONT_PX * scale).max(6.0)))
                    .text_color(text_dim())
                    .child(cv.text.clone()),
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
                    )
                    .child(
                        // 跳到定义:切到被调用函数的那张图。
                        div()
                            .id("menu-goto")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .overflow_hidden()
                            .text_xs()
                            .child("跳转到定义")
                            .on_click(cx.listener(|v, _e, _w, cx| {
                                v.goto_menu_node(cx);
                            })),
                    ),
            );
        }

        // ---- 容器:三个交互里"移动"和"缩放"挂在这里,"按下"也挂在这里 ----
        let canvas_el = div()
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            // 画布自己持有焦点(见 `Viewer::canvas_focus` 的注释):
            // 这样"根 → 画布"总在键盘 / modifier 事件的分发路径上。
            .track_focus(&canvas_focus)
            .bg(canvas_bg())
            // 中键拖 = 平移画布,和 AutoCAD / Blender 的约定一致,用 move 光标。
            .cursor_move()
            // 拖空白处平移。按在节点上时节点的 handler 已经 stop_propagation,
            // 所以冒到这里的事件一定是落在空白处的。
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|v, e: &MouseDownEvent, _w, _cx| {
                    v.drag = Some(Drag::Pan {
                        last: from_px(e.position),
                    });
                }),
            )
            // 左键落在空白处 = **框选**。节点上的按下已经 stop_propagation,
            // 冒到这里的一定是空白处。
            //
            // 起手点记在**容器局部**坐标里(和节点框同一个坐标系),框选
            // 判交时不用来回换算。
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|v, e: &MouseDownEvent, window, cx| {
                    // 点画布空白 = 离开左栏,和 IDEA 里点编辑器一样把焦点交给
                    // 编辑器(这里就是画布),打字不再进搜索框。
                    window.focus(&v.canvas_focus);
                    if v.menu.take().is_some() {
                        cx.notify();
                    }
                    let o = v.canvas_origin.get();
                    let at = from_px(e.position);
                    let local = point(at.x - o.x, at.y - o.y);
                    v.drag = Some(Drag::Marquee {
                        start: local,
                        cur: local,
                    });
                    v.marquee = Some((local, local));
                    cx.notify();
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
                v.update_hover(from_px(e.position), e.modifiers.control, cx);
                // `take` 出来再放回去:下面有一支要把 `drag` 直接清成 None,
                // 拿着 `&mut Drag` 就没法同时改 `v.drag`。
                let Some(mut d) = v.drag.take() else {
                    return;
                };
                // 左键=拖节点/框选,中键=平移,两种都在推进,别把一种挡掉。
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
                match &mut d {
                    Drag::Nodes {
                        indices,
                        last,
                        moved,
                        ..
                    } => {
                        let delta = point(now.x - last.x, now.y - last.y);
                        *last = now;
                        if delta.x != 0.0 || delta.y != 0.0 {
                            *moved = true;
                        }
                        // 位移要除以 `scale`,否则缩放越大节点跑得越快。
                        // `scale` 被夹在 [MIN_SCALE, MAX_SCALE],恒 > 0。
                        // 坐标按图存:改的是当前图里那几个节点。
                        if let Some(ps) = v.positions.get_mut(v.current) {
                            for i in indices.iter() {
                                if let Some(p) = ps.get_mut(*i) {
                                    p.x += delta.x / v.scale;
                                    p.y += delta.y / v.scale;
                                }
                            }
                        }
                    }
                    Drag::Pan { last } => {
                        let delta = point(now.x - last.x, now.y - last.y);
                        *last = now;
                        // 平移量与坐标系原点无关,所以直接用窗口坐标的增量。
                        v.origin.x += delta.x;
                        v.origin.y += delta.y;
                        // 回写进当前页签:切走再切回来要停在这里。
                        v.sync_view();
                    }
                    Drag::Marquee { start, cur } => {
                        // 框选的矩形用**容器局部**坐标 —— 和节点框同一坐标系。
                        let o = v.canvas_origin.get();
                        *cur = point(now.x - o.x, now.y - o.y);
                        v.marquee = Some((*start, *cur));
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
            // 左键松开:结束框选(把框里的节点选上),或者结束"拖了没动"的
            // 单击(把选区收成这一个节点)。
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|v, _e: &MouseUpEvent, _w, cx| {
                    let Some(d) = v.drag.take() else {
                        return;
                    };
                    match d {
                        Drag::Marquee { start, cur } => {
                            v.marquee_select(start, cur);
                            v.marquee = None;
                            cx.notify();
                        }
                        Drag::Nodes { primary, moved, .. } => {
                            // 按下没动 = 单击,选区收成这一个节点。
                            // (拖动过就不动选区 —— 那是"整组拖动"。)
                            if !moved {
                                v.select_only(primary);
                                cx.notify();
                            }
                        }
                        Drag::Pan { .. } => {}
                    }
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
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|v, _e: &MouseUpEvent, _w, cx| {
                    let Some(d) = v.drag.take() else {
                        return;
                    };
                    if let Drag::Marquee { start, cur } = d {
                        v.marquee_select(start, cur);
                        v.marquee = None;
                        cx.notify();
                    }
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
                // 回写进当前页签:切走再切回来要停在同样的缩放。
                v.sync_view();
                cx.notify();
            }))
            .child(edges_el)
            .child(layer);

        // ---- 左栏:图列表 ----
        // `overflow_y_scroll` 和 `on_click` 都挂在 `StatefulInteractiveElement` 上
        // 而不是 `Styled` / `InteractiveElement` 上,所以每一层都得先 `.id(..)`
        // 变成 stateful 元素。**`.id()` 必须在调用它们之前**,链式调用是从左往右
        // 推导接收者的,写反了就还是普通 `Div`。
        let toc_width = self.toc_width;
        // 左栏(而不是搜索框本身)拿焦点 —— 和 IDEA 的项目栏一样:
        // 焦点在左栏里时,键盘输入直接进搜索框。
        let toc_focused = self.search_focus.is_focused(window);
        let search_hint = self.search.is_empty();
        let search_text = if search_hint {
            "搜索节点名…".to_string()
        } else {
            self.search.clone()
        };
        let query = self.search.clone();
        let toc = div()
            .id("toc")
            // 焦点在**整个左栏**上,不在搜索框上:点在列表项、空白处都能直接打字,
            // 和 IDEA 的项目栏一样。键盘事件由此处的 `on_key_down` 收。
            .track_focus(&self.search_focus)
            .w(px(toc_width))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(panel_bg())
            .border_r_1()
            .border_color(hsla(0.0, 0.0, 1.0, 0.12))
            .p_2()
            .text_color(text_hi())
            .on_mouse_down(MouseButton::Left, cx.listener(|v, _e, window, cx| {
                window.focus(&v.search_focus);
                cx.notify();
            }))
            .on_key_down(cx.listener(|v, e: &gpui::KeyDownEvent, _w, cx| {
                let ks = &e.keystroke;
                // Ctrl+V 粘贴:搜索词只取一行,把粘贴进来的多行裁剪/压平。
                if ks.modifiers.control && ks.key == "v" {
                    if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
                        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !flat.is_empty() {
                            v.search.push_str(&flat);
                            cx.notify();
                        }
                    }
                    return;
                }
                // 其余 Ctrl/Alt/Win 组合留给全局快捷键(Ctrl+Z 等)。
                if ks.modifiers.control || ks.modifiers.alt || ks.modifiers.platform {
                    return;
                }
                match ks.key.as_str() {
                    "backspace" => {
                        v.search.pop();
                    }
                    "escape" => v.search.clear(),
                    _ => match &ks.key_char {
                        Some(ch) => v.search.push_str(ch),
                        None => return,
                    },
                }
                cx.notify();
            }))
            // 搜索框:自绘的最小输入框。gpui 0.2.2 没有现成的 TextInput,
            // 焦点 + `on_key_down` 就能收字符(`KeyDownEvent::keystroke.key_char`)。
            // 左栏有焦点时高亮,提示"现在打字的会进这里"。
            .child(
                div()
                    .id("search")
                    .mb_2()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .cursor_text()
                    .overflow_hidden()
                    .text_xs()
                    .border_1()
                    .border_color(if toc_focused {
                        hsla(0.0, 0.0, 1.0, 0.35)
                    } else {
                        hsla(0.0, 0.0, 1.0, 0.0)
                    })
                    .bg(hsla(0.0, 0.0, 1.0, if toc_focused { 0.14 } else { 0.08 }))
                    .when(search_hint, |d| d.text_color(text_dim()))
                    .child(search_text),
            )
            .child(
                div()
                    .id("toc-list")
                    .flex_1()
                    // flex 子项默认 `min-height: auto`,不加这条列表会把左栏撑高,
                    // 滚动条永远不出现。
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .children(
                        self.doc
                            .graphs
                            .iter()
                            .enumerate()
                            .filter(|(_, entry)| Self::matches_search(&entry.name, &query))
                            .map(|(i, entry)| {
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
                                    // 点左栏 = **在标签页里打开这张图**(不是偷偷换掉
                                    // 当前页签的内容)。这张图已经开在某个页签里就切
                                    // 过去,否则新开一页 —— 和编辑器 / 浏览器一致,
                                    // 也不会因为反复点同一项堆出一排重复页签。
                                    .on_click(cx.listener(move |v, _e, _w, cx| {
                                        v.open_graph_in_tab(i, cx)
                                    }))
                            }),
                    ),
            );

        // 分隔条:拖它改左栏宽度。宽度推进挂在**根节点**的 `on_mouse_move` 上,
        // 因为指针一旦离开这 5px,分隔条自己的移动事件就没有 hitbox 了。
        let divider = div()
            .id("toc-divider")
            .w(px(TOC_DIVIDER_WIDTH))
            .h_full()
            .flex_shrink_0()
            .bg(divider_bg())
            .hover(|s| s.bg(divider_hover_bg()))
            .cursor(gpui::CursorStyle::ResizeColumn)
            .on_mouse_down(MouseButton::Left, cx.listener(|v, e: &MouseDownEvent, _w, cx| {
                v.resizing_toc = true;
                v.toc_drag_x = from_px(e.position).x;
                // 别让这次按下冒到画布那边当成"点空白处"。
                cx.stop_propagation();
                cx.notify();
            }));

        // ---- 右栏:优化 ----
        // 按钮 = `eliminate_solo` 调用链上的每条规则,点一次 = 该规则在当前图的
        // 所有节点上各跑一次(**一遍**,不循环到不动点)。
        // 标签省掉 `eliminate_` 前缀:栏名已经说了这是优化,240px 宽放不下全名。
        //
        // 改写类按钮(链接全部 / 一键优化 / 循环改写 / lower)在**图已经 lower
        // 过**时全部禁用:判据是显式记下的 `lowered` 集合(不是从图上推,理由见
        // 那个字段的注释)。撤销回退到"没 lower 过"的那一帧后自动恢复。
        let lowered = self.is_lowered();
        let editable = !lowered;
        let rules_body = div()
            .id("rules")
            .w(px(RULES_WIDTH))
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .p_2()
            .text_color(text_hi())
            .child(
                // 自动整理不受 lower 影响:lower 之后的图照样需要排布。
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
                // 链接全部:把所有 Fn 节点展开成内联的复合节点
                // (右键「link」的批量版)。和「一键优化」分开 —— 展开会把
                // 整张图换掉,混在规则消除里看不出是哪一步改坏的。
                panel_button("link_all", "链接全部", editable)
                    .when(editable, |d| {
                        d.on_click(cx.listener(|v, _e, _w, cx| v.link_all(cx)))
                    }),
            )
            .child(
                // 一键优化:消除规则跑到不动点(不含展开 Fn)。
                panel_button("optimize_all", "一键优化", editable)
                    .when(editable, |d| {
                        d.on_click(cx.listener(|v, _e, _w, cx| v.optimize_all(cx)))
                    }),
            )
            .child(
                // 循环改写:控制流里的环 → native 的「有限循环」节点。
                panel_button("lower_loops", "循环改写", editable)
                    .when(editable, |d| {
                        d.on_click(cx.listener(|v, _e, _w, cx| v.lower_loops(cx)))
                    }),
            )
            .child(
                // lower:core 的**完整**降级流程(展开 Fn → 优化 → 改写环 →
                // 展开列表 → 类型降到 wire 层)。跑完之后上面那三个按钮就没有
                // 对象了,一起变灰 —— 见 `Viewer::is_lowered`。
                panel_button("lower", "lower", editable)
                    .mb_3()
                    .when(editable, |d| {
                        d.on_click(cx.listener(|v, _e, _w, cx| v.lower(cx)))
                    }),
            )
            .when(lowered, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(text_dim())
                        .mb_2()
                        .child("已 lower —— 改写按钮已停用(Ctrl+Z 可退回)"),
                )
            })
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
            // 撤销 / 重做**没有按钮**:只留 Ctrl+Z / Ctrl+Y,见 `main.rs` 的绑定。
            .child(
                div()
                    .mt_2()
                    .text_xs()
                    .text_color(text_dim())
                    .child("Ctrl+Z 撤销 · Ctrl+Y 重做"),
            )
            .when_some(self.status.clone(), |d, s| {
                d.child(div().mt_2().text_xs().text_color(text_dim()).child(s))
            });

        // ---- 「调用约定」面板:只对**复合节点图**显示(主图没有对外接口)----
        //
        // 视图的形状是「内外两层」:
        // - **外层** = 加载时冻结的那个调用节点(调用方看到的样子),永远不变;
        // - **内层** = 当前状态,画成**一个普通节点**,可以在外层里拖动。
        //
        // 连线按用户定的三条规则走(逻辑在 `conv.rs`,那里能直接测)。
        let conv_panel = self.render_conv_panel(cx);

        // 右栏 = 「调用约定」+ 优化按钮,上下排在一栏里。
        //
        // 放一栏而不是两栏:窗口本来就窄(左栏 240 + 右栏 240),再开第三栏
        // 画布就没剩多少了。调用约定是**当前图的补充说明**,和优化按钮是
        // 同一层的东西。
        let rules_panel = div()
            .id("right")
            .w(px(RULES_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(panel_bg())
            .border_l_1()
            .border_color(hsla(0.0, 0.0, 1.0, 0.12))
            .child(conv_panel)
            .child(rules_body);

        // ---- 标签栏:一行横向的页签,每页带关闭按钮 ----
        // 点页签切过去(记一条后退历史,用户要求切标签页可后退);
        // 中键点页签 = 关闭;页签上的 × 也是关闭。
        let tabs: Vec<(usize, String, bool)> = self
            .tabs
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| (i, self.tab_title(t), i == self.tabs.active))
            .collect();
        let tab_bar = div()
            .id("tab-bar")
            .flex()
            .flex_row()
            // 页签多了要能横向滚,而不是把画布挤窄。
            .overflow_x_scroll()
            .flex_shrink_0()
            .bg(panel_bg())
            .border_b_1()
            .border_color(hsla(0.0, 0.0, 1.0, 0.12))
            .children(tabs.into_iter().map(|(i, title, active)| {
                div()
                    .id(("tab", i))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py_1()
                    .flex_shrink_0()
                    .border_r_1()
                    .border_color(hsla(0.0, 0.0, 1.0, 0.10))
                    .text_xs()
                    .cursor_pointer()
                    .when(active, |d| d.bg(hsla(0.0, 0.0, 1.0, 0.18)))
                    .when(!active, |d| d.text_color(text_dim()))
                    .child(title)
                    // 关闭按钮。**只在 `on_click` 里 stop_propagation** ——
                    // 不能另挂 `on_mouse_down` 去停冒泡:gpui 的 `on_click` 靠
                    // mouse-down 的 bubble 监听器写 `pending_mouse_down`
                    // (`div.rs:2138-2144`),提前停掉冒泡会让自己的 click 也失效,
                    // 表现就是"点 × 没反应"。
                    .child(
                        div()
                            .id(("tab-close", i))
                            .px_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|s| s.bg(hsla(0.0, 0.0, 1.0, 0.16)))
                            .text_color(text_dim())
                            .child("×")
                            .on_click(cx.listener(move |v, _e, _w, cx| {
                                cx.stop_propagation();
                                v.close_tab(i, cx);
                            })),
                    )
                    .on_click(cx.listener(move |v, _e, _w, cx| v.switch_tab(i, cx)))
                    // 中键关闭。中键没有 `on_click` 可依赖(那个只认左键,
                    // 见 `div.rs:2139`),所以在 down 上直接关。
                    //
                    // **不调 `stop_propagation`**:这个 handler 挂在页签自己的
                    // hitbox 上,父容器的中键监听器本来就不处理这件事;停冒泡
                    // 反而会让画布的"中键平移"在页签区域彻底失灵。
                    .on_mouse_down(MouseButton::Middle, cx.listener(move |v, _e, _w, cx| {
                        v.close_tab(i, cx);
                    }))
            }));

        // ---- 整窗:接收文件拖放,左中右分栏 ----
        div()
            .flex()
            .flex_col()
            .size_full()
            .on_drop::<ExternalPaths>(cx.listener(|v, paths: &ExternalPaths, _w, cx| {
                v.handle_drop(paths, cx);
            }))
            // Ctrl 的按下/松开**不动鼠标**也要跟上(悬停复合节点要换成青色)。
            //
            // 挂在根 `div` 上而不是画布上:事件只沿"根 → 已聚焦元素"分发,
            // 焦点在左栏搜索框时画布不在路径上,根 `div` 则在(它是左右两栏的
            // 共同祖先)。画布始终持有焦点是为了"焦点为 None 时路径上只有根
            // View 那一个节点"这个坑,见 `Viewer::canvas_focus`。
            .on_modifiers_changed(cx.listener(|v, e: &ModifiersChangedEvent, _w, cx| {
                if v.set_ctrl_held(e.modifiers.control) {
                    cx.notify();
                }
            }))
            // 分隔条拖动:根节点覆盖整窗,指针走到哪都能收到移动事件。
            .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _w, cx| {
                // 「调用约定」面板里拖内层节点。**先**处理它:拖节点时指针
                // 会离开面板,面板自己的 move 监听器就收不到了,所以挂在
                // 覆盖整窗的根节点上(同分隔条拖动一个道理)。
                if v.conv_drag.is_some() {
                    v.drag_conv_inner(from_px(e.position), cx);
                    return;
                }
                // 分隔条拖动:根节点覆盖整窗,指针走到哪都能收到移动事件。
                if !v.resizing_toc {
                    return;
                }
                // 和画布拖拽一样的兜底:指针移出窗口再回来时按键已经没了。
                if !matches!(e.pressed_button, Some(MouseButton::Left)) {
                    v.resizing_toc = false;
                    cx.notify();
                    return;
                }
                let x = from_px(e.position).x;
                v.toc_width =
                    (v.toc_width + (x - v.toc_drag_x)).clamp(TOC_MIN_WIDTH, TOC_MAX_WIDTH);
                v.toc_drag_x = x;
                cx.notify();
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|v, _e: &MouseUpEvent, _w, cx| {
                if v.resizing_toc {
                    v.resizing_toc = false;
                    cx.notify();
                }
                // 「调用约定」面板里拖内层节点:松手结束。
                if v.conv_drag.take().is_some() {
                    cx.notify();
                }
            }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|v, _e: &MouseUpEvent, _w, cx| {
                if v.resizing_toc {
                    v.resizing_toc = false;
                    cx.notify();
                }
                // 指针在窗口外松开:同样要结束拖动。
                if v.conv_drag.take().is_some() {
                    cx.notify();
                }
            }))
            // 后退 / 前进:**两条语义消息**,不是"侧键"。
            //
            // gpui 把它们建模成 `MouseButton::Navigate(Back|Forward)`;Windows
            // 后端由 `WM_XBUTTONDOWN` 的 XBUTTON1/2 映射过来
            // (`gpui/src/platform/windows/events.rs:513-514`)。
            //
            // ⚠️ gpui 的窗口过程**完全不处理** `WM_APPCOMMAND`(全 crate 搜不到
            // 一处),而 XBUTTON 分支返回非 `None`,所以 `DefWindowProcW` 连那次
            // "侧键 → 浏览器后退"的系统翻译都不会发生。在这个宿主里,
            // `Navigate` 就是唯一能拿到这两条消息的途径。
            //
            // 挂在根 `div` 上:整个窗口范围内都生效,和撤销一样不依赖焦点。
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|v, _e: &MouseDownEvent, _w, cx| {
                    cx.stop_propagation();
                    v.nav_back(cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|v, _e: &MouseDownEvent, _w, cx| {
                    cx.stop_propagation();
                    v.nav_forward(cx);
                }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(toc)
                    .child(divider)
                    // 标签栏**只盖在画布上方**,不横跨左栏 / 右栏 ——
                    // 页签说的是"这个画布在看哪张图",和左栏的图列表、右栏的
                    // 优化面板都不是一个层级的东西。
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(tab_bar)
                            .child(canvas_el),
                    )
                    .child(rules_panel),
            )
            .into_any_element()
    }
}

impl Viewer {
    /// 开始拖**一组**节点。
    ///
    /// `indices` 是这次要一起移动的节点(展平下标)。按下时已经把选区
    /// 算好了 —— 这里只记下"要动哪些"和起点。
    fn begin_drag(&mut self, indices: Vec<usize>, primary: usize, last: Point<f32>) {
        self.drag = Some(Drag::Nodes {
            indices,
            primary,
            moved: false,
            last,
        });
    }

    /// 按下节点:决定这次拖哪些节点。
    ///
    /// - 该节点**已经在选区里** → 拖整个选区(多选的"整体拖动")。
    /// - 不在选区里 → 只选它自己,拖它自己(并且把它设成新的选区)。
    fn begin_node_drag(&mut self, index: usize, last: Point<f32>) {
        let indices = if self.selected.contains(&index) {
            self.selected.clone()
        } else {
            self.selected = vec![index];
            vec![index]
        };
        self.begin_drag(indices, index, last);
    }

    /// 单选:只留这一个节点。
    fn select_only(&mut self, index: usize) {
        self.selected = vec![index];
    }

    /// 框选:把和矩形相交的节点全选上。
    ///
    /// `a` / `b` 是**容器局部**坐标(和节点框同一个坐标系)。
    /// 判定用"相交"而不是"完全包住"——和文件管理器、浏览器一致:
    /// 擦过节点的一角就选中它,框小了也不容易一个都选不到。
    fn marquee_select(&mut self, a: Point<f32>, b: Point<f32>) {
        let (x0, x1) = if a.x <= b.x { (a.x, b.x) } else { (b.x, a.x) };
        let (y0, y1) = if a.y <= b.y { (a.y, b.y) } else { (b.y, a.y) };
        let Some(g) = self.doc.graphs.get(self.current) else {
            return;
        };
        let pos = self
            .positions
            .get(self.current)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut hit = Vec::new();
        for i in 0..g.nodes.len() {
            let Some((p, s)) = node_box(pos, &g.nodes, self.origin, self.scale, i) else {
                continue;
            };
            let nx0 = p.x;
            let ny0 = p.y;
            let nx1 = p.x + s.width;
            let ny1 = p.y + s.height;
            if nx0 <= x1 && nx1 >= x0 && ny0 <= y1 && ny1 >= y0 {
                hit.push(i);
            }
        }
        self.selected = hit;
    }

    /// 当前图里某个复合节点对应的**图下标**(跳转到定义用)。
    ///
    /// `IrNodeId::Fn` 里带的是链接键(和 `functions` 的键一致),
    /// 所以能直接对上。
    fn fn_target_graph(&self, node_index: usize) -> Option<usize> {
        let g = self.doc.graphs.get(self.current)?;
        let n = g.nodes.get(node_index)?;
        let IrNodeId::Fn(key) = &n.kind else {
            return None;
        };
        self.doc.graph_of_fn(key)
    }

    /// 跳转到复合节点的定义(那张函数图)。
    ///
    /// 走和左栏点图一样的规则:那张图已经开着就切过去,否则新开一页 ——
    /// 不覆盖当前页签。
    fn jump_to_definition(&mut self, node_index: usize, cx: &mut Context<Self>) {
        let Some(target) = self.fn_target_graph(node_index) else {
            self.status = Some("这个节点没有可跳转的定义".to_string());
            cx.notify();
            return;
        };
        if target == self.current {
            return;
        }
        self.open_graph_in_tab(target, cx);
    }

    /// 右键菜单里点了「跳转到定义」:跳到菜单记下的那个节点的定义。
    pub fn goto_menu_node(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else {
            return;
        };
        self.jump_to_definition(menu.node_index, cx);
    }

    /// 「调用约定」视图的悬停命中更新。
    ///
    /// `at_window` 是**窗口**坐标 —— 要减去面板左上角换成面板局部坐标。
    /// 面板的原点由面板自己的 canvas prepaint 每帧写进
    /// [`Viewer::conv_origin`]。
    fn update_conv_hover(&mut self, at_window: Point<f32>, cx: &mut Context<Self>) {
        let Some((outer, inner)) = self.conv_data() else {
            return;
        };
        let conv = conv::Convention::build(outer, inner);
        let width = RULES_WIDTH - 2.0 * CONV_PAD;
        let probe = conv::layout(outer, inner, &conv, (0.0, 0.0), width);
        let inner_pos = self.conv_inner_pos(&probe);
        let l = conv::layout(outer, inner, &conv, inner_pos, width);

        // 所有可命中的引脚。
        let mut pins = Vec::new();
        for (side, n) in [
            (conv::Side::OuterLeft, outer.params.len()),
            (conv::Side::OuterRight, outer.rets.len()),
            (conv::Side::InnerLeft, inner.params.len()),
            (conv::Side::InnerRight, inner.rets.len()),
        ] {
            for i in 0..n {
                pins.push(conv::Hit::Pin { side, index: i });
            }
        }

        let o = self.conv_origin.get();
        let at = point(at_window.x - o.x, at_window.y - o.y);
        let hit = conv::hit_test(&l, &pins, &conv.links, at);
        if self.conv_hover != hit {
            self.conv_hover = hit;
            cx.notify();
        }
    }

    /// 悬停气泡的文案。
    ///
    /// 引脚 → `名字 <类型>`;连线 → 两端各是什么。
    fn conv_hover_text(
        &self,
        hit: &conv::Hit,
        outer: &FnInterface,
        inner: &FnInterface,
    ) -> Option<String> {
        match hit {
            conv::Hit::Pin { side, index } => {
                let port = match side {
                    conv::Side::OuterLeft => outer.params.get(*index),
                    conv::Side::OuterRight => outer.rets.get(*index),
                    conv::Side::InnerLeft => inner.params.get(*index),
                    conv::Side::InnerRight => inner.rets.get(*index),
                }?;
                Some(conv::pin_tip(port))
            }
            conv::Hit::Link { index } => {
                let links = self.conv_links();
                let link = links.get(*index)?;
                let (from, to) = link_endpoints_text(outer, inner, link);
                Some(conv::link_tip(&from, &to))
            }
        }
    }

    /// 当前图的连线表(供悬停文案查两端)。
    fn conv_links(&self) -> Vec<conv::Link> {
        let Some((outer, inner)) = self.conv_data() else {
            return Vec::new();
        };
        conv::Convention::build(outer, inner).links
    }

    /// 拖「调用约定」视图里的内层节点。
    ///
    /// `at_window` 是事件的**窗口**坐标(`Point<f32>`)。
    ///
    /// ⚠️ **位置是相对「按下时」算的,不是累加每次的移动量。**
    ///
    /// 累加增量有个致命的坑:指针移出窗口再回来时,win32 的 wparam 不带按键
    /// 信息(`mouse_up` 收不到),而补上的那次 `mouse_move` 带着**整段**位移 ——
    /// 累加实现会把这一整段一次性加到位置上,节点"隔着大老远"被甩出去。
    /// 相对按下点算就没有这个问题:位置 = 按下时的位置 + (现在 - 按下时),
    /// 中间丢多少事件都不影响。
    fn drag_conv_inner(&mut self, at_window: Point<f32>, cx: &mut Context<Self>) {
        let Some(drag) = self.conv_drag else {
            return;
        };
        // 先算布局 —— 夹边界要用外层盒子的大小。
        let Some((outer, inner)) = self.conv_data() else {
            return;
        };
        let conv = conv::Convention::build(outer, inner);
        let width = RULES_WIDTH - 2.0 * CONV_PAD;
        let probe = conv::layout(outer, inner, &conv, (0.0, 0.0), width);
        // 相对**按下点**的位移,不是累加。
        let next = conv::drag_pos(drag.start, drag.anchor, at_window);
        let clamped = conv::clamp_inner(next, &probe);
        if clamped != drag.start {
            self.conv_pos.insert(drag.graph, clamped);
            cx.notify();
        }
    }
}

/// 「调用约定」视图的渲染。
///
/// 这个视图和主画布**完全独立**:它有自己的坐标系(右栏面板的局部坐标)、
/// 自己的缩放(固定 1 倍)、自己的命中测试。所以它不共用 `Drag` 那一套 ——
/// 那样会把"拖节点"和"拖内层节点"混在一起。
impl Viewer {
    /// 当前图的「调用约定」数据:外层(冻结) + 内层(当前)。
    ///
    /// 主图返回 `None` —— 主图没有对外接口,这个视图不显示。
    fn conv_data(&self) -> Option<(&FnInterface, &FnInterface)> {
        if !self.doc.is_fn_graph(self.current) {
            return None;
        }
        let outer = self.doc.frozen_interface(self.current)?;
        let inner = &self.doc.graphs.get(self.current)?.interface;
        Some((outer, inner))
    }

    /// 内层节点在面板里的当前位置(没拖过就在外层盒子中间偏左)。
    fn conv_inner_pos(&self, layout: &conv::Layout) -> (f32, f32) {
        let stored = self.conv_pos.get(&self.current).copied();
        let default = (
            layout.outer.x + 16.0,
            layout.outer.y + (layout.outer.h - layout.inner.h) / 2.0,
        );
        conv::clamp_inner(stored.unwrap_or(default), layout)
    }

    /// 渲染「调用约定」面板。
    ///
    /// 返回一个 `Stateful<Div>`(右栏里的一段)。主图时返回一个空的占位
    /// (什么都不画)。
    fn render_conv_panel(&mut self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let Some((outer, inner)) = self.conv_data() else {
            return div().id("conv");
        };
        let conv = conv::Convention::build(outer, inner);
        let width = RULES_WIDTH - 2.0 * CONV_PAD;
        // 内层位置要**先**布局一次(拿到外层盒子的大小),再夹一次 —— 拖动时
        // 就是靠 `conv_inner_pos` 把位置夹在外层里面。
        let probe = conv::layout(outer, inner, &conv, (0.0, 0.0), width);
        let inner_pos = self.conv_inner_pos(&probe);
        let l = conv::layout(outer, inner, &conv, inner_pos, width);

        // ---- 连线:canvas 画 ----
        // 线全是**面板局部**坐标;canvas 的 bounds 是窗口坐标,加个偏移。
        let conv_origin = self.conv_origin.clone();
        let outer_left = l.outer_left.clone();
        let outer_right = l.outer_right.clone();
        let inner_left = l.inner_left.clone();
        let inner_right = l.inner_right.clone();
        let links = conv.links.clone();
        // canvas 的闭包是 `move` 的 —— 给它**另一份**引脚位置,
        // 免得把渲染层还要用的那份搬走。
        let edge_pins = (
            l.outer_left.clone(),
            l.outer_right.clone(),
            l.inner_left.clone(),
            l.inner_right.clone(),
        );
        let edges_el = canvas(
            move |bounds: Bounds<Pixels>, _window, _cx| {
                // 面板容器和它的 `absolute().inset_0()` 子元素同源同大小,
                // 所以 canvas 的 bounds.origin 就是面板在窗口坐标系里的左上角。
                // 悬停命中要靠它把窗口坐标换算成面板局部坐标。
                conv_origin.set(from_px(bounds.origin));
                ()
            },
            move |bounds, _, window, _cx| {
                let (outer_left, outer_right, inner_left, inner_right) = &edge_pins;
                let o = bounds.origin;
                let at = |p: Point<f32>| add_px(o, p);
                // 取两个引脚位置的辅助:按种类取。
                let pick = |pins: &conv::Pins, ctrl: bool, i: usize| -> Option<Point<f32>> {
                    if ctrl {
                        pins.ctrl.get(i).map(|&(x, y)| point(x, y))
                    } else {
                        pins.value.get(i).map(|&(x, y)| point(x, y))
                    }
                };
                for link in &links {
                    let (from, to, ctrl) = match *link {
                        conv::Link::Param {
                            outer: o,
                            inner: i,
                        } => (
                            pick(&outer_left, false, o),
                            pick(&inner_left, false, i),
                            false,
                        ),
                        conv::Link::Ret {
                            outer: o,
                            inner: i,
                        } => (
                            pick(&inner_right, false, i),
                            pick(&outer_right, false, o),
                            false,
                        ),
                        conv::Link::PassThrough { param, ret } => (
                            pick(&outer_left, false, param),
                            pick(&outer_right, false, ret),
                            false,
                        ),
                        conv::Link::CtrlIn {
                            outer: o,
                            inner: i,
                        } => (
                            pick(&outer_left, true, o),
                            pick(&inner_left, true, i),
                            true,
                        ),
                        conv::Link::CtrlOut {
                            outer: o,
                            inner: i,
                        } => (
                            pick(&inner_right, true, i),
                            pick(&outer_right, true, o),
                            true,
                        ),
                        // 直接贯穿:从外层控制入**横穿**到外层控制出。
                        conv::Link::CtrlThrough { from, to } => (
                            pick(&outer_left, true, from),
                            pick(&outer_right, true, to),
                            true,
                        ),
                    };
                    let (Some(from), Some(to)) = (from, to) else {
                        continue;
                    };
                    let mut pb = PathBuilder::stroke(px(if ctrl { 2.0 } else { 1.0 }));
                    if !ctrl {
                        pb = pb.dash_array(&[px(4.0), px(3.0)]);
                    }
                    // 贝塞尔:水平方向拉开,和主画布一个画法。
                    let dx = ((to.x - from.x).abs() * 0.5).max(6.0);
                    pb.move_to(at(from));
                    pb.cubic_bezier_to(
                        at(to),
                        at(point(from.x + dx, from.y)),
                        at(point(to.x - dx, to.y)),
                    );
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, if ctrl { edge_ctrl() } else { edge_value() });
                    }
                }
            },
        )
        .absolute()
        .inset_0();

        // ---- 层级顺序:盒子 → 引脚 → 标签 ----
        //
        // ⚠️ **引脚必须画在盒子之后**(和主画布一样,见 `render` 里的
        // 「引脚:每个连接点都画」那段)。内层节点是不透明的
        // (`node_bg()`),引脚先画就会被它盖住 —— 表现就是"引脚被自己
        // 遮挡了"。
        //
        // 顺序照 [`conv::LAYER_ORDER`] 走(那边有测试盯着):
        // 边 → 外层盒子 → 内层节点 → 引脚 → 文字 → 悬停气泡。
        let mut layer = div().absolute().inset_0();

        // ---- 外层盒子:冻结的调用节点 ----
        let outer_box = div()
            .id("conv-outer")
            .absolute()
            .left(px(l.outer.x))
            .top(px(l.outer.y))
            .w(px(l.outer.w))
            .h(px(l.outer.h))
            .rounded_md()
            .border_2()
            .border_color(hsla(0.0, 0.0, 1.0, 0.28))
            .bg(hsla(0.0, 0.0, 1.0, 0.03));
        layer = layer.child(outer_box);

        // ---- 内层节点:当前状态,可以在外层里拖 ----
        //
        // 它就是一个**普通节点** —— 盒子和引脚都走主画布同一份实现
        // (`node_box_el` / `pin_els`),尺寸也走 core 的 `node_height`。
        // 自己另写一套的话,盒子高度和引脚位置迟早对不上(引脚会跑出框)。
        let inner_label = if inner.params.is_empty() && inner.rets.is_empty() {
            "空"
        } else {
            "当前"
        };
        // 按下时内层节点的位置 —— 闭包是 `'static'` 的,得先把值拷出来。
        let inner_start = (l.inner.x, l.inner.y);
        let inner_box = node_box_el(
            l.inner.x,
            l.inner.y,
            l.inner.w,
            l.inner.h,
            1.0,
            inner_label,
            hsla(0.0, 0.0, 1.0, 0.20).into(),
        )
        .id("conv-inner")
        .cursor_grab()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, e: &MouseDownEvent, _w, cx| {
                // 记下**按下时**的锚点:鼠标在哪、内层节点在哪。
                // 之后的位置按 `start + (现在 - anchor)` 算,不累加增量 ——
                // 指针移出窗口再回来时补上的那次 move 带着整段位移,累加
                // 会把节点甩到大老远(见 `ConvDrag` 的注释)。
                let at = from_px(e.position);
                let start = v.conv_pos.get(&v.current).copied().unwrap_or(inner_start);
                v.conv_drag = Some(ConvDrag {
                    graph: v.current,
                    anchor: at,
                    start,
                });
                cx.stop_propagation();
            }),
        );
        layer = layer.child(inner_box);

        // ---- 引脚:值流圆点 / 控制流三角形 ----
        //
        // 画在**盒子之后**(引脚压在节点边缘上,一半在内一半在外),走主画布
        // 同一份实现(`pin_els`)。位置由 `conv::pins_on_side` 按 core 的
        // `pin_row_y` 算好 —— 和外层盒子共用一条规则。
        let all_pins: Vec<(f32, f32, bool)> = [&outer_left, &outer_right, &inner_left, &inner_right]
            .into_iter()
            .flat_map(|p| {
                p.ctrl
                    .iter()
                    .map(|&(x, y)| (x, y, true))
                    .chain(p.value.iter().map(|&(x, y)| (x, y, false)))
            })
            .collect();
        for el in pin_els(&all_pins, 1.0) {
            layer = layer.child(el);
        }

        // ---- 文字:外层引脚名 + 常数 ----
        //
        // 排在引脚**之后** —— 名字和引脚挨着,名字压在上面才读得出来。
        //
        // 外层引脚名画在盒子**外侧**:左边(参数)右对齐贴盒子左边,右边
        // (返回值)左对齐贴盒子右边。**只写名字** —— 类型在悬停气泡里。
        for (right, y, name) in &l.outer_left_labels {
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(*right - conv::LABEL_GUTTER))
                    .top(px(*y - 8.0))
                    .w(px(conv::LABEL_GUTTER - 4.0))
                    .flex()
                    .justify_end()
                    .text_size(px(11.0))
                    .text_color(text_dim())
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(name.clone()),
            );
        }
        for (left, y, name) in &l.outer_right_labels {
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(*left))
                    .top(px(*y - 8.0))
                    .w(px(conv::LABEL_GUTTER - 4.0))
                    .text_size(px(11.0))
                    .text_color(text_dim())
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(name.clone()),
            );
        }
        // 常数:写在内侧(内层节点右侧)。
        for (x, y, text) in &l.constants {
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(*x))
                    .top(px(*y - 7.0))
                    .text_size(px(10.0))
                    .text_color(edge_value())
                    .whitespace_nowrap()
                    .child(text.clone()),
            );
        }

        // ---- 悬停气泡:显示**类型** ----
        //
        // 引脚旁边只写名字(用户要求),类型在气泡里。命中的是连线时也给
        // 两端的类型。
        if let Some(hit) = &self.conv_hover {
            if let Some(text) = self.conv_hover_text(hit, outer, inner) {
                // 气泡挂在命中点的附近。
                let (x, y) = match hit {
                    conv::Hit::Pin { .. } => conv::pin_pos(&l, hit).unwrap_or((l.outer.x, l.outer.y)),
                    conv::Hit::Link { index } => conv::link_endpoints(&l, &conv.links[*index])
                        .map(|(_, to)| (to.x, to.y))
                        .unwrap_or((l.outer.x, l.outer.y)),
                };
                layer = layer.child(
                    div()
                        .absolute()
                        .left(px(x + 10.0))
                        .top(px(y + 14.0))
                        .px_2()
                        .rounded_sm()
                        .bg(panel_bg())
                        .border_1()
                        .border_color(hsla(0.0, 0.0, 1.0, 0.35))
                        .text_color(text_hi())
                        .text_size(px(12.0))
                        .whitespace_nowrap()
                        .child(text),
                );
            }
        }

        // ---- 面板外壳 ----
        //
        // 主图没有这套接口 —— 那时返回**空的**占位(什么都不画)。
        // 这里不能画一个空框:用户会以为"这个函数没有参数"。
        if outer.params.is_empty()
            && outer.rets.is_empty()
            && outer.ctrl_in_num == 0
            && outer.ctrl_out_num == 0
        {
            return div().id("conv");
        }
        div()
            .id("conv")
            .w(px(RULES_WIDTH))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(hsla(0.0, 0.0, 1.0, 0.12))
            .p_2()
            .text_color(text_hi())
            .child(
                div()
                    .text_xs()
                    .text_color(text_dim())
                    .mb_1()
                    .child("调用约定 —— 外层 = 调用方看到的,内层 = 当前(悬停看类型)"),
            )
            .child(
                div()
                    .id("conv-body")
                    .relative()
                    .w(px(width))
                    // 高度跟着布局走 —— 引脚多的时候外层盒子会长高,
                    // 用固定高度会把它裁掉(真实产物里 20 个参数的函数有)。
                    .h(px(l.outer.h.max(CONV_HEIGHT)))
                    .overflow_hidden()
                    // 悬停命中:鼠标移动时重新算,和主画布同一套规矩。
                    .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _w, cx| {
                        v.update_conv_hover(from_px(e.position), cx);
                    }))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(|v, _e, _w, cx| {
                        if v.conv_hover.take().is_some() {
                            cx.notify();
                        }
                    }))
                    .child(edges_el)
                    .child(layer),
            )
    }
}

/// 一条连线两端的 `(名字, 类型)`。
///
/// 端点是哪一侧、哪个下标,取决于连线的种类:
/// - `Param`:外层参数 → 内层参数;
/// - `Ret`:内层返回值 → 外层返回值;
/// - `PassThrough`:外层参数 → 外层返回值。
fn link_endpoints_text(
    outer: &FnInterface,
    inner: &FnInterface,
    link: &conv::Link,
) -> ((String, String), (String, String)) {
    let param = |i: usize| -> (String, String) {
        outer
            .params
            .get(i)
            .map(|p| (p.name.clone(), p.type_label.clone()))
            .unwrap_or_default()
    };
    let ret = |i: usize| -> (String, String) {
        outer
            .rets
            .get(i)
            .map(|p| (p.name.clone(), p.type_label.clone()))
            .unwrap_or_default()
    };
    let inner_param = |i: usize| -> (String, String) {
        inner
            .params
            .get(i)
            .map(|p| (p.name.clone(), p.type_label.clone()))
            .unwrap_or_default()
    };
    let inner_ret = |i: usize| -> (String, String) {
        inner
            .rets
            .get(i)
            .map(|p| (p.name.clone(), p.type_label.clone()))
            .unwrap_or_default()
    };
    match *link {
        conv::Link::Param { outer: o, inner: i } => (param(o), inner_param(i)),
        conv::Link::Ret { outer: o, inner: i } => (inner_ret(i), ret(o)),
        conv::Link::PassThrough { param: p, ret: r } => (param(p), ret(r)),
        // 控制流没有类型,只写"控制流"。
        conv::Link::CtrlIn { .. } | conv::Link::CtrlOut { .. } | conv::Link::CtrlThrough { .. } => {
            (("控制流".into(), String::new()), ("控制流".into(), String::new()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use crate::doc::{NODE_H, NODE_W};

    /// 搜索:空词全命中,大小写不敏感,子串命中,没子串的过滤掉。
    #[test]
    fn search_filters_graph_names_by_substring() {
        assert!(Viewer::matches_search("test_drop_list", ""));
        assert!(Viewer::matches_search("test_drop_list", "DROP"));
        assert!(Viewer::matches_search("test_drop_list", "op_li"));
        assert!(!Viewer::matches_search("test_drop_list", "point"));
    }

    // ---------------------------------------------------- 标签页 / 前进后退
    //
    // 这些规则全在 [`TabHost`] 里,它不碰 GPUI 也不碰 `Doc` —— 所以能直接测。

    fn nav(g: usize, x: f32, s: f32) -> NavEntry {
        NavEntry {
            graph: g,
            origin: point(x, 0.0),
            scale: s,
        }
    }

    /// 带 y 的版本 —— 验"视野整份恢复"时要确认 y 也跟着回来。
    fn nav2(g: usize, x: f32, y: f32, s: f32) -> NavEntry {
        NavEntry {
            graph: g,
            origin: point(x, y),
            scale: s,
        }
    }

    /// 标签页:新开一个 → 数量 +1 且切过去;关掉 → 回到原来的。
    #[test]
    fn opening_and_closing_tabs() {
        let mut h = TabHost::new();
        assert_eq!(h.tabs.len(), 1);
        assert_eq!(h.active, 0);

        let t = h.open(1);
        assert_eq!(t.graph, 1);
        assert_eq!(h.tabs.len(), 2);
        assert_eq!(h.active, 1, "新开的标签页应当成为活动页");

        h.open(2);
        assert_eq!(h.tabs.len(), 3);
        assert_eq!(h.active, 2);

        // 关掉左边那个(下标 1):活动页是 2,在它右边,所以下标要跟着左移。
        let active = h.close(1).expect("关掉之后还有活动标签页");
        assert_eq!(h.tabs.len(), 2);
        assert_eq!(h.active, 1);
        assert_eq!(active.graph, 2, "关掉左边的标签不该换掉当前图");

        // 关掉当前标签:落到新的活动页。
        let active = h.close(1).expect("还有一张");
        assert_eq!(h.tabs.len(), 1);
        assert_eq!(h.active, 0);
        assert_eq!(active.graph, h.tabs[0].graph);
    }

    /// 只剩一个标签页时点关闭**不真的关**:窗口总得有东西可看。
    #[test]
    fn closing_the_last_tab_keeps_a_tab() {
        let mut h = TabHost::new();
        h.store_view(0, point(50.0, 60.0), 2.0);
        let tab = h.close(0).expect("关最后一个也要给回一个标签页");
        assert_eq!(h.tabs.len(), 1, "最后一个标签页不能真的关掉");
        assert_eq!(tab.scale, 1.0, "关最后一个 = 回到刚打开的样子");
        assert_eq!(tab.origin, point(0.0, 0.0));
    }

    /// 切标签页可以后退 —— 用户明确要求。后退同时恢复**视野**,不只是图。
    #[test]
    fn switching_tabs_can_be_undone_by_back() {
        let mut h = TabHost::new();
        h.open(1);
        h.store_view(1, point(120.0, -40.0), 1.5);

        // 切到第 0 页(记下"切之前"的位置)。
        h.record(nav2(1, 120.0, -40.0, 1.5));
        let tab = h.switch(0).expect("切到第 0 页");
        assert_eq!(tab.graph, 0);
        assert_eq!(h.active, 0);

        // 后退:回到刚才那一页,视野也一起回来。
        let back = h.back(nav(0, 0.0, 1.0)).expect("应当能后退");
        assert_eq!(back.graph, 1);
        assert_eq!(back.origin, point(120.0, -40.0), "视野也要一起回来");
        assert_eq!(back.scale, 1.5);

        // 再前进,回到第 0 页。
        let fwd = h.forward(back).expect("应当能前进");
        assert_eq!(fwd.graph, 0);
    }

    /// 后退到底 / 前进到头时返回 `None`(调用方据此给状态提示)。
    #[test]
    fn back_at_the_beginning_is_a_noop() {
        let mut h = TabHost::new();
        assert!(h.back(nav(0, 0.0, 1.0)).is_none());
        assert!(h.forward(nav(0, 0.0, 1.0)).is_none());
    }

    /// 新的导航会清空前进历史(浏览器语义)。
    #[test]
    fn a_new_navigation_clears_forward_history() {
        let mut h = TabHost::new();
        h.record(nav(0, 0.0, 1.0));
        h.record(nav(1, 10.0, 1.0));
        assert!(h.back(nav(2, 20.0, 1.0)).is_some());
        assert!(!h.nav.forward.is_empty(), "后退之后前进栈里应当有东西");

        // 做一次新导航 → 前进分支作废。位置要和栈里已有的都不同,
        // 否则会被 `record` 的去重挡掉,测不到清空。
        h.record(nav(0, 30.0, 1.0));
        assert!(h.nav.forward.is_empty(), "新导航必须清掉前进历史");
    }

    /// 连续两次导航到同一位置**不重复记账** —— 否则来回点同一张图会把
    /// 后退栈塞满同一条,按十次后退还在原地。
    #[test]
    fn recording_the_same_position_twice_is_a_noop() {
        let mut h = TabHost::new();
        h.record(nav(0, 0.0, 1.0));
        h.record(nav(0, 0.0, 1.0));
        assert_eq!(h.nav.back.len(), 1);
    }

    /// 同一张图**不会**开出多个页签:已经开在某个页签里就切过去。
    ///
    /// 这是左栏点图那条路径的规则(`Viewer::open_graph_in_tab`)——
    /// 它先查 `tabs` 里有没有这张图。这里直接测查重本身。
    #[test]
    fn a_graph_is_opened_at_most_once() {
        let mut h = TabHost::new();
        h.open(1);
        h.open(2);
        assert_eq!(h.tabs.len(), 3);

        // 第 1 张已经在第 1 个页签里了 —— 查得到,所以不该再开。
        assert_eq!(h.find_graph(1), Some(1), "图 1 开在页签 1");
        assert_eq!(h.find_graph(2), Some(2), "图 2 开在页签 2");
        assert_eq!(h.find_graph(3), None, "图 3 没开过");
    }

    /// 后退**不能**把当前页签的内容换成目标图 —— 那是"替换",不是导航。
    ///
    /// 目标图已经开在另一页时,应当切到那一页(页数不变)。
    #[test]
    fn back_switches_to_the_existing_tab_instead_of_overwriting() {
        let mut h = TabHost::new();
        // 页签 0 显示图 0;再开一个页签显示图 1。
        h.open(1);
        assert_eq!(h.tabs.len(), 2);
        assert_eq!(h.active, 1);

        // 假装刚从图 0 导航到图 1(那时记的是"图 0")。
        h.record(nav(0, 0.0, 1.0));

        // 后退到一个指向图 0 的位置:应当切到页签 0,而不是把页签 1 改成图 0。
        let back = h.back(nav(1, 0.0, 1.0)).expect("有历史");
        assert_eq!(back.graph, 0);

        let tab = h.activate_graph(back.graph).expect("图 0 已经开着");
        assert_eq!(tab.graph, 0);
        assert_eq!(h.active, 0, "应当切到已经开着图 0 的那一页");
        assert_eq!(h.tabs.len(), 2, "页签数量不能变 —— 不能替换当前页");
        // 关键:原来那一页仍然是图 1,没被改写。
        assert_eq!(h.tabs[1].graph, 1, "另一半页签的内容必须原样保留");
    }

    /// 每个页签各记一份视野:**切走再切回来不能复位**。
    ///
    /// 这条锁的是「平移 / 缩放之后要回写页签」——`Viewer::sync_view`
    /// (`render` 里的平移、缩放两条路径都调它)。只把视野存在 `Viewer` 上
    /// 的实现会在这里失败。
    #[test]
    fn each_tab_remembers_its_own_view() {
        let mut h = TabHost::new();

        // 第 0 页:挪一挪、放大。
        h.store_view(0, point(120.0, -40.0), 2.5);
        let a = h.open(1);
        assert_eq!(a.origin, point(0.0, 0.0), "新页签从原点开始");

        // 第 1 页:另一个视野。
        h.store_view(1, point(-300.0, 77.0), 0.5);

        // 切回第 0 页,视野必须是它自己的那份。
        let back_to_0 = h.switch(0).expect("切回第 0 页");
        assert_eq!(back_to_0.origin, point(120.0, -40.0), "第 0 页的平移要回来");
        assert_eq!(back_to_0.scale, 2.5, "第 0 页的缩放要回来");

        // 再切到第 1 页,同样。
        let to_1 = h.switch(1).expect("切到第 1 页");
        assert_eq!(to_1.origin, point(-300.0, 77.0));
        assert_eq!(to_1.scale, 0.5);
    }

    /// 历史条目带着视野:切走再后退回来,视野也要一起回来。
    #[test]
    fn nav_entries_carry_the_view() {
        let mut h = TabHost::new();
        h.open(1);
        // 在页签 1 里调到某个视野,然后记录(模拟"要切走了")。
        h.store_view(1, point(55.0, 66.0), 3.0);
        let here = nav2(1, 55.0, 66.0, 3.0);
        h.record(here);

        // 切到页签 0(视野复位)。
        let t0 = h.switch(0).expect("切到第 0 页");
        assert_eq!(t0.scale, 1.0);

        // 后退:回到页签 1 的位置,视野照历史还原。
        let back = h.back(nav(0, 0.0, 1.0)).expect("有历史");
        assert_eq!(back.graph, 1);
        assert_eq!(back.origin, point(55.0, 66.0));
        assert_eq!(back.scale, 3.0);
    }

    /// 历史记的那张图**所有页签都关掉了**:这时按"打开这张图"新开一页,
    /// 仍然不改写当前页签。
    #[test]
    fn back_reopens_a_graph_whose_tab_was_closed() {
        let mut h = TabHost::new();
        h.open(1);
        // 关掉图 1 那一页,再切回图 0 的页。
        h.close(1);
        assert_eq!(h.tabs.len(), 1);
        assert_eq!(h.tabs[0].graph, 0);

        // 历史里记着"曾经在图 1"。现在图 1 那一页已经关掉了。
        h.record(nav(1, 30.0, 2.0));

        let back = h.back(nav(0, 0.0, 1.0)).expect("有历史");
        assert_eq!(back.graph, 1);
        assert_eq!(h.find_graph(back.graph), None, "图 1 确实没开着");

        let tab = h.open(back.graph);
        assert_eq!(tab.graph, 1);
        assert_eq!(h.tabs.len(), 2, "应当新开一页,而不是替换");
        assert_eq!(h.tabs[0].graph, 0, "原来那一页不能被动");
    }

    /// 侧边 pin 排布:**控制流靠上、值流紧接其后**,整排不居中。
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

    /// 注释要跟着节点进 `NodeView`,并带上 core 那套尺寸估算 ——
    /// 渲染时按它把文本框摆在节点上方。
    #[test]
    fn comments_reach_the_node_view_with_their_estimated_size() {
        let mut n = node(1, 1, 0, 0);
        n.comment = Some("第一行
第二行".into());
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![],
            exit: vec![],
            nodes: vec![n],
            edges: vec![],
            interface: Default::default(),
        };
        let pos = vec![point(40.0, 80.0)];
        let ns = build_nodes(&g, &pos, point(0.0, 0.0), 2.0, &[]);
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
        let g = GraphEntry {
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
        };
        let pos = vec![point(0.0, 0.0), point(300.0, 0.0)];
        let es = build_edges(&g, &pos, point(0.0, 0.0), 1.0);
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
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![0],
            exit: vec![],
            nodes: vec![n0],
            edges: vec![],
            interface: Default::default(),
        };
        let pos = vec![point(10.0, 20.0)];
        let bs = build_boundaries(&g, &pos, point(0.0, 0.0), 1.0);
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
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![0],
            exit: vec![],
            nodes: vec![node(1, 1, 2, 2)],
            edges: vec![],
            interface: Default::default(),
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
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![0],
            exit: vec![],
            nodes: vec![n0],
            edges: vec![],
            interface: Default::default(),
        };
        let pos = vec![point(10.0, 20.0)];
        let ds = build_defaults(&g, &pos, point(0.0, 0.0), 1.0);
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
        let g = GraphEntry {
            name: "t".to_string(),
            index_of: HashMap::new(),
            entry: vec![],
            exit: vec![],
            nodes: vec![node_with_types(1, 0, 1, 0, &["Int"], &[])],
            edges: vec![],
            interface: Default::default(),
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
            interface: Default::default(),
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
            interface: Default::default(),
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
            interface: Default::default(),
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
            interface: Default::default(),
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
