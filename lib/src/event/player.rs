use rust2genshin_lib_internal::event;
use crate::player::Player;
use crate::event::character::SettlementStatus;


/// 玩家所有角色都倒下时
#[event(284)]
pub struct PlayerAllDownEvent {
    pub player: Player,
    pub cause: SettlementStatus,
}

/// 角色异常倒下并复活时,输出受影响的玩家
#[event(285)]
pub struct PlayerAbnormalReviveEvent {
    pub player: Player,
}

/// 玩家所有角色都复活时
#[event(286)]
pub struct PlayerRevivedEvent {
    pub player: Player,
}
