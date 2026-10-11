#import "/lib/lib.typ": *;
#let title = [事件];
#show: template;

#set raw(lang: "rust");

事件是节点图的执行入口：游戏里发生某件事，事件节点被触发，随后那条执行流上的节点依次运行。

= 监听事件

在 `pub fn` 上加 `#[event_listener]`。函数*有且只有一个*参数，就是事件本身：

```rust
#[event_listener]
pub fn on_tab_selected(event: TabSelectedEvent) {
    log(event.guid);
    event.entity.delete();
}
```

`#[event_listener]` 宏会自动给函数加上 `#[unsafe(no_mangle)]`（所以它是个导出入口），并把事件节点和对应的调用插进主图。函数名随便取，事件类型由参数的*类型*决定 —— 类型上带着 `#[event(N)]` 属性，编译器从这里读出该触发哪个事件。

一个事件可以有多个监听器，函数名不能重复。

*如果你想自己加一个事件类型*（往 `lib/src/event/` 里加）：`#[event(N)]` 的 `N` 是事件节点的 ID，struct 的*字段顺序必须和游戏端事件节点的出参顺序一致*，因为编译器就是按顺序把字段类型拼成节点的出参的。struct 和字段都要是 `pub`。写法见#link("/dev/add-native.html")[加一个原生调用]。

= 内置事件

事件类型按所在场景分三个模块。字段直接 `event.xxx` 访问。

== character（角色）

```rust
use rust2genshin_lib::event::character::{CharacterDownEvent, CharacterReviveEvent};
```

- `CharacterDownEvent` —— 角色倒下。字段：`character`、`cause`（`DownCause`）、`source`（`Option<Entity>`）
- `CharacterReviveEvent` —— 角色复活。字段：`character`

== entity（实体）

```rust
use rust2genshin_lib::event::entity::*;
```

- `PresetStatusChangeEvent` —— `entity`、`guid`、`preset_index`、`old_value`、`new_value`
- `EntityCreatedEvent` —— `entity`、`guid`
- `EntityRemovedEvent` —— `guid`（实体已消失，只有 `guid`）
- `EntityDestroyedEvent` —— 实体销毁时触发。`entity`、`guid`、`position`、`rotation`、`kind`、`faction`、`damage_source`、`owner`
- `TimerTriggerEvent` —— 定时器触发。`entity`、`guid`、`timer_name`、`timer_sequence`、`loop_number`
- `MotionStopEvent` —— 运动停止。`entity`、`guid`、`mover_name`
- `TriggerEnterEvent` —— 进入触发区。`enterer_entity`、`enterer_guid`、`trigger_entity`、`trigger_guid`、`trigger_index`
- `TriggerExitEvent` —— 离开触发区。`leaver_entity`、`leaver_guid`、`trigger_entity`、`trigger_guid`、`trigger_index`
- `ReachWaypointEvent` —— 抵达路径点。`entity`、`guid`、`device_name`、`path_id`
- `FactionChangeEvent` —— 阵营变化。`entity`、`guid`、`old`、`new`（两个都是 `Faction`）
- `TeleportCompleteEvent` —— 传送完成。`entity`、`guid`
- `TabSelectedEvent` —— 选项卡选中。`entity`、`guid`、`index`、`selector`、`hidden_guid`

== player（玩家）

```rust
use rust2genshin_lib::event::player::{PlayerAllDownEvent, PlayerAbnormalReviveEvent, PlayerRevivedEvent};
```

- `PlayerAllDownEvent` —— 玩家所有角色都倒下时。`player`、`cause`（`SettlementStatus`）
- `PlayerAbnormalReviveEvent` —— 角色异常倒下并复活时，输出受影响的玩家。`player`
- `PlayerRevivedEvent` —— 玩家所有角色都复活时。`player`

= 还没做的

自定义事件、事件的参数传递规则（哪些类型能当事件参数）都还没实现 —— 目前只能用上表这些内置事件。
