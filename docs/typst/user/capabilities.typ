#import "/lib/lib.typ": *;
#let title = [能力速览];
#show: template;

#set raw(lang: "rust");

这页是「有什么、去哪找」的索引，不是参考手册 —— 只列分组和典型入口，不列全量签名。要找某个函数，直接搜 `lib/src/`。

= 数学与位运算（`lib/src/math.rs`）

`Vec3` 有一套方法：`add` / `sub` / `scale` / `normalize` / `length` / `distance` / `dot` / `cross` / `angle` / `rotate`。

自由函数：`pi`、`rad_to_deg` / `deg_to_rad`、`random_int` / `random_float`、`timestamp` / `timezone` / `time_to_timestamp` / `timestamp_to_weekday`。

位运算：`left_shift`、`bit_and` / `bit_or` / `bit_xor` / `bit_not`、`read_bit` / `write_bit`。

= 实体（`lib/src/entity.rs`）

`Entity` 是节点图里大部分操作的入口，方法最多。

- *取实体* —— `Entity::current()`、`Entity::get(guid)`、`owner()`、`owner_player()`
- *属性* —— `guid` / `nickname` / `faction` / `capacity` / `kind` / `forward` / `right` / `up`
- *生命周期* —— `spawn_preset` / `kill` / `delete` / `revive_character` / `set_model_visible`
- *计时器与运动* —— `pause_timer` / `resume_timer` / `stop_timer`、`pause_motion` / `resume_motion` / `stop_motion`
- *状态与仇恨* —— `has_status` / `status_stacks` / `status_applier`、`aggro_value` / `aggro_target` / `aggro_multiplier`
- *装备与物品* —— `equipment_config` / `affix_config` / `affix_value`、`item_amount` / `currency_amount`

`Player` 是 `Entity` 的一个视角：`entity.as_player()` 拿到，`player.as_entity()` 转回去。分工见#link("/user/handles.html")[句柄类型]。

`Entity` 上不带 `self` 的（`is_hostile`、`elapsed_time`、`faction_rank`、`global_aggro_multiplier`）是自由函数式的全局查询。

= 玩家（`lib/src/player.rs`）

`guid` / `class` / `class_level` / `ui_layout` / `rank` / `escape_legal`、`revives_left` / `revive_time` / `is_all_down`、`id_to_guid` / `guid_to_id`、`box_quantity` / `box_consumption`、`defeat` / `revive`。

= 容器

- *`List<T>`*（`lib/src/list.rs`）—— 别名到 `Box<[T]>`。用 `list![a, b, c]` 宏构造，`push` / `insert` 追加
- *`Dict<K, V>`*（`lib/src/dict.rs`）—— 键值表，`K` 要满足 `DictKey`
- *`Box<T>`*（`lib/src/boxed.rs`）—— 对应节点图的对象引用；`Box::new` / `into_inner` / `unbox`，`borrow` / `borrow_mut` 取借用
- *`Sc<T>`*（`lib/src/sc.rs`）—— 带引用计数的共享单元

= 句柄

`Guid` / `Faction` / `Config` / `Prefab`，都只能在编译期构造。见#link("/user/handles.html")[句柄类型]。

= 日志

`log(s)`（`lib/src/lib.rs`），参数实现 `ToString`。

= 异步

`#[asynchronous]` / `#[async_loop]` / `sleep!` / `Task::spawn`。见#link("/user/async.html")[异步]。

= 事件

`#[event_listener]` 加内置事件结构体，按 character / entity / player 三组列在#link("/user/events.html")[事件]页。
