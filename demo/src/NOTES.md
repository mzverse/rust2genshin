# demo 的组织方式

`src/lib.rs` 是入口,底下按主题分成几个模块,每个模块导出一个**完整的示例程序**
—— 有输入、有过程、有输出。示例内部那些「只为了测一个后端特性」的小函数都收在
模块里,不对外暴露。

编译产物是 `target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia`。
每个示例函数是一个独立的复合节点资产,主图 `rust2genshin_demo` 是实体节点图,
入口是 `on_entity_created`。

## 模块

| 模块 | 示例 | 覆盖的东西 |
|---|---|---|
| `sorting` | `sorting_demo` / `times99` | 嵌套循环、分支、数组按下标读写 |
| `containers` | `container_demo` | `List` 全套操作、数组、嵌套列表、结构体列表、析构 |
| `ownership` | `ownership_demo` / `ownership_drop_demo` | `Box` 的分配 / 读 / 借用 / 析构 |
| `values` | `value_demo` | 数组、元组、`&str`、`Option`、枚举、结构体、ZST、`usize` |
| `mutability` | `mutability_demo` | `&mut` 字段写、借用、`PartialMut`、`IndexMut` |
| `flow` | `flow_demo` / `coroutine_demo` | 循环与 `continue`、协程 |
| `unsupported` | (默认不编译) | 后端还没落地的缺口 |

## 编译

```shell
cargo run -p build-demo --bin beyond-server
```

`build-demo` 会先编后端 cdylib,再写目标平台描述(`genshin-unknown-beyond-server.json`),
最后用 `-Zbuild-std=core` 编 demo。**别用 `cargo build -p rust2genshin-demo` 单独编** ——
那样编不过(没有 `panic_handler`,而且不走本后端)。

想复现「不支持」的报错:

```shell
cargo build --release -p rust2genshin-demo \
  --target ./genshin-unknown-beyond-server.json \
  -Zunstable-options -Zjson-target-spec -Zbuild-std=core \
  --features unsupported
```

## 已知不支持

见 `src/unsupported.rs`。目前两条:

1. **`Box<结构体>`** —— 堆变量类型是 `List<Struct>`。图变量节点
   (`Get_Graph_Variable` ID 337 / `Set_Graph_Variable` ID 323)各有 153 个变体,
   里面没有结构体档(最靠后的是 `C<T:D<Pfb,L<Cfg>>>`),所以这种黑板变量在游戏里
   没有对应的节点。列表节点是有结构体变体的(见 `core/src/node/list.rs`),
   但图变量节点没有。

2. **`*b = v` 整体赋值** —— 节点图里没有写句柄,写元素走「写视图」
   (`list_set` / `Modify`)。由视图造出来的 `&mut` 是**假的**:读得了、借得了字段、
   调得了 `&mut self` 的方法,但整体赋值要真句柄,降级时报 `Unsupported`。
   要改一个值请用 `List::set` 或字段赋值。
