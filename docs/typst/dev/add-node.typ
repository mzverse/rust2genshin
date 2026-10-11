#import "/lib/lib.typ": *;
#let title = [加一个节点];
#show: template;

#set raw(lang: "rust");

节点在代码里就是一个 `NodeKind`：一个 id、几个控制流引脚数、入参类型列表、出参类型列表。它是*构造*出来的，不是登记在表里的。

要看两种典型用法，直接读 `core/src/node/arithmetic.rs` 和 `core/src/compile/native.rs`。

= 一、构造节点

在 `core/src/node/<域>.rs` 里写一个构造函数。现有的域：`arithmetic` / `control` / `execution` / `query` / `trigger` / `composite` / `decl` / `client` / `hidden` / `layout`。

```rust
pub fn node_add(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(200, vec![ty.clone(), ty.clone()], ty.clone());
    // ...按 ty 调整 kernel / 引脚
    result
}
```

四个按语义分的构造器，选哪个决定了节点有没有控制流：

- `expr(id, 入参类型, 出参类型)` —— 纯值节点，没有控制流。*每次取出参都会重新计算*，见#link("/arch/low/dataflow.html")[数据流与所有权]
- `func(id, 入参类型, 出参类型)` —— 有控制流，一进一出
- `procedure(id, 入参类型)` —— 有控制流，没有出参
- `trigger(id, 出参类型)` —— 事件触发，无入参、一个出口

底层还有 `NodeKind::simple(id, 控制流入, 控制流出, 入参类型, 出参类型)`，上面四个都是对它的封装；需要非标准的进/出口数量时直接用它。

`id` 是*游戏侧的节点 ID*。按约定写 `/// <名字>(ID <n>)` 的注释，和外部节点编辑器包的 `node_data/nodes.json` 对齐 —— 那是节点 ID / 引脚 / 分类的真相源。

= 二、在调用点用它

节点光构造出来不会出现在图里，得有人在翻译 MIR 时用它：

- 操作符（`+`、`*`、`&&`…）走 `core/src/compile/func.rs` 的 Rvalue 翻译
- 控制流走 `core/src/compile/ir.rs`

如果一个节点只是想让用户在 `lib/` 里调到一个函数，不用动这里 —— 那条路见#link("/dev/add-native.html")[加一个原生调用]。

= 三、类型要能编码

入参和出参的类型必须能变成 `NativeKind`（`core/src/value.rs` 里的枚举）。

现成的变体覆盖 `()`、`bool`、`i32`、`f32`、`i64`、`String`、`Vec3`，以及容器 `Vec<Box<dyn NativeValue>>` 和 `BTreeMap<Box<dyn NativeValue>, Box<dyn NativeValue>>`。

要加新类型：在 `core/src/value.rs` 里给它 `impl NativeValue`（现有实现参见文件末尾），并在 `NativeKind` 里加一个变体。

= 顺手确认

改完跑一遍端到端构建，看产物里有没有新节点：

```shell
cargo run -p build-demo --bin beyond-server
python tools/gia2txt.py target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia
```
