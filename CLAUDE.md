# CLAUDE.md

本文件是给 agent 的开发指南。

> **与 `CLAUDE_alt.md` 的关系**：仓库根下还有一个 `CLAUDE_alt.md`。**两处冲突时以 `CLAUDE_alt.md` 为准**；本文件的其余内容（agent 的工作指引、命令、目录结构速查）不变。叙述性的文档在已发布的站点上，不在这两个文件里。

**面向人的文档在 <https://mzverse.github.io/rust2genshin/>** —— 叙述性的说明（构建流程、调试技巧、加节点/加原生调用、节点图语义）都在那边。这里只留 agent 干活必需的信息和指路，避免两处各写一份然后分叉。

改动本文件时：**每条断言都要能对应到代码**，写下 `文件:行`。这个文件曾经漂移得很厉害（工具链版本、目录结构、编译路径全对不上），不要再让它变成叙述性文档。

## 项目概述

`rust2genshin` 是一个**非官方**的 **rustc codegen 后端**：以 `Box<dyn CodegenBackend>` 形式被 `rustc -Zcodegen-backend` 加载，把目标 crate 的 Rust 代码编译成原神千星奇域的节点图资产 `.gia`。目前仅支持服务器节点图。

后端**不产生机器码**（同 rustc 自带的 `DummyCodegenBackend`）：`codegen_crate` 导出 `.gia` 后返回空的 `CompiledModules`，`link` 走共享的 `link_binary`，产物是仅含 rmeta 的 rlib。详见 `core/src/compile/backend.rs` 的文件头注释。

入口符号：`core/src/compile/backend.rs::__rustc_codegen_backend`。

参考项目：**非官方** 的 <https://github.com/Wu-Yijun/Genshin-Impact-Miliastra-Wonderland-Code-Node-Editor-Pack>，是节点 ID、proto 字段、`.gia` 字节格式的外部依据（它和本项目都与米哈游无关联）。

涉及节点 ID、proto 字段含义或 wire 格式问题时**先查该包再看本项目**。`tools/node_data/nodes.json` 只有结构没有描述，要查节点语义得用它 `utils/node_data/data.json`。详见站点「参考项目」页。

## 文档地图

| 想知道 | 去哪 |
|---|---|
| 构建流程、工具链、CI | <https://mzverse.github.io/rust2genshin/dev/build.html> |
| 调试技巧、增量构建、Python 工具 | <https://mzverse.github.io/rust2genshin/dev/debugging.html> |
| 加一个节点 | <https://mzverse.github.io/rust2genshin/dev/add-node.html> |
| 加一个原生调用、`#[event(N)]` | <https://mzverse.github.io/rust2genshin/dev/add-native.html> |
| **lib API 的约定**：注释风格、命名、句柄类型约束、已踩的坑 | <https://mzverse.github.io/rust2genshin/dev/conventions.html> |
| 参考项目（节点 ID / proto / `.gia` 格式的依据） | <https://mzverse.github.io/rust2genshin/arch/reference.html> |
| **viewer**：怎么跑、怎么看中间态 IR、布局与优化的设计取舍 | <https://mzverse.github.io/rust2genshin/dev/viewer.html> |
| **playground**：怎么跑、配置项、webhook、磁盘怎么算 | <https://mzverse.github.io/rust2genshin/dev/playground.html> |
| 节点图本身的语义 | <https://mzverse.github.io/rust2genshin/arch/low/index.html> |
| 怎么用（写节点图、句柄类型、可空性、Entity/Player） | <https://mzverse.github.io/rust2genshin/user/index.html> |
| 文档站怎么改（含三个 CSS 坑） | <https://mzverse.github.io/rust2genshin/dev/docs/index.html> |

站点源码在 `docs/typst/`，逐页对应上面的 URL。**加内容优先加到站点**，本文件只留指路。

## 常用命令

```shell
# 端到端构建：重编译后端 cdylib + 用后端编译 demo
# 注意：build-demo 有两个 bin，必须带 --bin
cargo run -p build-demo --bin beyond-server

# 单独重新构建后端 cdylib（本地快速验证 backend 改动）
cargo build -p rust2genshin

# 解码 .gia 检查产物（需要 python + protobuf）
python tools/gia2txt.py target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia
python tools/gia2txt.py target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia --no-unknown
python tools/diff_gia.py export.gia target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia

# 构建 viewer（不在 default-members 里）
cargo build -p rust2genshin-viewer

# 起 playground 服务（需要本机有 docker 与 git；见站点「playground」页）
cargo run -p rust2genshin-playground
```

产物路径是 `target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia`（`<target 名>/release/<crate 名>.gia`）。

不要手动传 `-Coverflow-checks=off` 或 `-Zalways-encode-mir=yes` —— 构建脚本和 CI 里都没有，也不需要有（原因见站点「调试」页）。原因是设计上的：后端把 `i32` 算术编译成 unchecked 节点（`compile/func.rs:142-147` 把 `AddUnchecked` / `AddWithOverflow` 折到普通 `node_add`），`Assert` 终结符又在翻译时直接丢弃，溢出在节点图里本来就是按 wrapping 语义走的。

## 工作区结构

```
core/                rustc 后端 (cdylib `rust2genshin` + bin `rust2genshin-linker`)
  build.rs           prost-build 生成 proto 绑定
  proto/asset.proto  .gia 的 protobuf 层
  src/
    lib.rs           #![feature(rustc_private)] + extern crate rustc_*
    asset.rs         AssetBundle：.gia 的 20 字节头 + 4 字节尾封装；save 字节比对保 mtime
    value.rs         NativeValue trait + NativeKind 枚举（节点图类型体系）
    structure.rs     StructureDefinition / StructField，assemble/destructure/modify 节点
    linker.rs        bin `rust2genshin-linker`（target spec 的 linker 字段指向它）
    compile/
      mod.rs         Compiler + compile() 入口；get_expn_macro_attr
      backend.rs     CodegenBackend impl + __rustc_codegen_backend
      ir.rs          NodeGraphIr / Optimizer / lower
      link.rs        Linker / Target：布局 + 编码成 AssetBundle
      func.rs        CompilingFn：逐 basic_block / terminator 把 MIR 翻译成节点；FnDecl 在这
      place.rs       CompiledLocal / CompilingLocals：Place 投影展平
      ty.rs          类型 mangling、compile_ty
      native.rs      compile_native_ty / compile_native_call
      helper.rs      诊断等公共辅助
    node/            节点定义（NodeKind 的构造函数）
      mod.rs         NodeKind / Node / Connection / Link / NodeRef；NODE_ID_BEGIN = 1
      arithmetic.rs  算术域
      list.rs        列表节点（泛型变体：shell 固定、kernel 与选择器随元素类型变）
      control.rs     控制流（If / 分支）
      execution.rs   执行节点（局部变量 get/set、log）
      query.rs       查询节点
      trigger.rs     事件触发
      composite.rs   复合节点（decl + graph_ref）
      decl.rs        NodeDecl 与引脚声明
      hidden.rs      内部节点（变量修改事件）
      layout.rs      图的布局
      client.rs      客户端节点占位

lib/                 rust2genshin-lib，目标 crate 的运行时 API（no_std 友好）
  src/               asynchronous / boxed / dict / entity / event / list / math / player / sc / task
lib-internal/        proc-macro crate：native / native_calc / native_exec / native_enum /
                     event / event_listener / async_loop / asynchronous
demo/                示例目标 crate —— 被后端编译成 .gia 的就是它（节点图写在这里）
build-demo/          一键构建脚本（bin: beyond-server / classic-server）
viewer/              ogia-viewer（crate `rust2genshin-viewer`，bin `rust2genshin_viewer`；不在 default-members；见站点「viewer」页）
playground/          rust2genshin-playground：文档站「摆弄」页的后端（WebSocket +
                     一次性 docker 容器跑 cargo build；crate `rust2genshin-playground`；不在 default-members；见站点「playground」页）
docs/                文档站构建器（bin: compile / watch）+ docs/typst/ 站点源码
tools/               (被 .gitignore) Python 调试工具，需要 protobuf 运行时
```

`genshin-unknown-{beyond,classic}-server.json.template` 是目标平台描述模板，`build-demo` 把 `{backend}` 换成后端 dll 的绝对路径后写出 `.json`。后端由该 spec 的 `default-codegen-backend` 字段选中。

## 编译路径（MIR → .gia）

`core/src/compile/mod.rs` 是入口。

1. `codegen_crate(tcx)`（`compile/backend.rs`）在 analysis 之后调 `compile::compile(tcx)`。注意 `cargo check` 不会走到这里。
2. `compile::compile`：先查 crate 属性 —— **不是 `#![no_std]` 只 warn**（只有 proc-macro / test crate 才真的跳过）。再 `Compiler::new`。
3. `Compiler::new`：在依赖里找 `rust2genshin_lib`（`LIB_NAME` 常量），找不到直接报错。
4. `Compiler::run`：
   - 入口点来自 `tcx.exported_generic_symbols(LOCAL_CRATE)` + `exported_non_generic_symbols(LOCAL_CRATE)`（**不是**扫描 HIR 找 `contains_extern_indicator`）
   - 每个入口先试 `compile_native_call`，是原生调用就跳过；否则 `touch_fn` 递归编译依赖
   - 入口带 `#[event_listener]` 时，取第一个参数的类型的 `#[event(id)]`，在主图里插入 trigger 节点 + 复合节点调用块
5. `Compiler::save` 写的是**中间态** `.ogia`（bincode 序列化的 `Target`，viewer 读它）。
6. 最终 `.gia` 由 `compile/link.rs` 写出：`layout` 布局 → 编码成 `AssetBundle` → `assets.save(output)`。`AssetBundle::save` 内容不变时跳过写盘以保留 mtime。

`fn` → 复合节点：**每个**被编译到的函数都成为独立的复合节点资产（decl + 内部子图，靠 `graph_ref` 关联），调用点是 `link_node`（`compile/ir.rs:243`）里对 `touch_fn`（`compile/link.rs:95`）产物的引用。后端**不做内联**；`exported`（`compile/mod.rs:348`）只在 `compile/link.rs:72` 决定该图是否 `set_primary`（主资产 vs 依赖资产）。函数是否在调用点消失由 rustc 的 MIR 内联器决定（跑在后端之前）。

## 关键概念

- **`NativeValue` / `NativeKind`**（`core/src/value.rs`）：节点图的类型体系。`NativeKind` 是枚举，`NativeValue` 是各类型实现的 trait。（旧文档里的 `AnyValue` / `INativeValue` / downcast 已经不存在。）
- **`IrKind`**（`compile/ir.rs`）：IR 层的类型。`Native(NativeKind)` 是原生类型，`List(Box<IrKind>)` 是列表（元素也是 IR 类型，降级时才落成 `NativeKind::List`），`Adt(key)` 是结构体。`lower()` 把它降成 `NativeKind`。
- **`NodeKind`**（`core/src/node/mod.rs`）：节点的 id + 进/出口数 + 入参类型 + 出参类型。**运行时构造**出来的，没有登记表。构造器 `new` / `simple` / `expr` / `func` / `procedure` / `trigger`。
- **`NodeRef`** 序列化为 `id + NODE_ID_BEGIN`，`NODE_ID_BEGIN = 1`。
- **`CompiledLocal` / `CompilingLocals`**（`compile/place.rs`）：把 MIR 的 `Place` 投影（元组字段、struct 字段、引用解引）递归展平成节点图局部变量。闭包参数因此变成 `#closure + args` 的双层。
- **`FnDecl`**（`compile/func.rs`）：函数对外签名 —— 入参、`params` / `ret` 扁平列表、`proxies_in` / `proxies_out`。
- **`Optimizer`**（`compile/ir.rs`）：`compile_fn` 里只调 `verify`（`compile/mod.rs:343`）；`optimize()` 到不动点（`ir.rs:447`）是 `lower` 里跑的（`ir.rs:283`），`lower` 由 `link.rs` 调用。
- **`.gia` 容器**（`core/src/asset.rs`）：5 × u32 大端 **20 字节头**（总大小、schema 版本、0x0326、文件类型 3、proto 长度）+ proto 数据 + **4 字节尾** `0x0679`。总共 24 字节开销。
- **proto 绑定**：改了 `core/proto/asset.proto` 后要 `cargo build` 让 `prost-build` 重新生成（`OUT_DIR/rust2genshin.rs`，在 `core/src/asset.rs` 里 `include!`）。Python 侧不用重生成，`tools/gia.py` 运行时读 proto。

## 扩展

详细步骤见站点，这里只列落点：

- **加节点**：`core/src/node/<域>.rs` 加构造函数 → 在 `compile/func.rs`（操作符）或 `compile/ir.rs`（控制流）的翻译处用。新类型要在 `core/src/value.rs` 加 `NativeKind` 变体 + `impl NativeValue`。
- **加原生调用**：`lib/` 里加带 `#[native_calc(N)]` / `#[native_exec(N)]` 的无体函数即可，**不用改 core** —— 编译器按签名合成节点。只有 `#[native("名字")]` 形式要在 `compile/native.rs` 的 `compile_native_ty` / `compile_native_call` 里加分支
- **加事件**：`lib/src/event/<分组>.rs` 里加带 `#[event(N)]` 的结构体

## 调试要点

- 后端通过 `tcx.dcx().warn/err` 输出诊断（例：`compile/mod.rs` 的 `compile()` 里那条 no_std 警告）
- **`Assert` / `panic` 终结符被直接跳过**（`compile/func.rs` 打 `Ignored assert` / `Ignored panic`）。溢出检查、越界检查的断言都不会进图，编译照样成功
- `demo/build.rs` 对 `src/`、后端 cdylib、`.gia` 都声明了 `cargo:rerun-if-changed`；`AssetBundle::save` 的字节比对让 `.gia` 的 mtime 只在内容真的变了时才前进，避免重建死循环
- `tools/gia2txt.py` 输出里的数字键（`2: 123`）代表 proto 未声明字段 —— 用 `tools/dev/wire_tree.py` 识别类型，`tools/check_unknown.py` 统计覆盖度
- 改 `docs/typst/` 时后台挂 `cargo run -p rust2genshin-docs --bin watch`，serve 3000 并增量编译；**查错看它的输出**

## 设计文档

`docs/superpowers/` 下分 `specs/`（`YYYY-MM-DD-<topic>-design.md`）和 `plans/`（`YYYY-MM-DD-<topic>-implementation.md`），记录功能设计与实施计划；新功能在动手前应先写设计稿。
