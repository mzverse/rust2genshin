#import "/lib/lib.typ": *;
#let title = [句柄类型];
#show: template;

#set raw(lang: "rust");

`Guid` / `Faction` / `Config` / `Prefab`是指向游戏数据的引用，不自带数据

都是`#[repr(transparent)]`的`Copy`类型，*只能在编译期构造*

= 四个类型

`Guid`实体ID　`Faction`阵营　`Config`配置ID　`Prefab`元件ID

```rust
let e = Entity::get(guid!(42));        // Entity::get(id: Guid) -> Option<Entity>
let f = e.faction();                   // Entity::faction() -> Faction
e.has_status(config!(100));            // Entity::has_status(config_id: Config) -> bool
```

= 构造

```rust
let g = guid!(42);         // 编译期生成 Guid(42)
let f = faction!(1);       // Faction
let c = config!(100);      // Config
let p = prefab!(7);        // Prefab
```

宏参数必须是const表达式 —— 字面量、`const`绑定、算术常量都行

`Guid::new`等标了`#[rustc_comptime]`（不是`const fn`），*非const上下文调用直接编译错*

```rust
let id = compute_id();              // 运行期变量
let g = guid!(id);                  // 编译错：不是 const 表达式
let g = Guid::new(42);              // 编译错：只允许 const 上下文
let _ = some_guid.0;                // 编译错：字段私有
some_guid.0 = 99;                   // 编译错：字段私有
```

宏只是把`const { Xxx::new(x) }`藏起来：

```rust
const G: Guid = Guid::new(42);      // 顶层 const
let g = const { Guid::new(42) };    // 内联 const 块
```

编译期构造，运行期只是一个整数ID

= 0 不是合法值

内部是`NonZero<i64>`，`new`里`NonZero::new(value).unwrap()`

```rust
let g = guid!(0);                   // 编译期 panic
```

= 不能用 `==` 比较

四个类型都没实现`PartialEq`，`a == b`编译不过，改用语义化的API：

```rust
is_hostile(a, b);                    // Faction 关系
faction_rank(faction);               // Faction 序号
```

= 什么时候可能是空的

API用`Option<T>`表达句柄失效

*可能为空*

- 「查找」「查所有者」「按ID取值」语义下的返回，如`Entity::get(id) -> Option<Entity>`
- 事件字段，如`CharacterDownEvent.source: Option<Entity>`
- `Entity::guid()` —— 只有preset和玩家才有guid
- `Player::class()` —— 只有玩家有职业

*必然非空*

- `Faction` —— 每个实体必然属于某个阵营，wire上`0`也合法
- `Entity::current()`、`Entity::faction()`这类保证有值的
- `i32` / `f32` / `bool` —— 不在句柄范畴，取不到就是`0` / `0.0` / `false`

拿不到值时别自己fallback（比如把`None`当成`self`），让调用方决定

= 实体与玩家

`Entity`和`Player`是两个不同的*Rust*类型，wire层却是同一种资源

拿到的`Player`一定是玩家，所以：

- `Player::guid() -> Guid`（不是`Option<Guid>`）
- 玩家特有的（`class_level`、`ui_layout`、`defeat`、`revive`…）在`Player`上
- `Entity`只放通用的（`is_active`、`faction`、`owner`、`guid`）

签名因此更诚实：`Entity::owner_player() -> Option<Player>`

两者没有隐式转换，需要时走API，不要用`transmute`
