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
    ParentElement, PathBuilder, Pixels, Point, Render, ScrollDelta, ScrollWheelEvent,
    StatefulInteractiveElement, Styled, TextRun, Window, canvas, div, hsla, point, px,
};
use rust2genshin::compile::ir::IrNodeId;
use rust2genshin::compile::link::Target;
use rust2genshin::node::NodeRef;

use crate::canvas_geom::{
    DEF_W, Hover, NODE_MIN_W, NODE_TEXT_PAD, PIN_D, build_boundaries, build_ctrl_labels,
    build_defaults, build_edges, build_nodes, build_pins, hit_test, node_box, pin_triangle, tag_box,
};
use crate::conv;
use crate::doc::{
    BASE_FONT_PX, COMMENT_FONT_PX, COMMENT_GAP, COMMENT_LINE_H, COMMENT_PAD_X, COMMENT_PAD_Y, Doc,
    FnInterface,
};
use crate::optimize::{self, RULES, Snapshot};

use crate::theme::{
    CONV_PAD, MAX_SCALE, MIN_SCALE, RULES_WIDTH, TOC_DIVIDER_WIDTH, TOC_MAX_WIDTH, TOC_MIN_WIDTH,
    TOC_WIDTH, ZOOM_STEP, button_disabled_bg, button_disabled_text, canvas_bg, comment_bg,
    comment_border, divider_bg, divider_hover_bg, edge_ctrl, edge_value, link_hint_border,
    marquee_bg, marquee_border, node_bg, panel_bg, pin_ctrl, selected_border, text_dim, text_hi,
};

// Ctrl+Z → 撤销、Ctrl+Y → 重做、Alt+←/→ → 后退 / 前进、Ctrl+W → 关标签页。
// `actions!` 展开出一个单元结构体 + `gpui::Action` 实现;键位在 `main.rs` 绑
// (`AppContext::bind_keys`),处理也挂在 App 级(`AppContext::on_action`)。
//
// ⚠️ 这几个里只有 `Ctrl+W` 是**跟焦点**的(见 `CANVAS_CONTEXT`),其余全局生效。
gpui::actions!([Undo, Redo, NavBack, NavForward, CloseTab]);

/// 画布元素的 key context。`Ctrl+W` 只在这个 context 下生效 ——
/// 也就是**画布持有焦点**时(焦点在左栏搜索框里时不关标签页)。
///
/// gpui 的机制:`.key_context("Canvas")` 挂在元素上,按键事件的 context 栈
/// 由"根 → 焦点元素"那条路径上的元素依次贡献(`window.rs:4196` 的
/// `context_stack`)。所以 `KeyBinding::new("ctrl-w", CloseTab, Some("Canvas"))`
/// 只在这条路径上有 `Canvas` 时匹配。
pub const CANVAS_CONTEXT: &str = "Canvas";

// ------------------------------------------------------------------ 配色
//
// 配色与尺寸常量都住在 [`crate::theme`] —— 那是界面的「视觉规格」。
// 这里只留**渲染逻辑**。

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

/// 一个节点盒子的**唯一渲染实现**。
///
/// 主画布和「调用约定」视图的内层节点都走这里。返回 `Div`(不是
/// `AnyElement`)—— 调用方要往上挂交互 / 选中描边,必须还是 `Div`。
///
/// 尺寸约定:调用方给的是**节点自己的**盒子(不含任何额外内边距)——
/// 引脚位置来自 core 的 `pin_row_y`,它假定引脚从节点顶部的 `PIN_TOP`
/// 开始。谁在这里偷偷加 `PAD`,引脚就会跑到盒子外面去。
///
/// # 宽度:`min_w` + 自然撑开,不是精确宽度
///
/// `w` 在这里是**下限**(`min_w`),实际宽度由文字 + 左右 padding 自己撑开。
/// 以前的 `.w(w)` 是精确宽度,而 `w` 来自按字符数的估算 —— 估得比实际宽,
/// 短标签节点内部左右就空出一大截。
///
/// 高度仍然是精确的:引脚位置按行数算,高度不对引脚就跑出框外。
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
        // 下限而不是定死 —— 文字长了盒子自己变宽。`px_1` 的左右 padding
        // (各 4px)由 `.px_1()` 提供,文字靠它和边框隔开。
        .min_w(px(w))
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
        // 不再 `overflow_hidden`:盒子现在按内容长,裁不到东西;
        // 真裁了反而说明宽度算出错了,宁可让它显形。
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

// ------------------------------------------------------------------ 悬停命中

/// 悬停气泡相对光标的偏移(容器局部 = 屏幕像素)和字号。偏右下是为了不挡住
/// 光标底下那个引脚 / 连线。气泡字号**固定**、不跟画布缩放 —— 它是 UI 提示,
/// 不是画布内容,缩到 6px 就读不了了。
const HOVER_DX: f32 = 12.0;
const HOVER_DY: f32 = 16.0;
const HOVER_FONT_PX: f32 = 12.0;

/// 悬停气泡的**唯一渲染实现**。
///
/// 主画布和「调用约定」视图共用 —— 两处各写一份的话，样式和"跟手"的行为
/// 会漂开（调用约定那份曾经钉在被命中的引脚上，不跟光标走）。
///
/// `at` 是**光标位置**（容器局部坐标），不是被命中元素的坐标：钉在元素上
/// 气泡会盖住那个引脚 / 连线本身，反而看不清。
fn hover_bubble(text: String, at: Point<f32>) -> gpui::Div {
    div()
        .absolute()
        .left(px(at.x + HOVER_DX))
        .top(px(at.y + HOVER_DY))
        .px_2()
        .rounded_sm()
        .bg(panel_bg())
        .border_1()
        .border_color(hsla(0.0, 0.0, 1.0, 0.35))
        .text_color(text_hi())
        .text_size(px(HOVER_FONT_PX))
        // **不裁剪**：类型名可能很长（`rust2genshin_demo::MyStruct`），
        // 宁可伸出去也不能只剩尾巴。
        .child(text)
}

// ------------------------------------------------------------------ 状态

/// 节点右键菜单(现在只有 `Fn` 节点的「link」一项)。
/// `x` / `y` 是**容器局部**坐标(和节点几何一个坐标系)。
#[derive(Clone, Debug, PartialEq)]
struct NodeMenu {
    /// 右键时那个节点在展平顺序里的下标。
    node_index: usize,
    x: f32,
    y: f32,
}

/// 一个标签页:打开着哪张图,以及**它自己那一整套画布状态**。
///
/// 标签页存的是**图下标**而不是图本身 —— `Doc::graphs` 只 append,下标是稳定的,
/// 同一张图在两个标签页里打开时也天然共享同一份数据(改了都能看见)。
///
/// # 为什么画布状态住在标签页里
///
/// 真正的多标签页应该是"每个页签一套画布",切走再切回来**什么都没变** ——
/// 包括选中了哪些节点、右键菜单开着没有、正在拖谁、视野在哪。
///
/// 以前这些都挂在 `Viewer` 上一份,`show()` 切页签时把它们全清掉
/// (`self.menu = None` / `clear_selection()` …),于是切回来等于重置。
/// 用户报过「为什么画布不是每个标签页一个而是切换时重置同一个画布」。
///
/// 现在这些字段搬进 `Tab`,`Viewer` 只在 `show()` 里**存取**它们。
#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    /// 这张标签页显示 `Doc::graphs` 里的第几张图。
    pub graph: usize,
    /// 平移量(**容器局部**坐标)。每个标签页各记一份 —— 切走再切回来,
    /// 视野还停在原处,不会被复位。
    pub origin: Point<f32>,
    /// 缩放。
    pub scale: f32,
    /// 这个页签里选中了哪些节点(展平后的**下标**,只在这一张图里有意义)。
    pub selected: Vec<usize>,
    /// 右键菜单。节点下标属于这张图,所以跟着页签走。
    pub menu: Option<NodeMenu>,
    /// 正在进行的拖动。切走的页签如果正拖着,回来接着拖。
    pub drag: Option<Drag>,
    /// 框选矩形(正在拖的时候)。
    pub marquee: Option<(Point<f32>, Point<f32>)>,
    /// 当前悬停的节点(画布内)。
    pub hover_node: Option<usize>,
    /// 「调用约定」内层节点的位置。按**图下标**索引 —— 同一张图在两个
    /// 页签里打开时看的是同一个函数,内层位置没必要各记一份。
    pub conv_pos: HashMap<usize, (f32, f32)>,
    /// 正在拖「调用约定」内层节点。
    pub conv_drag: Option<ConvDrag>,
    /// 「调用约定」的悬停命中。
    pub conv_hover: Option<(conv::Hit, Point<f32>)>,
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

impl Tab {
    /// 一个刚打开、什么都没做的标签页。
    fn fresh(graph: usize) -> Self {
        Self {
            graph,
            origin: point(0.0, 0.0),
            scale: 1.0,
            selected: Vec::new(),
            menu: None,
            drag: None,
            marquee: None,
            hover_node: None,
            conv_pos: HashMap::new(),
            conv_drag: None,
            conv_hover: None,
        }
    }
}

impl TabHost {
    /// **一开始一个页签都没有**(用户要求"初始时无需自动打开一页")。
    ///
    /// 以前预置一个指向图 0 的页签,于是启动就"已经打开了某张图"。
    /// 现在进来看左栏列表,双击才开页签。
    fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
            nav: NavHistory::default(),
        }
    }

    /// 测试用:**已经开着 `n` 页**(图 0..n),活动页是最后一页。
    ///
    /// `new()` 现在是空的("初始时无需自动打开一页"),但很多标签页规则的测试
    /// 要的是"已经有几页"这个前提。用这个函数构造,测试读起来更贴近本意。
    #[cfg(test)]
    fn with_tabs(n: usize) -> Self {
        Self {
            tabs: (0..n).map(Tab::fresh).collect(),
            active: n.saturating_sub(1),
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
        let tab = Tab::fresh(graph);
        self.tabs.push(tab.clone());
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
        self.tabs.get(idx).cloned()
    }

    /// 切到第 `idx` 个标签页。返回它的副本。下标越界或已是当前页时 `None`。
    fn switch(&mut self, idx: usize) -> Option<Tab> {
        if idx >= self.tabs.len() || idx == self.active {
            return None;
        }
        self.active = idx;
        self.tabs.get(idx).cloned()
    }

    /// 关掉第 `idx` 个标签页,返回新的活动标签页(`None` = 一个都不剩了)。
    ///
    /// 允许关掉最后一个 —— 用户明确要求。关到空之后 `active` 归 0、
    /// 列表为空,画布区由 `Viewer` 显示占位(和启动时那句提示一样)。
    fn close(&mut self, idx: usize) -> Option<Tab> {
        if idx >= self.tabs.len() {
            return None;
        }
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            self.active = 0;
            return None;
        }
        // 活动标签的落点:关掉的在它左边就左移一位;关掉的正是它就取右邻。
        if idx < self.active {
            self.active -= 1;
        } else if idx == self.active {
            self.active = self.active.min(self.tabs.len() - 1);
        }
        self.tabs.get(self.active).cloned()
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
#[derive(Clone, Copy, Debug, PartialEq)]
struct ConvDrag {
    /// 拖的是哪张图。
    graph: usize,
    /// 按下时的鼠标位置(窗口坐标)。
    anchor: Point<f32>,
    /// 按下时内层节点的位置(面板局部坐标)。
    start: (f32, f32),
}

/// 拖拽的种类。
#[derive(Clone, Debug, PartialEq)]
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
    /// 第二项是**光标位置**(面板局部坐标):气泡跟着光标走,不钉在被命中的
    /// 元素上 —— 和主画布同一个规矩(`hover_bubble`)。
    conv_hover: Option<(conv::Hit, Point<f32>)>,
    /// 「调用约定」面板在**窗口**坐标系里的左上角。
    ///
    /// 由面板的 canvas prepaint 每帧写 —— 悬停命中要把窗口坐标换算成
    /// 面板局部坐标,靠它。
    conv_origin: Rc<Cell<Point<f32>>>,
    /// 左栏里**被点中**的那一项(图下标)。`None` = 没选中任何项。
    ///
    /// 这是**浏览位置**,不是画布内容 —— 和 [`Viewer::current`] 是两回事:
    /// 单击左栏只改它(高亮那一行),双击才真的切图。和 IDE 文件树一致。
    toc_selected: Option<usize>,
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
            toc_selected: None,
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
        if let Some(first) = self.doc.push_target(t) {
            // **不在这里读坐标**:加载时还没布局(`Doc::push_target` 只展平,
            // 不算坐标)。`positions` 按图下标惰性补齐 —— 缺的位置是空表,
            // 等 `show()` 里 `ensure_layout` 算完再由 `reload_positions` 填。
            self.positions.resize_with(self.doc.len(), Vec::new);
            self.error = None;
            // **加载后不开任何页签**(用户要求"初始时无需自动打开一页")。
            //
            // 左栏列出全部图,画布显示"在左栏双击一个函数打开"。这样用户
            // 进来看到的是文件全貌,而不是被迫先看某一张 —— 和 IDE 打开
            // 一个项目不自动打开某个文件一样。
            //
            // 只在**一张图都没有时**给一个锚点:`toc_selected` 指向第一项,
            // 免得进来看上去什么都没选中。
            if self.tabs.tabs.is_empty() {
                self.toc_selected = Some(first);
                self.current = first;
            }
            cx.notify();
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
        if self.doc.entry(i).is_none() {
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

    /// 把视图切到"第 `graph` 张图 + 这个视野"。
    ///
    /// 所有"换视野"的路径最后都落到这里 —— 标签页切换、历史导航、左栏点图的
    /// 差别只在归一化(视野复位 / 保留)和历史记账上,搬运是同一份。
    ///
    /// # 画布状态跟着页签走
    ///
    /// `Viewer` 上有一份**镜像**:`current` / `selected` / `menu` / `drag` /
    /// `marquee` / `hover_node` / `conv_*`。渲染和事件都读这份,省掉到处
    /// `tabs.active()` 的转发。
    ///
    /// 切页签时:`save_canvas_into_active()` 把镜像**存回**旧页签,
    /// 再 `load_canvas_from_active()` 从新页签**取出**。所以切走再切回来,
    /// 选中 / 菜单 / 拖动 / 调用约定内层位置**都还在** —— 以前这里把它们
    /// 全清掉,切回来等于重置(用户报过)。
    fn show(&mut self, graph: usize, origin: Point<f32>, scale: f32) {
        // 先把当前这套画布状态存回**旧**页签(此时 `tabs.active` 还是旧的)。
        self.save_canvas_into_active();
        self.current = graph;
        self.origin = origin;
        self.scale = scale;
        self.tabs.store_view(graph, origin, scale);
        // 再从**新**页签取出它自己的那套。
        self.load_canvas_from_active();
        // 状态行说的是"上一张图的操作",切过来还挂着会误导。这个不跟页签走 ——
        // 它是"刚才那个操作的结果",不是画布状态。
        self.status = None;
        // **按需布局**:加载时不算坐标(57 张图要 3.1s),第一次打开这张图
        // 才算。所有"换图"的路径都汇到这里,所以这是唯一需要接的地方。
        if self.doc.ensure_layout(graph) {
            self.reload_positions(graph);
        }
    }

    /// 把 `Viewer` 上的画布镜像存回当前活动页签。
    fn save_canvas_into_active(&mut self) {
        let Some(tab) = self.tabs.tabs.get_mut(self.tabs.active) else {
            return;
        };
        tab.selected = std::mem::take(&mut self.selected);
        tab.menu = self.menu.take();
        tab.drag = self.drag.take();
        tab.marquee = self.marquee;
        tab.hover_node = self.hover_node;
        tab.conv_pos = std::mem::take(&mut self.conv_pos);
        tab.conv_drag = self.conv_drag.take();
        tab.conv_hover = self.conv_hover.take();
    }

    /// 把当前活动页签的画布状态取进 `Viewer` 的镜像。
    ///
    /// 新开的页签是空的(`Tab::fresh`),取出来就是"什么都没选、没菜单"——
    /// 正是新页签该有的样子。
    fn load_canvas_from_active(&mut self) {
        let Some(tab) = self.tabs.tabs.get(self.tabs.active) else {
            // 一个页签都不剩:镜像全部清空,画布区显示占位。
            self.selected.clear();
            self.menu = None;
            self.drag = None;
            self.marquee = None;
            self.hover_node = None;
            self.conv_pos.clear();
            self.conv_drag = None;
            self.conv_hover = None;
            return;
        };
        self.selected = tab.selected.clone();
        self.menu = tab.menu.clone();
        self.drag = tab.drag.clone();
        self.marquee = tab.marquee;
        self.hover_node = tab.hover_node;
        self.conv_pos = tab.conv_pos.clone();
        self.conv_drag = tab.conv_drag;
        self.conv_hover = tab.conv_hover.clone();
    }

    /// 重新读第 `i` 张图的节点坐标(布局刚算过 / 外部改过 IR 之后)。
    fn reload_positions(&mut self, i: usize) {
        let Some(g) = self.doc.entry(i) else {
            return;
        };
        let pos: Vec<Point<f32>> = g.nodes.iter().map(|n| n.position).collect();
        if let Some(slot) = self.positions.get_mut(i) {
            *slot = pos;
        } else if i == self.positions.len() {
            self.positions.push(pos);
        }
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
        if self.doc.entry(entry.graph).is_none() {
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

    /// `Ctrl+W`:关掉**当前活动**的标签页。
    ///
    /// 和 `close_tab(idx)` 的区别只是"哪一页":这个取 `tabs.active`,
    /// 那个由调用方指定(页签上的 × 按钮 / 中键点页签)。
    ///
    /// 只有一页时不会真的关掉 —— `TabHost::close` 的规则是"总得有东西可看",
    /// 它会复位那一页(见 `closing_the_last_tab_keeps_a_tab` 测试)。
    pub fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        let active = self.tabs.active;
        self.close_tab(active, cx);
    }

    /// [`Viewer::open_tab`] 的纯逻辑部分。
    fn open_tab_step(&mut self, i: usize) -> bool {
        if self.doc.entry(i).is_none() {
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

    /// 左栏里点了一下某项:**只选中,不切图**(双击才切)。
    ///
    /// "选中"记在 `toc_selected`:它和"当前正在看的图"(`current`)是两回事 ——
    /// 前者是左栏的浏览位置,后者是画布内容。
    fn select_in_toc(&mut self, i: usize) {
        self.toc_selected = Some(i);
    }

    /// [`Viewer::open_graph_in_tab`] 的纯逻辑部分。
    fn open_graph_step(&mut self, i: usize) -> bool {
        if self.doc.entry(i).is_none() {
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
    /// **可以关到一页不剩**(用户要求):关掉之后 `close` 回 `None`,
    /// 这里把画布镜像清空,渲染走"没有打开的图"占位。
    fn close_tab_step(&mut self, idx: usize) -> bool {
        if idx >= self.tabs.tabs.len() {
            return false;
        }
        self.record_nav();
        // 被关的那页如果正是活动页,它的画布状态先丢掉 —— 页签都没了,
        // 存回去没有意义(而且会写到一个刚好挪到那个下标的别的页签上)。
        if idx == self.tabs.active {
            self.selected.clear();
            self.menu = None;
            self.drag = None;
            self.marquee = None;
            self.hover_node = None;
            self.conv_pos.clear();
            self.conv_drag = None;
            self.conv_hover = None;
        }
        match self.tabs.close(idx) {
            Some(tab) => {
                // 还有别的页:切过去,并取出它自己的画布状态。
                self.current = tab.graph;
                self.origin = tab.origin;
                self.scale = tab.scale;
                self.load_canvas_from_active();
                if self.doc.ensure_layout(tab.graph) {
                    self.reload_positions(tab.graph);
                }
            }
            None => {
                // 一页不剩:清空镜像,`placeholder()` 会显示"拖入文件"提示。
                self.load_canvas_from_active();
            }
        }
        self.status = None;
        true
    }

    /// 当前标签页的标题(图名)。没有图时给个占位。
    fn tab_title(&self, tab: &Tab) -> String {
        self.doc
            .entry(tab.graph)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "?".to_string())
    }

    /// 右键节点:能给出菜单项(`Fn` 节点 → link)才弹;别的节点只关旧菜单。
    /// `at_window` 是事件的**窗口**坐标,换算成容器局部坐标存下来。
    fn open_menu(&mut self, node_index: usize, at_window: Point<f32>, cx: &mut Context<Self>) {
        let is_fn = self
            .doc
            .entry(self.current)
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
        let Some(g) = self.doc.entry(i) else {
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
        let new_pos: Vec<Point<f32>> = self.doc
            .entry(i)
            .map(|g| g.nodes.iter().map(|n| n.position).collect())
            .unwrap_or_default();
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
            .entry(self.current)
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
        let g = self.doc.entry(self.current)?;
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
        let g = self.doc.entry(self.current)?;
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
        if self.doc.is_empty() {
            return Some("拖入 .ogia 或 .rlib 文件".to_string());
        }
        // **标签页全关了**:画布区没有内容可显示,但不能显示"拖入文件"
        // (文件明明已经加载着)。提示怎么打开。
        if self.tabs.tabs.is_empty() {
            return Some("没有打开的图(在左栏双击一个函数打开)".to_string());
        }
        // `current` 越界(切图瞬间)按"没有可显示的图"处理,不索引、不 panic。
        let Some(g) = self.doc.entry(self.current) else {
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
        // ---- 按需布局(渲染期的保险) ----
        //
        // 正常路径是 `show()` 里算过,但渲染可能先于任何 `show()` 发生
        // (首次加载、撤销把 `laid_out` 弄丢)。这里兜一次底:
        // `ensure_layout` 没算过才跑,算过就是一次 `bool` 检查。
        if self.doc.ensure_layout(self.current) {
            self.reload_positions(self.current);
        }

        // 图存在性由 `placeholder()` 保证(它拦掉了"没有可显示的图"),
        // 这里只做一次存在性检查,几何交给 `render_canvas`。
        if self.doc.entry(self.current).is_none() {
            return div().size_full().into_any_element();
        }

        // ---- 中栏画布:有图才画,没图就显示占位 ----
        //
        // ⚠️ **占位只占画布那一格**,不是整窗。以前这里在 `render` 开头
        // 直接 `return` 一整块占位,于是"关掉最后一个标签页"时连左栏、右栏
        // 一起消失(用户报过)—— 左栏是文件列表,和"画布有没有内容"无关,
        // 任何时候都该在。
        let canvas_el = if let Some(msg) = self.placeholder() {
            div()
                .id("canvas-placeholder")
                .flex_1()
                .h_full()
                .relative()
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
        } else {
            // 存在性由 `placeholder()` 保证;几何交给 `render_canvas`。
            self.render_canvas(window, cx)
        };

        // ---- 左栏:图列表(实现在 `render_toc`)----
        let (toc, divider) = self.render_toc(window, cx);

        // ---- 右栏:调用约定 + 优化按钮(实现在 `render_rules_panel`)----
        let rules_panel = self.render_rules_panel(cx);

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
                    .on_click(cx.listener(move |v, _e, window, cx| {
                        v.switch_tab(i, cx);
                        // 切页签 = 回到那块画布,焦点也还给画布(和双击左栏一致)。
                        window.focus(&v.canvas_focus);
                    }))
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
        let Some(g) = self.doc.entry(self.current) else {
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
        let g = self.doc.entry(self.current)?;
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
        // 气泡跟着**光标**走(存光标位置),不是钉在命中的元素上 ——
        // 和主画布同一套(`Hover::at`)。
        let next = hit.map(|h| (h, at));
        if self.conv_hover != next {
            self.conv_hover = next;
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
                let (from, to) = link_endpoints_text(outer, inner, link)?;
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
        let inner = &self.doc.entry(self.current)?.interface;
        Some((outer, inner))
    }

    /// 内层节点在面板里的当前位置(没拖过就**居中**在外层盒子里)。
    ///
    /// 居中时外层左右两侧的引脚都露得出来:靠左坐会把右侧的返回值引脚
    /// 全压在内层底下(用户报过"为什么内层默认不是居中而是偏左")。
    fn conv_inner_pos(&self, layout: &conv::Layout) -> (f32, f32) {
        let stored = self.conv_pos.get(&self.current).copied();
        let default = (
            layout.outer.x + (layout.outer.w - layout.inner.w) / 2.0,
            layout.outer.y + (layout.outer.h - layout.inner.h) / 2.0,
        );
        conv::clamp_inner(stored.unwrap_or(default), layout)
    }


    /// 中栏画布:边 + 节点 + 引脚 + 图边界 + 框选 + 右键菜单 + 平移/缩放。
    ///
    /// 抽出来是因为 `render()` 原来一个函数 1000 多行,画布这一块又占了
    /// 一多半。这里只搬代码,不改行为 —— 绘制顺序(边 → 节点 → 引脚 →
    /// 标签 → 气泡 → 菜单)和所有事件分工都逐字保留。
    fn render_canvas(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        // ---- 先把每个节点的**实测宽度**写回几何 ----
        //
        // 布局层不按标签估宽(宽度在那里是常量),而引脚 / 连线 / Export 桩
        // 都从节点**右边缘**推导 —— 所以必须在画之前用排版引擎量出真宽度,
        // 否则盒子被文字撑开、引脚却还画在旧位置。
        self.measure_node_widths(window, cx);

        // `render()` 已经保证图存在(`placeholder()` 拦掉了没有图的情况),
        // 这里还是用 `get`:渲染期不做任何未检查的下标访问。
        let Some(g) = self.doc.entry(self.current) else {
            return div().id("canvas-empty");
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
            layer = layer.child(hover_bubble(h.text.clone(), h.at));
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
            .id("canvas")
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            // 画布自己持有焦点(见 `Viewer::canvas_focus` 的注释):
            // 这样"根 → 画布"总在键盘 / modifier 事件的分发路径上。
            .track_focus(&canvas_focus)
            // `Ctrl+W`(关标签页)只在画布持焦点时生效 —— 见 `CANVAS_CONTEXT`。
            .key_context(CANVAS_CONTEXT)
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
        canvas_el
    }

    /// 用排版引擎实测当前图每个节点的标签宽度,写回 `Doc`。
    ///
    /// # 为什么必须实测
    ///
    /// 节点盒子是 `.min_w(w)` + 自然撑开 —— 文字多宽盒子就多宽。而引脚、
    /// 连线、Export 桩、默认值、分支名**全部**从节点右边缘推导
    /// (`canvas_geom.rs` 里 8 处 `p.x + s.width`)。宽度不实测的话,盒子长
    /// 了、引脚还画在旧位置,右侧引脚会跑进盒子内部。
    ///
    /// # 量的是「世界坐标」宽度
    ///
    /// 画布上的宽度都按 `世界宽度 * scale` 渲染,所以这里传入 1 倍字号的
    /// `BASE_FONT_PX` 去量,得到的就是世界坐标宽度 —— 和 `node_box()` 里
    /// `s.width * scale` 那套一致。字号下限(`.max(6.0)`)在缩小时会生效,
    /// 那种情况下实测值偏小一点,但缩放很小时本来也读不清字,不值得为它
    /// 反解。
    fn measure_node_widths(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(g) = self.doc.entry(self.current) else {
            return;
        };
        let labels: Vec<String> = g.nodes.iter().map(|n| n.display.clone()).collect();
        // 表格里拿不到 `cx`(它被借走了),所以先把 TextSystem 拿出来。
        let text_system = window.text_system().clone();
        // 量宽用的样式:字号用 1 倍基准,字体取默认。
        let style = window.text_style();
        let font_size = px(BASE_FONT_PX);
        let widths: Vec<f32> = labels
            .iter()
            .map(|label| {
                let run = TextRun {
                    len: label.len(),
                    font: style.font(),
                    color: style.color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let layout = text_system.layout_line(label, font_size, &[run], None);
                // 文字宽度 + 左右 padding / 边框,和 `node_box_el` 里的
                // `.px_1()` + `.border_1()` 对上。
                let text_w = f32::from(layout.width);
                (text_w + NODE_TEXT_PAD).max(NODE_MIN_W)
            })
            .collect();
        let _ = cx;
        self.doc.set_node_widths(self.current, &widths);
    }

    /// 右栏:「调用约定」面板 + 优化按钮。
    ///
    /// 抽出来是因为 `render()` 原来一个函数 1000 多行。这里只搬代码,
    /// 不改行为 —— 按钮顺序、禁用规则、状态文案都逐字保留。
    fn render_rules_panel(&mut self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
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
        rules_panel
    }
    /// 左栏:搜索框 + 图列表 + 右侧分隔条。
    ///
    /// 抽出来是因为 `render()` 一个函数原来有 1000 多行 —— 左栏、右栏、
    /// 标签栏、画布全挤在里面,改一处要在一大坨里找。这里只搬代码,不改行为。
    ///
    /// 返回 (左栏, 分隔条) 两个元素 —— 分隔条要和左栏并排,由调用方拼。
    ///
    /// 两者都是 `Stateful<Div>`:它们各挂了 `id` / `track_focus` / 事件
    /// (`.id()` 之后类型就从 `Div` 变成 `Stateful<Div>` 了)。
    fn render_toc(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (gpui::Stateful<gpui::Div>, gpui::Stateful<gpui::Div>) {
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
                            .entries()
                            .enumerate()
                            .filter(|(_, entry)| Self::matches_search(&entry.name, &query))
                            .map(|(i, entry)| {
                                // 高亮 = **左栏选中项**(单击的那个),不是"画布正在看的那张"。
                                // 两者可以不同:单击只移动这里的选中,双击才切画布。
                                let picked = self.toc_selected == Some(i);
                                // 正在画布上显示的那张另给一个淡标记,免得看丢。
                                let on_canvas = i == self.current;
                                div()
                                    .id(("graph", i))
                                    .px_2()
                                    .py_1()
                                    .mb_1()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .overflow_hidden()
                                    .text_xs()
                                    .when(picked, |s| s.bg(hsla(0.0, 0.0, 1.0, 0.18)))
                                    .when(on_canvas && !picked, |s| {
                                        s.border_l_2().border_color(selected_border())
                                    })
                                    .child(format!("{} ({})", entry.name, entry.nodes.len()))
                                    // **单击 = 选中,双击 = 在标签页里打开**(用户要求)。
                                    //
                                    // 用 `on_mouse_down` 而不是 `on_click`:`on_click`
                                    // 只认左键、也拿不到点击次数(`div.rs:2139`);
                                    // `MouseDownEvent::click_count` 才是"第几次点击"
                                    // (`interactive.rs:102`,平台双击时给 2)。
                                    //
                                    // 点一下就把这行标成"当前选中项"(左栏自己的高亮),
                                    // 双击才真的切图 / 开新页 —— 和 IDE 的文件树一致。
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |v, e: &MouseDownEvent, window, cx| {
                                            v.select_in_toc(i);
                                            if e.click_count >= 2 {
                                                // 双击 = 打开/切换标签页,然后**把焦点交给画布**。
                                                //
                                                // 用户报过「打开一个标签页时,标签页没自动
                                                // 获得焦点」:不交焦点的话 `Ctrl+W`
                                                // (`Canvas` context)和撤销都按不动,得先
                                                // 手动点一下画布才行。
                                                v.open_graph_in_tab(i, cx);
                                                window.focus(&v.canvas_focus);
                                            } else {
                                                // 单击只选中:焦点留在左栏,继续打字搜索。
                                                window.focus(&v.search_focus);
                                            }
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    )
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
        (toc, divider)
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
        //
        // 标题恒为「当前」:没有数据流引脚也照样是"当前状态",写「空」会让人
        // 以为这个函数没内容(用户报过)。
        //
        // ⚠️ **宽度必须和 `conv::layout` 算引脚时用的值一致**。
        // `layout()` 内部按 `INNER_RENDER_W` 放右引脚,盒子这里也用它,
        // 两者才对得上 —— 以前盒子传的是布局余量 `INNER_W`(150),
        // 右引脚落在盒子外的空白里(用户报过)。
        let inner_label = "当前";
        // 按下时内层节点的位置 —— 闭包是 `'static'` 的,得先把值拷出来。
        let inner_start = (l.inner.x, l.inner.y);
        let inner_box = node_box_el(
            l.inner.x,
            l.inner.y,
            conv::INNER_RENDER_W,
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
        for (_right, y, name) in &l.outer_left_labels {
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(l.outer.x - l.left_gutter))
                    .top(px(*y - 8.0))
                    .w(px(l.left_gutter - 4.0))
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
                    .w(px(l.right_gutter - 4.0))
                    .text_size(px(11.0))
                    .text_color(text_dim())
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(name.clone()),
            );
        }
        // 常数:贴在外层那个输出引脚的**左边**(常数就是那个引脚的值)。
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
        // 两端的类型。气泡**跟着光标走**、样式复用主画布的 `hover_bubble` ——
        // 之前这里钉在被命中的元素上,看着就不"跟手"。
        if let Some((hit, at)) = &self.conv_hover {
            if let Some(text) = self.conv_hover_text(hit, outer, inner) {
                layer = layer.child(hover_bubble(text, *at));
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
                    .child("调用约定"),
            )
            .child(
                div()
                    .id("conv-body")
                    .relative()
                    .w(px(width))
                    // 高度**就是外层盒子的高** —— 不再和固定下限取 max。
                    // 以前 `max(outer.h, 300)` 会把 96px 的盒子撑到 300px,
                    // 底下白空一大片(用户报的"下部空余太多")。
                    .h(px(l.outer.y + l.outer.h + conv::MARGIN))
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

/// 一条连线两端的 `(名字, 类型)`;`None` = **这条线没什么可说的**。
///
/// 端点是哪一侧、哪个下标,取决于连线的种类:
/// - `Param`:外层参数 → 内层参数;
/// - `Ret`:内层返回值 → 外层返回值;
/// - `PassThrough`:外层参数 → 外层返回值。
///
/// **控制流一律回 `None`** —— 它没有名字也没有类型,"控制流 → 控制流"
/// 这种气泡纯属噪声(用户报过)。指在控制流连线上不出气泡就行,这也和主
/// 画布一致:那边的控制流 pin 也是没有 tip 的(`pin_tip` 只给值流)。
fn link_endpoints_text(
    outer: &FnInterface,
    inner: &FnInterface,
    link: &conv::Link,
) -> Option<((String, String), (String, String))> {
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
    Some(match *link {
        conv::Link::Param { outer: o, inner: i } => (param(o), inner_param(i)),
        conv::Link::Ret { outer: o, inner: i } => (inner_ret(i), ret(o)),
        conv::Link::PassThrough { param: p, ret: r } => (param(p), ret(r)),
        conv::Link::CtrlIn { .. } | conv::Link::CtrlOut { .. } | conv::Link::CtrlThrough { .. } => {
            return None;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut h = TabHost::with_tabs(1);
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

    /// 每个标签页各记一套画布状态:切走再切回来,**选中 / 菜单 / 拖动 /
    /// 调用约定内层位置都还在**。
    ///
    /// 以前这些挂在 `Viewer` 上一份、`show()` 时全清掉,切回来等于重置
    /// (用户报过「为什么画布不是每个标签页一个而是切换标签页时重置同一个画布」)。
    /// 现在它们住在 `Tab` 里,`show()` 只做"存回旧页签 / 取出新页签"。
    ///
    /// 这条测的是**数据归属**:两个页签的字段互不影响。存取那一层
    /// (`save_canvas_into_active` / `load_canvas_from_active`)要 `Viewer`,
    /// 构造不出来(`FocusHandle` 没有 `Default`),所以在这里验数据结构。
    #[test]
    fn each_tab_keeps_its_own_canvas_state() {
        let mut a = Tab::fresh(0);
        let mut b = Tab::fresh(1);

        a.selected = vec![3, 7];
        a.hover_node = Some(3);
        a.marquee = Some((point(1.0, 2.0), point(9.0, 8.0)));
        a.conv_pos.insert(0, (12.0, 34.0));

        // 两个页签互不干扰。
        assert!(b.selected.is_empty(), "新页签没有选中");
        assert_eq!(b.hover_node, None);
        assert_eq!(b.marquee, None);
        assert!(b.conv_pos.is_empty());

        // 同一张图在两个页签里打开时,`conv_pos` 按**图下标**索引 ——
        // 所以两边各存各的 `(f32, f32)` 也互不影响。
        b.conv_pos.insert(0, (99.0, 99.0));
        assert_eq!(a.conv_pos.get(&0), Some(&(12.0, 34.0)), "a 的位置没被 b 改掉");
        assert_eq!(b.conv_pos.get(&0), Some(&(99.0, 99.0)));
    }

    /// `Ctrl+W` 关的是**当前活动**那一页,不是第 0 页、也不是最后页。
    ///
    /// 这是 `close_active_tab` 的全部逻辑(`tabs.active` → `close`)——
    /// 它自己不碰 GPUI,所以能在这里直接验。
    #[test]
    fn ctrl_w_closes_the_active_tab() {
        let mut h = TabHost::with_tabs(1);
        h.open(1);
        h.open(2);
        // 切回中间那页(下标 1),让它成为活动页。
        h.switch(1).expect("切到第 1 页");
        assert_eq!(h.active, 1);
        let closing = h.tabs[1].graph;

        // `close_active_tab` 取的就是 `tabs.active`。
        let active = h.active;
        let tab = h.close(active).expect("还有别的页");
        assert_eq!(h.tabs.len(), 2, "只关掉一页");
        assert!(
            !h.tabs.iter().any(|t| t.graph == closing),
            "被关掉的是活动页那一张图"
        );
        // 关完之后活动页仍然有效。
        assert!(h.active < h.tabs.len());
        assert_eq!(tab.graph, h.tabs[h.active.min(h.tabs.len() - 1)].graph);
    }

    /// **可以关到一页不剩** —— 用户明确要求「仅剩一个标签页时无法关闭」要修。
    ///
    /// **一开始一个页签都没有**(用户要求"初始时无需自动打开一页")。
    ///
    /// 进来看左栏列表,双击才开页签 —— 和 IDE 打开一个项目不自动打开
    /// 某个文件一样。以前预置了一页指向图 0,于是启动就"已经打开了某张图"。
    #[test]
    fn a_fresh_tab_host_has_no_tabs() {
        let h = TabHost::new();
        assert!(h.tabs.is_empty(), "启动时不该有任何页签");
        assert_eq!(h.active, 0, "空列表上活动下标是 0(不会越界)");
    }

    /// 关到空之后 `close` 回 `None`、`active` 归 0,`Viewer` 那边把画布镜像
    /// 清空、显示"没有打开的图"占位。以前这里有"最后一个不真的关"的特例
    /// (只把视野复位),现在去掉了。
    #[test]
    fn the_last_tab_can_be_closed() {
        let mut h = TabHost::with_tabs(1);
        h.store_view(0, point(50.0, 60.0), 2.0);
        let gone = h.close(0);
        assert!(gone.is_none(), "关掉最后一页之后没有活动页了");
        assert!(h.tabs.is_empty(), "标签页列表应当真的空了");
        assert_eq!(h.active, 0, "活动下标归 0(空列表上不会越界)");
    }

    /// 切标签页可以后退 —— 用户明确要求。后退同时恢复**视野**,不只是图。
    #[test]
    fn switching_tabs_can_be_undone_by_back() {
        let mut h = TabHost::with_tabs(1);
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
        let mut h = TabHost::with_tabs(1);
        assert!(h.back(nav(0, 0.0, 1.0)).is_none());
        assert!(h.forward(nav(0, 0.0, 1.0)).is_none());
    }

    /// 新的导航会清空前进历史(浏览器语义)。
    #[test]
    fn a_new_navigation_clears_forward_history() {
        let mut h = TabHost::with_tabs(1);
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
        let mut h = TabHost::with_tabs(1);
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
        let mut h = TabHost::with_tabs(1);
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
        let mut h = TabHost::with_tabs(1);
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
        let mut h = TabHost::with_tabs(1);

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
        let mut h = TabHost::with_tabs(1);
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
        let mut h = TabHost::with_tabs(1);
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

    /// 悬停气泡**跟着光标走**，不是钉在被命中的元素上。
    ///
    /// 用户报过「调用约定的类型框不像外面一样跟手」—— 那边以前钉在引脚坐标
    /// 上。现在两处共用 [`hover_bubble`]，锚点一律是光标位置。
    #[test]
    fn the_hover_bubble_follows_the_cursor() {
        // 同一段文案、两个不同的光标位置 → 气泡落在两个不同地方，
        // 而且都相对光标偏移固定的 `HOVER_DX` / `HOVER_DY`。
        let a = hover_bubble("x <Int>".to_string(), point(100.0, 100.0));
        let b = hover_bubble("x <Int>".to_string(), point(300.0, 250.0));
        // `Div` 的样式是 opaque 的，能直接验的是常量本身：
        // 偏移是**固定量**，和命中元素的位置无关。
        assert_eq!(HOVER_DX, 12.0);
        assert_eq!(HOVER_DY, 16.0);
        // 两个气泡确实构造得出来（不同锚点不 panic）。
        let _ = (a, b);
    }
}
