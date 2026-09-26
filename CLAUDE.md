# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

`rust2genshin` 是一个**非官方**的 **rustc codegen 后端**:以 `Box<dyn CodegenBackend>` 形式被 `rustc -Zcodegen-backend` 加载,把目标 crate 的 Rust 代码编译成原神的节点图资产文件 `.gia`。当前仅支持服务器节点图。

参考项目:**非官方** 的 `D:\projects\Genshin-Impact-Miliastra-Wonderland-Code-Node-Editor-Pack`,提供:
- 节点 ID / 引脚 / 分类的真相源 —— `tools/node_data/nodes.json` 由该包 `game_nodes.ts` 解析而来;`core/src/asset/node_graph/*.rs` 里的 `/// ID N` 注释对齐它。
- `core/proto/asset.proto` 的字段定义由真实 `.gia` 导出文件逆推。
- `.gia` 文件 24 字节头 / 4 字节尾的格式参考该包 `utils/protobuf/decode.ts` 的 `unwrap_gia/wrap_gia`。

写代码 / 改 proto / 加节点时,涉及节点 ID 或 wire 格式问题先查该参考包;涉及游戏内部数据时保持现状(从导出文件逆推,不复制受版权保护的素材)。

入口符号:`core/src/backend.rs::__rustc_codegen_backend`(暴露为 cdylib `rust2genshin`)。后端**不产生机器码**(同 `DummyCodegenBackend`),link 阶段返回空 `CompiledModules`。

## 工具链与依赖

- **Rust nightly** 由 `rust-toolchain.toml` 钉死为 `nightly-2026-09-17`,必须安装 `rustc-dev`、`rust-src`、`llvm-tools-preview` 三个组件。
- 后端依赖 `#![feature(rustc_private)]` 与多个 `rustc_*` crate(`rustc_driver` 等只在工具链内随 `rmeta` 发布),靠 dylib 复用符号(同 miri 做法)。
- `core/proto/asset.proto` 通过 `core/build.rs` 的 `prost-build` 在编译时生成 Rust 绑定(`OUT_DIR/rust2genshin.rs`),由 `core/src/asset/mod.rs::generated` 引入。
- CI(GitHub Actions `build.yml`)在 Linux 上安装 `protobuf-compiler` 后跑 `cargo run -p build-demo`。
- Python 工具(`tools/`,**被 `.gitignore` 忽略**)需要 `protobuf` 运行时(`pip install protobuf`)。

## 常用命令

```shell
# 端到端构建:重新编译后端 cdylib + 用后端编译 demo,产物是 target/rust2genshin_demo.gia
cargo run -p build-demo

# 单独重新构建后端 cdylib(用于本地快速验证 backend 改动)
cargo build -p rust2genshin

# 跑 demo 测试(在 build-demo 里也会触发)
cargo test -p rust2genshin-demo

# 直接用后端编译 demo(不经过 build-demo 的 cargo 套娃)
cargo rustc --release -p rust2genshin-demo -- \
    -Coverflow-checks=off \
    -Zalways-encode-mir=yes \
    -Zcodegen-backend=$(pwd)/target/debug/librust2genshin$(rustc -vV | sed -n 's/host: //p')

# 解码 .gia 检查产物(需要 python + protobuf)
python tools/gia2txt.py target/rust2genshin_demo.gia
python tools/gia2txt.py target/rust2genshin_demo.gia --no-unknown
python tools/diff_gia.py export.gia target/rust2genshin_demo.gia
```

> `-Coverflow-checks=off` 必备 —— 默认会让 `i32` 四则运算出现 `CheckedAdd (i32, bool)` 元组,MIR 编译路径无法处理。
> `-Zalways-encode-mir=yes` 是为了能拿到非泛型实例的 MIR。

## 工作区结构

```
core/                rustc 后端实现 (cdylib)
  src/
    backend.rs       CodegenBackend trait impl + __rustc_codegen_backend
    parser.rs        HIR 访问器(导出 dump / 入口点收集;主编译路径未使用)
    asset/           节点图资产层
      mod.rs         AssetBundle 容器、.gia 文件 24 字节头/尾的封装(save 字节比对保 mtime)
      generated/     prost-build 生成的 proto 绑定
      value.rs       AnyValue 类型体系(每种节点图类型一个 downcast 实现)
      structure.rs   StructureDefinition + StructField + assemble/destructure/modify 节点
      node_graph/
        mod.rs       NodeGraph / Node / NodeKind / Connection / ValueIn
        arithmetic.rs   算术域节点(向量加减、比较、模、位运算…)
        control.rs      控制流节点(If/分支)
        execution.rs    执行节点(局部变量 get/set、log)
        query.rs        查询节点(获取局部变量等)
        trigger.rs      事件触发节点
        composite.rs    复合节点(decl + graph_ref)
        decl.rs         NodeDecl 与引脚声明
        client.rs       客户端节点占位
        hidden.rs       内部使用节点(变量修改事件)
    compile/         MIR → 节点图编译
      mod.rs         Compiler struct + run/touch_fn/compile_fn 入口
      func.rs        CompilingFn:逐 basic_block / terminator 把 MIR 翻译成节点
      place.rs       CompiledLocal / CompilingLocals:把 Place 投影展平为本地变量
      ty.rs          类型 mangling、AnyValue 编译(compile_ty)、IR 占位类型 ValueIrNever/Adt/Mut
      optimize.rs    Optimizer:在编码前对图做简化
      native.rs      #[native] / #[native_calc] / #[native_exec] / #[native_enum] 调用派发

lib/                 rust2genshin-lib,目标 crate 的运行时 API(no_std 友好)
  src/               entity / math / list / dict / boxed / player / event
  Cargo.toml         只依赖 lib-internal

lib-internal/        rust2genshin-lib-internal,proc-macro crate
  src/lib.rs         #[native] / #[native_calc] / #[native_exec] / #[native_enum] / #[event] / #[event_listener]

demo/                示例目标 crate —— 这就是被后端编译成 .gia 的东西
  src/lib.rs         pub fn + #[unsafe(no_mangle)] / #[event_listener] 入口

build-demo/          一键构建脚本
  src/main.rs        1) cargo build -p rust2genshin  2) clean demo  3) cargo rustc -p demo -- -Zcodegen-backend=...

tools/               (被 .gitignore) Python 调试工具,详见 tools/README.md
  gia.py             .gia 容器 + proto3 描述符运行时构建(自包含,无需 protoc)
  gia2txt.py         .gia → 文本(优先用)
  view_gia.py / diff_gia.py / check_unknown.py / dev/* / node_data/*
```

## 编译路径(MIR → .gia)

`core/src/compile/mod.rs` 是入口。`codegen_crate(tcx)` 阶段:

1. 校验目标 crate 是 `#![no_std]` 且不是 proc-macro / test,否则提前返回。
2. `Compiler::new(tcx)` 在依赖里找 `rust2genshin-lib`(`LIB_NAME` 常量),找不到就硬报错。
3. `Compiler::run()`:
   - 遍历整个 HIR,挑出 `contains_extern_indicator()` 的 `fn`(包括 `#[event_listener]` 注入的 `#[unsafe(no_mangle)]`)。
   - 对每个 `Instance::mono` 调 `touch_fn`,递归编译依赖。
   - `event_listener` 的事件参数类型若带有 `#[event(id)]`,在 main 图里插入 trigger 节点 + composite 调用块。
   - 若 main 图非空,标记为 primary asset。
4. `Compiler::save(out_dir)` 把 `AssetBundle` 编码为 `<crate_name>.gia`(字节级比对保 mtime)。
5. `compile_fn`:为每个函数建 `CompositeNodeGraph` → 展平所有 locals(`place.rs`)→ 逐 basic_block 调 `CompilingFn::compile_basic_block` 翻译语句 / 调 `compile_terminator` 翻译终结符 → `Optimizer::lower` 收尾。

`fn` → `CompositeNodeGraph` 的一一映射:`#[unsafe(no_mangle)]` 标记的函数成为 primary 的复合节点存根(decl + 内部子图,靠 `graph_ref` 关联);非 export 的辅助函数被内联进调用方图。

## 关键概念

- **AnyValue**:每个节点图类型一个 struct 实现 `Value` trait(`get_server_type` / `get_client_type` / `encode_storage` / `encode_field_value`)。新类型只需写一个 + 注册。`downcast` crate 提供运行时类型查询。
- **NodeKind / Node / Connection / Link**:节点图基础类型。`NodeRef` 序列化为 `id + 1`(见 `NODE_ID_BEGIN`)。
- **CompiledLocal<LocalRef>** / **CompilingLocals**:把 MIR 的 `Place` 投影(元组字段、struct 字段、引用解引)递归展平成节点图局部变量节点,这是为什么闭包参数变成 `#closure + args` 的双层。
- **FnDecl**:函数对外签名 —— 控制入参、`params` / `ret` 的扁平列表、`proxies_in` / `proxies_out`(用于参数转发与多返回值)。
- **Optimizer**:在节点图生成后、`Asset` 编码前对图做简化(消冗余 local setter 等)。
- **proto 绑定**:改了 `core/proto/asset.proto` 后必须重新跑 `cargo build` 让 `prost-build` 重新生成。`tools/gia.py` 在运行时重读 proto 文件,所以 Python 端不需要重生成。

## 添加新节点类型(常见改动)

1. 在 `core/src/asset/value.rs` 加一个 `ValueXxx` + `AnyValue` impl。
2. 在 `core/src/asset/node_graph/<domain>.rs` 用 `NodeKind::new/expr/func/procedure/trigger` 声明 `static NODE_X: LazyLock<NodeKind>`。
3. 若需要调用该节点,在 `core/src/compile/func.rs` 的 Rvalue/MIR 翻译处加分支。

## 添加新原生调用

`lib-internal/src/lib.rs` 提供属性宏。新原生类型在 `native.rs::compile_native_ty` 加分支,新原生调用在 `native.rs::compile_native_call` 加分支。`#[event_listener]` 会自动给函数加 `#[unsafe(no_mangle)]` + 重写 span(call_site),所以编译路径靠 `get_expn_macro_attr` 从 span 取宏参数。

## 调试技巧

- 后端通过 `tcx.dcx().warn/err` 输出诊断;`-Coverflow-checks=off` 已去掉默认溢出检查告警。
- `core/src/asset/mod.rs::AssetBundle::save` 内容不变不写盘(mtime 保留),保证 cargo 增量缓存不抖动;`demo/build.rs` 显式 `cargo:rerun-if-changed` 后端 cdylib 与 `.gia`。
- Python 工具(`tools/gia2txt.py`)输出数字键(`2: 123`)代表 proto 未声明字段 —— 用 `tools/dev/wire_tree.py` 识别类型,`tools/check_unknown.py` 统计覆盖度。
- 节点 ID 编号来自外部节点编辑器包(`tools/node_data/nodes.json`);`arithmetic.rs` 里的 `/// ID N` 注释对齐它。

## 设计文档

`docs/superpowers/` 下按 `YYYY-MM-DD-<topic>-{design,implementation}.md` 命名,记录功能设计 + 实施计划;新功能在动手前应先写设计稿。
