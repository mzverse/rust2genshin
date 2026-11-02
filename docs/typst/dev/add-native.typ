#import "/lib/lib.typ": *;
#let title = [加一个原生调用];
#show: template;

#set raw(lang: "rust");

让用户在 `lib/` 里调到一个函数、编译出来是一个节点。

= 三种写法

#table(
    columns: (auto, auto, auto, 1fr),
    inset: 6pt,
    fill: (_, y) => if calc.odd(y) { luma(235) },
    [*属性*], [*参数*], [*改 core?*], [*用在哪*],

    [`#[native_calc(N)]`], [节点 ID], [不用], [无控制流的节点],
    [`#[native_exec(N)]`], [节点 ID], [不用], [有控制流的节点],
    [`#[native("名字")]`], [字符串], [*要*], [签名不足以描述节点时],
)

前两个只差一个布尔：编译器按 Rust 签名直接合成
`NodeKind::simple(id, 控制流入, 控制流出, 入参类型, 出参类型)`。所以*不需要*
先在 `core/src/node/` 里声明这个节点，把 ID 写对就行。

```rust
impl Entity {
    #[native_calc(73)]
    pub fn current() -> Self;

    #[native_exec(70)]
    pub fn spawn_preset(id: Guid, list: List<i32>);
}
```

函数*没有实现* —— 属性宏会把函数体换成 `unreachable!()`，只留下签名当声明，
并加上 `#[inline(never)]` 和 `#[rustc_no_mir_inline]` 防止调用点被内联掉。

= 用这些属性的 crate 要两行

```rust
#![feature(register_tool)]
#![register_tool(rust2genshin)]
```

少了它们，编译器会报「cannot find crate `rust2genshin`」—— 那个提示看不出要干什么。
proc macro 只能改自己展开的那段，没法把提示挪到 crate 属性上。

= `#[native("名字")]` 的情形

字符串形式用于签名不足以描述节点的情况（类型相关、或参数/返回值不是直通）。
它在 `core/src/compile/native.rs` 里加分支：

- *调用* —— `compile_native_call` 的名字匹配：`power` / `to_string` / `enum_eq` /
  `free` / `list_get` / `list_set` / `list_insert` / `list_len` / `list_remove` /
  `box_heap_get` / `box_free_get` 等
- *类型* —— `compile_native_ty` 的名字匹配：`Guid` / `Entity` / `Faction` /
  `Config` / `Prefab` / `Vec3` / `List` / `Dict` / `PartialMut`

类型声明的写法是把一个不透明的外部类型挂上属性：

```rust
unsafe extern "Rust" {
    #[native("Entity")]
    pub(crate) type EntityInternal;
}
```

`PartialMut` 的三个内建（`partial_mut_from` / `partial_mut_deref` /
`partial_mut_deref_mut`）是例外：它们在 `compile_call` 里特判，不走
`compile_native_call`。因为 `&mut T` 在 IR 里是元组 `(LocalSetter, PartialMut<T>)`，
`partial_mut_from` 要把它*拆开*取读槽，而不是值直通。

= 泛型原生类型

`List` 是*泛型*原生类型：属性标在 `struct List<T>` 上，元素类型从调用点的泛型
实参里取：

```rust
#[native("List")]
pub struct List<T> { /* 字段在节点图里不存在 */ }
```

编译器把它编成 `IrKind::List(T)`，降级时才落成 `NativeKind::List`。因此
`[i32; N]` 和 `List<i32>` 是*同一个*类型，两者之间的 `transmute` 是空转换。

元素类型必须是节点图里有列表变体的原生类型 —— 整数、字符串、实体、GUID、
浮点、三维向量、布尔、配置、预制件、阵营。结构体、枚举、嵌套列表会报
`Unsupported list element type`（`core/src/node/execution.rs:46`）。

原生类型（句柄）的*析构*是空操作：它的字段在节点图里并不存在。

= 事件

事件标的是 *struct* 而不是函数：

```rust
#[event(307)]
pub struct TabSelectedEvent {
    pub entity: Entity,
    pub guid: Guid,
    pub index: i32,
    // ...
}
```

- `N` 同样是节点 ID
- 节点形态是 *trigger*：*没有入参*，一个出口
- *字段顺序必须和游戏端事件节点的出参顺序一致* —— 编译器按顺序把字段类型拼成 trigger 的出参
- struct 和字段都必须是 `pub`

落点在 `core/src/compile/mod.rs` 的 `run()`：遍历带 `#[event_listener]` 的入口时，
取第一个参数类型的 `#[event(N)]`，拼出 trigger，然后在主图里插入 trigger 和
对应的复合节点调用块。

= 限制

入参和返回值的类型都必须能变成 `NativeKind` —— 编译器在这两处直接 `.unwrap()`，
遇到转不了的类型会 panic。

= tool attribute 一览

宏把信息写成 tool attribute，后端用 `tcx.get_all_attrs()` 直接读结构化属性，
不回源码里抠（原因是#link("/arch/backend.html")[这里]）。

#table(
    columns: (auto, auto, 1fr),
    inset: 6pt,
    fill: (_, y) => if calc.odd(y) { luma(235) },
    [*属性*], [*参数*], [*用途*],

    [`rust2genshin::call("名字")`], [字符串], [签名不足以描述节点，按名字查（要改 core）],
    [`rust2genshin::calc(N)`], [整数], [无控制流的节点],
    [`rust2genshin::exec(N)`], [整数], [有控制流的节点],
    [`rust2genshin::ty("名字")`], [字符串], [标在类型上：原生类型],
    [`rust2genshin::enumty(N)`], [整数], [标在枚举上：`#[native_enum(N)]`],
    [`rust2genshin::event(N)`], [整数], [标在事件 struct 上],
    [`rust2genshin::listener`], [无], [标在 `#[event_listener]` 的函数上],
)

`enumty` 而不是 `enum`，因为 `enum` 是关键字，不能当路径段。

= 相关

`#[event_listener]` 是同一套机制的特例：它自动给函数加上
`#[unsafe(no_mangle)]`，把它变成导出入口。用法见#link("/user/events.html")[事件]。
