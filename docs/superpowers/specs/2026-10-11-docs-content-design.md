# rust2genshin 文档站内容规划 — Design Spec

**Date:** 2026-10-11
**Status:** Draft (awaiting review)
**Scope:** `docs/typst/` 站点里 `user/` `dev/` `arch/` 三栏该有哪些页面、每页写什么、内容从哪来。不改文档站的构建基础设施（那是另一件事，见 Out of scope）。

## Context

`docs/typst/` 是用 Typst + CSS + JS 构建的静态文档站，构建器在 `docs/`（Rust crate，两个 bin：`compile` / `watch`）。站点骨架已经搭好：

```
docs/typst/
  index.typ        首页
  user/index.typ   「使用」—— 空壳
  dev/index.typ    「开发」—— 空壳
    dev/docs/{index,card,typst}.typ   文档站自身的说明，已经写好
  arch/index.typ   「架构」—— 空壳
    arch/docs.typ  文档构建流程，已经写好
  lib/             基础设施（模板、组件、CSS、JS）
```

空壳三栏和一份没进站的 README 之间存在断层：

- **`README.md` 有 2000 多行**，内容质量不错，但目前同时承担三个角色：GitHub 首屏、项目介绍、唯一的用户文档。它是「关于节点图数据流 / 控制流 / 循环 / 所有权」的**唯一记录** —— 删了就没了。
- **`CLAUDE.md` 是给 agent 的开发指南**，里面有构建机制、调试技巧、加节点步骤，和将来的 `dev/` 高度重叠。

结果是：新读者只能从 README 的零散小节里拼出完整认知，而贡献者要读的东西（构建机制、加节点）分散在 CLAUDE.md 里、不会被站点导航收录。

## 决策

**栏目语义** —— 三栏是三个不同的轴，不是「两类读者 + 一个横切」：

| 栏目 | 回答什么问题 | 读它是为了 |
|---|---|---|
| `user/` | 怎么用 `rust2genshin` 写节点图 | 用 |
| `dev/` | 怎么继续开发 `rust2genshin`（加节点、调 bug 从哪下手） | 动手 |
| `arch/` | `rust2genshin` 这个项目是怎么搭起来的（分层、管线、格式） | 理解 |

`arch/` 里的 `low/` 子目录装*目标平台本身*的语义（节点图怎么运作），它和本项目无关但整个项目都建立在它之上。`arch/` 顶层装*项目自身*：编译器怎么映射、产物是什么格式。

**README 降为入口** —— 收缩成简介 + 构建步骤 + 指向站点，保留一个最小可跑通的 demo 段（GitHub 首屏不至于劝退）。其余内容全部进站，站点是唯一真源。

**CLAUDE.md 逐步缩成指路牌** —— 逐步把「人需要读的部分」搬进 `dev/`，CLAUDE.md 留下链接指过去。CLAUDE.md 不能删（agent 依赖它），但不应成为唯一记录。

**具体 API 不进站** —— `lib/` 约 100 个 `pub` 项，手抄进 Typst 必然漂移。当前阶段不写全量 API 参考页；是否用 rustdoc 承载、什么时候做，留待后续。站里只写「怎么用」的页面（句柄、事件、导出、原生节点、异步）和能力分组速览。

**两个垃圾文件由用户自行处理** —— `docs/typst/lib/trait.typ`（0 字节却被 `bundle_document` 编译成页面、出现在导航树里）、`docs/typst/.gitignore`（被 `walk()` 当成资源发布成 `/.gitignore`）。本 spec 不含这两项。

## 页面清单

### `user/` — 写节点图的人

按「我要做什么」排序，不按模块名。

| 页 | 内容要点 | 来源 |
|---|---|---|
| `index.typ` | 总览 + 阅读路线图（新手 → 熟手 → 遇到问题的三条路径） | 新写 |
| `getting-started.typ` | 装 rustup / protoc / nightly 三组件 → clone → 改 demo 模块名 + workspace `members` → `cargo run -p build-demo` → 产物路径 → 怎么验证（反解 `.gia` / viewer） | README「Build」扩写 |
| `project.typ` | 项目定位、目标平台（目前仅服务器节点图）、产物是什么 | README「简介」 |
| `export.typ` | `#[unsafe(no_mangle)]` / `export_name`、复合节点、**「主图始终导出，未导出或未被引用的资产无法导入」** | README 迁入 |
| `events.typ` | `#[event_listener]` 用法；内置事件按 character / entity / player 分组列出 | README 迁入 + 补清单 |
| `handles.typ` | Guid / Faction / Config / Prefab：只能在编译期构造、`xxx!` 宏、`#[rustc_comptime]` 为什么不是 `const fn`、错误写法反例对照（运行期变量 / `Guid::new` / 访问私有字段） | README 迁入 |
| `native.typ` | *只写用法*：无体函数当普通函数调（每条编译成一个节点）、`#[native_enum(N)]` 的 N 与变体值规则、原生枚举的 `==`。四个宏各自的实现语义属于 `dev/add-native`，不在本页重复 | `lib/src/event/character.rs` + `lib-internal/src/lib.rs` |
| `async.typ` | `async fn` / `#[async_loop]` / `sleep!` / `async_point` / `async_jump` / `async_continue!`；为什么用它代替递归调用 | `lib/src/asynchronous.rs` |
| `compatibility.typ` | 溢出语义（自动溢出）、整数除法边界、扁平化局部变量、内部可变性、可变借用 | README 迁入 |
| `unsupported.typ` | 不被支持的特性：`i8`/`i16`、`#[repr(u32)]`、递归调用、`dyn` trait object，以及每条「为什么」 | README 迁入 |
| `capabilities.typ` | 能力分组速览：数学 / 实体 / 玩家 / 容器（`List` `Dict` `Box` `Sc`）/ 句柄 / 异步 / 事件。只列分组和典型入口，**不列全量签名** | `lib/` 公开 API |
| `limitations.typ` | Todo List：语言特性、类型、原生节点、原生枚举、事件的待办与路线图 | README 迁入 |

`capabilities.typ` 是「知道有什么、去哪查」的索引页，不承担参考手册的职责 —— 后者明确推迟到 rustdoc 决策之后。

### `arch/` — 横切原理

`arch/` 分两层：

- **`arch/low/`** —— *节点图本身*的语义。和 `rust2genshin` 无关，直接用节点编辑器手拼节点也一样成立。对应 README 原来的「关于节点图」一节。
- **`arch/` 顶层** —— *这个编译器*怎么把 Rust 映射过去、产物 `.gia` 长什么样。

前五页几乎全是 README 已有内容的搬迁，风险最低；后两页需要新写。

| 页 | 内容要点 | 来源 |
|---|---|---|
| `index.typ` | 导览：目标产物那层去 `low/`，编译器、产物格式、viewer 留本层 | 新写 |
| `low/index.typ` | 节点图语义总览 + 五页入口 | 新写 |
| `low/dataflow.typ` | 有控制流的节点（执行节点、事件）出参可像局部变量一样直接获取；无控制流的节点（查询、运算）每次获取出参都**重新获取入参并计算**；默认值规则；所有权（节点图/局部变量和列表/结构体成员持有数据而非引用，写入自动克隆，其它地方均为引用）；顺带记「模运算节点实为取余」 | README「数据流」「所有权」「运算」迁入 |
| `low/locals.typ` | `获取局部变量` 存 MaybeUninit；每次访问都访问「初始值」引脚（因为那是入参），无论是否已初始化；**因此不建议把初始值接到其它节点**；每次事件开始时局部变量已确保未初始化 | README 迁入 |
| `low/loops.typ` | `有限循环` 只储存当前循环值、终止值每次重取；`列表迭代循环` 不储存元素也不储存长度（等价于每次重新求 `list[i]` 和 `list.size()`） | README 迁入 |
| `low/controlflow.typ` | 排除「退出循环」后控制流不允许成环；反复执行的块必须被反复重新进入；「退出循环」本质是给所有目标循环打终止标记，可理解为「下一轮跳出」；有限循环用 `<=`，上界 `i32::MAX` 时无法正常结束 | README 迁入 |
| `low/mutation-event.typ` | `设置节点图变量` 且 `触发事件` 为 true 时立即克隆新旧数据、生成修改事件并入队 | README 迁入 |
| `compile/index.typ` | 入口来自 `tcx.exported_generic_symbols` + `exported_non_generic_symbols`（`compile/mod.rs:172`，**不是**扫 HIR 找 `contains_extern_indicator` —— 后者只在 `mod.rs:348` 给每个函数算 `exported` 标志）、`touch_fn` 递归编译依赖、`#[event_listener]` 注入的入口、函数 → 复合节点、basic_block / terminator 逐句翻译、`Optimizer::verify` / `lower` | **新写** |
| `compile/locals.typ` | `CompiledLocal` 把 Place 投影递归展平（这为什么让闭包参数变成双层）、`LocalRef` = 一对引脚、读写靠 assemble / destructure / modify、为什么不把整个元组存成一个局部变量 | **新写** |
| `compile/cast.typ` | `as` 的三条路（同类型直通 / 查表建节点 / 报错）、`node_cast` 的 kernel 表（180–189）与多态输入引脚的 `selected_in` | **新写** |
| `asset.typ` | `.gia` **20 字节头**（5 × u32 大端）+ **4 字节尾**（`core/src/asset.rs:111-126`）、proto 字段逆向来源、`NativeValue` 类型体系与 `NativeKind` 枚举（**没有** `AnyValue` / `INativeValue`）、`NodeRef` 序列化为 `id + NODE_ID_BEGIN`（= 1）、StructField 与 assemble/destructure/modify 节点 | **新写** |
| `viewer.typ` | 看的是中间态 IR 而非最终 `.gia`、布局迁进 core、ALAP 就近锚定、列内交叉消减、坐标按图存、一键优化 + Ctrl+Z 存档、为什么排除出 workspace `default-members` | **新写**（设计从 `dev/viewer` 移来） |

### `dev/` — 改 rust2genshin

`dev/docs/` 三页（文档站说明、卡片、Typst 教程）已经写好，不动。

| 页 | 内容要点 | 来源 |
|---|---|---|
| `index.typ` | 开发总览：代码地图（`core` / `lib` / `lib-internal` / `demo` / `build-demo` / `viewer`）+ 从哪下手 | 新写 |
| `build.typ` | nightly 钉版与三个组件（`rustc-dev` / `rust-src` / `llvm-tools-preview`）、`-Zcodegen-backend` 加载机制、`#![feature(rustc_private)]` 与 dylib 符号复用（miri 做法）、`prost-build` 与 `build.rs` 的 `rerun-if-changed`、`AssetBundle::save` 的字节比对保 mtime、CI `build.yml` | CLAUDE.md 迁入 |
| `debugging.typ` | `tcx.dcx().warn/err`；`Assert` / `panic` 终结符被**直接跳过**（`compile/func.rs`），故溢出检查等断言根本不进图 —— 与 CLAUDE.md 曾宣称的「必须 `-Coverflow-checks=off`」相反，**已实测证伪**；`-Zalways-encode-mir=yes` 的用途；两层增量构建；Python 工具（`gia2txt` / `diff_gia` / `check_unknown` / `dev/wire_tree`）被 gitignore 且需 `pip install protobuf`；`gia2txt.py` 输出数字键代表 proto 未声明字段 | CLAUDE.md「调试技巧」迁入 + 实测订正 |
| `add-node.typ` | 两步：`core/src/value.rs` 加 `NativeKind` 变体 + `impl NativeValue` → 在 `core/src/node/<域>.rs` 写 `NodeKind` 构造函数，并在 `compile/func.rs`（操作符）或 `compile/ir.rs`（控制流）的翻译处用它。**没有** `ValueXxx`，也**没有**静态节点登记表 —— `NodeKind` 是运行时构造的 | CLAUDE.md 迁入 + 订正 |
| `add-native.typ` | `compile/native.rs::compile_native_ty` 加类型分支、`compile_native_call` 加调用分支；`#[event_listener]` 何时自动加 `#[unsafe(no_mangle)]` 并改写 span(call_site)，编译路径靠 `get_expn_macro_attr` 从 span 取宏参数 | CLAUDE.md 迁入 |
| `viewer.typ` | 只讲**怎么用**：输入从哪来（`build-demo` 的 rlib entry）、怎么跑（四种 CLI 用法）、`--check` 先跑、界面三栏。设计取舍移到 `arch/viewer.typ` | 新写 |
| `docs/index.typ` | （已有，不动） | 已有 |
| `docs/card.typ` | （已有，不动） | 已有 |
| `docs/typst.typ` | （已有，不动） | 已有 |

## 推进顺序

| 阶段 | 内容 | 理由 |
|---|---|---|
| 1 | `user/index` `getting-started` `project`，README 收缩成入口 | 先让新读者跑通；也是 README 降级的前置 |
| 2 | `user/compatibility` `unsupported` `handles` `export` `events` `limitations` | 纯 README 搬迁，成本低、防呆价值最高 |
| 3 | `arch/low/` 前六页 | 也是搬迁，把 README 掏空 |
| 4 | `dev/build` `debugging` `add-node` `add-native` `index`，CLAUDE.md 缩指路牌 | CLAUDE.md 搬迁 |
| 5 | `user/native` `async` `capabilities` | 需要真写 |
| 6 | `arch/mir-mapping` `asset-format`、`dev/viewer` | 需要真写，篇幅最大 |

阶段 1–4 基本都是「把 README 和 CLAUDE.md 里已经写对的东西搬进站并重新编排」，风险低。5–6 才是新写。

## 每页的完成判据

一页算写完，当且仅当：

1. **可导航** —— `title` 绑定存在（`toc.typ` 的树是从 `query(document)` 的 `x.title` 取的，不是文件名）；出现在导航树里且层级正确。
2. **代码块能跑** —— 页里的 Rust 片段至少在 `demo/` 的语境下类型正确；`user/` 下的片段应当能被 `cargo build -p demo` 通过或明确标注是片段。
3. **无孤儿** —— 页面里引用的每个概念，要么在本页定义，要么链到站内某一页。站内链接用站内绝对路径（`/user/handles.html`），不用 `meta.root` 拼接（README 里的 `//dev/...` 协议相对 URL 问题就是这么来的）。
4. **README / CLAUDE.md 对应段落已删或已改为链接** —— 否则内容分叉。

## 测试

Typst 侧没有单元测试框架，靠两条验证：

- **构建通过** —— `cargo run -p rust2genshin-docs --bin compile` 的退出码可靠（`compile.rs` 里是 `exit(run("compile"))`，`typst.rs::run()` 用 `.status()` 取真实退出码）。非 0 即构建失败。
- **每页浏览器实测** —— 打开该页确认渲染、锚点、代码块高亮正常。这是唯一能发现「Typst 生成的 DOM 形状和 CSS 选择器对不上」的手段（本轮 `span.block` 包裹、`nav.outline` 幽灵、`.block` 缺 `flex-direction` 三个 bug 都是这么发现的）。构建通过不等于渲染正确。

页内 Typst 语法参照 `dev/docs/typst.typ`（已写好的教程），不重复记录。

## Out of scope

- **全量 API 参考** —— 明确推迟。是否用 rustdoc 承载、怎么和站点共存，留待后续单独决策。
- **文档站基础设施的重构** —— `<toc-component>` + fetch + shadow DOM 的架构问题（每个页面的导航树拖着 9 个 `<link>` 但只有 1 个起作用；shadow 隔离导致 CSS 必须重复一份；URL 比对导致当前页高亮失配），以及构建器层的 10 个问题（typst 版本未锁、exit status 丢弃、输出目录从不清理等），是独立的一件事。
- **`lib/` 的 `///` 注释风格** —— 按既有约定，公开 API 的注释不写节点 ID（那是实现细节）。若将来走 rustdoc，这条仍然成立。