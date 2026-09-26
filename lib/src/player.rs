use rust2genshin_lib_internal::{native, native_calc, native_exec};
use crate::{Config, Guid, String, ToString};
use crate::entity::Entity;

/// A Gc ref of a player entity
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Player(pub(crate) &'static crate::entity::EntityInternal);

impl ToString for Player {
    #[native("to_string")]
    fn to_string(&self) -> String;
}

impl Player {
    #[rustc_force_inline]
    pub fn as_entity(self) -> Entity {
        Entity(self.0)
    }
    
    #[native_calc(76)]
    pub fn guid(self) -> Guid;

    /// 获取职业,仅当 `self` 是玩家时返回 `Some`
    #[native_calc(387)]
    pub fn class(self) -> Option<Config>;

    /// 获取指定职业的等级
    #[native_calc(388)]
    pub fn class_level(self, class: Config) -> i32;

    /// 获取当前UI布局
    #[native_calc(317)]
    pub fn ui_layout(self) -> i32;

    /// 获取剩余复苏次数
    #[native_calc(275)]
    pub fn revives_left(self) -> i32;

    /// 获取下次复苏倒计时(秒)
    #[native_calc(277)]
    pub fn revive_time(self) -> i32;

    /// 是否所有角色都倒下
    #[native_calc(287)]
    pub fn is_all_down(self) -> bool;

    /// 玩家结算排名
    #[native_calc(651)]
    pub fn rank(self) -> i32;

    /// 是否合法逃脱
    #[native_calc(662)]
    pub fn escape_legal(self) -> bool;

    #[native_calc(750)]
    pub fn id_to_guid(id: i32) -> Option<Guid>;

    #[native_calc(751)]
    pub fn guid_to_id(guid: Guid) -> i32;

    /// 千星奇域盒数量
    #[native_calc(773)]
    pub fn box_quantity(self, box_index: i32) -> i32;

    /// 千星奇域盒消耗
    #[native_calc(774)]
    pub fn box_consumption(self, box_index: i32) -> i32;

    /// defeat all characters
    #[native_exec(282)]
    pub fn defeat(self);

    /// revive all characters
    /// # Arguments
    /// * `deduct_revives` - 是否消耗复苏次数
    #[native_exec(283)]
    pub fn revive(self, deduct_revives: bool);
}
