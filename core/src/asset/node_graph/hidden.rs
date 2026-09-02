//! 隐藏域节点(Server,Hidden)
//!
//! 人工设计:相机/震屏/名牌/GM 等隐藏功能节点。

use std::sync::LazyLock;
use crate::asset::node_graph::NodeKind;
use crate::asset::value::{
    ValueBool, ValueConfig, ValueDefault, ValueEntity, ValueEntityList, ValueFloat,
    ValueGuid, ValueInt, ValueIntList, ValueString,
};

/// 激活实体相机(ID 262)
pub static NODE_ACTIVATE_ENTITY_CAMERA: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(262, vec![ValueEntityList::def(), ValueEntity::def()])
});

/// 关闭实体相机(ID 263)
pub static NODE_DISABLE_ENTITY_CAMERA: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(263, vec![ValueEntityList::def()])
});

/// 激活聚焦相机(ID 264)
pub static NODE_ACTIVATE_FOCUS_CAMERA: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(264, vec![ValueEntityList::def(), ValueEntity::def()])
});

/// 关闭聚焦相机(ID 265)
pub static NODE_DISABLE_FOCUS_CAMERA: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(265, vec![ValueEntityList::def()])
});

/// 屏幕震动(ID 266)
pub static NODE_PLAY_SCREEN_SHAKE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(266, vec![ValueEntityList::def(), ValueFloat::def(), ValueFloat::def(), ValueFloat::def()])
});

/// 设置干扰器状态(ID 366)
pub static NODE_SET_DISRUPTOR_STATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(366, vec![ValueEntity::def(), ValueEntity::def(), ValueBool::def()])
});

/// 设置原生值(ID 445)
pub static NODE_SET_NATIVE_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(445, vec![ValueEntity::def(), ValueString::def(), ValueInt::def(), ValueBool::def(), ValueBool::def()])
});

/// 添加名牌(ID 615)
pub static NODE_ADD_NAMEPLATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(615, vec![ValueEntity::def(), ValueConfig::def()])
});

/// 移除名牌(ID 616)
pub static NODE_REMOVE_NAMEPLATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(616, vec![ValueEntity::def(), ValueConfig::def()])
});

/// 更新排行榜(ID 678)
pub static NODE_UPDATE_LEADERBOARD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(678, vec![ValueIntList::def(), ValueInt::def(), ValueInt::def()])
});

/// 读取原生值(ID 459):值查询,无 flow
pub static NODE_GET_NATIVE_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(459, vec![ValueEntity::def(), ValueString::def(), ValueBool::def()], ValueInt::def())
});

/// 原生值变化(ID 428)
pub static NODE_ON_NATIVE_VALUE_CHANGE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(428, vec![ValueEntity::def(), ValueGuid::def(), ValueString::def(), ValueInt::def(), ValueInt::def(), ValueBool::def()])
});

/// GM 调用(ID 100000)
pub static NODE_ON_GM_CALL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(100000, vec![ValueEntity::def(), ValueGuid::def(), ValueInt::def(), ValueInt::def(), ValueString::def(), ValueString::def()])
});
