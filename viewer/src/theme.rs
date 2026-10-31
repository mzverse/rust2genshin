//! 配色与尺寸常量 —— 界面的「视觉规格」集中在这里。
//!
//! 之前这些散在 `view.rs` 顶部(20 个颜色函数 + 10 个尺寸常量),和渲染逻辑
//! 混在一起。拆出来之后:
//!
//! - 想调主题只改这一个文件;
//! - 别的模块(比如 `conv.rs` 的面板)也能用同一套配色,不必各自复制一份;
//! - `view.rs` 里剩下的就是**行为**。
//!
//! # 为什么颜色是函数而不是常量
//!
//! gpui 的 `rgba` / `hsla` **不是** `const fn`(见 gpui 0.2.2 的
//! `src/color.rs:20` / `:336`),没法写成 `const Rgba`。而它们只是四个
//! 字节除以 255 —— 每次调用构造一次的开销可以忽略,不值得为此套一层
//! `OnceLock`。所以保持函数形式。

use gpui::Rgba;
use gpui::{hsla, rgba};

// ------------------------------------------------------------------ 底色

/// 画布底色。
pub fn canvas_bg() -> Rgba {
    rgba(0x11141aff)
}

/// 左栏底色。比画布**亮一档**,既和画布分开、也让字有足够对比度。
/// 之前用 0x161a22 太暗,字和底色几乎糊在一起。
pub fn panel_bg() -> Rgba {
    rgba(0x232a36ff)
}

/// 左栏分隔条的底色。**不透明** —— 之前没设背景,空隙里透出的是窗口底色,
/// 看起来像左栏裂了一道缝。
pub fn divider_bg() -> Rgba {
    rgba(0x2c3442ff)
}

/// 分隔条悬停时的底色(和按下拖动的反馈)。
pub fn divider_hover_bg() -> Rgba {
    rgba(0x3d4a5eff)
}

/// 节点底色。
pub fn node_bg() -> Rgba {
    rgba(0x1b2130ff)
}

/// 注释框底色。比节点淡一档,读起来是「备注」而不是另一个节点。
pub fn comment_bg() -> Rgba {
    rgba(0x1b2130cc)
}

/// 注释框边框。
pub fn comment_border() -> Rgba {
    rgba(0x8a93a655)
}

// ------------------------------------------------------------------ 连线与引脚

/// 控制流**边**的颜色。
pub fn edge_ctrl() -> Rgba {
    rgba(0x7aa2f7c0)
}

/// 控制流引脚(三角形)的填充色。比 [`edge_ctrl`]**更深、不透明** ——
/// 三角形就压在节点的边缘和浅蓝的控制流线上,用同一个色会和线糊在一起,
/// 分不出哪是端点哪是线。
pub fn pin_ctrl() -> Rgba {
    rgba(0x3b5bdbff)
}

/// 值流**边**的颜色(和值流引脚同色系)。
pub fn edge_value() -> Rgba {
    rgba(0xe0af6888)
}

// ------------------------------------------------------------------ 文字

/// 主要文字色。
pub fn text_hi() -> Rgba {
    rgba(0xe6e9f0ff)
}

/// 次要文字色(引脚名、说明文字)。
pub fn text_dim() -> Rgba {
    rgba(0x8a93a6ff)
}

// ------------------------------------------------------------------ 强调

/// 选中节点的描边色。亮黄 —— 和各类节点自己的颜色都区分得开。
pub fn selected_border() -> Rgba {
    rgba(0xffd866ff)
}

/// 按住 Ctrl 悬停复合节点时的描边色。青色 —— 提示"点一下能跳进定义"。
pub fn link_hint_border() -> Rgba {
    rgba(0x4ec9b0ff)
}

/// 框选矩形的边框 / 底色。
pub fn marquee_border() -> Rgba {
    rgba(0x7aa2f7ff)
}

/// 框选矩形的底色。
pub fn marquee_bg() -> Rgba {
    rgba(0x7aa2f733)
}

// ------------------------------------------------------------------ 按钮

/// 右栏按钮的**禁用**底色:比可用的暗一档,读起来像"按不动"。
pub fn button_disabled_bg() -> Rgba {
    rgba(0xffffff08)
}

/// 右栏按钮**禁用**时的文字色。
pub fn button_disabled_text() -> Rgba {
    rgba(0x8a93a680)
}

/// 节点类别色(描边用)。
///
/// 配色**必须**按类别区分 —— 这是可读性要求,和标签是两回事:
/// 标签只特判两个带载荷的变体(见 `doc::label`),这里必须穷尽八个。
pub fn node_color(kind: &rust2genshin::compile::ir::IrNodeId) -> Rgba {
    use rust2genshin::compile::ir::IrNodeId;
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

// ------------------------------------------------------------------ 尺寸

/// 右栏(优化按钮 + 调用约定)的宽度(px)。
pub const RULES_WIDTH: f32 = 360.0;

/// 「调用约定」面板的内边距。
pub const CONV_PAD: f32 = 8.0;

/// 缩放下限。再小节点上就是一个点,看不清。
pub const MIN_SCALE: f32 = 0.2;

/// 缩放上限。再大单个节点糊满整屏,也看不清边。
pub const MAX_SCALE: f32 = 4.0;

/// 一格滚轮的缩放倍率。
pub const ZOOM_STEP: f32 = 1.1;

/// 左栏默认宽度(px)。
pub const TOC_WIDTH: f32 = 240.0;

/// 左栏宽度下限:再窄连函数名都放不下。
pub const TOC_MIN_WIDTH: f32 = 160.0;

/// 左栏宽度上限。
pub const TOC_MAX_WIDTH: f32 = 720.0;

/// 左栏右侧分隔条的宽度(px)。
pub const TOC_DIVIDER_WIDTH: f32 = 5.0;
