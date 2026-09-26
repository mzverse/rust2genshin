use rust2genshin_lib_internal::{event, native_enum};
use crate::entity::Entity;


#[native_enum(21)]
pub enum DownCause {
    System = 1605,
    Normal,
    Abnormal,
}

#[native_enum(34)]
pub enum SettlementStatus {
    Battle = 0,
    Settled = 1,
    Escape = 2,
}

#[native_enum(15)]
pub enum EntityKind {
    Stage = 1401,
    Object = 1402,
    Player = 1403,
    Character = 1404,
    Creation = 1405,
}

#[native_enum(14)]
pub enum ElementType {
    Pyro = 1501,
    Hydro = 1502,
    Anemo = 1503,
    Electro = 1504,
    Dendro = 1505,
    Cryo = 1506,
    Geo = 1507,
    Physical = 1508,
}

#[native_enum(13)]
pub enum CoordinateSystem {
    World = 0,
    Local = 1,
}

#[native_enum(12)]
pub enum FollowType {
    Linear = 0,
    Smooth = 1,
    Animation = 2,
}

#[event(280)]
pub struct CharacterDownEvent {
    pub character: Entity,
    pub cause: DownCause,
    pub source: Option<Entity>,
}

#[event(281)]
pub struct CharacterReviveEvent {
    pub character: Entity,
}
