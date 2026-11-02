#import "/lib/lib.typ": *;
#let title = [编译管线];
#show: template;

#set raw(lang: "rust");

`codegen_crate` → `compile::compile` → `Compiler::run` → `compile_fn` → `lower`。
入口在 `core/src/compile/mod.rs`。

这一页讲每一段为什么存在、放弃了什么。要动手改某一处，去#link("/dev/index.html")[开发]栏。

= 编译哪些函数

`tcx.collect_and_partition_mono_items(())`（`compile/mod.rs:160`）—— rustc 给每个
后端的那份 mono item 清单，LLVM / cranelift / jvm 用的也是它。

*不是* `exported_generic_symbols` / `exported_non_generic_symbols`。那两张表只有
「导出符号」，而本 crate 里 `pub` 的非泛型函数、它们的可达调用、以及本 crate
实例化的上游泛型特化都不在里面 —— 用它们会出现「调用了但没编」的洞。

从 mono items 里筛掉三类：

- `core` / `alloc` / `std` 的实现 —— 它们不是用这个后端编的，没有 `.ogia`；
  在节点图里由原生节点承担
- shim（drop glue 之类自动生成的胶水）—— 不是用户函数，不该各占一个复合节点资产
- 静态变量 / 全局汇编 —— 节点图里没有对应表示

`exported` 标志另算：`codegen_fn_attrs(...).contains_extern_indicator()`
（`compile/mod.rs:377`），决定该图是主资产还是依赖资产。

用 rustc 给的清单而不是自己扫 HIR，好处是「该编什么」只有一份定义；代价是要
理解上面那三条过滤 —— 少一条就会多出莫名其妙的空图，或者编不出 `lib` 里的函数。

前置：`Compiler::new` 在依赖里找 `rust2genshin_lib`，找不到直接报错
（`compile/mod.rs:116-138`）。这是硬依赖：`lib` 里那些无体函数就是节点声明的
唯一来源，没有它连 `log` 都编不出来。

= 一个入口怎么展开

1. 先试 `compile_native_call` —— 是原生调用就跳过，不建图
2. 否则 `touch_fn` 递归编译它依赖的函数
3. 带 `#[event_listener]` 的入口：取第一个参数类型的 `#[event(id)]`，在主图里插
   trigger 节点 + 复合节点调用块

*`fn` → 复合节点*：*每个*被编译到的函数都成为节点图里一个独立的复合节点
（`decl` + 内部子图，靠 `graph_ref` 关联）。后端**不做内联** —— 内联是 rustc 的
MIR 内联器的职责，它跑在后端之前。后端再做一遍等于把「哪个函数还在」这件事
变成两个作者。

`link_node`（`compile/optimize.rs:225`）把调用点换成 `touch_fn`（`compile/mod.rs:234`
+ `compile/link.rs:206`）产出的复合节点资产。`exported`（`compile/mod.rs:377`）只决定
该图是否 `set_primary`（`compile/link.rs:94`），即主资产还是依赖资产，跟成不成图无关。

= compile_fn

`compile/mod.rs:260`。顺序：

1. `solve_local` 逐个建局部变量（见#link("/arch/compile/locals.html")[局部变量]）
2. 逐 `basic_block`、逐 `terminator` 把 MIR 翻译成节点
3. 收尾调 `Optimizer::verify`（`compile/optimize.rs:127`）—— 目前只转发给
   `graph.verify()`，别的都还没实现（`compile/mod.rs:372`）

`Assert` / `panic` 终结符被*直接跳过*（`compile/func.rs` 打 `Ignored assert` /
`Ignored panic`），不建节点、不报错。

这是一个有意识的取舍：节点图里的运算节点本身就是 unchecked 语义（溢出回绕、
除零给 0），断言要真想表达出来，得先有一个「报错」节点可用 —— 目前没有。
与其编出错的东西，不如明确地丢掉，并且把这件事写进使用文档的
#link("/user/compatibility.html")[兼容性]页。

= lower

`optimize.rs:306` 的 `lower_ir`（`lower` 是它的外壳，`optimize.rs:599`），由
`compile/link.rs` 调用：

1. 图变量先登记进 `FnDecl::variables` —— 下一步会把图变量节点换成 native 节点，
   之后就认不出名字了，而链接期要拿这张表并进主图的黑板表
2. 每个 `IrNodeId::Fn` 节点 `link_node` —— 展开成它的内部图
3. `optimize()` 到不动点（`optimize.rs:728`）
4. `verify_types()` —— 优化会改接数据流，改接后的类型契约要当场校验
5. `lower_loops` —— 把控制流环换成有限循环节点。放在 optimize 之后：优化器先
   消掉 Nop / IF 并重连，改写面对的是干净的控制流
6. 再 `optimize()` 一遍 —— 循环内部现在是一段独立的执行流，里面有优化器自己留下的中间节点
7. `graph.verify()`、`compact_value_imports` / `compact_value_exports`
8. 列表类节点按元素类型展开（元素是结构体时要往图里插结构体依赖声明），
   图变量节点同理
9. 最后把 `IrNodeId` 换成 `NativeNodeId`、`IrKind` 换成 `NativeKind`，不改变任何
   连线或语义

之后 `layout` 排版、编码成 `AssetBundle`、写 `.gia`。格式见#link("/arch/asset.html")[资产格式]。

这么切而不是一步到底，是因为中间每一步都要能单独观察 —— viewer 的「各条消除
规则各一个按钮」正是建立在这个切分上（见#link("/arch/viewer.html")[viewer 的设计]）。
合并成一个函数会更短，但出错时只剩「图坏了」这一条信息。

第 1 步和图变量的语义直接相关：节点图变量是*图级*的，复合节点内部读写的那份
存在主图的变量表里，按名字索引 —— 所以同名变量在各函数里指的是同一个，类型必须
一致。停在第 1 步之后才 lower，是因为 lower 之后名字就丢了。

= 这一组

#list(
    link("/arch/backend.html", [后端为何这样接]) + [ —— 为什么是 codegen 后端、为什么不产机器码],
    link("/arch/compile/locals.html", [局部变量]) + [ —— 为什么复合类型必须打散成标量槽],
    link("/arch/compile/loops.html", [循环改写]) + [ —— 环怎么变有限循环、计数循环怎么认],
    link("/arch/compile/cast.html", [类型转换]) + [ —— `as` 的三条路与转换表],
    link("/arch/ir/index.html", [IR]) + [ —— 中间形态为什么不能省],
    link("/arch/calling-convention.html", [调用约定]) + [ —— 复合节点的对外形态],
)
