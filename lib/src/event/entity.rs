use rust2genshin_lib_internal::event;
use crate::entity::Entity;
use crate::{Faction, Guid, String};
use crate::event::character::EntityKind;
use crate::math::Vec3;

#[event(67)]
pub struct PresetStatusChangeEvent {
    pub entity: Entity,
    pub guid: Guid,
    pub preset_index: i32,
    pub old_value: i32,
    pub new_value: i32,
}

#[event(71)]
pub struct EntityCreatedEvent {
    pub entity: Entity,
    pub guid: Guid,
}

#[event(72)]
pub struct EntityRemovedEvent {
    pub guid: Guid,
}

#[event(83)]
pub struct TimerTriggerEvent {
    pub entity: Entity,
    pub guid: Guid,
    pub timer_name: String,
    pub timer_sequence: i32,
    pub loop_number: i32,
}

#[event(89)]
pub struct MotionStopEvent {
    pub entity: Entity,
    pub guid: Guid,
    pub mover_name: String,
}

#[event(91)]
pub struct TriggerExitEvent {
    pub leaver_entity: Entity,
    pub leaver_guid: Guid,
    pub trigger_entity: Entity,
    pub trigger_guid: Guid,
    pub trigger_index: i32,
}

#[event(92)]
pub struct TriggerEnterEvent {
    pub enterer_entity: Entity,
    pub enterer_guid: Guid,
    pub trigger_entity: Entity,
    pub trigger_guid: Guid,
    pub trigger_index: i32,
}

#[event(177)]
pub struct ReachWaypointEvent {
    pub entity: Entity,
    pub guid: Guid,
    pub device_name: String,
    pub path_id: i32,
}

#[event(251)]
pub struct FactionChangeEvent {
    pub entity: Entity,
    pub guid: Guid,
    pub old: Faction,
    pub new: Faction,
}

#[event(289)]
pub struct TeleportCompleteEvent {
    pub entity: Entity,
    pub guid: Guid,
}

#[event(307)]
pub struct TabSelectedEvent {
    pub entity: Entity,
    pub guid: Guid,
    pub index: i32,
    pub selector: Entity,
    /// Always 0. Don't use it
    pub hidden_guid: Guid,
}

/// 实体销毁时触发
#[event(373)]
pub struct EntityDestroyedEvent {
    pub entity: Entity,
    pub guid: Guid,
    pub position: Vec3,
    pub rotation: Vec3,
    pub kind: EntityKind,
    pub faction: Faction,
    pub damage_source: Entity,
    pub owner: Entity,
    // TODO: custom_vars_snap: VarSnapshotRef (Vss 类型 API 尚未实现)
}
