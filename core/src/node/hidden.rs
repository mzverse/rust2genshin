//! 隐藏域节点(Server,Hidden)
//!
//! 人工设计:相机/震屏/名牌/GM 等隐藏功能节点。

use crate::asset::Identifier;
use crate::asset::generated::node_instance::DependencyDeclaration;
use crate::asset::generated::type_definition::StructReference;
use crate::asset::generated::{identifier, type_definition};
use crate::node::decl::{DeclPin, NodeDecl, decl_pins_value_out};
use crate::node::{NodeGraph, NodeKind, PinType};
use crate::value::NativeKind::{self, *};
use std::collections::HashMap;
use std::sync::LazyLock;
use tap::Tap;

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
    // TODO: for other type
    let mut result = NodeKind::trigger(428, vec![Entity, Guid, String, kind.clone(), kind.clone(), Bool]);
    let Struct(kind) = kind else {
        todo!();
    };
    let decl = DependencyDeclaration {
        identity: Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: identifier::AssetKind::SysCallStub as i32,
            guid: 0,
            runtime_id: 428,
        }.into(),
        type1: 174,
        unknown1: 1,
        st: StructReference {
            schema_id: kind.id.guid,
        }.into(),
    };
    graph.embedded.entry(decl).or_insert_with(|| NodeDecl {
        name: "".to_string(),
        description: "".to_string(),
        pins: HashMap::new().tap_mut(|pins| {
            pins.insert(PinType::OutControl, vec![DeclPin {
                name: "".to_string(),
                kind: None,
                meta: None,
            }]);
            pins.insert(PinType::OutValue, decl_pins_value_out(&result.values_out_types));
        }),
        implementation: Default::default(),
        template_root: Default::default(),
        template_sub: Default::default(),
        references: vec![],
    });
    result.id.kernel = 0;
    result.using_struct = Some(decl.into());
    result.selectors_out[3] = 21.into();
    result.selectors_out[4] = 21.into();
    result.imps_out[3] = type_definition::server_type::Implementation::Struct;
    result.imps_out[4] = type_definition::server_type::Implementation::Struct;
    result.references = vec![kind.id];
    result
}

/// GM 调用(ID 100000)
pub static NODE_ON_GM_CALL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::trigger(100000, vec![Entity, Guid, Int, Int, String, String])
});
