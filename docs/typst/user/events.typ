#import "/lib/lib.typ": *;
#let title = [事件];
#show: template;

#set raw(lang: "rust");

事件是节点图的执行入口 —— 游戏里发生某件事，事件节点被触发，随后的执行流依次运行

= 监听

在`pub fn`上加`#[event_listener]`，函数*有且只有一个*参数，就是事件本身

```rust
#[event_listener]
pub fn on_tab_selected(event: TabSelectedEvent) {
    log(event.guid);
    event.entity.delete();
}
```

宏会自动加上`#[unsafe(no_mangle)]`，把事件节点和对应调用插进主图

函数名随便取，事件类型由参数*类型*决定 —— 类型上带着`#[event(N)]`，编译器从这里读出该触发哪个事件

一个事件可以有多个监听器，函数名不能重复

= 内置事件

字段直接`event.xxx`访问

== 角色

```rust
use rust2genshin_lib::event::character::{CharacterDownEvent, CharacterReviveEvent};
```

- `CharacterDownEvent` —— `character`、`cause`（`DownCause`）、`source`（`Option<Entity>`）
- `CharacterReviveEvent` —— `character`

== 实体

```rust
use rust2genshin_lib::event::entity::*;
```

- `PresetStatusChangeEvent` —— `entity`、`guid`、`preset_index`、`old_value`、`new_value`
- `EntityCreatedEvent` —— `entity`、`guid`
- `EntityRemovedEvent` —— `guid`
- `EntityDestroyedEvent` —— `entity`、`guid`、`position`、`rotation`、`kind`、`faction`、`damage_source`、`owner`
- `TimerTriggerEvent` —— `entity`、`guid`、`timer_name`、`timer_sequence`、`loop_number`
- `MotionStopEvent` —— `entity`、`guid`、`mover_name`
- `TriggerEnterEvent` —— `enterer_entity`、`enterer_guid`、`trigger_entity`、`trigger_guid`、`trigger_index`
- `TriggerExitEvent` —— `leaver_entity`、`leaver_guid`、`trigger_entity`、`trigger_guid`、`trigger_index`
- `ReachWaypointEvent` —— `entity`、`guid`、`device_name`、`path_id`
- `FactionChangeEvent` —— `entity`、`guid`、`old`、`new`
- `TeleportCompleteEvent` —— `entity`、`guid`
- `TabSelectedEvent` —— `entity`、`guid`、`index`、`selector`、`hidden_guid`

== 玩家

```rust
use rust2genshin_lib::event::player::{PlayerAllDownEvent, PlayerAbnormalReviveEvent, PlayerRevivedEvent};
```

- `PlayerAllDownEvent` —— `player`、`cause`（`SettlementStatus`）
- `PlayerAbnormalReviveEvent` —— `player`
- `PlayerRevivedEvent` —— `player`

= 自己加一个

往`lib/src/event/`里加：`#[event(N)]`的`N`是事件节点ID，struct的*字段顺序必须和游戏端出参顺序一致*，编译器按顺序把字段类型拼成节点的出参

struct和字段都要`pub`，写法见#link("/dev/add-native.html")[加一个原生调用]

= 还没做的

自定义事件、事件参数传递规则都没实现，目前只能用上面这些
