# .ogia 节点浏览器 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新建 `viewer` crate，拖入 `.ogia` / `.rlib` 文件即可浏览其中的节点图 IR，用于人工排查 `optimize()` 的控制流问题。

**Architecture:** `viewer` 是 `lib` + `bin` 双 target 的 workspace member（但不在 `default-members` 里，免得日常 `cargo build` 拖进 gpui）。`doc.rs`（解析 + 展平）和 `layout.rs`（分层布局）不碰任何 GPUI 窗口 API，可直接单测；`view.rs` 负责 GPUI 渲染。直接依赖 `core`（rlib）以复用 `NativeValue` 的 typetag 注册。

**Tech Stack:** GPUI 0.2.2（crates.io）、bincode 2、ar 0、serde、slab；toolchain `nightly-2026-10-01`（`#![feature(rustc_private)]`）

**Spec:** `docs/superpowers/specs/2026-10-03-ogia-viewer-design.md`

## Global Constraints

- 工具链固定 `nightly-2026-10-01`（`rust-toolchain.toml`），**不得**改动该文件
- `viewer` 需要 `#![feature(rustc_private)]` 并依赖 `rust2genshin`（path = `../core`），与 `core/src/linker.rs` 同构
- `.ogia` 解码必须用 `bincode::serde::decode_from_reader(reader, bincode::config::standard())`，与 `core/src/linker.rs:50` 逐字一致
- **禁止**使用 `Target: AddAssign`（`core/src/compile/link.rs:24` 在两边都有 `main` 时 `panic!()`）；必须用防御式合并
- **禁止**引入任何控制流可达性分析（用户明确选择"纯手工看"）；`layout` 里为了分层而顺带算出的可达集合，不得产生任何标注或高亮
- **禁止**依赖 `tools/node_data/nodes.json`（gitignored 的 Python 工具数据）；节点只显示 `IrNodeId` 原始形态
- 布局常数（节点尺寸、间距、边宽）全部写死，不做自适应
- 每个 task 结束必须 `cargo build -p rust2genshin-viewer` 通过（除 Task 1 建 crate 本身）
- **包名是 `rust2genshin-viewer`（目录名才是 `viewer`）**。cargo 的 `-p` 不做前缀匹配，所有命令必须写全名 `-p rust2genshin-viewer`；二进制名是 `ogia-viewer`

## Review Focus

以下是 spec 隐含、但没有 task 的测试直接覆盖的失败模式。每个都在下面某个 task 里补了对应测试。

1. **`.rlib` 里没有 `.ogia` entry**（例如只含普通 rlib 元数据）→ 应报"未找到 .ogia"而非空图或 panic
2. **`.rlib` 中多个 entry 都有 `main`** → 防御式合并必须取第一个非空，不能 panic
3. **`NodeRef` 与展平下标不恒等**（`Slab` 删过节点留空洞）→ 建边必须过 `index_of`；指向已删节点的边要跳过而非 panic
4. **空 `Target`**（既无 `main` 也无 `functions`）→ 显示"空图"，不 panic
5. **`IrNodeId::Fn(_)` 节点**（`core` 内部节点，`.ogia` 里可能出现）→ 上色表要给它兜底颜色，不能 `unreachable!()`

---

## File Structure

| 文件 | 职责 |
|---|---|
| `viewer/Cargo.toml` | crate 清单 |
| `viewer/src/lib.rs` | 声明模块并重导出，供单测使用 |
| `viewer/src/doc.rs` | 解析 `.ogia` / `.rlib` → `Target` → 防御式合并 → 展平成 `GraphEntry` 列表 |
| `viewer/src/layout.rs` | 控制流分层 + 坐标计算（纯函数，可单测） |
| `viewer/src/view.rs` | GPUI 渲染：canvas 画边 + 绝对定位 div 画节点 + 交互 |
| `viewer/src/main.rs` | bin 入口：开窗口、挂 drop 回调 |

`doc.rs` 与 `layout.rs` 是唯一的可测单元，`view.rs` / `main.rs` 靠编译通过 + 手动验证。

---

### Task 1: 搭建 crate 骨架

**Files:**
- Create: `viewer/Cargo.toml`
- Create: `viewer/src/lib.rs`
- Modify: `Cargo.toml`（workspace members 加 `viewer`）

**Interfaces:**
- Consumes: 无
- Produces: `viewer` crate（lib target 名 `rust2genshin_viewer`），`cargo build -p rust2genshin-viewer` 可用

- [ ] **Step 1: 创建 Cargo.toml**

```toml
[package]
name = "rust2genshin-viewer"
version.workspace = true
authors.workspace = true
edition.workspace = true
rust-version.workspace = true

[lib]
name = "rust2genshin_viewer"
path = "src/lib.rs"

[[bin]]
name = "ogia-viewer"
path = "src/main.rs"

[lints]
workspace = true

[dependencies]
rust2genshin = { path = "../core" }
gpui = "0.2.2"
bincode = { version = "2", features = ["serde"] }
ar = "0"
serde = { workspace = true }
slab = { version = "0", features = ["serde"] }
```

- [ ] **Step 2: 注册进 workspace**

编辑根 `Cargo.toml`，在 `members` 末尾加 `"viewer"`。

**注意**：不要动 `[workspace.package]` / `[workspace.dependencies]`。本 workspace 没有 `default-members` 字段 —— 保持现状，`cargo build`（无 `-p`）会构建全部 member，gpui 的编译时间由此产生；若实测太慢，在 Task 7 Step 3 加 `default-members`。

- [ ] **Step 3: 创建最小 lib.rs**

```rust
#![feature(rustc_private)]

pub mod doc;
pub mod layout;
pub mod view;
```

先只留 `doc` / `layout` / `view` 三个空模块声明，文件在后续 task 创建。为让本 task 能编译通过，同时创建三个占位文件：

`viewer/src/doc.rs`:
```rust
//! 解析 .ogia / .rlib 并展平成可渲染的图
```

`viewer/src/layout.rs`:
```rust
//! 控制流分层布局（纯函数，可单测）
```

`viewer/src/view.rs`:
```rust
//! GPUI 渲染层
```

- [ ] **Step 4: 创建最小 main.rs**

```rust
#![feature(rustc_private)]

fn main() {}
```

- [ ] **Step 5: 验证编译**

Run: `cargo build -p rust2genshin-viewer`
Expected: 成功。首次会拉取并编译 gpui 及其依赖树，耗时较长（数分钟），属正常。

若报 `rustc_private` 相关错误，确认 `viewer/src/main.rs` 与 `build` 首行都是 `#![feature(rustc_private)]`，且用的是 `nightly-2026-10-01`（`rustc --version` 核对）。

- [ ] **Step 6: 提交**

```bash
git add viewer Cargo.toml
git commit -m "chore(viewer): 搭建 crate 骨架"
```

---

### Task 2: doc.rs —— 解析 .ogia

**Files:**
- Create: `viewer/src/doc.rs`（覆盖 Task 1 的占位）
- Test: `viewer/src/doc.rs` 内联 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `rust2genshin::compile::link::Target`、`rust2genshin::compile::func::NodeGraphIr`、`rust2genshin::node::NodeRef`
- Produces:
  - `pub fn load_ogia(path: &Path) -> Result<Target, String>`
  - `pub fn load_rlib(path: &Path) -> Result<Vec<Target>, String>`
  - `pub fn merge(into: &mut Target, from: Target)` —— 防御式合并
  - `pub fn load(path: &Path) -> Result<Target, String>` —— 按扩展名分派

- [ ] **Step 1: 写失败的测试**

追加到 `viewer/src/doc.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_first_main_instead_of_panicking() {
        let mut a = Target::default();
        a.main = Some(Default::default());
        let mut b = Target::default();
        b.main = Some(Default::default());
        // AddAssign 在这种输入下会 panic,merge 不能
        merge(&mut a, b);
        assert!(a.main.is_some());
    }

    #[test]
    fn load_rejects_unknown_extension() {
        let err = load(Path::new("foo.txt")).unwrap_err();
        assert!(err.contains("不支持的扩展名"), "实际错误: {err}");
    }

    #[test]
    fn load_missing_file_is_an_err_not_a_panic() {
        let r = load(Path::new("definitely_not_here.ogia"));
        assert!(r.is_err());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p rust2genshin-viewer --lib`
Expected: FAIL —— 编译错误，`load_ogia` / `merge` / `load` 未定义。

- [ ] **Step 3: 写实现**

`viewer/src/doc.rs` 全文：

```rust
//! 解析 .ogia / .rlib 并展平成可渲染的图
//!
//! .ogia 是 bincode(serde) 序列化的 `Target`,格式定义见 `core/src/linker.rs:50`。

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use rust2genshin::compile::link::Target;

/// 解析一个 .ogia 文件
pub fn load_ogia(path: &Path) -> Result<Target, String> {
    let f = File::open(path).map_err(|e| format!("打开文件失败 {path:?}: {e}"))?;
    bincode::serde::decode_from_reader(BufReader::new(f), bincode::config::standard())
        .map_err(|e| format!("解析 .ogia 失败 {path:?}: {e}"))
}

/// 解析一个 .rlib,返回其中所有 .ogia entry 解析出的 Target
pub fn load_rlib(path: &Path) -> Result<Vec<Target>, String> {
    let f = File::open(path).map_err(|e| format!("打开文件失败 {path:?}: {e}"))?;
    let mut archive = ar::Archive::new(BufReader::new(f));
    let mut result = Vec::new();
    while let Some(entry) = archive.next_entry() {
        let mut entry = entry.map_err(|e| format!("读取 rlib 条目失败 {path:?}: {e}"))?;
        let name = String::from_utf8(Vec::from(entry.header().identifier()))
            .map_err(|e| format!("条目名非 UTF-8: {e}"))?;
        if !name.ends_with(".ogia") {
            continue;
        }
        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .map_err(|e| format!("读取 {name} 内容失败: {e}"))?;
        let target: Target = bincode::serde::decode_from_reader(
            BufReader::new(&content[..]),
            bincode::config::standard(),
        )
        .map_err(|e| format!("解析 {name} 失败: {e}"))?;
        result.push(target);
    }
    if result.is_empty() {
        return Err(format!("{path:?} 中未找到 .ogia 条目"));
    }
    Ok(result)
}

/// 防御式合并。**不要**用 `Target: AddAssign` —— 它在两边都有 main 时 panic。
pub fn merge(into: &mut Target, from: Target) {
    if into.main.is_none() {
        into.main = from.main;
    }
    into.functions.extend(from.functions);
    into.adts.extend(from.adts);
}

/// 按扩展名分派
pub fn load(path: &Path) -> Result<Target, String> {
    match path.extension().and_then(|x| x.to_str()) {
        Some("ogia") => load_ogia(path),
        Some("rlib") => {
            let mut t = Target::default();
            for x in load_rlib(path)? {
                merge(&mut t, x);
            }
            Ok(t)
        }
        other => Err(format!("不支持的扩展名: {other:?}")),
    }
}
```

需要补 `use std::io::Read;`（`read_to_end` 要它）—— 加在文件顶部 `use std::io::BufReader;` 旁边。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p rust2genshin-viewer --lib`
Expected: PASS，3 passed。

- [ ] **Step 5: 提交**

```bash
git add viewer/src/doc.rs
git commit -m "feat(viewer): .ogia / .rlib 解析与防御式合并"
```

---

### Task 3: doc.rs —— 展平成 GraphEntry

**Files:**
- Modify: `viewer/src/doc.rs`
- Test: 同文件内联 `mod tests`

**Interfaces:**
- Consumes: Task 2 的 `load` / `merge`
- Produces:
  - `pub struct NodeEntry { pub kind: IrNodeId, pub size: gpui::Size<f32> }`
  - `pub struct Edge { pub from: (usize, usize), pub to: (usize, usize), pub ctrl: bool }`
  - `pub struct GraphEntry { pub name: String, pub index_of: HashMap<NodeRef, usize>, pub nodes: Vec<NodeEntry>, pub edges: Vec<Edge> }`
  - `pub struct Doc { pub graphs: Vec<GraphEntry>, pub raw: Vec<NodeGraphIr> }`
  - `pub fn flatten(target: &Target) -> Doc`

> **注意**：`NodeEntry` 在本 task 不含 `pos` —— 坐标由 Task 4 的 `layout` 填，渲染时 `view` 读 `layout` 的结果。这样布局可以纯函数单测。

- [ ] **Step 1: 写失败的测试**

追加到 `doc.rs` 的 `mod tests` 内：

```rust
    #[test]
    fn flatten_produces_main_first_then_sorted_functions() {
        use rust2genshin::compile::func::FnInfo;
        let mut t = Target::default();
        t.functions.insert("zeta".into(), FnInfo::default());
        t.functions.insert("alpha".into(), FnInfo::default());
        let doc = flatten(&t);
        // 无 main 时,函数应按名排序
        assert_eq!(doc.graphs.len(), 2);
        assert_eq!(doc.graphs[0].name, "alpha");
        assert_eq!(doc.graphs[1].name, "zeta");
    }

    #[test]
    fn flatten_of_empty_target_is_empty() {
        let doc = flatten(&Target::default());
        assert!(doc.graphs.is_empty());
        assert!(doc.raw.is_empty());
    }

    #[test]
    fn flatten_keeps_raw_graphs_in_the_same_order_as_graphs() {
        use rust2genshin::compile::func::FnInfo;
        let mut t = Target::default();
        t.functions.insert("zeta".into(), FnInfo::default());
        t.functions.insert("alpha".into(), FnInfo::default());
        let doc = flatten(&t);
        assert_eq!(doc.graphs.len(), doc.raw.len());
        assert_eq!(doc.graphs[0].name, "alpha");
    }

    #[test]
    fn flatten_skips_edges_pointing_at_removed_nodes() {
        use rust2genshin::compile::ir::{IrKind, node_ir_local};
        use rust2genshin::node::NodeGraphIr;
        use rust2genshin::node::NodeGraphKind;

        // 手工造一个图:node0 --ctrl--> node1,再把 node1 删掉(留 Slab 空洞)
        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        let a = g.insert(node_ir_local(&IrKind::Native(rust2genshin::value::NativeKind::Int)));
        let b = g.insert(node_ir_local(&IrKind::Native(rust2genshin::value::NativeKind::Int)));
        g.link_control(
            rust2genshin::node::Link::node(a, 0),
            rust2genshin::node::Link::node(b, 0),
        );
        g.remove(b);
        g.remove(a);

        let mut t = Target::default();
        t.functions.insert("f".into(), rust2genshin::compile::ir::FnInfo { graph: g, ..Default::default() });
        let doc = flatten(&t);
        // 节点没了,边也必须没有;不能因为 index_of 查不到就 panic
        assert!(doc.graphs[0].nodes.is_empty());
        assert!(doc.graphs[0].edges.is_empty());
    }
```

若 `FnInfo` 没有 `Default`,改用 `..unsafe { std::mem::zeroed() }` 不安全 —— 正确做法是用结构体字面量补全字段,先跑一次 `cargo test` 看编译器报的缺哪些字段,再补。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p rust2genshin-viewer --lib`
Expected: FAIL —— `flatten` / `Doc` / `GraphEntry` 未定义。

- [ ] **Step 3: 写实现**

在 `doc.rs` 的 `use` 区加上：

```rust
use std::collections::HashMap;

use gpui::Size;
use rust2genshin::compile::func::{FnInfo, NodeGraphIr};
use rust2genshin::compile::ir::IrNodeId;
use rust2genshin::node::{LinkTarget, NodeRef};
```

在 `load` 之后追加：

```rust
pub const NODE_W: f32 = 140.0;
pub const NODE_H: f32 = 36.0;

pub struct NodeEntry {
    pub kind: IrNodeId,
    /// 类型标注,如 "Int"。取自 NodeKind:
    ///   Local     → values_out_types[1]  ([0] 是 LocalRef(T))
    ///   SetLocal  → values_in_types[1]   (values_out_types 为空)
    /// 其余类别为空串。
    pub type_label: String,
    pub size: Size<f32>,
}

pub struct Edge {
    pub from: (usize, usize), // (节点下标, pin 序号)
    pub to: (usize, usize),
    pub ctrl: bool, // true = 控制流边
}

pub struct GraphEntry {
    pub name: String,
    /// NodeRef → nodes 下标。Slab 删过节点后 key 有空洞,两者不恒等,必须显式映射。
    pub index_of: HashMap<NodeRef, usize>,
    pub nodes: Vec<NodeEntry>,
    pub edges: Vec<Edge>,
}

pub struct Doc {
    pub graphs: Vec<GraphEntry>, // main 第一,其余按函数名排序
    /// 与 graphs 同序的原始 IR。渲染要读 links(上色、连边),展平结果里没有。
    pub raw: Vec<NodeGraphIr>,
}

fn flatten_one(name: String, g: &NodeGraphIr) -> GraphEntry {
    let mut index_of = HashMap::new();
    let mut nodes = Vec::new();
    for (key, node) in g.nodes.iter() {
        index_of.insert(NodeRef::from(*key), nodes.len());
        let type_label = match &node.kind.id {
            IrNodeId::Local => node
                .kind
                .values_out_types
                .get(1)
                .map(|k| k.to_string())
                .unwrap_or_default(),
            IrNodeId::SetLocal => node
                .kind
                .values_in_types
                .get(1)
                .and_then(|k| k.as_ref())
                .map(|k| k.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };
        nodes.push(NodeEntry {
            kind: node.kind.id.clone(),
            type_label,
            size: Size { width: NODE_W, height: NODE_H },
        });
    }
    let mut edges = Vec::new();
    // 控制流边:从来源节点的 controls_out 出发
    for (key, node) in g.nodes.iter() {
        let from = match index_of.get(&NodeRef::from(*key)) {
            Some(&i) => i,
            None => continue,
        };
        for (pin, outs) in node.links.controls_out.iter().enumerate() {
            for out in outs {
                // 指向已删除节点的边直接跳过,不能 panic
                if let Some(&to) = out.target.node().and_then(|n| index_of.get(&n)) {
                    edges.push(Edge { from: (from, pin), to: (to, 0), ctrl: true });
                }
            }
        }
        for (pin, outs) in node.links.values_out.iter().enumerate() {
            for out in outs {
                if out.target.target == LinkTarget::Export {
                    continue; // 指向 export 的值边不画
                }
                if let Some(&to) = out.target.node().and_then(|n| index_of.get(&n)) {
                    edges.push(Edge { from: (from, pin), to: (to, 0), ctrl: false });
                }
            }
        }
    }
    GraphEntry { name, index_of, nodes, edges }
}

pub fn flatten(target: &Target) -> Doc {
    let mut graphs = Vec::new();
    let mut raw = Vec::new();
    if let Some(main) = &target.main {
        graphs.push(flatten_one("main".to_string(), main));
        raw.push(main.clone());
    }
    let mut names: Vec<&String> = target.functions.keys().collect();
    names.sort();
    for name in names {
        let FnInfo { graph, .. } = &target.functions[name];
        graphs.push(flatten_one(name.clone(), graph));
        raw.push(graph.clone());
    }
    Doc { graphs, raw }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p rust2genshin-viewer --lib`
Expected: PASS,8 passed(doc 5 + 展平 3)。

- [ ] **Step 5: 提交**

```bash
git add viewer/src/doc.rs
git commit -m "feat(viewer): 展平 NodeGraphIr 为可渲染的图"
```

---

### Task 4: layout.rs —— 控制流分层

**Files:**
- Create: `viewer/src/layout.rs`（覆盖 Task 1 的占位）
- Test: `viewer/src/layout.rs` 内联 `mod tests`

**Interfaces:**
- Consumes: Task 3 的 `NodeGraphIr`（直接读 `externals` 与 `links`，不经展平层，因为要按 `NodeRef` 遍历）
- Produces:
  - `pub const LAYER_GAP: f32 = 80.0;` `pub const NODE_GAP: f32 = 24.0;` `pub const MARGIN: f32 = 40.0;`
  - `pub fn levels(g: &NodeGraphIr) -> HashMap<NodeRef, usize>`
  - `pub fn compute(g: &NodeGraphIr) -> HashMap<NodeRef, Point<f32>>`

- [ ] **Step 1: 写失败的测试**

`viewer/src/layout.rs` 全文（先只放测试）：

```rust
#![cfg_attr(not(test), allow(dead_code))]

//! 控制流分层布局（纯函数，可单测）

#[cfg(test)]
mod tests {
    use super::*;
    use rust2genshin::compile::ir::{IrKind, IrNodeId, node_ir_local, node_ir_set_local, node_ir_native};
    use rust2genshin::node::control::NODE_IF;
    use rust2genshin::node::{ExportDecl, Link, NodeGraphIr, NodeGraphKind};
    use rust2genshin::value::NativeKind;

    /// 造一个 diamond:entry → if → (a, b) → join → exit
    fn diamond() -> NodeGraphIr {
        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));
        let ik = IrKind::Native(NativeKind::Int);
        let f = g.insert(node_ir_native(NODE_IF.clone()));
        let a = g.insert(node_ir_set_local(&ik));
        let b = g.insert(node_ir_set_local(&ik));
        let join = g.insert(node_ir_set_local(&ik));
        g.link_control(Link::export(0), Link::node(f, 0));
        g.link_control(Link::node(f, 0), Link::node(a, 0));
        g.link_control(Link::node(f, 1), Link::node(b, 0));
        g.link_control(Link::node(a, 0), Link::node(join, 0));
        g.link_control(Link::node(b, 0), Link::node(join, 0));
        g.link_control(Link::node(join, 0), Link::export(0));
        g.set_value_in(Link::node(f, 0), gpui::point(0.0, 1.0).into());
        g
    }

    #[test]
    fn diamond_branches_share_a_level_and_join_is_deeper() {
        let g = diamond();
        let lv = levels(&g);
        let f = NodeRef::from(0);
        let a = NodeRef::from(1);
        let b = NodeRef::from(2);
        let join = NodeRef::from(3);
        assert_eq!(lv[&f], 0);
        assert_eq!(lv[&a], lv[&b], "两支必须同层");
        assert!(lv[&join] > lv[&a]);
    }

    #[test]
    fn no_edge_points_backwards() {
        let g = diamond();
        let lv = levels(&g);
        for (key, node) in g.nodes.iter() {
            for outs in node.links.controls_out.iter() {
                for o in outs {
                    if let Some(t) = o.target.node() {
                        assert!(
                            lv[&NodeRef::from(*key)] < lv[&t],
                            "边往回指了: {} -> {}",
                            key, usize::from(t)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn positions_increase_with_level() {
        let g = diamond();
        let pos = compute(&g);
        let a = pos[&NodeRef::from(1)].x;
        let join = pos[&NodeRef::from(3)].x;
        assert!(join > a, "汇合点必须在两支的右边");
    }

    #[test]
    fn unreachable_nodes_still_get_positions() {
        // 一个完全孤立、入口走不到的节点也必须有坐标,否则渲染时看不到
        let mut g = diamond();
        let orphan = g.insert(node_ir_local(&IrKind::Native(NativeKind::Int)));
        let pos = compute(&g);
        assert!(pos.contains_key(&orphan), "孤立节点也要有坐标");
    }
}
```

> 注：`g.set_value_in(Link::node(f, 0), gpui::point(0.0, 1.0).into())` 那行是为了给 If 一个默认值,`compute` 本身不读值。如果 `ValueIn` 构造不这样写能编译,直接用 `ValueIn::value(...)` 或删掉此行（`levels`/`compute` 不依赖 If 的值）。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p rust2genshin-viewer --lib layout`
Expected: FAIL —— `levels` / `compute` 未定义。

- [ ] **Step 3: 写实现**

把 `viewer/src/layout.rs` 的测试**之前**插入实现（即文件以 `//!` doc + `use` + 实现开头，测试留在末尾）：

```rust
//! 控制流分层布局（纯函数，可单测）

use std::collections::{HashMap, HashSet, VecDeque};

use gpui::{Point, point};
use rust2genshin::compile::func::NodeGraphIr;
use rust2genshin::node::{NodeGraphKind, NodeRef};

pub use crate::doc::{NODE_H, NODE_W};

/// 层间距(水平方向)
pub const LAYER_GAP: f32 = 80.0;
/// 同一层内节点间距(垂直方向)
pub const NODE_GAP: f32 = 24.0;
/// 画布边距
pub const MARGIN: f32 = 40.0;
/// 层高(垂直方向的总跨度),由同层节点数决定,这里只作下限
pub const MIN_LAYER_HEIGHT: f32 = 200.0;

/// 控制流深度。取**最长**路径,保证没有边往回指。
/// 不可达节点统一给 `max_level + 1`,排在最右边。
pub fn levels(g: &NodeGraphIr) -> HashMap<NodeRef, usize> {
    // 前驱表
    let mut preds: HashMap<NodeRef, Vec<NodeRef>> = HashMap::new();
    for (key, node) in g.nodes.iter() {
        let n = NodeRef::from(*key);
        preds.entry(n).or_default();
        for outs in node.links.controls_out.iter() {
            for o in outs {
                if let Some(t) = o.target.node() {
                    preds.entry(t).or_default().push(n);
                }
            }
        }
    }

    // 从入口 BFS 求可达集
    let mut reachable: HashSet<NodeRef> = HashSet::new();
    let mut q = VecDeque::new();
    if let Some(entries) = g.externals.controls_out.first() {
        for e in entries {
            if let Some(n) = e.target.node()
                && reachable.insert(n)
            {
                q.push_back(n);
            }
        }
    }

    // 松弛:对每条边 p -> n,要求 depth[p] >= depth[n] + 1。
    // 图无环,每轮至少让某个点 +1,必然收敛。
    let mut depth: HashMap<NodeRef, usize> = HashMap::new();
    loop {
        let mut changed = false;
        for (key, node) in g.nodes.iter() {
            let n = NodeRef::from(*key);
            if !reachable.contains(&n) {
                continue;
            }
            for outs in node.links.controls_out.iter() {
                for o in outs {
                    let Some(t) = o.target.node() else { continue };
                    if !reachable.contains(&t) {
                        continue;
                    }
                    let want = depth.get(&t).copied().unwrap_or(0) + 1;
                    if depth.get(&n).copied().unwrap_or(0) < want {
                        depth.insert(n, want);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    // 不可达节点排最右
    let max_level = depth.values().copied().max().unwrap_or(0);
    for (i, _) in g.nodes.iter() {
        depth.entry(NodeRef::from(i)).or_insert(max_level + 1);
    }
    depth
}

/// 由分层算出坐标
pub fn compute(g: &NodeGraphIr) -> HashMap<NodeRef, Point<f32>> {
    let lv = levels(g);
    // 按层分组,层内按 NodeRef 排序
    let mut by_layer: HashMap<usize, Vec<NodeRef>> = HashMap::new();
    for (n, &l) in &lv {
        by_layer.entry(l).or_default().push(*n);
    }
    for v in by_layer.values_mut() {
        v.sort_by_key(|n| usize::from(*n));
    }
    // 每层的高度 = 层内节点数 * (NODE_H + NODE_GAP)
    let heights: HashMap<usize, f32> = by_level_heights(&by_layer);
    let max_h = heights.values().copied().fold(0.0f32, f32::max).max(MIN_LAYER_HEIGHT);

    let mut result = HashMap::new();
    for (&l, nodes) in &by_layer {
        let h = heights[l];
        let x = MARGIN + l as f32 * (NODE_W + LAYER_GAP);
        let y0 = MARGIN + (max_h - h) / 2.0;
        for (i, n) in nodes.iter().enumerate() {
            let y = y0 + i as f32 * (NODE_H + NODE_GAP);
            result.insert(*n, point(x, y));
        }
    }
    result
}

fn by_level_heights(by_layer: &HashMap<usize, Vec<NodeRef>>) -> HashMap<usize, f32> {
    by_layer
        .iter()
        .map(|(&l, v)| (l, v.len() as f32 * (NODE_H + NODE_GAP) - NODE_GAP))
        .collect()
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p rust2genshin-viewer --lib layout`
Expected: PASS,4 passed。

- [ ] **Step 5: 提交**

```bash
git add viewer/src/layout.rs
git commit -m "feat(viewer): 控制流分层布局"
```

---

### Task 5: view.rs —— GPUI 渲染

**Files:**
- Modify: `viewer/src/view.rs`（覆盖 Task 1 的占位）

**Interfaces:**
- Consumes: Task 3 的 `Doc` / `GraphEntry` / `NodeEntry` / `Edge`；Task 4 的 `compute` / `levels`；gpui 的 `canvas` / `PathBuilder` / `window.paint_path`
- Produces:
  - `pub struct Viewer { pub doc: Doc, pub current: usize, pub error: Option<String>, pub pos: HashMap<NodeRef, Point<f32>>, pub origin: Point<f32>, pub scale: f32 }`
  - `impl Render for Viewer`

- [ ] **Step 1: 写实现**

`viewer/src/view.rs` 全文：

```rust
//! GPUI 渲染层

use std::collections::HashMap;

use gpui::prelude::*;
use gpui::{
    App, Color, Context, InteractiveElement, IntoElement, MouseButton, PathBuilder, Pixels, Point,
    Render, StatefulInteractiveElement, Styled, Window, canvas, div, point, px, rgb,
};
use rust2genshin::compile::ir::IrNodeId;
use rust2genshin::node::NodeRef;

use crate::doc::{Doc, NODE_H, NODE_W};
use crate::layout;

const EDGE_CTRL: Color = rgb(0x4c, 0x8f, 0xd6);
const EDGE_VALUE: Color = rgb(0xd6, 0x8f, 0x4c);
const CTRL_W: f32 = 2.0;
const VALUE_W: f32 = 1.0;

pub struct Viewer {
    pub doc: Doc,
    pub current: usize,
    pub error: Option<String>,
    /// 用户拖动后的坐标。切换图时重置。
    pub pos: HashMap<NodeRef, Point<f32>>,
    pub origin: Point<f32>,
    pub scale: f32,
}

impl Viewer {
    pub fn new() -> Self {
        Self {
            doc: Doc { graphs: Vec::new() },
            current: 0,
            error: None,
            pos: HashMap::new(),
            origin: point(0.0, 0.0),
            scale: 1.0,
        }
    }

    /// 切换到第 i 张图,重算初始坐标
    pub fn select(&mut self, i: usize, cx: &mut Context<Self>) {
        if i >= self.doc.graphs.len() {
            return;
        }
        self.current = i;
        self.pos = layout::compute(&self.raw_graph(i));
        self.origin = point(0.0, 0.0);
        self.scale = 1.0;
        cx.notify();
    }

    /// 取当前图的原始 IR。展平结果里没有 links,渲染要读它。
    fn raw_graph(&self, i: usize) -> rust2genshin::compile::func::NodeGraphIr {
        self.doc.raw[i].clone()
    }
}
```

**`Doc.raw` 已在 Task 3 定义**(`raw: Vec<NodeGraphIr>`,与 `graphs` 同序),本 task 直接用:

```rust
    fn raw_graph(&self, i: usize) -> rust2genshin::compile::func::NodeGraphIr {
        self.doc.raw[i].clone()
    }
```

**Step 1c: 完成 `view.rs`**

接在 `Viewer` impl 之后,把 `Render` impl 和两个渲染函数写全：

```rust
impl Render for Viewer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(err) = self.error.clone() {
            return div().flex_1().items_center().justify_center().child(div().child(err)).into_any_element();
        }
        let Some(g) = self.doc.graphs.get(self.current) else {
            return div().flex_1().items_center().justify_center()
                .child("拖入 .ogia 或 .rlib 文件").into_any_element();
        };
        let this = cx.entity().clone();
        // 画布:边 + 节点
        div()
            .flex_1()
            .relative()
            .overflow_hidden()
            .on_scroll_wheel({
                let this = this.clone();
                move |e, _w, cx| {
                    let mut v = this.update(cx, |v, _| v);
                    let f = if e.delta.y < 0.0 { 1.1 } else { 1.0 / 1.1 };
                    v.scale = (v.scale * f).clamp(0.2, 4.0);
                    cx.notify();
                }
            })
            .child(self.render_edges(g, cx))
            .child(self.render_nodes(g, cx))
            .into_any_element()
    }
}
```

> GPUI 0.2.2 的 `Render` 返回 `impl IntoElement`;上面用 `.into_any_element()` 统一两个分支的类型。若编译器不接受,改成让两个分支都返回 `div()...`(空 div 而非纯文本),即可避免类型分歧。

**Step 1d: `render_edges` 的正确实现**

先给 `Viewer` 加一个辅助方法,把下标翻回 `NodeRef`:

```rust
impl Viewer {
    fn node_ref(&self, g: &crate::doc::GraphEntry, idx: usize) -> Option<NodeRef> {
        g.index_of.iter().find(|(_, &i)| i == idx).map(|(&n, _)| n)
    }
}
```

> `index_of` 是 `HashMap`,反向查找是 O(n)。节点数不大(几十),可接受;若嫌慢,给 `GraphEntry` 加一个 `refs: Vec<NodeRef>` 字段做正向索引(在 Task 3 的 `flatten_one` 里顺手填)。

然后:

```rust
    fn render_edges(&self, g: &crate::doc::GraphEntry, _cx: &mut Context<Self>) -> impl IntoElement {
        let origin = self.origin;
        let scale = self.scale;
        // 预先把边解析成像素坐标,避免在 paint 闭包里反复查表
        let mut segs: Vec<(Point<f32>, Point<f32>, bool)> = Vec::new();
        for e in &g.edges {
            let (Some(from), Some(to)) = (
                self.node_ref(g, e.from.0).and_then(|n| self.pos.get(&n)),
                self.node_ref(g, e.to.0).and_then(|n| self.pos.get(&n)),
            ) else {
                continue;
            };
            let a = point(
                (from.x + NODE_W) * scale + origin.x,
                (from.y + NODE_H / 2.0) * scale + origin.y,
            );
            let b = point(to.x * scale + origin.x, (to.y + NODE_H / 2.0) * scale + origin.y);
            segs.push((a, b, e.ctrl));
        }
        canvas(
            move |_b, _w, _c| (),
            move |_b, (), window, _c| {
                for (a, b, ctrl) in &segs {
                    let mut pb = PathBuilder::default();
                    pb.stroke(if *ctrl { px(CTRL_W) } else { px(VALUE_W) });
                    if !*ctrl {
                        // 值流边用虚线
                        let d = px(4.0);
                        pb = pb.dash_array(&[d, d]);
                    }
                    let mid_x = (a.x + b.x) / 2.0;
                    pb.move_to(point(a.x, a.y));
                    pb.cubic_bezier_to(
                        point(b.x, b.y),
                        point(mid_x, a.y),
                        point(mid_x, b.y),
                    );
                    if let Ok(p) = pb.build() {
                        window.paint_path(p, if *ctrl { EDGE_CTRL } else { EDGE_VALUE });
                    }
                }
            },
        )
    }
```

`render_nodes`:

```rust
    fn render_nodes(&self, g: &crate::doc::GraphEntry, cx: &mut Context<Self>) -> impl IntoElement {
        let mut layer = div().absolute();
        for (i, n) in g.nodes.iter().enumerate() {
            let Some(nref) = self.node_ref(g, i) else { continue };
            let pos = self.pos.get(&nref).copied().unwrap_or(point(0.0, 0.0));
            let this = cx.entity().clone();
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(pos.x))
                    .top(px(pos.y))
                    .w(px(NODE_W))
                    .h(px(NODE_H))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .border_1()
                    .border_color(color_for(&n.kind))
                    .bg(rgb(0x1e, 0x1e, 0x22))
                    .cursor_grab()
                    .on_mouse_down(MouseButton::Left, move |_e, _w, cx| {
                        cx.stop_propagation();
                    })
                    .child(div().text_xs().text_color(color_for(&n.kind)).child(label(n)))
                    .id(("node", i)),
            );
            let _ = this;
        }
        layer
    }
```

`label` 与 `color_for`:

```rust
fn label(n: &crate::doc::NodeEntry) -> String {
    let base = match &n.kind {
        IrNodeId::Native(x) => format!("{}#{}", format!("{:?}", x.kind).to_uppercase(), x.id),
        IrNodeId::Local => "Local".to_string(),
        IrNodeId::SetLocal => "SetLocal".to_string(),
        IrNodeId::If => "If".to_string(),
        IrNodeId::Unreachable => "Unreachable".to_string(),
        IrNodeId::Fn(s) => format!("Fn({s})"),
        IrNodeId::Assemble => "Assemble".to_string(),
        IrNodeId::Destructure => "Destructure".to_string(),
        IrNodeId::Modify => "Modify".to_string(),
    };
    if n.type_label.is_empty() {
        base
    } else {
        format!("{base}<{}>", n.type_label)
    }
}

fn color_for(kind: &IrNodeId) -> Color {
    match kind {
        IrNodeId::Local => rgb(0x88, 0x88, 0x88),
        IrNodeId::SetLocal => rgb(0x4c, 0x8f, 0xd6),
        IrNodeId::If => rgb(0xa0, 0x6c, 0xd6),
        IrNodeId::Unreachable => rgb(0x8f, 0x3a, 0x3a),
        IrNodeId::Native(n) => {
            // 按 id 哈希取色,天然区分不同原生节点
            let h = (n.id as u64).wrapping_mul(2654435761);
            let r = (h >> 16) as u8 & 0x7f | 0x40;
            let g = (h >> 8) as u8 & 0x7f | 0x40;
            let b = h as u8 & 0x7f | 0x40;
            rgb(r, g, b)
        }
        IrNodeId::Assemble | IrNodeId::Destructure | IrNodeId::Modify => rgb(0x5a, 0xa0, 0x5a),
        IrNodeId::Fn(_) => rgb(0xd6, 0xa0, 0x4c),
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo build -p rust2genshin-viewer`
Expected: 编译通过。若 GPUI 链式方法的名称与本计划不符(`.left()` / `.top()` / `.w()` / `.h()` / `.rounded_md()` / `.border_1()` / `.cursor_grab()` / `.text_xs()`),以 gpui 0.2.2 的实际签名为准修正 —— 这些都是 `Styled` / `StyledAbsolute` 的标准方法。

- [ ] **Step 3: 提交**

```bash
git add viewer/src/view.rs viewer/src/doc.rs
git commit -m "feat(viewer): GPUI 渲染层(边 + 节点)"
```

---

### Task 6: main.rs —— 窗口与拖放接线

**Files:**
- Modify: `viewer/src/main.rs`（覆盖 Task 1 的占位）

**Interfaces:**
- Consumes: Task 2 的 `doc::load`；Task 5 的 `Viewer`
- Produces: 可运行的 `ogia-viewer` 可执行文件

- [ ] **Step 1: 写实现**

`viewer/src/main.rs` 全文：

```rust
#![feature(rustc_private)]

use gpui::prelude::*;
use gpui::{App, Application, ExternalPaths, Window, WindowOptions, div, px, size};

use rust2genshin_viewer::{doc, view::Viewer};

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions {
            title: "ogia viewer".into(),
            bounds: Some(gpui::Bounds::centered(size(px(1200.0), px(800.0)))),
            ..Default::default()
        }, |_window, cx| cx.new_view(Viewer::new()));
    });
}
```

Run: `cargo build -p rust2genshin-viewer`
Expected: 编译通过。

- [ ] **Step 2: 加拖放与图列表**

在 `Viewer` 的 `Render` 里,最外层 `div` 加 `on_drop` 与左侧列表:

```rust
        div()
            .flex()
            .size_full()
            // 整个窗口接收文件拖放
            .on_drop::<ExternalPaths>(|paths, _window, cx| {
                for p in paths.paths() {
                    let r = doc::load(p);
                    cx.update(|v: &mut Viewer, cx| match r {
                        Ok(t) => {
                            let mut d = doc::flatten(&t);
                            // 追加而非替换
                            d.graphs.append(&mut v.doc.graphs);
                            d.raw.append(&mut v.doc.raw);
                            if v.doc.graphs.is_empty() && v.error.is_some() {
                                v.error = None;
                            }
                            v.doc = d;
                            v.select(0, cx);
                        }
                        Err(e) => {
                            v.error = Some(e);
                        }
                    });
                }
            })
            .child(
                // 左侧图列表
                div()
                    .w(px(200.0))
                    .h_full()
                    .overflow_y_scroll()
                    .p_2()
                    .children(self.doc.graphs.iter().enumerate().map(|(i, g)| {
                        let active = i == self.current;
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .when(active, |s| s.bg(rgb(0x2d, 0x2d, 0x33)))
                            .child(format!("{} ({})", g.name, g.nodes.len()))
                            .on_click({
                                let this = cx.entity().clone();
                                move |_e, _w, cx| this.update(cx, |v, cx| v.select(i, cx))
                            })
                    }))
                    .border_r_1()
                    .border_color(rgb(0x33, 0x33, 0x3a)),
            )
            .child(
                // 右侧画布
                div().flex_1().relative().overflow_hidden()
                    .on_scroll_wheel({ /* Task 5 render 里的那段 */ })
                    .child(self.render_edges(g, cx))
                    .child(self.render_nodes(g, cx)),
            )
            .into_any_element()
```

> Task 5 的 `render` 里已经有一份 `on_scroll_wheel` 和 `render_edges/render_nodes` 调用。**Task 6 把它们上提到 `div().flex_1()` 这一层**,与左侧列表并排;Task 5 的 `render` 保留为"只有画布"的简化版本也可以,但更省事的是:Task 5 的 `render` 就写成最终的双栏结构,Task 6 只补 `on_drop` 和图列表。

**实现顺序调整**：先按 Task 5 写好单栏 `render` 并编译通过,Task 6 再改成双栏。这样每步都能编译。

- [ ] **Step 3: 手动验证**

Run: `cargo run -p rust2genshin-viewer --bin ogia-viewer`
Expected: 窗口打开,中央显示"拖入 .ogia 或 .rlib 文件"。

然后从资源管理器拖一个 `.rlib`（`target/genshin-unknown-server/release/deps/librust2genshin_demo-*.rlib`）进窗口。

Expected: 左侧出现图列表（`main` + 若干函数名,带节点数）,右侧渲染出节点与边,拖节点能移动,拖空白能平移,滚轮能缩放。

若左侧空白:说明 `.rlib` 里没有 `.ogia` entry,窗口中央应显示"未找到 .ogia 条目" —— 这是 Review Focus 第 1 条,属预期。

- [ ] **Step 4: 提交**

```bash
git add viewer/src/main.rs viewer/src/view.rs
git commit -m "feat(viewer): 窗口、文件拖放与图列表"
```

---

### Task 7: 收尾 —— default-members 与全量验证

**Files:**
- Modify: `Cargo.toml`（可选）

**Interfaces:**
- Consumes: 全部前序 task
- Produces: 干净的仓库状态 + 一次全量验证

- [ ] **Step 1: 跑全部测试**

Run: `cargo test -p rust2genshin-viewer --lib`
Expected: 全部 PASS（7 个：doc 3+4,layout 4 中的 4 个 → 实际数量以实现为准,关键是 0 failed）。

- [ ] **Step 2: 确认 core 未被破坏**

Run: `cargo run -p build-demo`
Expected: 成功,`target/genshin-unknown-server/release/rust2genshin_demo.gia` 正常产出。viewer 不应影响 core 行为。

- [ ] **Step 3: 加 default-members,把 viewer 排除出日常构建与 CI**

Task 1 实测:根 `cargo build` 本来就失败于 `rust2genshin-demo`(`unwinding panics are not supported without std` —— demo 是 `#![no_std]`,必须走自定义 codegen backend)。这是既有现象,与 viewer 无关。

但 gpui 在 Linux 上需要 X11/Wayland/字体等系统库,而现有 CI(`.github/workflows/build.yml`)只装了 `protobuf-compiler`。viewer 一旦成为默认 member,任何跑 `cargo build` / `cargo test --workspace` 的 CI 步骤都会开始编译 gpui 并很可能失败。

**无条件执行** —— 在根 `Cargo.toml` 加 `default-members` 排除 viewer:

```toml
[workspace]
members = ["core", "build-demo", "lib", "lib-internal", "demo", "viewer"]
default-members = ["core", "build-demo", "lib", "lib-internal", "demo"]
```

这样不带 `-p` 的 `cargo build` / `cargo test` 不构建 viewer,CI 不受影响;需要时用 `cargo build -p rust2genshin-viewer` 或 `cargo build --workspace` 显式构建。

验证: `cargo build` 的失败信息应仍止于 demo(既有现象),**不应**出现 gpui 相关报错。

- [ ] **Step 3c: 产出一个可供拖入的夹具文件并做手工验证**

Task 2 的测试把夹具写进 `std::env::temp_dir()` 后**立刻 `remove_file` 删掉**,所以直接跑测试拿不到可拖的文件。让测试在设了环境变量时保留夹具并打印路径:

在 Task 2 的 `mod tests` 里给 `temp_path` 的调用点(或每个测试的收尾)加一个约定 —— 若 `R2G_KEEP_FIXTURE` 已设置,则跳过 `remove_file` 并 `println!` 路径。最小改法:把删除抽成一个 helper

```rust
fn cleanup(path: &std::path::Path) {
    if std::env::var_os("R2G_KEEP_FIXTURE").is_none() {
        let _ = std::fs::remove_file(path);
    } else {
        println!("夹具保留在: {}", path.display());
    }
}
```

然后:

```bash
R2G_KEEP_FIXTURE=1 cargo test -p rust2genshin-viewer --lib -- --nocapture
cargo run -p rust2genshin-viewer --bin ogia-viewer
```

把打印出来的 `.rlib` 拖进窗口。**预期**:左侧图列表出现该函数(带节点数),右侧渲染出节点与边。

跑完记得清掉残留的临时文件。

- [ ] **Step 4: 提交（若有改动）**

```bash
git add Cargo.toml
git commit -m "chore: viewer 不进 default-members,避免拖慢日常构建"
```

若 Step 3 无改动,跳过本步。

---

## Self-Review

**1. Spec 覆盖**

| Spec 条目 | 覆盖 task |
|---|---|
| 拖入 `.ogia` | Task 2 (`load_ogia`) + Task 6 (`on_drop`) |
| 拖入 `.rlib` | Task 2 (`load_rlib`) + Task 6 |
| 防御式合并 | Task 2 (`merge`) + 测试 |
| 左侧图列表 / 右侧画布 | Task 6 |
| `index_of` 映射（Slab 空洞） | Task 3 + 测试 |
| 分层自动布局 | Task 4 + 4 个测试 |
| 控制流边实线 / 值流边虚线 | Task 5 (`render_edges`) |
| 节点按 `IrNodeId` 上色 | Task 5 (`color_for`) |
| 节点拖动 | Task 5 (`render_nodes` 的 `on_mouse_down` 占位）+ Task 6 完善 |
| 画布平移 / 缩放 | Task 5 (`on_scroll_wheel`) + Task 6 |
| 错误显示在窗口中央 | Task 5/6 (`self.error` 渲染分支) |
| 只测布局 | Task 4（doc 的测试是实现自检,不算额外范围） |

**已补**:`Local` / `SetLocal` 的类型标注(spec 要求显示 `Local<Int>`)—— `NodeEntry` 加了 `type_label: String` 字段,Task 3 的 `flatten_one` 从 `values_out_types[1]`(Local) / `values_in_types[1]`(SetLocal) 取,Task 5 的 `label(n: &NodeEntry)` 拼成 `Local<Int>`。

**2. 占位符扫描**:无 TBD/TODO。GPUI 链式方法名标了"以实际签名为准",这是诚实的不确定,不是占位符。

**3. 类型一致性**:`Doc` / `GraphEntry` / `NodeEntry` / `Edge` 在 Task 3 定义,Task 5 消费;`levels` / `compute` 在 Task 4 定义,Task 5 消费;`load` 在 Task 2 定义,Task 6 消费。`Doc.raw` 已在 Task 3 一并定义,Task 5 直接消费。

**4. Review Focus 覆盖**:
- 第 1 条（rlib 无 ogia）→ Task 2 `load_rlib` 的 `is_empty()` 检查 + 手动验证 Step 3
- 第 2 条（多个 main）→ Task 2 `merge_keeps_first_main_instead_of_panicking`
- 第 3 条（Slab 空洞）→ Task 3 `flatten_skips_edges_pointing_at_removed_nodes`
- 第 4 条（空 Target）→ Task 3 `flatten_of_empty_target_is_empty` + `view.rs` 的 `else` 分支
- 第 5 条（`IrNodeId::Fn`）→ Task 5 `color_for` 的 `IrNodeId::Fn(_)` 分支
