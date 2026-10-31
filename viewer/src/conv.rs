//! 「调用约定」视图的**纯逻辑** —— 内外两层的引脚配对、连线分类与几何。
//!
//! 这个视图回答的问题是:「**调用方**看到的那个复合节点,和这个函数**现在**
//! 长什么样差在哪?」
//!
//! - **外层**:加载时的那份接口,**冻结**,永远不变 —— 代表调用方看到的节点;
//! - **内层**:当前的接口,优化 / lower 之后会变(引脚被消掉、常数被算出来、
//!   参数被原样透传出去),画成**一个普通节点**,可以在外层里拖动。
//!
//! 连线的三条规则(用户定的):
//!
//! - 外层的每个**数据流出引脚**,要么连到内层的出引脚(函数体算出来的),
//!   要么连到外层的入引脚(原样把某个参数还回去),要么是一个常数;
//! - 控制流引脚要么连到内层,要么**直接贯穿**;
//! - 外层要有足够的空间,内层可以在其中拖动。
//!
//! 这里不碰 GPUI:输入输出都是 `FnInterface` 和几个结构体,所以能在
//! `cargo test` 里直接验证(渲染层只是这些数据的搬运工)。

use crate::doc::{FnInterface, PortInfo, RetOrigin};
use gpui::{Point, point};

// ------------------------------------------------------------------ 连线

/// 一条连线。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// 外层参数 → 内层参数:调用方喂进来的参数进了**当前**的函数体。
    Param {
        outer: usize,
        inner: usize,
    },
    /// 内层返回值 → 外层返回值:函数体算出来的值返回给调用方。
    Ret {
        outer: usize,
        inner: usize,
    },
    /// 外层参数 → 外层返回值:函数**原样**把某个参数还回去(没做任何计算)。
    PassThrough {
        param: usize,
        ret: usize,
    },
    /// 外层控制入 → 内层控制入。
    CtrlIn {
        outer: usize,
        inner: usize,
    },
    /// 内层控制出 → 外层控制出。
    CtrlOut {
        outer: usize,
        inner: usize,
    },
    /// 外层控制入 → 外层控制出:**直接贯穿**(当前函数体没有控制流)。
    CtrlThrough {
        from: usize,
        to: usize,
    },
}

/// 一个常数:某个外层返回值的当前取值。
///
/// 常数**写在内侧**(用户要求)—— 它是函数体里的一个事实,不是接口的一部分。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constant {
    /// 外层返回值的下标。
    pub ret: usize,
    /// 常数的文本(`Debug` 格式)。
    pub text: String,
    /// 内层哪个返回值引脚是这个常数(内层下标)。
    ///
    /// 常数画在**内层节点旁边**、对准内层那个出引脚 —— 它是函数体里的
    /// 一个事实,所以位置跟着内层节点走。
    pub inner: usize,
}

// ------------------------------------------------------------------ 配对

/// 按**名字**把内层引脚配到外层引脚。
///
/// ⚠️ **必须按名字,不能按下标** —— 优化会压缩引脚(`compact_value_imports` /
/// `compact_value_exports`),**下标会移位**,名字不会。
///
/// 返回值是 `inner` 下标 → `outer` 下标(`None` = 没有对应的外层引脚)。
///
/// 名字对不上时退化成**按顺序配**:理论上门槛不该发生(外层是加载时的
/// 冻结快照,内层是它的子序列),但退化总比丢连线好。
pub fn match_by_name(outer: &[PortInfo], inner: &[PortInfo]) -> Vec<Option<usize>> {
    let mut used = vec![false; outer.len()];
    let mut out = vec![None; inner.len()];
    // 第一轮:名字完全相同。
    for (i, p) in inner.iter().enumerate() {
        if let Some(j) = outer
            .iter()
            .enumerate()
            .find(|(j, q)| !used[*j] && q.name == p.name)
            .map(|(j, _)| j)
        {
            used[j] = true;
            out[i] = Some(j);
        }
    }
    // 第二轮:剩下的按顺序配。
    let mut rest: Vec<usize> = (0..outer.len()).filter(|&j| !used[j]).collect();
    for slot in out.iter_mut() {
        if slot.is_none()
            && let Some(j) = rest.first().copied()
        {
            rest.remove(0);
            *slot = Some(j);
        }
    }
    out
}

// ------------------------------------------------------------------ 约定

/// 内外两层的配对结果。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Convention {
    /// 所有连线。
    pub links: Vec<Link>,
    /// 常数(写在内侧)。
    pub constants: Vec<Constant>,
    /// 外层里**已经没有**对应内层引脚的参数(优化把它消掉了)。
    pub dropped_params: Vec<usize>,
    /// 外层里**已经没有**对应内层返回值的返回值。
    pub dropped_rets: Vec<usize>,
}

impl Convention {
    /// 配对内外两层,算出连线、常数与"已被消掉"的引脚。
    ///
    /// `outer` 是**冻结**的那份(加载时的接口),`inner` 是当前状态。
    pub fn build(outer: &FnInterface, inner: &FnInterface) -> Convention {
        let param_map = match_by_name(&outer.params, &inner.params);
        let ret_map = match_by_name(&outer.rets, &inner.rets);

        let mut links = Vec::new();
        let mut constants = Vec::new();

        // 参数:外层 → 内层。
        for (i, j) in param_map.iter().enumerate() {
            if let Some(outer_idx) = *j {
                links.push(Link::Param {
                    outer: outer_idx,
                    inner: i,
                });
            }
        }

        // 返回值:按**当前**的来源分三类。
        for (i, j) in ret_map.iter().enumerate() {
            let Some(outer_idx) = *j else { continue };
            match inner.ret_origins.get(i) {
                // 函数体里算出来的 → 连到内层的出引脚。
                Some(RetOrigin::Body) => links.push(Link::Ret {
                    outer: outer_idx,
                    inner: i,
                }),
                // 原样透传某个参数 → 连到**外层**的入引脚。
                Some(RetOrigin::Param(pname)) => {
                    if let Some(p) = outer.params.iter().position(|q| &q.name == pname) {
                        links.push(Link::PassThrough {
                            param: p,
                            ret: outer_idx,
                        });
                    }
                }
                // 一个常数 → 写在内侧(对着内层那个出引脚)。
                Some(RetOrigin::Const(text)) => constants.push(Constant {
                    ret: outer_idx,
                    text: text.clone(),
                    inner: i,
                }),
                // 没接上 / 来源不明:什么都不画。
                Some(RetOrigin::Unset) | None => {}
            }
        }

        // 控制流:当前函数体**有**控制流就连到内层,没有就**直接贯穿**。
        if inner.ctrl_in_num > 0 {
            for k in 0..inner.ctrl_in_num.min(outer.ctrl_in_num) {
                links.push(Link::CtrlIn {
                    outer: k,
                    inner: k,
                });
            }
        } else if outer.ctrl_in_num > 0 {
            for k in 0..outer.ctrl_in_num.min(outer.ctrl_out_num) {
                links.push(Link::CtrlThrough { from: k, to: k });
            }
        }
        if inner.ctrl_out_num > 0 {
            for k in 0..inner.ctrl_out_num.min(outer.ctrl_out_num) {
                links.push(Link::CtrlOut {
                    outer: k,
                    inner: k,
                });
            }
        }

        // 外层里没人配上的引脚 = 已被优化消掉。
        let mut dropped_params = Vec::new();
        for j in 0..outer.params.len() {
            if !param_map.iter().any(|x| *x == Some(j)) {
                dropped_params.push(j);
            }
        }
        let mut dropped_rets = Vec::new();
        for j in 0..outer.rets.len() {
            if !ret_map.iter().any(|x| *x == Some(j)) {
                dropped_rets.push(j);
            }
        }

        Convention {
            links,
            constants,
            dropped_params,
            dropped_rets,
        }
    }
}

// ------------------------------------------------------------------ 几何

/// 一个矩形(面板局部坐标,单位 px)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// 一侧的引脚排布:控制流引脚在**上**,值流引脚在**下**(和主画布一致)。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pins {
    /// 控制流引脚(三角形)。
    pub ctrl: Vec<(f32, f32)>,
    /// 值流引脚(圆点)。
    pub value: Vec<(f32, f32)>,
}

/// 视图的完整几何。
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// 外层(冻结的调用节点)的盒子。
    pub outer: Rect,
    /// 内层(当前状态)的节点盒子。
    pub inner: Rect,
    pub outer_left: Pins,
    pub outer_right: Pins,
    pub inner_left: Pins,
    pub inner_right: Pins,
    /// 常数文本:画在**内层节点右侧**的 `(x, y)` + 文本。
    pub constants: Vec<(f32, f32, String)>,
    /// 外层左侧引脚的标签(参数名),画在盒子**外侧**。
    ///
    /// `(右对齐的右缘 x, y, 文本)` —— 右缘贴盒子左边,文字往左展开。
    pub outer_left_labels: Vec<(f32, f32, String)>,
    /// 外层右侧引脚的标签(返回值名),画在盒子**外侧**。
    ///
    /// `(左对齐的左缘 x, y, 文本)` —— 左缘贴盒子右边,文字往右展开。
    pub outer_right_labels: Vec<(f32, f32, String)>,
    /// 左边那条引脚名槽的宽度(渲染层用它给标签定宽)。
    pub left_gutter: f32,
    /// 右边那条引脚名槽的宽度。
    pub right_gutter: f32,
}

/// 外层盒子左右各留的边距(盒**内**的留白)。引脚从这之后开始排。
pub const PAD: f32 = 12.0;
/// 内层节点的宽度。
pub const INNER_W: f32 = 150.0;
/// 内层节点的最小高度(和画布的 `node_height` 取大的那个)。
pub const INNER_MIN_H: f32 = 32.0;
/// 外层盒子的最小高度。
pub const OUTER_MIN_H: f32 = 96.0;
/// 盒**外**留给引脚名的横向空间的**下限**(左右各一份)。
///
/// 引脚名画在盒子外侧(用户要求),盒子不能占满整个面板宽度 —— 名字得有
/// 地方放。实际宽度按**最长的那个名字**估(见 [`label_gutter`]),
/// 名字都短的时候不白留一大条空白。
pub const LABEL_GUTTER_MIN: f32 = 8.0;
/// 盒**外**留给引脚名的横向空间的**上限**。
///
/// 名字极长时不能把盒子挤没 —— 超出的部分由标签自己的 `overflow_hidden`
/// 裁掉(省略号没有,裁掉比压扁盒子好)。
pub const LABEL_GUTTER_MAX: f32 = 96.0;
/// 面板左右各留的边距。
pub const MARGIN: f32 = 4.0;

/// 按名字长度估一条引脚名槽要多宽。
///
/// `names` 是**这一侧全部**引脚名。估宽用和画布 `node_width` 同一套规则
/// (ASCII 6.6px/字、非 ASCII 按 11px),左右各再留一点间距。
///
/// 名字都短时返回 [`LABEL_GUTTER_MIN`],不会白占地方;名字很长时封顶到
/// [`LABEL_GUTTER_MAX`]。
pub fn label_gutter<'a>(names: impl IntoIterator<Item = &'a str>) -> f32 {
    let widest = names
        .into_iter()
        .map(|s| {
            s.chars()
                .map(|c| if c.is_ascii() { 6.6 } else { 11.0 })
                .sum::<f32>()
        })
        .fold(0.0f32, f32::max);
    // 文字两侧各留 6px(贴盒子那侧 6px + 边距 4px 取整)。
    (widest + 10.0).clamp(LABEL_GUTTER_MIN, LABEL_GUTTER_MAX)
}

/// 一层要画的东西。
///
/// 顺序就是**绘制顺序**(后面的盖在前面的上面)。
///
/// ⚠️ **引脚必须在盒子之后** —— 内层节点是不透明的,引脚先画就会被它盖住,
/// 表现就是"引脚被自己遮挡了"。这和主画布的顺序一致(那边也是
/// 「节点 → 引脚」)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// 边(画在最底下)。
    Edges,
    /// 外层盒子(冻结的调用节点)。
    OuterBox,
    /// 内层节点(当前状态)。
    InnerBox,
    /// 引脚(值流圆点 + 控制流三角形)。
    Pins,
    /// 文本(引脚名、常数)。
    Labels,
    /// 悬停气泡(最上面)。
    Hover,
}

/// 这个视图的绘制顺序。
///
/// 抽成数据是为了能直接测 —— 渲染层只负责照这个顺序往 `layer` 上挂东西。
pub const LAYER_ORDER: &[Layer] = &[
    Layer::Edges,
    Layer::OuterBox,
    Layer::InnerBox,
    Layer::Pins,
    Layer::Labels,
    Layer::Hover,
];

/// 一侧引脚的排布:控制流靠上、值流紧接其后,整排**靠上**不居中。
///
/// ⚠️ **必须用画布那套**(core 的 [`pin_rows`] / [`pin_row_y`]),不要自己
/// 写一份 —— 两边各写一份的话,画布上的引脚和调用约定里的引脚就会对不上。
///
/// 值流的起始行由 `value_start` 给出(跨两侧取最大 —— 见 core 里
/// `pin_rows` 的注释),所以左右控制流数量不同时,两侧的第一个值流引脚
/// 仍然对齐。
///
/// `top` 是这一侧盒子的顶部,`scale` 恒为 1(这个视图不缩放)。
fn pins_on_side(
    x: f32,
    top: f32,
    ctrl_n: usize,
    value_n: usize,
    rows: crate::doc::PinRows,
) -> Pins {
    use crate::doc::pin_row_y;
    Pins {
        ctrl: (0..ctrl_n)
            .map(|i| (x, top + pin_row_y(i)))
            .collect(),
        value: (0..value_n)
            .map(|j| (x, top + pin_row_y(rows.value_start + j)))
            .collect(),
    }
}

/// 算几何。
///
/// `inner_pos` 是内层节点的左上角(面板局部坐标)—— 用户拖出来的位置。
/// 它会被**夹**在外层盒子里面:拖不出外层。
///
/// `width` 是**面板**的总宽度。盒子只占中间那段,左右各留
/// [`LABEL_GUTTER`] 给引脚名(引脚名画在盒子外侧)。
pub fn layout(
    outer: &FnInterface,
    inner: &FnInterface,
    conv: &Convention,
    inner_pos: (f32, f32),
    width: f32,
) -> Layout {
    use crate::doc::{PIN_GAP, node_height, pin_rows};

    // 两侧的引脚行数 —— 和画布同一套(core 的 `pin_rows`)。值流起点跨
    // 两侧取最大,所以左右控制流数量不同时第一个值流引脚仍然对齐。
    let (outer_left_rows, outer_right_rows) = pin_rows(
        outer.ctrl_in_num,
        outer.params.len(),
        outer.ctrl_out_num,
        outer.rets.len(),
    );
    let (inner_left_rows, inner_right_rows) = pin_rows(
        inner.ctrl_in_num,
        inner.params.len(),
        inner.ctrl_out_num,
        inner.rets.len(),
    );

    // 内层节点的高:和画布同一个公式(引脚靠上排,按行数算)。
    let inner_h = node_height(inner_left_rows.rows.max(inner_right_rows.rows))
        .max(INNER_MIN_H);

    // 外层盒子的高:装下**两侧**的引脚,也要装得下内层节点(内层要在里面拖)。
    // 盒子顶部先留 `PAD`(引脚从 `PAD` 之后的第一行开始)。
    let outer_rows = outer_left_rows.rows.max(outer_right_rows.rows).max(1);
    let outer_h = (outer_rows as f32 * PIN_GAP + 2.0 * PAD)
        .max(OUTER_MIN_H)
        .max(inner_h + 2.0 * PAD + PIN_GAP);

    // 盒子左右各留一条给引脚名的槽 —— 宽度按**最长的名字**算,名字都短
    // 就不白留空白(用户报"左右空余太多")。
    let left_gutter = label_gutter(outer.params.iter().map(|p| p.name.as_str()));
    let right_gutter = label_gutter(outer.rets.iter().map(|p| p.name.as_str()));
    let outer_x = MARGIN + left_gutter;
    // 盒子宽度 = 面板宽 - 两条槽 - 两条边距。窄的时候不能算成负数。
    let outer_w =
        (width - (MARGIN + left_gutter) - (MARGIN + right_gutter)).max(INNER_W + 8.0);
    let outer_rect = Rect {
        x: outer_x,
        y: MARGIN,
        w: outer_w,
        h: outer_h,
    };

    // 内层节点的左上角:夹在外层里面(留 2px 余量)。
    let (mut ix, mut iy) = inner_pos;
    ix = ix.clamp(
        outer_rect.x + 2.0,
        (outer_rect.x + outer_rect.w - INNER_W - 2.0).max(outer_rect.x + 2.0),
    );
    iy = iy.clamp(
        outer_rect.y + 2.0,
        (outer_rect.y + outer_rect.h - inner_h - 2.0).max(outer_rect.y + 2.0),
    );
    let inner_rect = Rect {
        x: ix,
        y: iy,
        w: INNER_W,
        h: inner_h,
    };

    let outer_left = pins_on_side(
        outer_rect.x,
        outer_rect.y + PAD,
        outer.ctrl_in_num,
        outer.params.len(),
        outer_left_rows,
    );
    let outer_right = pins_on_side(
        outer_rect.x + outer_rect.w,
        outer_rect.y + PAD,
        outer.ctrl_out_num,
        outer.rets.len(),
        outer_right_rows,
    );
    // 内层的引脚直接贴在节点顶部 —— **不加 `PAD`**。
    //
    // 内层节点的高度是用画布那套 `node_height(rows)` 算的,那套公式假定
    // 引脚从节点顶部的 `PIN_TOP` 开始。再加 `PAD` 会整排下移,末行跑到
    // 节点底部**之外**。外层盒子是自绘的容器,它有自己的 `PAD`。
    let inner_left = pins_on_side(
        inner_rect.x,
        inner_rect.y,
        inner.ctrl_in_num,
        inner.params.len(),
        inner_left_rows,
    );
    let inner_right = pins_on_side(
        inner_rect.x + inner_rect.w,
        inner_rect.y,
        inner.ctrl_out_num,
        inner.rets.len(),
        inner_right_rows,
    );

    // 常数文本:画在**内层节点右侧**、纵向对准内层那个出引脚 —— 常数是
    // 函数体里的事实,位置跟着内层走。
    let constants = conv
        .constants
        .iter()
        .filter_map(|c| {
            let y = inner_right.value.get(c.inner).map(|p| p.1)?;
            Some((inner_rect.x + inner_rect.w + 8.0, y, c.text.clone()))
        })
        .collect();

    // 引脚名:画在盒子**外侧**。
    // - 左边(参数):右缘贴盒子左边,文字往左展开;
    // - 右边(返回值):左缘贴盒子右边,文字往右展开。
    //
    // **只写名字,不写类型** —— 类型在悬停气泡里(用户要求)。
    let outer_left_labels = outer
        .params
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let y = outer_left.value.get(i).map(|q| q.1)?;
            Some((outer_rect.x - 6.0, y, p.name.clone()))
        })
        .collect();
    let outer_right_labels = outer
        .rets
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let y = outer_right.value.get(i).map(|q| q.1)?;
            Some((outer_rect.x + outer_rect.w + 6.0, y, p.name.clone()))
        })
        .collect();

    // 内层引脚名:**不画**。
    //
    // 内层是"一个普通节点"(用户原话)—— 普通节点上不写引脚名,类型也是
    // 悬停才看。写出来会和外层的标签打架:两层挨得近,名字叠在一起反而
    // 读不出来。外层的名字已经足够定位每个引脚了。

    Layout {
        outer: outer_rect,
        inner: inner_rect,
        outer_left,
        outer_right,
        inner_left,
        inner_right,
        constants,
        outer_left_labels,
        outer_right_labels,
        left_gutter,
        right_gutter,
    }
}

/// 把内层节点的位置**夹**进外层盒子。
///
/// 拖动时每帧都要过一遍 —— 内层不能被拖出外层。
pub fn clamp_inner(pos: (f32, f32), layout: &Layout) -> (f32, f32) {
    let o = layout.outer;
    let i = layout.inner;
    let x = pos.0.clamp(o.x + 2.0, (o.x + o.w - i.w - 2.0).max(o.x + 2.0));
    let y = pos.1.clamp(o.y + 2.0, (o.y + o.h - i.h - 2.0).max(o.y + 2.0));
    (x, y)
}

/// 拖动时的位置:相对**按下点**算,不是累加每次的移动量。
///
/// `start` 是按下时内层节点的位置(面板局部坐标),`anchor` 是按下时的鼠标
/// 位置,`now` 是现在的鼠标位置。
///
/// ⚠️ **不能累加增量** —— 指针移出窗口再回来时,win32 的 wparam 不带按键
/// 信息(`mouse_up` 收不到),补上的那次 `mouse_move` 带着**整段**位移。
/// 累加实现会把这一整段一次性加到位置上,节点"隔着大老远"被甩出去。
///
/// 相对按下点算就没有这个问题:中间丢多少事件都不影响,位置只取决于
/// "现在鼠标在哪儿"。
pub fn drag_pos(start: (f32, f32), anchor: Point<f32>, now: Point<f32>) -> (f32, f32) {
    (
        start.0 + (now.x - anchor.x),
        start.1 + (now.y - anchor.y),
    )
}

// ------------------------------------------------------------------ 悬停

/// 一个引脚的悬停文案。
///
/// 类型**只在这里出现** —— 引脚旁边只写名字(用户要求)。悬停时给的是
/// `名字 <类型>`,类型为空时只写名字。
pub fn pin_tip(port: &PortInfo) -> String {
    if port.type_label.is_empty() {
        port.name.clone()
    } else {
        format!("{} <{}>", port.name, port.type_label)
    }
}

/// 一条连线的悬停文案:两端各是什么。
///
/// `from` / `to` 是 `(名字, 类型)` —— 两端**一致**就只显示一个类型,不一致
/// 就显示 `A → B`(和主画布的边气泡一个规矩)。
pub fn link_tip(from: &(String, String), to: &(String, String)) -> String {
    let (fn_, ft) = (&from.0, &from.1);
    let (tn, tt) = (&to.0, &to.1);
    match (ft.is_empty(), tt.is_empty()) {
        (true, true) => format!("{fn_} → {tn}"),
        (true, false) => format!("{fn_} → {tn} <{tt}>"),
        (false, true) => format!("{fn_} <{ft}> → {tn}"),
        (false, false) => {
            if ft == tt {
                format!("{fn_} <{ft}> → {tn}")
            } else {
                format!("{fn_} <{ft}> → {tn} <{tt}>")
            }
        }
    }
}

/// 命中半径(面板局部坐标)。这个视图固定 1 倍缩放,不随缩放变。
pub const PIN_HIT_R: f32 = 8.0;
pub const LINK_HIT_R: f32 = 6.0;

/// 一个可悬停的东西。
#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    /// 一个引脚。
    Pin { side: Side, index: usize },
    /// 一条连线。
    Link { index: usize },
}

/// 引脚在哪一侧。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// 外层左边(参数)。
    OuterLeft,
    /// 外层右边(返回值)。
    OuterRight,
    /// 内层左边(当前参数)。
    InnerLeft,
    /// 内层右边(当前返回值)。
    InnerRight,
}

/// 命中测试:离光标最近的引脚 / 连线。
///
/// 引脚优先于连线 —— 引脚是小目标、又压在连线上面(用户要的是"这个引脚
/// 是什么",不是"那条线是什么")。
///
/// 连线用**两端连线**近似贝塞尔:命中测试够用,画的时候仍然是贝塞尔
/// (和主画布的 `view::bezier_dist` 同一种"够用就够"的取舍)。
pub fn hit_test(
    layout: &Layout,
    pins: &[Hit],
    links: &[Link],
    at: Point<f32>,
) -> Option<Hit> {
    // 引脚优先。
    let mut best: Option<(f32, Hit)> = None;
    for h in pins {
        let (x, y) = match pin_pos(layout, h) {
            Some(p) => p,
            None => continue,
        };
        let d = dist(point(x, y), at);
        if d <= PIN_HIT_R && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
            best = Some((d, h.clone()));
        }
    }
    if let Some((_, h)) = best {
        return Some(h);
    }
    // 再找连线。
    let mut best: Option<(f32, Hit)> = None;
    for (i, link) in links.iter().enumerate() {
        let (from, to) = match link_endpoints(layout, link) {
            Some(p) => p,
            None => continue,
        };
        let d = seg_dist(at, from, to);
        if d <= LINK_HIT_R && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
            best = Some((d, Hit::Link { index: i }));
        }
    }
    best.map(|(_, h)| h)
}

/// 一个引脚的位置。
pub fn pin_pos(layout: &Layout, hit: &Hit) -> Option<(f32, f32)> {
    let Hit::Pin { side, index } = hit else {
        return None;
    };
    let pins = match side {
        Side::OuterLeft => &layout.outer_left,
        Side::OuterRight => &layout.outer_right,
        Side::InnerLeft => &layout.inner_left,
        Side::InnerRight => &layout.inner_right,
    };
    pins.value.get(*index).copied()
}

/// 一条连线的两端(直线近似 —— 命中测试够用)。
pub fn link_endpoints(layout: &Layout, link: &Link) -> Option<(Point<f32>, Point<f32>)> {
    // `Pins` 里存的是 `(x, y)` 元组,不是 `Point` —— 统一在这里转。
    let p = |side: Side, i: usize| {
        pin_pos(layout, &Hit::Pin { side, index: i }).map(|(x, y)| point(x, y))
    };
    Some(match *link {
        Link::Param { outer, inner } => (p(Side::OuterLeft, outer)?, p(Side::InnerLeft, inner)?),
        Link::Ret { outer, inner } => (p(Side::InnerRight, inner)?, p(Side::OuterRight, outer)?),
        Link::PassThrough { param, ret } => {
            (p(Side::OuterLeft, param)?, p(Side::OuterRight, ret)?)
        }
        Link::CtrlIn { outer, inner } => (p(Side::OuterLeft, outer)?, p(Side::InnerLeft, inner)?),
        Link::CtrlOut { outer, inner } => {
            (p(Side::InnerRight, inner)?, p(Side::OuterRight, outer)?)
        }
        Link::CtrlThrough { from, to } => (p(Side::OuterLeft, from)?, p(Side::OuterRight, to)?),
    })
}

/// 两点距离。
fn dist(a: Point<f32>, b: Point<f32>) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// 点到线段的最短距离。
fn seg_dist(p: Point<f32>, a: Point<f32>, b: Point<f32>) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 <= f32::EPSILON {
        return dist(p, a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    dist(p, point(a.x + dx * t, a.y + dy * t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::PortInfo;

    fn port(name: &str) -> PortInfo {
        PortInfo {
            name: name.to_string(),
            type_label: "Int".to_string(),
        }
    }

    fn iface(
        params: &[&str],
        rets: &[&str],
        origins: Vec<RetOrigin>,
        ctrl_in: usize,
        ctrl_out: usize,
    ) -> FnInterface {
        FnInterface {
            params: params.iter().map(|n| port(n)).collect(),
            rets: rets.iter().map(|n| port(n)).collect(),
            ctrl_in_num: ctrl_in,
            ctrl_out_num: ctrl_out,
            ret_origins: origins,
        }
    }

    /// 配对按**名字**:优化会压缩引脚、下标会移位,按下标配就错位了。
    #[test]
    fn matching_is_by_name_not_by_index() {
        let outer = vec![port("a"), port("b"), port("c")];
        // 内层只剩 b 和 c —— 但它们的**下标**是 0 和 1(压缩后前移了)。
        let inner = vec![port("b"), port("c")];
        let m = match_by_name(&outer, &inner);
        assert_eq!(m, vec![Some(1), Some(2)], "b 要配到外层的 b,c 要配到外层的 c");
    }

    /// 返回值来自函数体 → 连到**内层**的出引脚。
    #[test]
    fn a_return_from_the_body_links_to_the_inner_out_pin() {
        let outer = iface(&["x"], &["return"], vec![], 1, 1);
        let inner = iface(&["x"], &["return"], vec![RetOrigin::Body], 1, 1);
        let c = Convention::build(&outer, &inner);
        assert!(
            c.links.contains(&Link::Ret {
                outer: 0,
                inner: 0
            }),
            "应当连到内层出引脚:{:?}",
            c.links
        );
        assert!(c.constants.is_empty());
    }

    /// 返回值是某个参数的透传 → 连到**外层**的入引脚。
    #[test]
    fn a_return_passthrough_links_to_the_outer_in_pin() {
        let outer = iface(&["x", "y"], &["return"], vec![], 1, 1);
        let inner = iface(&["x", "y"], &["return"], vec![RetOrigin::Param("y".into())], 1, 1);
        let c = Convention::build(&outer, &inner);
        assert!(
            c.links.contains(&Link::PassThrough {
                param: 1,
                ret: 0
            }),
            "应当连到外层的 y 参数:{:?}",
            c.links
        );
        assert!(
            !c.links
                .iter()
                .any(|l| matches!(l, Link::Ret { .. })),
            "透传不该再算成函数体产出的:{:?}",
            c.links
        );
    }

    /// 返回值是个常数 → 写成常数,画在内侧。
    #[test]
    fn a_constant_return_is_written_on_the_inner_side() {
        let outer = iface(&[], &["return"], vec![], 1, 1);
        let inner = iface(&[], &["return"], vec![RetOrigin::Const("5".into())], 1, 1);
        let c = Convention::build(&outer, &inner);
        assert_eq!(
            c.constants,
            vec![Constant {
                ret: 0,
                text: "5".into(),
                inner: 0
            }]
        );
        assert!(
            !c.links.iter().any(|l| matches!(
                l,
                Link::Ret { .. } | Link::PassThrough { .. }
            )),
            "常数不该有线:{:?}",
            c.links
        );
    }

    /// 当前函数体没有控制流 → 外层的控制流引脚**直接贯穿**。
    #[test]
    fn control_flow_passes_through_when_the_body_has_none() {
        let outer = iface(&[], &[], vec![], 1, 1);
        let inner = iface(&[], &[], vec![], 0, 0);
        let c = Convention::build(&outer, &inner);
        assert!(
            c.links.contains(&Link::CtrlThrough { from: 0, to: 0 }),
            "应当直接贯穿:{:?}",
            c.links
        );
    }

    /// 当前函数体**有**控制流 → 控制流引脚连到内层。
    #[test]
    fn control_flow_links_to_the_inner_layer_when_present() {
        let outer = iface(&[], &[], vec![], 1, 1);
        let inner = iface(&[], &[], vec![], 1, 1);
        let c = Convention::build(&outer, &inner);
        assert!(
            c.links.contains(&Link::CtrlIn {
                outer: 0,
                inner: 0
            }),
            "控制入应当连到内层:{:?}",
            c.links
        );
        assert!(
            c.links.contains(&Link::CtrlOut {
                outer: 0,
                inner: 0
            }),
            "控制出应当连到内层:{:?}",
            c.links
        );
        assert!(
            !c.links
                .iter()
                .any(|l| matches!(l, Link::CtrlThrough { .. })),
            "有控制流时不该贯穿:{:?}",
            c.links
        );
    }

    /// 参数被优化消掉 → 外层那个引脚进"已消掉"列表,不再连线。
    #[test]
    fn a_dropped_param_is_reported() {
        let outer = iface(&["x", "y"], &[], vec![], 1, 1);
        let inner = iface(&["x"], &[], vec![], 1, 1);
        let c = Convention::build(&outer, &inner);
        assert_eq!(c.dropped_params, vec![1], "y 已被消掉");
        assert_eq!(c.dropped_params.len(), 1);
        assert!(
            c.links
                .iter()
                .all(|l| !matches!(l, Link::Param { outer: 1, .. })),
            "消掉的参数不该连线:{:?}",
            c.links
        );
    }

    /// 内层节点拖不出外层盒子。
    #[test]
    fn the_inner_node_cannot_be_dragged_out_of_the_outer_box() {
        let outer = iface(&["x"], &["return"], vec![], 1, 1);
        let inner = iface(&["x"], &["return"], vec![RetOrigin::Body], 1, 1);
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 224.0);
        let (x, y) = clamp_inner((-500.0, -500.0), &l);
        assert!(x >= l.outer.x && y >= l.outer.y, "不能拖到外层外面:{x} {y}");
        let (x2, y2) = clamp_inner((9999.0, 9999.0), &l);
        assert!(
            x2 + l.inner.w <= l.outer.x + l.outer.w + 0.01,
            "右边不能超出:{x2}"
        );
        assert!(
            y2 + l.inner.h <= l.outer.y + l.outer.h + 0.01,
            "下边不能超出:{y2}"
        );
    }

    /// 外层盒子要装得下内层节点 —— 内层得能在里面拖。
    #[test]
    fn the_outer_box_is_tall_enough_for_the_inner_node() {
        let outer = iface(&[], &[], vec![], 1, 1);
        let inner = iface(&["a", "b", "c"], &[], vec![], 0, 0);
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 224.0);
        assert!(
            l.outer.h >= l.inner.h + 2.0 * PAD,
            "外层 {:0.1} 要装得下内层 {:0.1}",
            l.outer.h,
            l.inner.h
        );
    }

    /// **优化 / lower 之后内层要跟着变** —— 用户明确要求的那一条。
    ///
    /// 这里直接模拟"外层冻结、内层被优化改小"这一帧:连线与"掉参"列表
    /// 必须重新算出来,而不是沿用上一帧。
    #[test]
    fn the_inner_layer_follows_the_optimized_state() {
        let outer = iface(&["x", "y"], &["r"], vec![], 1, 1);
        let before = iface(&["x", "y"], &["r"], vec![RetOrigin::Body], 1, 1);
        let c1 = Convention::build(&outer, &before);
        assert_eq!(c1.dropped_params, Vec::<usize>::new(), "优化前两个参数都在");
        assert_eq!(
            c1.links.iter().filter(|l| matches!(l, Link::Param { .. })).count(),
            2
        );

        // 优化把 `y` 消掉、并且返回值改成了透传 `x`。
        let after = iface(&["x"], &["r"], vec![RetOrigin::Param("x".into())], 1, 1);
        let c2 = Convention::build(&outer, &after);
        assert_eq!(c2.dropped_params, vec![1], "y 已被优化消掉");
        assert_eq!(
            c2.links.iter().filter(|l| matches!(l, Link::Param { .. })).count(),
            1,
            "只剩一条参数连线"
        );
        assert!(
            c2.links.contains(&Link::PassThrough { param: 0, ret: 0 }),
            "返回值改成了透传 x:{:?}",
            c2.links
        );
    }

    /// 控制流从"连到内层"变成"直接贯穿"—— 优化把函数体压平之后会这样。
    #[test]
    fn control_flow_can_switch_from_inner_to_passthrough() {
        let outer = iface(&[], &[], vec![], 1, 1);
        let with_body = iface(&[], &[], vec![], 1, 1);
        assert!(
            Convention::build(&outer, &with_body)
                .links
                .iter()
                .any(|l| matches!(l, Link::CtrlIn { .. }))
        );

        let flattened = iface(&[], &[], vec![], 0, 0);
        let c = Convention::build(&outer, &flattened);
        assert!(
            c.links.contains(&Link::CtrlThrough { from: 0, to: 0 }),
            "内层没有控制流了,外层应当直接贯穿:{:?}",
            c.links
        );
    }

    // ------------------------------------------------------- 引脚名在外侧

    /// 引脚名画在盒子**外侧**(用户要求):左边贴盒子左边、右边贴盒子右边。
    ///
    /// 名字**只写名字** —— 类型在悬停气泡里。
    #[test]
    fn pin_names_sit_outside_the_outer_box() {
        let outer = iface(&["alpha", "beta"], &["gamma"], vec![], 1, 1);
        let inner = iface(&["alpha", "beta"], &["gamma"], vec![RetOrigin::Body], 1, 1);
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 344.0);

        assert_eq!(l.outer_left_labels.len(), 2, "两个参数名");
        assert_eq!(l.outer_right_labels.len(), 1, "一个返回值名");

        // 左边:右缘在盒子左边**之外**。
        let (right, _, name) = &l.outer_left_labels[0];
        assert_eq!(name, "alpha");
        assert!(
            *right < l.outer.x,
            "左边的名字应当贴在盒子左边之外:{right} vs {}",
            l.outer.x
        );
        // 右边:左缘在盒子右边**之外**。
        let (left, _, name) = &l.outer_right_labels[0];
        assert_eq!(name, "gamma");
        assert!(
            *left > l.outer.x + l.outer.w,
            "右边的名字应当贴在盒子右边之外:{left}"
        );
    }

    /// 引脚名**不写类型** —— 类型只在悬停气泡里(用户要求)。
    #[test]
    fn pin_names_do_not_show_the_type() {
        let outer = iface(&["alpha"], &[], vec![], 1, 1);
        let inner = iface(&["alpha"], &[], vec![], 1, 1);
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 344.0);
        let (_, _, name) = &l.outer_left_labels[0];
        assert_eq!(name, "alpha", "名字就是名字,不带类型");
        assert!(!name.contains("Int"), "不该带类型:{name}");
    }

    /// 盒子左右各留一条给引脚名的槽 —— 盒子不能占满整个面板宽度。
    ///
    /// 槽宽按**最长的名字**估,不是固定值:名字短就不白留空白(用户报
    /// "左右空余太多")。
    #[test]
    fn the_box_leaves_room_for_the_names() {
        let outer = iface(&["x"], &[], vec![], 1, 1);
        let inner = iface(&["x"], &[], vec![], 1, 1);
        let c = Convention::build(&outer, &inner);
        let width = 344.0;
        let l = layout(&outer, &inner, &c, (0.0, 0.0), width);
        assert_eq!(l.outer.x, MARGIN + l.left_gutter, "左边槽宽就是 left_gutter");
        assert!(
            l.outer.x + l.outer.w <= width - l.right_gutter,
            "右边也要有槽:{} + {} vs {} - {}",
            l.outer.x,
            l.outer.w,
            width,
            l.right_gutter
        );
    }

    /// 槽宽**跟着名字长度走**:短名字窄、长名字宽(有上下限)。
    #[test]
    fn the_gutter_follows_the_longest_name() {
        let short = label_gutter(["x"]);
        // 一个字符 + 内边距,比下限略宽 —— 但**远小于**以前固定的 74px。
        assert!(short < 24.0, "短名字不该白留一大条:{short}");
        let longer = label_gutter(["abc"]);
        assert!(longer > short, "名字变长,槽也变宽:{longer} vs {short}");
        // 封顶:再长也不能把盒子挤没。
        let huge = label_gutter(["x".repeat(200).as_str()]);
        assert_eq!(huge, LABEL_GUTTER_MAX, "超长名字封顶");
        // 取**最长**的那个,不是第一个。
        assert_eq!(
            label_gutter(["a", "a_longer_one"]),
            label_gutter(["a_longer_one"]),
            "按最长的那一个算"
        );
    }

    // --------------------------------------------------------------- 悬停

    /// 悬停引脚 → 气泡给 `名字 <类型>`。
    #[test]
    fn hovering_a_pin_shows_its_type() {
        let p = port("x");
        assert_eq!(pin_tip(&p), "x <Int>");
        let untyped = PortInfo {
            name: "y".into(),
            type_label: String::new(),
        };
        assert_eq!(pin_tip(&untyped), "y", "没类型就只写名字");
    }

    /// 一条连线的悬停文案:两端各是什么。
    #[test]
    fn link_tip_shows_both_ends() {
        assert_eq!(
            link_tip(&("x".into(), "Int".into()), &("y".into(), "Int".into())),
            "x <Int> → y"
        );
        assert_eq!(
            link_tip(&("x".into(), "Int".into()), &("y".into(), "Bool".into())),
            "x <Int> → y <Bool>"
        );
    }

    /// 控制流引脚没有 tip（和主画布一致：`pin_tip` 只给值流）。
    #[test]
    fn control_pins_have_no_tip() {
        let outer = iface(&["x"], &["r"], vec![], 1, 1);
        let inner = iface(&["x"], &["r"], vec![RetOrigin::Body], 1, 1);
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 344.0);
        // 控制流入 / 出都在,但它们的文案由渲染层按 `link_endpoints_text`
        // 判 —— 控制流一律 `None`。这里锁"控制流连线存在"这个前提。
        assert!(
            c.links.iter().any(|x| matches!(x, Link::CtrlIn { .. })),
            "有控制流入连线"
        );
    }

    /// 命中测试:指着引脚就命中那个引脚(不是压在它上面的线)。
    #[test]
    fn hovering_near_a_pin_hits_the_pin() {
        let outer = iface(&["x"], &["r"], vec![], 1, 1);
        let inner = iface(&["x"], &["r"], vec![RetOrigin::Body], 1, 1);
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 344.0);
        let pins = vec![
            Hit::Pin {
                side: Side::OuterLeft,
                index: 0,
            },
            Hit::Pin {
                side: Side::OuterRight,
                index: 0,
            },
        ];
        let (px, py) = pin_pos(&l, &pins[0]).expect("引脚在");
        let hit = hit_test(&l, &pins, &c.links, point(px + 2.0, py + 2.0));
        assert_eq!(hit, Some(pins[0].clone()), "应当命中那个引脚");
        // 离得远 → 不命中。
        assert_eq!(hit_test(&l, &pins, &c.links, point(px + 90.0, py + 90.0)), None);
    }

    /// 命中测试:指着连线就命中那条线。
    #[test]
    fn hovering_a_line_hits_the_line() {
        let outer = iface(&["x"], &["r"], vec![], 1, 1);
        let inner = iface(&["x"], &["r"], vec![RetOrigin::Body], 1, 1);
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 344.0);
        let pins: Vec<Hit> = vec![];
        // 参数连线:外层左 → 内层左。取中点。
        let link = c.links[0];
        let (from, to) = link_endpoints(&l, &link).expect("两端都在");
        let mid = point((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
        let hit = hit_test(&l, &pins, &c.links, mid);
        assert!(
            matches!(hit, Some(Hit::Link { .. })),
            "应当命中连线:{hit:?}"
        );
    }

    /// **内层的引脚也要能命中** —— 内层是"一个普通节点",它的引脚同样是
    /// 引脚,悬停也该给类型。
    #[test]
    fn the_inner_pins_are_hit_too() {
        let outer = iface(&["x"], &["r"], vec![], 1, 1);
        let inner = iface(&["x"], &["r"], vec![RetOrigin::Body], 1, 1);
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 344.0);
        let pins = vec![
            Hit::Pin {
                side: Side::InnerLeft,
                index: 0,
            },
            Hit::Pin {
                side: Side::InnerRight,
                index: 0,
            },
        ];
        let (px, py) = pin_pos(&l, &pins[0]).expect("引脚在");
        let hit = hit_test(&l, &pins, &c.links, point(px + 1.0, py + 1.0));
        assert_eq!(hit, Some(pins[0].clone()), "内层引脚也要能命中");
    }

    // --------------------------------------------------------------- 拖动

    /// 拖动位置**相对按下点**算,不累加每次的移动量。
    ///
    /// 这条锁的是用户报的那个坑:指针移出窗口再回来时,补上的那次
    /// `mouse_move` 带着**整段**位移(win32 的 wparam 在指针离开窗口时
    /// 不带按键信息,`mouse_up` 收不到)。累加的实现会把这一整段一次性
    /// 加到位置上,节点"隔着大老远"被甩出去。
    ///
    /// 相对按下点算就没有这个问题 —— 中间丢多少事件都不影响位置。
    #[test]
    fn dragging_is_relative_to_the_press_not_accumulated() {
        let start = (100.0, 100.0);
        let anchor = point(500.0, 400.0);
        // 鼠标先移一小段,再"移出窗口又回来"(中间那次带着整段位移)。
        let mid = point(510.0, 405.0);
        let back = point(90.0, 300.0);

        // 第一次:相对按下点 10,5。
        assert_eq!(
            drag_pos(start, anchor, mid),
            (110.0, 105.0),
            "第一次移动只走 10,5"
        );
        // 第二次:相对按下点 -410,-100 —— **不是** 第一次的量再加上这一段。
        assert_eq!(
            drag_pos(start, anchor, back),
            (-310.0, 0.0),
            "位置只取决于现在鼠标在哪,不累加"
        );
    }

    /// 拖动**夹**在外层盒子里面 —— 拖不出去,但也**不会**因为夹紧而
    /// 在下一次移动时把夹掉的量又加回来。
    ///
    /// 这是"相对按下点"与"累加"的第二个区别:累加实现里,节点贴到左墙
    /// 之后,下一次往右拖会从"墙上的位置"再加增量,等于把之前被夹掉的
    /// 那段距离还回来 —— 节点会"粘"在墙上,松手才跳回去。
    #[test]
    fn a_pinned_inner_node_does_not_drift_when_dragged_back() {
        let outer = iface(&["x"], &["r"], vec![], 1, 1);
        let inner = iface(&["x"], &["r"], vec![RetOrigin::Body], 1, 1);
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 344.0);

        // 按下时节点已经在左墙边上。
        let pinned = conv_clamp(&l);
        let anchor = point(600.0, 400.0);
        // 往左拖 50px:被夹住,位置不变。
        let left = drag_pos(pinned, anchor, point(550.0, 400.0));
        assert_eq!(clamp_inner(left, &l), pinned, "拖到墙外被夹住");
        // 往右拖 20px:相对按下点算,应该真的往右挪 20px(而不是纹丝不动)。
        let right = drag_pos(pinned, anchor, point(620.0, 400.0));
        assert!(
            clamp_inner(right, &l).0 > pinned.0,
            "往回拖要能真的动:{:?} vs {:?}",
            clamp_inner(right, &l),
            pinned
        );
    }

    /// 把内层节点放到外层盒子的左墙边上(贴着夹紧边界)。
    fn conv_clamp(l: &Layout) -> (f32, f32) {
        (l.outer.x + 2.0, l.outer.y + 2.0)
    }

    /// **引脚的排布必须和画布那套一样** —— 两边各算一份的话,同一个函数
    /// 在画布和调用约定里会画得不一样。
    ///
    /// 这里逐个比对:同样的控制流/值流数量,调用约定的引脚纵坐标必须和
    /// core 的 `pin_row_y` 算出来的**逐一相等**,而且整排**靠上**(不是居中)。
    #[test]
    fn the_pin_positions_match_the_canvas() {
        use crate::doc::pin_row_y;

        // 外层左边:1 个控制流入 + 2 个值流入。
        let outer = iface(&["a", "b"], &["c", "d"], vec![], 1, 1);
        let inner = iface(&["a", "b"], &["c"], vec![RetOrigin::Body], 1, 1);
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 344.0);

        let top = l.outer.y + PAD;
        assert_eq!(l.outer_left.ctrl.len(), 1, "一个控制流入");
        assert_eq!(l.outer_left.value.len(), 2, "两个值流入");
        assert_eq!(
            l.outer_left.ctrl[0].1,
            top + pin_row_y(0),
            "控制流首个引脚在首行(靠上)"
        );
        assert_eq!(
            l.outer_left.value[0].1,
            top + pin_row_y(1),
            "值流紧接控制流之后"
        );
        // 值流排在控制流**下方** —— 和画布、和游戏内编辑器一个规矩。
        assert!(l.outer_left.value[0].1 > l.outer_left.ctrl[0].1);
        // **靠上**:首个引脚不该浮在盒子中间。
        assert!(
            l.outer_left.ctrl[0].1 < l.outer.y + l.outer.h / 2.0,
            "引脚应当靠上排,不是居中"
        );
    }

    /// **内层的每个引脚都必须落在内层节点框里** —— 用户报"数据流引脚总有
    /// 超出节点范围的"。
    ///
    /// 根因是内层用了画布的 `node_height(rows)`(它假定引脚从节点顶部
    /// `PIN_TOP` 开始),却又给引脚额外加了 `PAD` 偏移 —— 整排下移,末行
    /// 就跑出框外。内层节点的高度和引脚位置**必须来自同一套几何**。
    #[test]
    fn every_inner_pin_stays_inside_the_inner_node() {
        // 参数多、返回值也多 —— 行数拉满,最容易露馅。
        let outer = iface(&["a", "b", "c"], &["r1", "r2"], vec![], 1, 1);
        let inner = iface(
            &["a", "b", "c"],
            &["r1", "r2"],
            vec![RetOrigin::Body, RetOrigin::Const("0".into())],
            1,
            1,
        );
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 344.0);

        let top = l.inner.y;
        let bottom = l.inner.y + l.inner.h;
        for (side, pins) in [
            ("左", &l.inner_left),
            ("右", &l.inner_right),
        ] {
            for &(_, y) in pins.ctrl.iter().chain(pins.value.iter()) {
                assert!(
                    y > top && y < bottom,
                    "{side}侧引脚 y={y} 超出内层节点 [{top}, {bottom}]"
                );
            }
        }
    }

    ///
    /// 内层节点是不透明的,引脚必须画在它**之后**才不会被遮住 ——
    /// 那是渲染层的层级问题(见 `render_conv_panel`),这里锁的是几何:
    /// 引脚的 x 必须正好落在节点盒子的左右边缘上。
    #[test]
    fn the_inner_pins_sit_on_the_node_edges() {
        let outer = iface(&["a"], &["c"], vec![], 1, 1);
        let inner = iface(&["a"], &["c"], vec![RetOrigin::Body], 1, 1);
        let c = Convention::build(&outer, &inner);
        let l = layout(&outer, &inner, &c, (0.0, 0.0), 344.0);
        assert_eq!(l.inner_left.value[0].0, l.inner.x, "内层左引脚贴在左边");
        assert_eq!(
            l.inner_right.value[0].0,
            l.inner.x + l.inner.w,
            "内层右引脚贴在右边"
        );
    }

    /// **引脚必须画在盒子之后** —— 用户报"层级不对,引脚被自己遮挡了"。
    ///
    /// 内层节点是不透明的(`node_bg()`),引脚先画就被它盖住。
    /// 这条锁住绘制顺序:引脚排在两个盒子**之后**。
    #[test]
    fn the_pins_are_drawn_after_the_boxes() {
        let idx = |l: Layer| LAYER_ORDER.iter().position(|x| *x == l).expect("在顺序表里");
        assert!(
            idx(Layer::Pins) > idx(Layer::InnerBox),
            "引脚必须画在内层节点之后,不然被它盖住"
        );
        assert!(
            idx(Layer::Pins) > idx(Layer::OuterBox),
            "引脚必须画在外层盒子之后"
        );
        assert!(idx(Layer::Edges) < idx(Layer::OuterBox), "边在最底下");
        assert!(idx(Layer::Hover) > idx(Layer::Pins), "悬停气泡在最上面");
        assert!(idx(Layer::Labels) > idx(Layer::Pins), "文字盖在引脚上");
    }
}
