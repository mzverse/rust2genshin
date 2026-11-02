#import "/lib/lib.typ": *;
#let title = [能力速览];
#show: template;

#set raw(lang: "rust");

只列入口，不列签名。找具体函数的完整签名直接搜 `lib/src/`。

= 数学

`Vec3`的方法：`add` / `sub` / `scale` / `normalize` / `length` / `distance` / `dot` / `cross` / `angle` / `rotate`

全局：`pi` / `rad_to_deg` / `deg_to_rad` / `random_int` / `random_float` / `timestamp` / `timezone` / `time_to_timestamp` / `timestamp_to_weekday`

位运算：`left_shift` / `bit_and` / `bit_or` / `bit_xor` / `bit_not` / `read_bit` / `write_bit`

= 实体

- 获取 —— `current` / `get` / `owner` / `owner_player`
- 属性 —— `guid` / `nickname` / `faction` / `capacity` / `kind` / `forward` / `right` / `up` / `is_active` / `is_in_combat`
- 生命周期 —— `spawn_preset` / `kill` / `delete` / `revive_character` / `set_model_visible`
- 计时器与运动 —— `pause_timer` / `resume_timer` / `stop_timer` / `timer_time` / `pause_motion` / `resume_motion` / `stop_motion`
- 状态与仇恨 —— `has_status` / `status_stacks` / `status_applier` / `get_preset_status` / `set_preset_status` / `aggro_value` / `aggro_target` / `aggro_multiplier`
- 装备与物品 —— `equipment_config` / `affix_config` / `affix_value` / `item_amount` / `currency_amount` / `loot_item_amount` / `loot_currency_amount`
- 其它 —— `is_achievement_completed` / `active_scan_tag` / `creation_target` / `set_follow_target_guid`

`Player`是`Entity`的一个视角，`as_player` / `as_entity`互转，分工见#link("/user/handles.html")[句柄类型]

不带`self`的是全局查询：`is_hostile` / `elapsed_time` / `faction_rank` / `global_aggro_multiplier`

= 玩家

`guid` / `class` / `class_level` / `ui_layout` / `rank` / `escape_legal`、`revives_left` / `revive_time` / `is_all_down`、`id_to_guid` / `guid_to_id`、`box_quantity` / `box_consumption`、`defeat` / `revive`

= 容器

- `List<T>` —— 见#link("/user/list.html")[列表]
- `Dict<K, V>` —— 键值表，`K`要满足`DictKey`
- `Box<T>` —— 见#link("/user/heap.html")[堆分配]
- `Sc<T>` —— 带引用计数的共享单元，`Sc::new`

= 句柄

`Guid` / `Faction` / `Config` / `Prefab`，只能在编译期构造，见#link("/user/handles.html")[句柄类型]

= 日志

`log(s)`，参数实现`ToString`

= 异步

`#[asynchronous]` / `#[async_loop]` / `sleep!` / `Task::spawn`，见#link("/user/async.html")[异步]

= 事件

`#[event_listener]`加内置事件结构体，见#link("/user/events.html")[事件]
