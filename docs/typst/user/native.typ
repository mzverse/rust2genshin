#import "/lib/lib.typ": *;
#let title = [原生调用与枚举];
#show: template;

#set raw(lang: "rust");

`lib/` 里有一批*没有函数体*的函数。它们不是 Rust 代码，而是节点图里的一个节点 —— 编译器读到调用点，就地合成那个节点。

= 直接调用

你不需要任何特殊语法，当成普通函数用：

```rust
co.kill();
let e = Entity::current();
log(e.nickname());
```

每条这样的调用在节点图里占一个节点，不在 Rust 里执行。所以它们没有函数体也无所谓。

顺带两个后果：

- 别去 `lib/` 里找这些函数的实现 —— 没有
- 它们不能写进 `const` 上下文，也不能当成函数指针传来传去

= 定义游戏枚举

游戏端的枚举（实体种类、元素类型、结算状态……）在 Rust 侧对应 `#[native_enum(N)]` 标过的枚举：

```rust
#[native_enum(15)]
pub enum EntityKind {
    Stage = 1401,
    Object = 1402,
    Player = 1403,
    Character = 1404,
    Creation = 1405,
}
```

两条要点：

- *`N` 是枚举类型 ID 的紧凑形式* —— 参考项目 `data.json` 里 `EnumTypes[].TypeID` 减 `10000`。ID 错了节点图导不进游戏
- *变体的值就是游戏枚举值* —— 必须显式写出来。省略的话按 Rust 规则递增，很容易和游戏端对不上

这类枚举可以用 `==` 比较，比较会编译成一个节点：

```rust
if e.kind() == EntityKind::Character {
    // ...
}
```

定义在 `lib/src/event/character.rs`。

= 要加一个新的原生调用

那是改 `lib/` 的事，做法见#link("/dev/add-native.html")[加一个原生调用]。
