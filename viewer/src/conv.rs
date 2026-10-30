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
    /// 常数文本:右对齐到外层出引脚左侧的 `(right_x, y)` + 文本。
    pub constants: Vec<(f32, f32, String)>,
}

/// 外层盒子左右各留的边距。
pub const PAD: f32 = 10.0;
/// 一行引脚占的高度。
pub const PIN_ROW: f32 = 14.0;
/// 内层节点的宽度。
pub const INNER_W: f32 = 120.0;
/// 内层节点的最小高度。
pub const INNER_MIN_H: f32 = 28.0;
/// 外层盒子的最小高度。
pub const OUTER_MIN_H: f32 = 84.0;

/// 把 `n` 个引脚均分在 `[lo, lo+len]` 上,首尾各留一格。
fn spread(lo: f32, len: f32, n: usize) -> Vec<f32> {
    if n == 0 {
        return Vec::new();
    }
    (0..n)
        .map(|i| lo + len * (i as f32 + 1.0) / (n as f32 + 1.0))
        .collect()
}

/// 一侧引脚的排布:控制流在上、值流在下,两者在整条边上均分。
fn pins_on_side(x: f32, lo: f32, len: f32, ctrl_n: usize, value_n: usize) -> Pins {
    let ys = spread(lo, len, ctrl_n + value_n);
    Pins {
        ctrl: ys[..ctrl_n].iter().map(|&y| (x, y)).collect(),
        value: ys[ctrl_n..].iter().map(|&y| (x, y)).collect(),
    }
}

/// 算几何。
///
/// `inner_pos` 是内层节点的左上角(面板局部坐标)—— 用户拖出来的位置。
/// 它会被**夹**在外层盒子里面:拖不出外层。
pub fn layout(
    outer: &FnInterface,
    inner: &FnInterface,
    conv: &Convention,
    inner_pos: (f32, f32),
    width: f32,
) -> Layout {
    // 内层节点的高:按它自己两侧最多的引脚数。
    let inner_rows = (inner.params.len() + inner.ctrl_in_num)
        .max(inner.rets.len() + inner.ctrl_out_num)
        .max(1);
    let inner_h = (inner_rows as f32 * PIN_ROW).max(INNER_MIN_H);

    // 外层盒子的高:装下**两侧**的引脚,也要装得下内层节点(内层要在里面拖)。
    let outer_rows = (outer.params.len() + outer.ctrl_in_num)
        .max(outer.rets.len() + outer.ctrl_out_num)
        .max(1);
    let outer_h = (outer_rows as f32 * PIN_ROW + 2.0 * PAD)
        .max(OUTER_MIN_H)
        .max(inner_h + 2.0 * PAD);

    let outer_rect = Rect {
        x: 0.0,
        y: 0.0,
        w: width,
        h: outer_h,
    };

    // 内层节点的左上角:夹在外层里面(留 2px 余量)。
    let (mut ix, mut iy) = inner_pos;
    ix = ix.clamp(outer_rect.x + 2.0, (outer_rect.x + outer_rect.w - INNER_W - 2.0).max(outer_rect.x + 2.0));
    iy = iy.clamp(outer_rect.y + 2.0, (outer_rect.y + outer_rect.h - inner_h - 2.0).max(outer_rect.y + 2.0));
    let inner_rect = Rect {
        x: ix,
        y: iy,
        w: INNER_W,
        h: inner_h,
    };

    let outer_left = pins_on_side(
        outer_rect.x,
        outer_rect.y,
        outer_rect.h,
        outer.ctrl_in_num,
        outer.params.len(),
    );
    let outer_right = pins_on_side(
        outer_rect.x + outer_rect.w,
        outer_rect.y,
        outer_rect.h,
        outer.ctrl_out_num,
        outer.rets.len(),
    );
    let inner_left = pins_on_side(
        inner_rect.x,
        inner_rect.y,
        inner_rect.h,
        inner.ctrl_in_num,
        inner.params.len(),
    );
    let inner_right = pins_on_side(
        inner_rect.x + inner_rect.w,
        inner_rect.y,
        inner_rect.h,
        inner.ctrl_out_num,
        inner.rets.len(),
    );

    // 常数文本:画在**内层节点右侧**、纵向对准内层那个出引脚 —— 常数是
    // 函数体里的事实,位置跟着内层走。
    let constants = conv
        .constants
        .iter()
        .filter_map(|c| {
            let y = inner_right.value.get(c.inner).map(|p| p.1)?;
            Some((inner_rect.x + inner_rect.w + 6.0, y, c.text.clone()))
        })
        .collect();

    Layout {
        outer: outer_rect,
        inner: inner_rect,
        outer_left,
        outer_right,
        inner_left,
        inner_right,
        constants,
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
}
