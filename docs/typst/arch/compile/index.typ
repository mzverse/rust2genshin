#import "/lib/lib.typ": *;
#let title = [编译管线];
#show: template;

#set raw(lang: "rust");

`codegen_crate` → `compile::compile` → `Compiler::run` → `compile_fn` → `lower`。入口在 `core/src/compile/mod.rs`。

= 入口点从哪来

`tcx.exported_generic_symbols(LOCAL_CRATE)` + `exported_non_generic_symbols(LOCAL_CRATE)`（`compile/mod.rs:172`）。

*不是*扫 HIR 找 `contains_extern_indicator` —— 那个函数在 `compile/mod.rs:348` 另有用途：给每个函数算 `exported` 标志。

前置：`Compiler::new` 在依赖里找 `rust2genshin_lib`，找不到直接报错（`compile/mod.rs:150-156`）。

= 一个入口怎么展开

1. 先试 `compile_native_call` —— 是原生调用就跳过，不建图
2. 否则 `touch_fn` 递归编译它依赖的函数
3. 带 `#[event_listener]` 的入口：取第一个参数类型的 `#[event(id)]`，在主图里插 trigger 节点 + 复合节点调用块

*`fn` → 复合节点*：*每个*被编译到的函数都成为节点图里一个独立的复合节点（`decl` + 内部子图，靠 `graph_ref` 关联）—— 后端不做内联，`link_node`（`compile/ir.rs:243`）把调用点换成 `touch_fn`（`compile/link.rs:95`）产出的复合节点资产。`exported`（`compile/mod.rs:348`）只决定该图是否 `set_primary`（`compile/link.rs:72`），即「主资产」还是「依赖资产」，跟成不成图无关。调用点是否真的存在，取决于 rustc 的 MIR 内联器跑在后端之前的结果。

= compile_fn

`compile/mod.rs:250`。顺序：

1. `solve_local` 逐个建局部变量（见#link("/arch/compile/locals.html")[局部变量]）
2. 逐 `basic_block`、逐 `terminator` 把 MIR 翻译成节点
3. 收尾调 `Optimizer::verify`（`compile/mod.rs:343`）—— 目前只转发给 `graph.verify()`，别的都还没实现

`Assert` / `panic` 终结符被*直接跳过*（`compile/func.rs` 打 `Ignored assert` / `Ignored panic`），不建节点、不报错。溢出检查、越界检查的断言因此都不进图。

= lower

`ir.rs:278`，由 `compile/link.rs` 调用：

1. 每个 `IrNodeId::Fn` 节点 `link_node` —— 展开成它的内部图
2. `optimize()` 到不动点（`ir.rs:283` → `ir.rs:447`）
3. `graph.verify()`
4. 把 IR 节点 id 换成真实节点 id（`IrNodeId::Local` / `Assemble` 这些抽象节点在这里落成具体节点）

之后 `layout` 排版、编码成 `AssetBundle`、写 `.gia`。格式见#link("/arch/asset.html")[资产格式]。

= 这一组的三页

#list(
    link("/arch/compile/locals.html", [局部变量]) + [ —— MIR 的 `Local` / `Place` 投影怎么变成节点图的局部变量],
    link("/arch/compile/cast.html", [类型转换]) + [ —— `as` 怎么变成转换节点],
)
