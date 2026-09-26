use crate::list::List;
use crate::{Config, Faction, Guid, String, ToString};
use rust2genshin_lib_internal::{native, native_calc, native_exec};
use crate::player::Player;
use crate::event::character::{CoordinateSystem, EntityKind, FollowType};
use crate::math::Vec3;

/// A Gc ref of an entity
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Entity(pub(crate) &'static EntityInternal);

unsafe extern "Rust" {
    #[native("Entity")]
    pub(crate) type EntityInternal;
}

impl ToString for Entity {
    #[native("to_string")]
    fn to_string(&self) -> String;
}

impl Entity {
    #[rustc_force_inline]
    pub fn as_player(self) -> Player {
        Player(self.0)
    }

    /// Get the entity who mounted the current node graph
    #[native_calc(73)]
    pub fn current() -> Self;

    /// Get preset by GUID, returns `None` if no such entity
    #[native_calc(75)]
    pub fn get(id: Guid) -> Option<Self>;

    /// Get GUID, returns `Some` when `self` is a preset or a player
    #[native_calc(76)]
    pub fn guid(self) -> Option<Guid>;

    ///
    /// # Returns
    /// 不返回生成的实体
    #[native_exec(70)]
    pub fn spawn_preset(id: Guid, list: List<i32>);

    /// 销毁实体，触发销毁逻辑后移除
    /// 或击倒角色
    /// 要直接移除另见[`Self::delete`]
    #[native_exec(69)]
    pub fn kill(self);

    /// 直接移除实体
    /// 对玩家和角色无效
    /// 要触发销毁逻辑另见[`Self::kill`]
    #[native_exec(372)]
    pub fn delete(self);

    /// 设置模型可见性
    #[native_exec(308)]
    pub fn set_model_visible(self, visible: bool);

    /// 设置预设状态
    /// # Arguments
    /// * `value` - 通常`0`表示关闭，`1`表示开启
    #[native_exec(66)]
    pub fn set_preset_status(self, index: i32, value: i32);

    /// 复苏角色
    #[native_exec(279)]
    pub fn revive_character(self);

    /// 暂停计时器
    #[native_exec(80)]
    pub fn pause_timer(self, name: String);

    /// 恢复计时器
    #[native_exec(81)]
    pub fn resume_timer(self, name: String);

    /// 停止计时器
    #[native_exec(82)]
    pub fn stop_timer(self, name: String);

    /// 暂停运动装置
    #[native_exec(87)]
    pub fn pause_motion(self, name: String);

    /// 恢复运动装置
    #[native_exec(88)]
    pub fn resume_motion(self, name: String);

    /// 停止并删除运动装置
    #[native_exec(86)]
    pub fn stop_motion(self, name: String, stop_all: bool);

    /// 是否激活 / 存活
    #[native_calc(507)]
    pub fn is_active(self) -> bool;

    /// 是否在战斗中
    #[native_calc(610)]
    pub fn is_in_combat(self) -> bool;

    /// 获取昵称
    #[native_calc(767)]
    pub fn nickname(self) -> String;

    #[native_calc(249)]
    pub fn faction(self) -> Faction;

    /// 获取所有者实体,无主时返回 `None`
    #[native_calc(744)]
    pub fn owner(self) -> Option<Self>;

    /// 获取预设状态值
    #[native_calc(68)]
    pub fn get_preset_status(self, preset_index: i32) -> i32;

    /// 是否有该 config 状态
    #[native_calc(508)]
    pub fn has_status(self, config_id: Config) -> bool;

    /// 仇恨值,无仇恨关系返回 `None`
    #[native_calc(603)]
    pub fn aggro_value(self, owner: Self) -> Option<i32>;

    /// 仇恨倍率
    #[native_calc(604)]
    pub fn aggro_multiplier(self) -> f32;

    /// 仇恨目标,无目标返回 `None`
    #[native_calc(606)]
    pub fn aggro_target(self) -> Option<Self>;

    /// 成就是否达成
    #[native_calc(644)]
    pub fn is_achievement_completed(self, achievement_index: i32) -> bool;

    /// 仓库容量
    #[native_calc(689)]
    pub fn capacity(self) -> i32;

    /// 物品数量
    #[native_calc(690)]
    pub fn item_amount(self, item: Config) -> i32;

    /// 装备词条 config
    #[native_calc(676)]
    pub fn affix_config(equip_index: i32, entry_index: i32) -> Config;

    /// 装备词条数值
    #[native_calc(677)]
    pub fn affix_value(equip_index: i32, entry_index: i32) -> f32;

    /// 获取该角色所属的玩家实体
    #[native_calc(259)]
    pub fn owner_player(self) -> Option<Player>;

    /// 获取实体类型(stage/object/player/character/creation)
    #[native_calc(260)]
    pub fn entity_type(self) -> EntityKind;

    /// 前向向量
    #[native_calc(516)]
    pub fn forward(self) -> Vec3;

    /// 右向向量
    #[native_calc(517)]
    pub fn right(self) -> Vec3;

    /// 上向向量
    #[native_calc(518)]
    pub fn up(self) -> Vec3;

    /// 获取造物目标
    #[native_calc(376)]
    pub fn creation_target(self) -> Option<Self>;

    /// 全局计时器当前时间
    #[native_calc(310)]
    pub fn timer_time(self, name: String) -> f32;

    /// 货币数量
    #[native_calc(691)]
    pub fn currency_amount(self) -> i32;

    /// 战利品物品数量
    #[native_calc(728)]
    pub fn loot_item_amount(self) -> i32;

    /// 战利品货币数量
    #[native_calc(729)]
    pub fn loot_currency_amount(self) -> i32;

    /// 当前生效的 scan tag config
    #[native_calc(737)]
    pub fn active_scan_tag(self) -> Option<Config>;

    /// 状态层数
    #[native_calc(746)]
    pub fn status_stacks(self, status: Config) -> i32;

    /// 状态施加者
    #[native_calc(747)]
    pub fn status_applier(self, status: Config) -> Option<Self>;

    /// 装备 config id
    #[native_calc(749)]
    pub fn equipment_config(equip_index: i32) -> Config;

    // 千星奇域盒相关方法见 player.rs

    // TODO: ID 245
    // TODO: ID 668
    // TODO: ID 250

    // TODO: 修改模型的颜色和材质
}

/// 判断两个阵营是否敌对
#[native_calc(614)]
pub fn is_hostile(a: Faction, b: Faction) -> bool;

/// 已流逝时间(秒)
#[native_calc(290)]
pub fn elapsed_time() -> i32;

/// 全局仇恨倍率
#[native_calc(605)]
pub fn global_aggro_multiplier() -> f32;

/// 阵营排行
#[native_calc(655)]
pub fn faction_rank(faction: Faction) -> i32;

/// 按 GUID 切换跟随运动装置的目标
#[native_exec(245)]
pub fn set_follow_target_guid(
    target_entity: Entity,
    follow_guid: Guid,
    socket_name: String,
    pos_offset: Vec3,
    rot_offset: Vec3,
    coord_sys: CoordinateSystem,
    follow_type: FollowType,
);
