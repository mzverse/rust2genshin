#![feature(rustc_private)]

//! `ogia-viewer` —— 浏览 `.ogia` / `.rlib` 里的节点图 IR。
//!
//! 本体在 lib(`rust2genshin_viewer`),这个 bin 只负责开窗口和解析命令行。
//!
//! # `ogia-viewer <路径>`
//!
//! 启动时直接加载一个 `.ogia` / `.rlib`,不走拖放。
//!
//! **为什么要有这个入口**:gpui 0.2.2 的 Windows 后端在 `DragEnter` 里只认
//! `CF_HDROP` 这一种剪贴板文件格式(`platform/windows/window.rs` 的
//! `WindowsDragDropHandler_Impl::DragEnter`),拿不到就把 `pdweffect` 设成
//! `DROPEFFECT_NONE`,Windows 于是显示"禁止"光标。拖放能不能用是**平台**的问题,
//! 与本工具的解析渲染无关 —— 命令行入口把这两件事隔离开。
//!
//! # `ogia-viewer --emit-fixture <路径>`
//!
//! 生成一个合成 `.ogia` 夹具后退出。
//!
//! 真实数据已经有了 —— `cargo run -p build-demo` 会在
//! `target/genshin-unknown-server/release/` 下产出 `librust2genshin_demo.rlib`,
//! 里面就带 `rust2genshin_demo.ogia` entry。所以这个夹具只是给**测试**用的
//! 确定性输入,不是"没有真数据可用时的替代品"。

pub mod doc;
pub mod optimize;
pub mod view;

use std::path::PathBuf;

use gpui::AppContext as _;
use gpui::{App, Bounds, KeyBinding, WindowBounds, WindowOptions, px, size};

use view::{Undo, Viewer};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let arg = match args.next() {
        None => {
            run_window(None);
            return;
        }
        Some(a) => a,
    };

    if arg == "--check" {
        // 把文件解析的结果打到 stdout,不开窗口。排查"拖进去显示 X"这类问题时,
        // 先用它确认到底是文件里没有图,还是图里没有节点。
        let Some(path) = args.next().map(PathBuf::from) else {
            eprintln!("用法: ogia-viewer --check <路径>");
            std::process::exit(2);
        };
        match doc::load(&path) {
            Err(e) => {
                println!("解析失败: {e}");
                std::process::exit(1);
            }
            Ok(t) => {
                println!(
                    "main: {}, functions: {} 个, adts: {} 个",
                    if t.main.is_some() { "有" } else { "无" },
                    t.functions.len(),
                    t.adts.len()
                );
                let d = doc::flatten(t);
                println!("展平出 {} 张图", d.graphs.len());
                for g in &d.graphs {
                    println!(
                        "  {:<24} 节点 {:>4}  边 {:>4}(控制 {:>3} 值 {:>3})  入口 {:?}  返回 {:?}",
                        g.name,
                        g.nodes.len(),
                        g.edges.len(),
                        g.edges.iter().filter(|e| e.ctrl).count(),
                        g.edges.iter().filter(|e| !e.ctrl).count(),
                        g.entry,
                        g.exit,
                    );
                }
            }
        }
        return;
    }

    if arg == "--emit-fixture" {
        let Some(path) = args.next().map(PathBuf::from) else {
            eprintln!("用法: ogia-viewer --emit-fixture <输出路径>");
            std::process::exit(2);
        };
        if let Err(e) = doc::fixture::write_ogia(&path) {
            eprintln!("写出夹具失败 {}: {e}", path.display());
            std::process::exit(1);
        }
        println!("已写出夹具: {}", path.display());
        return;
    }

    // 其余参数当作要打开的文件。
    run_window(Some(PathBuf::from(arg)));
}

/// 开窗(唯一一条真的建 GPUI 应用的路径;`--check` / `--emit-fixture` 不经这里)。
fn run_window(initial: Option<PathBuf>) {
    gpui::Application::new().run(move |cx| {
        // Ctrl+Z → `view::Undo`。context 为 `None` ⇒ 不看焦点 / 上下文,全局生效;
        // 处理挂在 App 级(见 [`open_window`] 里的 `cx.on_action`)。
        cx.bind_keys([KeyBinding::new("ctrl-z", Undo, None)]);
        open_window(cx, initial);
    });
}

fn open_window(cx: &mut App, initial: Option<PathBuf>) {
    // gpui 0.2.2 的 `WindowOptions` **没有** `title` 字段 ——
    // 标题在 `titlebar: Option<TitlebarOptions>` 里。
    let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
    // `open_window` 返回 `Result`,这里 `expect` 的是"窗口开不出来"(显示器没了之类);
    // .ogia 解析失败走的是 UI 上的错误文案,不是这里。
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("ogia viewer".into()),
                ..Default::default()
            }),
            window_min_size: Some(size(px(640.0), px(480.0))),
            ..Default::default()
        },
        |_window, cx| {
            let viewer = cx.new(move |cx| {
                let mut v = Viewer::new();
                if let Some(p) = &initial {
                    // 解析失败不 panic,走 UI 上的错误文案(spec 错误处理章)。
                    match doc::load(p) {
                        Ok(t) => v.add_target(t, cx),
                        Err(e) => v.error = Some(format!("{}: {e}", p.display())),
                    }
                }
                v
            });
            // Ctrl+Z 的处理挂在 **App 级**:窗口里没有任何元素拿焦点
            // (节点是普通 div,没有 focus),动作冒泡到最后只有这里能兜住。
            let weak = viewer.downgrade();
            cx.on_action(move |_: &Undo, cx: &mut App| {
                if let Some(v) = weak.upgrade() {
                    // App 的 `update` 是必成(`C::Result<()> == ()`),不用处理错误。
                    v.update(cx, |v, cx| v.undo_step(cx));
                }
            });
            viewer
        },
    )
    .expect("打开窗口失败");
}
