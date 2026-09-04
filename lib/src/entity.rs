use crate::Guid;
use rust2genshin_lib_internal::{native_calc, native_exec};
use crate::list::List;

/// A Gc ref of an entity
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Entity(&'static EntityInternal);

unsafe extern "Rust" {
    pub type EntityInternal;
}

impl Entity {
    #[native_calc(75)]
    pub fn get(id: Guid) -> Self;

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
    pub fn revive(self);

    /// 击倒玩家的所有角色
    #[native_exec(282)]
    pub fn defeat_all_characters(self);

    /// 复苏玩家的所有角色
    /// # Arguments
    /// * `deduct_revives` - 是否消耗复苏次数
    #[native_exec(283)]
    pub fn revive_all_characters(self, deduct_revives: bool);

    // TODO: ID 245
    // TODO: ID 668
    // TODO: ID 250

    // TODO: 修改模型的颜色和材质
}
