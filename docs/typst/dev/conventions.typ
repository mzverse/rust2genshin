#import "/lib/lib.typ": *;
#let title = [约定];
#show: template;

#set raw(lang: "rust");

写 `lib/` 公开 API 和 `core/` 节点时的既定约定。每条都有具体原因，破坏它会让约束失效或让用户困惑。

= 文档注释不写节点 ID

`lib/src/` 里面向用户的 `///` 注释*不要*写节点 ID：

- 不要：`/// 获取阵营(节点 ID 249)`
- 要：`/// 获取阵营`

节点 ID 是后端跟游戏引擎对齐的内部编号，跟用户写逻辑无关。注释跟 `std` 风格一致：简短描述、参数说明、`# Returns` / `# Errors`。

*反过来*，`core/src/node/<域>.rs` 里是*应该*写 ID 的，那里是给后端维护者看的：

```rust
/// 向量加法(ID 10):a + b
```

= 命名

*方法名*按参考项目的 `Identifier` 蛇形化 + Rust 习惯精简：

- `Get_` / `Fetch_` / `Retrieve_` 前缀*省掉* —— `Get_Capacity` → `capacity`
- `Is_` / `Has_` / `Can_` 前缀*保留*（且只在参考项目标识符里本来就有这些前缀时）—— `Is_All_Down` → `is_all_down`、`Is_Completed` → `is_completed`
- 在 `impl Player` 上的 `Get_Player_XXX` 再省一层 `Player_` —— `Get_Player_Rank` → `rank`
- 同名 ID 不同语义要消歧义 —— 实体上的 `revive`（单角色）和玩家上的 `revive`（所有角色），改成 `revive_character` / `revive`
- bool 返回值*不一定*带 `is_` —— 看参考项目的 Identifier 有没有 `Is_`

*枚举名*避免和 Rust 类型系统的概念撞：

- 不要 `EntityType`，要 `EntityKind`
- 表示「种类 / 类别 / 状态」时用 `Kind` / `Category` / `Mode` / `State` 后缀，*不要* `Type`
- 方法和字段名同步改：`entity_type()` → `kind()`

*事件字段名*里，主体的 entity / guid *不加* `source_` 前缀：

- 不要 `source_entity` / `source_guid` —— `source_` 是废话
- 要 `entity` / `guid`
- 前缀只用来区分*语义角色*：`leaver_entity` / `trigger_entity` / `damage_source` / `selector` / `owner`

*字段名跟类型名对齐*：类型叫 `Faction`，字段就叫 `faction`（不要叫 `camp`）；`Config` 字段叫 `config`；`Entity` 字段叫 `entity`。

= 句柄类型的实现约束

`Guid` / `Faction` / `Config` / `Prefab` 四个类型（都在 `lib/src/lib.rs`）靠三层机制保证「只能在编译期构造」：

```rust
#[native("Guid")]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Guid(NonZero<i64>);   // 字段私有

impl Guid {
    #[rustc_force_inline]
    #[rustc_comptime]            // 非 const 上下文调用直接编译错
    pub fn new(value: i64) -> Self { Self(NonZero::new(value).unwrap()) }
}

#[macro_export]
macro_rules! guid {
    ($id:expr) => { const { $crate::Guid::new($id) } };
}
```

- 私有字段挡住 `.0` 的读 / 写
- `#[rustc_comptime]` 让 `Guid::new(x)` 在非 const 上下文编译失败
- 宏只做语法糖，把 `const { ... }` 包起来。*匹配器必须用 `:expr`*，不要用 `:literal` —— 约束完全由 `#[rustc_comptime]` 做，`1 + 1` 这种合法的 const 表达式也应当能通过，`:literal` 会把它拒掉

新增一个句柄类型时三项缺一不可。不要退回 `pub i64` / 无 comptime 的 `new` / 加 `Default` —— 任何一项都会让约束失效。句柄*不带* `Default`，因为「默认值」是「未初始化的句柄」，语义上不该存在。

编译器侧还要在 `core/src/compile/native.rs::compile_native_ty` 里给新类型加一个分支（现有的 `"Guid" => IrKind::Native(NativeKind::Guid)` 等），否则 `#[native("Xxx")]` 会报 unknown。

= 可空性

见#link("/user/handles.html")[句柄类型]——那是给使用者看的规则；实现时按同一判断来定签名。

= `#[native_enum(N)]` 的 N

`N` 是枚举类型 ID 的*紧凑形式*：参考项目 `data.json` 里 `EnumTypes[].TypeID` 减去 `10000`。

- `DWNR`（`DownCause`）：TypeID 10021 → `#[native_enum(21)]`
- `ENTY`（`EntityKind`）：TypeID 10015 → `#[native_enum(15)]`
- `SETL`（`SettlementStatus`）：TypeID 10034 → `#[native_enum(34)]`

编码时加回 10000（`core/src/value.rs` 里 `Enum(id) => 10000 + *id`）。ID 错了节点图导不进游戏。

枚举的*变体值*是另一回事 —— 对应参考项目 `Enums[].ID` 字段，由 MIR 的 discriminant 决定，不用手写。

= 先查参考项目的描述

`tools/node_data/nodes.json` 只有结构（id / system / domain / kind / 引脚），*没有* `InGameName` 和 `Description`。

写新的 lib 方法、事件，或审计命名 / 类型边界之前，要去参考项目的 `utils/node_data/data.json` 查描述 —— 只看 `nodes.json` 的 Identifier 蛇形化是不够的，`Identifier` 是 `domain.subdomain.name`，不是语义描述。

= 几条已经踩过的坑

*不要给泛型引脚的节点编具体类型。* 引脚类型含 `<T>` 的（`R<T>`、`L<R<T>>`、`L<R<K>>` 等）*直接跳过*，不要 fallback 成 `i32` 之类的具体类型 —— 那会破坏类型契约。曾经把 ID 36 `VariableChangeEvent` 的 `old_value` / `new_value`（类型是 `R<T>`）当成 `i32` 写过，是错的。等泛型 API 支持了再做。

*多输出节点*暂时不实现，等结构体 API。已知的几个：ID 580 `Get_Obj_Attr`（7 输出）、670 `Get_Adv_Attr`（8）、671 `Get_Elem_Attr`（16）、619 `Get_Patrol_Template`（3）、658 `Get_Rank_Info`（4）、753 `Timestamp_To_Time`（6）。两个以内的输出可以先用元组返回。

*还没定的*：`is_active` / `is_in_combat` 这类 bool 方法该挂在 `Entity` / `Player` 上还是做自由函数（可能要看节点 ID 的 domain）；`is_hostile(a, b)` 现在在 `entity.rs` 当自由函数，语义上偏阵营，但项目里没有 `faction.rs`；`faction_rank(camp)` 和 `Player::rank(self)` 两个「rank」命名要不要消歧。遇到相关改动时一并想。
