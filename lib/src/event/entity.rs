use rust2genshin_lib_internal::event;
use crate::entity::Entity;
use crate::Guid;

#[event(71)]
pub struct EntityCreatedEvent {
    pub entity: Entity,
    pub guid: Guid,
}

#[event(307)]
pub struct TabSelectedEvent {
    pub entity: Entity, // selectee
    pub guid: Guid,
    pub index: i32,
    pub selector: Entity,
}
