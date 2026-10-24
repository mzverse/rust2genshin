#import "/lib/lib.typ": *;
#let title = [加一个原生调用];
#show: template;

#set raw(lang: "rust");

这是让用户在 `lib/` 里调到一个函数、而编译出来是一个节点的做法。

`lib/` 里的原生函数*没有实现* —— 属性宏会把函数体换成 `unreachable!()`，只留下签名当声明：

```rust
impl Entity {
    #[native_calc(73)]
    pub fn current() -> Self;

    #[native_exec(70)]
    pub fn spawn_preset(id: Guid, list: List<i32>);
}
```

宏还会加上 `#[inline(never)]` 和 `#[rustc_no_mir_inline]`，避免这个声明被内联掉、调用点消失。

= 三个宏的区别

- `#[native_calc(N)]` —— 参数是节点 ID（整数），*无*控制流，不用改 core
- `#[native_exec(N)]` —— 参数是节点 ID（整数），*有*控制流，不用改 core
- `#[native("名字")]` —— 参数是字符串名字，控制流视分支而定，*要*改 core

前两个只差一个布尔：编译器直接按 Rust 签名合成节点 `NodeKind::simple(id, control, control, 入参类型, 出参类型)`。所以*不需要*先在 `core/src/node/` 里声明这个节点，把 ID 写对就行。

= `#[native("名字")]` 为什么要改 core

字符串形式用于签名不足以描述节点的情况（类型相关、或参数/返回值不是直通）。它需要在 `core/src/compile/native.rs` 里加分支：

- *调用* —— `compile_native_call` 里的名字匹配，现有的有 `power` / `to_string` / `enum_eq` / `free` / `list_get` / `list_set` / `list_insert` / `list_len`
- *类型* —— `compile_native_ty` 里的名字匹配，现有的有 `Guid` / `Entity` / `Faction` / `Config` / `Prefab` / `Vec3` / `List` / `Dict`

类型声明的写法是把一个不透明的外部类型挂上属性：

```rust
unsafe extern "Rust" {
    #[native("Entity")]
    pub(crate) type EntityInternal;
}
```

= 泛型原生类型

`List` 是*泛型*原生类型：属性标在 `struct List<T>` 的定义上，元素类型 `T` 从调用点的泛型实参里取：

```rust
#[native("List")]
pub struct List<T> { /* 字段在节点图里不存在 */ }
```

编译器把它编成 IR 层的 `IrKind::List(T)`（元素也是 IR 类型），降级时才落成 `NativeKind::List`。因此 `[i32; N]` 和 `List<i32>` 是*同一个*类型，两者之间的 `transmute` 是空转换 —— 数组字面量可以直接当列表用。

元素类型必须是节点图里有列表变体的原生类型（整数、字符串、实体、GUID、浮点、三维向量、布尔、配置、预制件、阵营）；结构体、枚举、嵌套列表都不行，会报 `Unsupported list element type`。

原生类型（句柄）的*析构*是空操作：它的字段在节点图里并不存在，析构胶水去读字段只会读到不存在的东西。

= 限制

入参和返回值的类型都必须能变成 `NativeKind` —— 编译器在这两处直接 `.unwrap()`，遇到转不了的类型会 panic。所以原生函数不能收任意类型。

= 编译器怎么认出这是原生调用

属性宏把 token 的 span 设成 `Span::call_site()`，于是调用点的 span 上带着「它由哪个属性宏展开而来」的信息。编译器用 `get_expn_macro_attr`（`core/src/compile/mod.rs`）把宏属性的 token 从 span 的源码片段里读回来，据此决定生成哪个节点。

因此 `lib-internal/src/lib.rs` 里 `native_calc` 和 `native_exec` 的宏体是*一模一样*的（都转调 `native`）—— 区分它们的是编译器，它看的是属性名。

= 事件：`#[event(N)]` 标在 struct 上

事件类型是同一个家族的另一支，但它标的是 *struct* 而不是函数：

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
- *字段顺序必须和游戏端事件节点的出参顺序一致* —— 编译器是按顺序把字段类型拼成 trigger 节点的出参的
- struct 和它的字段都必须是 `pub`

编译器侧的落点在 `core/src/compile/mod.rs` 的 `run()`：遍历带 `#[event_listener]` 的入口时，取出第一个参数的类型的 `#[event(N)]`，把字段类型拼成出参，然后在主图里插入 trigger 节点和对应的复合节点调用块。

= 相关

`#[event_listener]` 也是同一套机制的特例：它除了改写 span，还会自动给函数加上 `#[unsafe(no_mangle)]`，把它变成导出的入口。详见#link("/user/events.html")[事件]。
