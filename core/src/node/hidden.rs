//! 隐藏域节点(Server,Hidden)
//!
//! 人工设计:相机/震屏/名牌/GM 等隐藏功能节点。

use crate::node::{NodeGraph, NodeKind};
use crate::structure::node_struct;
use crate::value::NativeKind::{self, *};
use std::sync::LazyLock;

/// 激活实体相机(ID 262)
pub static NODE_ACTIVATE_ENTITY_CAMERA: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(262, vec![List(Entity.into()), Entity])
});

/// 关闭实体相机(ID 263)
pub static NODE_DISABLE_ENTITY_CAMERA: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(263, vec![List(Entity.into())])
});

/// 激活聚焦相机(ID 264)
pub static NODE_ACTIVATE_FOCUS_CAMERA: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(264, vec![List(Entity.into()), Entity])
});

/// 关闭聚焦相机(ID 265)
pub static NODE_DISABLE_FOCUS_CAMERA: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(265, vec![List(Entity.into())])
});

/// 屏幕震动(ID 266)
pub static NODE_PLAY_SCREEN_SHAKE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(266, vec![List(Entity.into()), Float, Float, Float])
});

/// 设置干扰器状态(ID 366)
pub static NODE_SET_DISRUPTOR_STATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(366, vec![Entity, Entity, Bool])
});

/// 设置原生值(ID 445)
pub static NODE_SET_NATIVE_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(445, vec![Entity, String, Int, Bool, Bool])
});

/// 添加名牌(ID 615)
pub static NODE_ADD_NAMEPLATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(615, vec![Entity, Config])
});

/// 移除名牌(ID 616)
pub static NODE_REMOVE_NAMEPLATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(616, vec![Entity, Config])
});

/// 更新排行榜(ID 678)
pub static NODE_UPDATE_LEADERBOARD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(678, vec![List(Int.into()), Int, Int])
});

/// 读取原生值(ID 459):值查询,无 flow
pub static NODE_GET_NATIVE_VALUE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(459, vec![Entity, String, Bool], Int)
});

/// 原生值变化(ID 428)
pub fn node_on_native_custom_value_change<NodeId, Kind>(graph: &mut NodeGraph<NodeId, Kind>, kind: &NativeKind) -> NodeKind {
    let mut result = NodeKind::trigger(428, vec![Entity, Guid, String, kind.clone(), kind.clone(), Bool]);
    let selector: i32 = match kind {
        Struct(kind) =>
            return node_struct(graph, result, kind, 174, 21, [], [3, 4]),
        _ => todo!(),
    };
    result.id.selectors_out[3] = selector.into();
    result.id.selectors_out[4] = selector.into();
    result
}

/// GM 调用(ID 100000)
pub static NODE_ON_GM_CALL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(100000, vec![Entity, Guid, Int, Int, String, String])
});
