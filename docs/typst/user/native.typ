#import "/lib/lib.typ": *;
#let title = [原生调用与枚举];
#show: template;

#set raw(lang: "rust");

`lib/`里有一批*没有函数体*的函数 —— 它们不是Rust代码，而是节点图里的一个节点，编译器读到调用点就地合成

= 直接调用

不需要特殊语法，当普通函数用

```rust
co.kill();
let e = Entity::current();
log(e.nickname());
```

每条这样的调用在图上占一个节点，不在Rust里执行，所以没有函数体也无所谓

两个后果：

- 别去`lib/`里找实现 —— 没有
- 不能写进`const`上下文，也不能当函数指针传来传去

= 游戏枚举

游戏端枚举（实体种类、元素类型、结算状态…）对应`#[native_enum(N)]`标过的枚举

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

- *`N`是枚举类型ID的紧凑形式* —— 参考项目`data.json`里`EnumTypes[].TypeID`减`10000`，错了节点图导不进游戏
- *变体的值就是游戏枚举值* —— 必须显式写出，省略会按Rust规则递增，很容易对不上

可以用`==`比较，比较会编译成一个节点

```rust
if e.kind() == EntityKind::Character {
    // ...
}
```

= 加一个新的

那是改`lib/`的事，见#link("/dev/add-native.html")[加一个原生调用]
