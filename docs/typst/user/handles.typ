#import "/lib/lib.typ": *;
#let title = [句柄类型];
#show: template;

#set raw(lang: "rust");

`Guid` / `Faction` / `Config` / `Prefab` 是「指向游戏数据的引用」类型，指向游戏里某个已存在的实体、阵营、配置项或元件，而不是自己携带数据。

它们是 `#[repr(transparent)]` 的 `Copy` 类型，*只能在编译期构造*。

= 四个类型

`Guid` 实体 ID　`Faction` 阵营　`Config` 配置 ID　`Prefab` 元件 ID

它们在 API 里到处都是，通常出现在返回值或参数位置：

```rust
let e = Entity::get(guid!(42));        // Entity::get(id: Guid) -> Option<Entity>
let f = e.faction();                   // Entity::faction() -> Faction
e.has_status(config!(100));            // Entity::has_status(config_id: Config) -> bool
```

= 构造

用对应的 `xxx!` 宏：

```rust
let g = guid!(42);         // 编译期生成 Guid(42)
let f = faction!(1);       // Faction
let c = config!(100);      // Config
let p = prefab!(7);        // Prefab
```

宏参数必须是 const 表达式 —— 字面量、`const` 绑定、算术常量都行。运行期变量会被编译器拒掉，因为 `Guid::new` 等标了 `#[rustc_comptime]`（不是 `const fn`），*不在 const 上下文调用直接编译失败*：

```rust
let id = compute_id();              // 运行期变量
let g = guid!(id);                  // 编译错：id 不是 const 表达式
let g = Guid::new(42);              // 编译错：comptime fn 只允许 const 上下文
let _ = some_guid.0;                // 编译错：字段私有
some_guid.0 = 99;                   // 编译错：字段私有
```

等价写法（宏只是把 `const { Xxx::new(x) }` 藏起来）：

```rust
const G: Guid = Guid::new(42);      // 顶层 const
let g = const { Guid::new(42) };    // 内联 const 块
```

因为是编译期构造，句柄本身不占运行期开销 —— 生成的只是一个整数 ID。

= 0 不是合法值

四个类型内部都是 `NonZero<i64>`，`new` 里是 `NonZero::new(value).unwrap()`。传 `0` 会在*编译期 panic*：

```rust
let g = guid!(0);                   // 编译期 panic
```

= 不能用 `==` 比较

四个类型都没有实现 `PartialEq`，`a == b` 编译不过。判断两个句柄的关系请走 API 上语义化的方法：

```rust
is_hostile(a, b);                    // Faction 关系
faction_rank(faction);               // Faction 序号
```

= 什么时候可能是空的

这里的「空」指句柄失效 —— 查不到对应的东西。API 用 `Option<T>` 诚实地表达它：

*可能为空（返回 `Option<T>`）*

- `Guid` / `Entity` / `Config` / `Prefab` 在「查找」「查所有者」「按 ID 取值」语义下的返回。例如 `Entity::get(id) -> Option<Entity>`
- 事件里的字段。例如 `CharacterDownEvent.source: Option<Entity>`
- `Entity::guid() -> Option<Guid>` —— 只有 preset 和玩家才有 guid
- `Player::class() -> Option<Config>` —— 只有玩家有职业，造物、装饰物都没有

*必然非空（直接返回 `T`）*

- `Faction` —— 每个实体必然属于某个阵营，wire 上 `0` 也是合法的 faction 值，包 `Option` 没有意义
- `Entity::current()`、`Entity::faction()` 这类保证有值的
- 基础类型 `i32` / `f32` / `bool` —— 不在句柄的范畴，取不到就用默认值（`0` / `0.0` / `false`）

拿不到值时别自己 fallback（比如把 `None` 当成 `self`），让调用方决定怎么处理。

= 实体与玩家

`Entity` 和 `Player` 是*两个不同的 Rust 类型*，wire 层却是同一种资源。

只要你能拿到一个 `Player`，它*一定*是玩家。所以：

- `Player::guid() -> Guid`（*不是* `Option<Guid>`）—— 类型本身已经保证了
- 玩家特有的方法（`class_level`、`ui_layout`、`defeat`、`revive`…）都在 `Player` 上，不在 `Entity` 上
- `Entity` 只放通用的（`is_active`、`faction`、`owner`、`guid`）

好处是签名更诚实：`Entity::owner_player() -> Option<Player>` 比 `Option<Entity>` 更准确，调用方也不用再自己转换。

`Player` 和 `Entity` 之间没有隐式转换 —— 需要转换时走 API，不要用 `transmute`。