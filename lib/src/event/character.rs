use rust2genshin_lib_internal::{event, native_enum};
use crate::entity::Entity;


#[native_enum(21)]
pub enum DownCause {
    System = 1605,
    Normal,
    Unnormal,
}

#[event(280)]
pub struct CharacterDownEvent {
    pub character: Entity,
    pub cause: DownCause,
    pub source: Option<Entity>,
}
