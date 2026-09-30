//! 客户端域节点(Client,ID 200000+)
//!
//! 人工设计:纯值节点用 `NodeType::expr`(单输出)/ `NodeType::new`(多输出),
//! 操作节点用 `NodeType::procedure`(无返回值)/ `NodeType::new`(多输出)。
//! 命名统一 `NODE_CLIENT_` + 尾部,避免与 Server 节点冲突。

use crate::node::NodeKind;
use std::sync::LazyLock;
use crate::asset::value::NativeKind::*;

// ========================================================================
// 端口 / 图入口
// ========================================================================

/// 图结束(布尔)(ID 200000)
pub static NODE_CLIENT_GRAPH_END_BOOL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200000, vec![], Bool)
});

// ========================================================================
// 客户端算术(布尔 / 数值)
// ========================================================================

/// 与(ID 200001)
pub static NODE_CLIENT_AND: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200001, vec![Bool, Bool], Bool)
});

/// 或(ID 200002)
pub static NODE_CLIENT_OR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200002, vec![Bool, Bool], Bool)
});

/// 非(ID 200003)
pub static NODE_CLIENT_NOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200003, vec![Bool], Bool)
});

/// 异或(ID 200004)
pub static NODE_CLIENT_XOR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200004, vec![Bool, Bool], Bool)
});

/// 枚举匹配(ID 200005)
pub static NODE_CLIENT_ENUM_MATCH: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200005, vec![Enum(todo!()), Enum(todo!())], Bool)
});

/// 相等(ID 200006)
pub static NODE_CLIENT_EQUAL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200006, vec![Float, Float], Bool)
});

/// 大于(ID 200007)
pub static NODE_CLIENT_GREATER_THAN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200007, vec![Float, Float], Bool)
});

/// 小于(ID 200008)
pub static NODE_CLIENT_LESS_THAN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200008, vec![Float, Float], Bool)
});

/// 小于等于(ID 200009)
pub static NODE_CLIENT_LESS_EQUAL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200009, vec![Float, Float], Bool)
});

/// 大于等于(ID 200010)
pub static NODE_CLIENT_GREATER_EQUAL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200010, vec![Float, Float], Bool)
});

/// 加(ID 200011)
pub static NODE_CLIENT_ADD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200011, vec![Float, Float], Float)
});

/// 减(ID 200012)
pub static NODE_CLIENT_SUBTRACT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200012, vec![Float, Float], Float)
});

/// 乘(ID 200013)
pub static NODE_CLIENT_MULTIPLY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200013, vec![Float, Float], Float)
});

/// 除(ID 200014)
pub static NODE_CLIENT_DIVIDE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200014, vec![Float, Float], Float)
});

/// 绝对值(ID 200015)
pub static NODE_CLIENT_ABS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200015, vec![Float], Float)
});

/// 随机(ID 200032):[min, max] 内随机
pub static NODE_CLIENT_RANDOM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200032, vec![Float, Float], Float)
});

/// 类型转换(ID 200022)
pub static NODE_CLIENT_CONVERT_TYPE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200022, vec![Int], Int)
});

// ========================================================================
// 客户端向量
// ========================================================================

/// 向量点积(ID 200063)
pub static NODE_CLIENT_VECTOR_DOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200063, vec![Vec3, Vec3], Float)
});

/// 向量叉积(ID 200064)
pub static NODE_CLIENT_VECTOR_CROSS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200064, vec![Vec3, Vec3], Vec3)
});

/// 拆分向量(ID 200065):Vec → x/y/z
pub static NODE_CLIENT_SPLIT_VECTOR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(200065, 0, 0, vec![Vec3], vec![Float, Float, Float])
});

/// 向量缩放(ID 200066)
pub static NODE_CLIENT_VECTOR_SCALE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200066, vec![Float, Vec3], Vec3)
});

/// 向量夹角(ID 200067)
pub static NODE_CLIENT_VECTOR_ANGLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200067, vec![Vec3, Vec3], Float)
});

/// 向量旋转(ID 200068)
pub static NODE_CLIENT_VECTOR_ROTATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200068, vec![Vec3, Vec3], Vec3)
});

/// 向量长度(ID 200069)
pub static NODE_CLIENT_VECTOR_LENGTH: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200069, vec![Vec3], Float)
});

/// 创建向量(ID 200070)
pub static NODE_CLIENT_CREATE_VECTOR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200070, vec![Float, Float, Float], Vec3)
});

/// 向量加(ID 200071)
pub static NODE_CLIENT_VECTOR_ADD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200071, vec![Vec3, Vec3], Vec3)
});

/// 向量减(ID 200072)
pub static NODE_CLIENT_VECTOR_SUBTRACT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200072, vec![Vec3, Vec3], Vec3)
});

/// 向量转旋转(ID 200073)
pub static NODE_CLIENT_VECTOR_TO_ROTATION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200073, vec![Vec3, Vec3], Vec3)
});

/// 朝向转旋转(ID 200074)
pub static NODE_CLIENT_ORIENTATION_TO_ROTATION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200074, vec![Vec3], Vec3)
});

/// 向量归一化(ID 200100)
pub static NODE_CLIENT_VECTOR_NORMALIZE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200100, vec![Vec3], Vec3)
});

// ========================================================================
// 客户端三角函数
// ========================================================================

/// 正弦(ID 200094)
pub static NODE_CLIENT_SIN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200094, vec![Float], Float)
});

/// 余弦(ID 200095)
pub static NODE_CLIENT_COS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200095, vec![Float], Float)
});

/// 正切(ID 200096)
pub static NODE_CLIENT_TAN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200096, vec![Float], Float)
});

/// 反正弦(ID 200097)
pub static NODE_CLIENT_ASIN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200097, vec![Float], Float)
});

/// 反余弦(ID 200098)
pub static NODE_CLIENT_ACOS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200098, vec![Float], Float)
});

/// 反正切(ID 200099)
pub static NODE_CLIENT_ATAN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200099, vec![Float], Float)
});

/// 弧度转角度(ID 200101)
pub static NODE_CLIENT_RAD_TO_DEG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200101, vec![Float], Float)
});

/// 角度转弧度(ID 200102)
pub static NODE_CLIENT_DEG_TO_RAD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200102, vec![Float], Float)
});

// ========================================================================
// 客户端变量 / 列表
// ========================================================================

/// 获取自定义变量(ID 200016)
pub static NODE_CLIENT_GET_VARIABLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200016, vec![Entity, String], Int)
});

/// 按下标取值(ID 200017)
pub static NODE_CLIENT_GET_AT_INDEX: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200017, vec![Int, List(Int.into())], Int)
});

/// 获取列表长度(ID 200018)
pub static NODE_CLIENT_GET_LENGTH: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200018, vec![List(Int.into())], Int)
});

/// 是否包含(ID 200019)
pub static NODE_CLIENT_CONTAINS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200019, vec![Int, List(Int.into())], Bool)
});

/// 列表最大值(ID 200020)
pub static NODE_CLIENT_GET_MAX: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200020, vec![List(Int.into())], Int)
});

/// 列表最小值(ID 200021)
pub static NODE_CLIENT_GET_MIN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200021, vec![List(Int.into())], Int)
});

// ========================================================================
// 客户端实体查询
// ========================================================================

/// 按 GUID 获取实体(ID 200023)
pub static NODE_CLIENT_GET_BY_GUID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200023, vec![Guid], Entity)
});

/// 获取玩家角色(ID 200024)
pub static NODE_CLIENT_GET_PLAYER_CHARACTER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200024, vec![Entity], Entity)
});

/// 获取玩家的主人(ID 200025)
pub static NODE_CLIENT_GET_OWNER_PLAYER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200025, vec![Entity], Entity)
});

/// 获取所有玩家(ID 200026)
pub static NODE_CLIENT_GET_ALL_PLAYERS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200026, vec![], List(Entity.into()))
});

/// 获取实体 GUID(ID 200027)
pub static NODE_CLIENT_GET_GUID: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200027, vec![Entity], Guid)
});

/// 获取状态(ID 200028)
pub static NODE_CLIENT_GET_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200028, vec![Entity, Int], Int)
});

/// 获取阵营(ID 200029)
pub static NODE_CLIENT_GET_FACTION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200029, vec![Entity], Faction)
});

/// 获取位置(ID 200030)
pub static NODE_CLIENT_GET_LOCATION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200030, vec![Entity], Vec3)
});

/// 获取旋转(ID 200031)
pub static NODE_CLIENT_GET_ROTATION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200031, vec![Entity], Vec3)
});

/// 获取自身(ID 200033)
pub static NODE_CLIENT_GET_SELF: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200033, vec![], Entity)
});

/// 获取目标(ID 200034)
pub static NODE_CLIENT_GET_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200034, vec![], Entity)
});

/// 获取攻击目标(ID 200035)
pub static NODE_CLIENT_GET_ATTACK_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200035, vec![Entity], Entity)
});

/// 获取相机模板(ID 200036)
pub static NODE_CLIENT_GET_CAMERA_TEMPLATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200036, vec![], Int)
});

/// 是否在战斗中(ID 200037)
pub static NODE_CLIENT_IS_IN_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200037, vec![], Bool)
});

/// 球体过滤(ID 200043)
pub static NODE_CLIENT_FILTER_SPHERE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200043, vec![Float, Vec3, Int, Enum(todo!())], List(Entity.into()))
});

/// 方形过滤(ID 200044)
pub static NODE_CLIENT_FILTER_SQUARE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200044, vec![Float, Float, Float, Vec3, Int, Enum(todo!())], List(Entity.into()))
});

/// 获取实体类型(ID 200045)
pub static NODE_CLIENT_GET_ENTITY_TYPE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200045, vec![Entity], Enum(todo!()))
});

/// 获取相机旋转(ID 200046)
pub static NODE_CLIENT_GET_CAMERA_ROTATION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200046, vec![], Vec3)
});

/// 获取挂点位置(ID 200047)
pub static NODE_CLIENT_GET_SOCKET_LOC: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200047, vec![Entity, String], Vec3)
});

/// 获取挂点旋转(ID 200048)
pub static NODE_CLIENT_GET_SOCKET_ROT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200048, vec![Entity, String], Vec3)
});

/// 获取当前角色(ID 200076)
pub static NODE_CLIENT_GET_CURRENT_CHARACTER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200076, vec![], Entity)
});

/// 获取标签列表(ID 200077)
pub static NODE_CLIENT_GET_TAGS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200077, vec![Entity], List(Int.into()))
});

/// 按标签获取实体(ID 200078)
pub static NODE_CLIENT_GET_BY_TAG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200078, vec![Int], List(Entity.into()))
});

/// 获取局部变量(ID 200082)
pub static NODE_CLIENT_GET_LOCAL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200082, vec![String], Int)
});

/// 获取仇恨目标(ID 200090)
pub static NODE_CLIENT_GET_AGGRO_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200090, vec![Entity], Entity)
});

/// 获取仇恨列表(ID 200091)
pub static NODE_CLIENT_GET_AGGRO_LIST: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200091, vec![Entity], List(Entity.into()))
});

/// 是否在仇恨战斗中(ID 200092)
pub static NODE_CLIENT_AGGRO_IS_IN_COMBAT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200092, vec![Entity], Bool)
});

/// 是否敌对(ID 200093)
pub static NODE_CLIENT_IS_HOSTILE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200093, vec![Faction, Faction], Bool)
});

/// 是否存活(ID 200103)
pub static NODE_CLIENT_IS_ACTIVE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200103, vec![Entity], Bool)
});

/// 获取重叠实体(ID 200107)
pub static NODE_CLIENT_GET_OVERLAPPING_ENTITIES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200107, vec![Entity, Int], List(Entity.into()))
});

/// 获取射线结果(ID 200109):命中点 + 命中实体
pub static NODE_CLIENT_GET_RAY_RESULT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(200109, 0, 0, vec![Entity, Vec3, Vec3, Float, Enum(todo!()), List(Enum(todo!()).into())], vec![Vec3, Entity])
});

/// 获取射线过滤器(ID 200110)
pub static NODE_CLIENT_GET_RAY_FILTERS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200110, vec![Enum(todo!()), Enum(todo!()), Enum(todo!()), Enum(todo!())], List(Enum(todo!()).into()))
});

/// 获取扫描实体(ID 200118)
pub static NODE_CLIENT_GET_SCANNED_ENTITY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(200118, 0, 0, vec![], vec![Entity, Config])
});

/// 获取可扫描实体(ID 200119)
pub static NODE_CLIENT_GET_SCANNABLE_ENTITIES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200119, vec![], List(Entity.into()))
});

/// 获取扫描状态(ID 200120)
pub static NODE_CLIENT_GET_SCAN_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200120, vec![Entity], Enum(todo!()))
});

/// 获取活跃扫描标签(ID 200121)
pub static NODE_CLIENT_GET_ACTIVE_SCAN_TAGS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200121, vec![Entity], Config)
});

/// 获取输入设备类型(ID 200123)
pub static NODE_CLIENT_GET_INPUT_TYPE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200123, vec![], Enum(todo!()))
});

/// 获取实体类型列表(ID 200050)
pub static NODE_CLIENT_GET_ENTITY_TYPES: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(200050, vec![Enum(todo!()), Enum(todo!()), Enum(todo!()), Enum(todo!())], List(Enum(todo!()).into()))
});

// ========================================================================
// 客户端操作(1 flow)
// ========================================================================

/// 播放定时特效(ID 200038)
pub static NODE_CLIENT_PLAY_TIMED_FX: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200038, vec![Config, Vec3, Vec3, Float, Bool])
});

/// 通知服务器(ID 200039)
pub static NODE_CLIENT_NOTIFY_SERVER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200039, vec![String, String, String])
});

/// 转身(ID 200040)
pub static NODE_CLIENT_TURN_PLAYER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200040, vec![Enum(todo!())])
});

/// 设置目标(ID 200041)
pub static NODE_CLIENT_SET_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200041, vec![Entity, Bool])
});

/// 触发命中框(位置)(ID 200051)
pub static NODE_CLIENT_TRIGGER_HITBOX_LOC: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200051, vec![Enum(todo!()), Vec3, Vec3, Float, Float, List(Enum(todo!()).into())])
});

/// 发射投射物(ID 200052)
pub static NODE_CLIENT_LAUNCH_PROJECTILE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200052, vec![Prefab, Vec3, Vec3, Entity, Faction])
});

/// 移动到点(ID 200053)
pub static NODE_CLIENT_MOVE_TO_POINT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200053, vec![Float, Float, Float, Vec3, Bool])
});

/// 添加状态(ID 200057)
pub static NODE_CLIENT_ADD_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200057, vec![Entity, Int, Config])
});

/// 移除状态(ID 200058)
pub static NODE_CLIENT_REMOVE_STATUS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200058, vec![Entity, Config])
});

/// 触发命中框(挂点)(ID 200059)
pub static NODE_CLIENT_TRIGGER_HITBOX_SOCKET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200059, vec![Enum(todo!()), String, Vec3, Vec3, Float, Float, List(Enum(todo!()).into())])
});

/// 移除设备(ID 200060)
pub static NODE_CLIENT_REMOVE_DEVICE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200060, vec![Enum(todo!())])
});

/// 修改重量(ID 200061)
pub static NODE_CLIENT_MODIFY_WEIGHT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200061, vec![Float, Bool])
});

/// 获取相机数据(ID 200062):位置 + 旋转
pub static NODE_CLIENT_GET_CAMERA_DATA: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(200062, 1, 1, vec![Enum(todo!()), Vec3, Float, Float], vec![Vec3, Vec3])
});

/// 恢复血量(ID 200075)
pub static NODE_CLIENT_RECOVER_HP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200075, vec![Entity, Float, Bool, Float, Int])
});

/// 面向(ID 200105)
pub static NODE_CLIENT_TURN_TO_FACE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200105, vec![Vec3])
});

/// 重置目标(ID 200106)
pub static NODE_CLIENT_RESET_TARGET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200106, vec![])
});

/// 退出瞄准(ID 200108)
pub static NODE_CLIENT_EXIT_AIMING: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200108, vec![])
});

/// 触发球体命中框(位置)(ID 200111)
pub static NODE_CLIENT_TRIGGER_SPHERE_HITBOX_LOC: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200111, vec![Enum(todo!()), Vec3, Vec3, Float, Float, List(Enum(todo!()).into())])
});

/// 触发矩形命中框(位置)(ID 200112)
pub static NODE_CLIENT_TRIGGER_RECT_HITBOX_LOC: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200112, vec![Enum(todo!()), Vec3, Vec3, Float, Float, List(Enum(todo!()).into())])
});

/// 触发扇形命中框(位置)(ID 200113)
pub static NODE_CLIENT_TRIGGER_SECTOR_HITBOX_LOC: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200113, vec![Enum(todo!()), Vec3, Vec3, Float, Float, List(Enum(todo!()).into())])
});

/// 触发球体命中框(挂点)(ID 200114)
pub static NODE_CLIENT_TRIGGER_SPHERE_HITBOX_SOCKET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200114, vec![Enum(todo!()), String, Vec3, Vec3, Float, Float, List(Enum(todo!()).into())])
});

/// 触发矩形命中框(挂点)(ID 200115)
pub static NODE_CLIENT_TRIGGER_RECT_HITBOX_SOCKET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200115, vec![Enum(todo!()), String, Vec3, Vec3, Float, Float, List(Enum(todo!()).into())])
});

/// 触发扇形命中框(挂点)(ID 200116)
pub static NODE_CLIENT_TRIGGER_SECTOR_HITBOX_SOCKET: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200116, vec![Enum(todo!()), String, Vec3, Vec3, Float, Float, List(Enum(todo!()).into())])
});

/// 发送到服务器(ID 200124)
pub static NODE_CLIENT_SEND_TO_SERVER: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200124, vec![])
});

/// 设置局部变量(ID 200081)
pub static NODE_CLIENT_SET_LOCAL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200081, vec![String, Int])
});

/// 设置仇恨(ID 200083)
pub static NODE_CLIENT_SET_AGGRO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200083, vec![Entity, Entity, Int])
});

/// 修改仇恨(ID 200084)
pub static NODE_CLIENT_MODIFY_AGGRO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200084, vec![Entity, Entity, Int])
});

/// 修改仇恨比例(ID 200085)
pub static NODE_CLIENT_MODIFY_AGGRO_RATIO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200085, vec![Entity, Entity, Float])
});

/// 转移仇恨(ID 200086)
pub static NODE_CLIENT_TRANSFER_AGGRO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200086, vec![Entity, Entity, Entity, Float])
});

/// 清空仇恨(ID 200087)
pub static NODE_CLIENT_CLEAR_AGGRO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200087, vec![Entity])
});

/// 移除仇恨(ID 200088)
pub static NODE_CLIENT_REMOVE_AGGRO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200088, vec![Entity, Entity])
});

/// 嘲讽(ID 200089)
pub static NODE_CLIENT_TAUNT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200089, vec![Entity, Entity])
});

/// 中断(ID 200080)
pub static NODE_CLIENT_BREAK: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::procedure(200080, vec![])
});
