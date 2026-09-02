//! 算术域节点(Server,Arithmetic)
//!
//! 人工设计,替换自动生成版本:
//! - 引脚按语义命名(a/b/vector/scale/x/y/z 等),不穷举 out_0/out_1
//! - 动态数量结构用 `Vec`(Assemble_List / Assemble_Dictionary 的参数不穷举字段)
//! - 泛型 `R<T>` 数值按 Float 语义;execute/get_value 仅模拟(todo!())

use std::sync::LazyLock;
use crate::asset::generated::ServerTypeId;
use crate::asset::node_graph::NodeKind;
use crate::asset::value::{
    AnyValue, ValueBool, ValueConfig, ValueDefault, ValueDict, ValueEntity, ValueEnum, ValueFloat,
    ValueGuid, ValueInt, ValueIntList, ValuePrefab, ValueString, ValueStruct, ValueVector,
};
use anyhow::Result;

// ========================================================================
// 向量运算
// ========================================================================

/// 拆分向量为分量(Arithmetic.Math.Split_Vector,ID 9):Vec → x/y/z
pub static NODE_SPLIT_VECTOR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(9, 0, 0, vec![ValueVector::def()], vec![ValueFloat::def(), ValueFloat::def(), ValueFloat::def()])
});

/// 向量加法(ID 10):a + b
pub static NODE_VECTOR_ADD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(10, vec![ValueVector::def(), ValueVector::def()], ValueVector::def())
});

/// 向量减法(ID 11):a - b
pub static NODE_VECTOR_SUBTRACT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(11, vec![ValueVector::def(), ValueVector::def()], ValueVector::def())
});

/// 向量缩放(ID 12):vector * scale
pub static NODE_VECTOR_SCALE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(12, vec![ValueVector::def(), ValueFloat::def()], ValueVector::def())
});

/// 向量夹角(ID 13):a 与 b 的夹角(度)
pub static NODE_VECTOR_ANGLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(13, vec![ValueVector::def(), ValueVector::def()], ValueFloat::def())
});

/// 向量归一化(ID 74):长度归一为 1
pub static NODE_VECTOR_NORMALIZE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(74, vec![ValueVector::def()], ValueVector::def())
});

/// 向量长度(ID 220):模长
pub static NODE_VECTOR_LENGTH: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(220, vec![ValueVector::def()], ValueFloat::def())
});

/// 两点距离(ID 244):a 与 b 的距离
pub static NODE_DISTANCE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(244, vec![ValueVector::def(), ValueVector::def()], ValueFloat::def())
});

/// 向量旋转(ID 474):按旋转量旋转
pub static NODE_VECTOR_ROTATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(474, vec![ValueVector::def(), ValueVector::def()], ValueVector::def())
});

/// 向量点积(ID 505)
pub static NODE_VECTOR_DOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(505, vec![ValueVector::def(), ValueVector::def()], ValueFloat::def())
});

/// 向量叉积(ID 506)
pub static NODE_VECTOR_CROSS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(506, vec![ValueVector::def(), ValueVector::def()], ValueVector::def())
});

/// 向量转旋转(ID 519):由前向/上向量构造旋转
pub static NODE_VECTOR_TO_ROTATION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(519, vec![ValueVector::def(), ValueVector::def()], ValueVector::def())
});

/// 创建向量(ID 225):x/y/z 分量
pub static NODE_CREATE_VECTOR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(225, vec![ValueFloat::def(), ValueFloat::def(), ValueFloat::def()], ValueVector::def())
});

// ========================================================================
// 数值二元运算(泛型 R<T>,按 Float 语义)
// ========================================================================

/// 加法(ID 200,泛型变体):shell 固定 200,kernel 随类型(Int→200、Flt→201)。
pub fn node_add(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(200, vec![ty.clone(), ty.clone()], ty.clone());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 200;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 201;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 减法(ID 202,泛型变体):shell 固定 202,kernel 随类型(Int→202、Flt→203)。
pub fn node_subtract(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(202, vec![ty.clone(), ty.clone()], ty.clone());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 202;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 203;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 乘法(ID 204,泛型变体):shell 固定 204,kernel 随类型(Int→204、Flt→205)。
pub fn node_multiply(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(204, vec![ty.clone(), ty.clone()], ty.clone());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 204;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 205;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 除法(ID 206,泛型变体):shell 固定 206,kernel 随类型(Int→206、Flt→207)。
pub fn node_divide(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(206, vec![ty.clone(), ty.clone()], ty.clone());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 206;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 207;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 幂运算(ID 209,泛型变体):shell 固定 209,kernel 随类型(Int→209、Flt→210)。
pub fn node_power(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(209, vec![ty.clone(), ty.clone()], ty.clone());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 209;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 210;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 取大值(ID 211,泛型变体):shell 固定 211,kernel 随类型(Int→211、Flt→212)。
pub fn node_max(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(211, vec![ty.clone(), ty.clone()], ty.clone());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 211;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 212;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 取小值(ID 213,泛型变体):shell 固定 213,kernel 随类型(Int→213、Flt→214)。
pub fn node_min(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(213, vec![ty.clone(), ty.clone()], ty.clone());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 213;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 214;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 取余(ID 208):整数取模
pub static NODE_MODULO: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(208, vec![ValueInt::def(), ValueInt::def()], ValueInt::def())
});

// ========================================================================
// 一元运算与夹取
// ========================================================================

/// 绝对值(ID 216,泛型变体):shell 固定 216,kernel 随类型(Int→216、Flt→217)。
pub fn node_abs(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(216, vec![ty.clone()], ty.clone());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 216;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 217;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 取符号(ID 218,泛型变体):-1 / 0 / 1;shell 固定 218,kernel 随类型(Int→218、Flt→219)。
pub fn node_sign(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(218, vec![ty.clone()], ty.clone());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 218;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 219;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 夹取(ID 222,泛型变体):value 限制在 [min, max];
/// shell 固定 222,kernel 随类型(Int→222、Flt→223)。
pub fn node_clamp(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(222, vec![ty.clone(), ty.clone(), ty.clone()], ty.clone());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 222;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 223;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_in[2] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 四舍五入(ID 224):value 按舍入模式取整
pub static NODE_ROUND: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(224, vec![ValueFloat::def(), ValueEnum::def()], ValueInt::def())
});

/// 平方根(ID 221)
pub static NODE_SQRT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(221, vec![ValueFloat::def()], ValueFloat::def())
});

/// 对数(ID 215):log_base(value)
pub static NODE_LOGARITHM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(215, vec![ValueFloat::def(), ValueFloat::def()], ValueFloat::def())
});

// ========================================================================
// 三角函数
// ========================================================================

/// 正弦(ID 291)
pub static NODE_SIN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(291, vec![ValueFloat::def()], ValueFloat::def())
});

/// 余弦(ID 292)
pub static NODE_COS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(292, vec![ValueFloat::def()], ValueFloat::def())
});

/// 正切(ID 293)
pub static NODE_TAN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(293, vec![ValueFloat::def()], ValueFloat::def())
});

/// 反正弦(ID 294)
pub static NODE_ASIN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(294, vec![ValueFloat::def()], ValueFloat::def())
});

/// 反余弦(ID 295)
pub static NODE_ACOS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(295, vec![ValueFloat::def()], ValueFloat::def())
});

/// 反正切(ID 296)
pub static NODE_ATAN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(296, vec![ValueFloat::def()], ValueFloat::def())
});

/// 弧度转角度(ID 321)
pub static NODE_RAD_TO_DEG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(321, vec![ValueFloat::def()], ValueFloat::def())
});

/// 角度转弧度(ID 322)
pub static NODE_DEG_TO_RAD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(322, vec![ValueFloat::def()], ValueFloat::def())
});

// ========================================================================
// 布尔逻辑
// ========================================================================

/// 与(ID 226)
pub static NODE_AND: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(226, vec![ValueBool::def(), ValueBool::def()], ValueBool::def())
});

/// 或(ID 227)
pub static NODE_OR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(227, vec![ValueBool::def(), ValueBool::def()], ValueBool::def())
});

/// 异或(ID 228)
pub static NODE_XOR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(228, vec![ValueBool::def(), ValueBool::def()], ValueBool::def())
});

/// 非(ID 229)
pub static NODE_NOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(229, vec![ValueBool::def()], ValueBool::def())
});

// ========================================================================
// 比较
// ========================================================================

/// 相等(ID 14,泛型变体):shell 固定 14,kernel 随类型(Str→14、Gid→15、
/// Ety→16、Vec→17、Int→370、Flt→371、Cfg→581、Pfb→582、Bol→786);输出 Bol。
pub fn node_equal(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(14, vec![ty.clone(), ty.clone()], ValueBool::def());
    // selected index 按变体顺序(Str,Gid,Ety,Vec,Fct,Int,Flt,Cfg,Pfb,Bol),与 kernel 无关
    let (selected, kernel) = if ty.is::<ValueString>() {
        (0, 14)
    } else if ty.is::<ValueGuid>() {
        (1, 15)
    } else if ty.is::<ValueEntity>() {
        (2, 16)
    } else if ty.is::<ValueVector>() {
        (3, 17)
    } else if ty.is::<ValueInt>() {
        (5, 370)
    } else if ty.is::<ValueFloat>() {
        (6, 371)
    } else if ty.is::<ValueConfig>() {
        (7, 581)
    } else if ty.is::<ValuePrefab>() {
        (8, 582)
    } else if ty.is::<ValueBool>() {
        (9, 786)
    } else {
        panic!("Unsupported type: {ty:?}");
    };
    result.kernel_id = kernel;
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}

/// 枚举相等(ID 475)
pub static NODE_ENUM_EQUAL: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(475, vec![ValueEnum::def(), ValueEnum::def()], ValueBool::def())
});

/// 小于(ID 230,泛型变体):shell 固定 230,kernel 随类型(Int→230、Flt→235);输出 Bol。
pub fn node_less_than(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(230, vec![ty.clone(), ty.clone()], ValueBool::def());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 230;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 235;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}

/// 小于等于(ID 231,泛型变体):shell 固定 231,kernel 随类型(Int→231、Flt→236);输出 Bol。
pub fn node_less_equal(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(231, vec![ty.clone(), ty.clone()], ValueBool::def());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 231;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 236;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}

/// 大于(ID 232,泛型变体):shell 固定 232,kernel 随类型(Int→232、Flt→237);输出 Bol。
pub fn node_greater_than(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(232, vec![ty.clone(), ty.clone()], ValueBool::def());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 232;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 237;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}

/// 大于等于(ID 233,泛型变体):shell 固定 233,kernel 随类型(Int→233、Flt→238);输出 Bol。
pub fn node_greater_equal(ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(233, vec![ty.clone(), ty.clone()], ValueBool::def());
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 233;
        selected = 0;
    } else if ty.is::<ValueFloat>() {
        result.kernel_id = 238;
        selected = 1;
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}

// ========================================================================
// 位运算
// ========================================================================

/// 左移(ID 778)
pub static NODE_LEFT_SHIFT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(778, vec![ValueInt::def(), ValueInt::def()], ValueInt::def())
});

/// 右移(ID 779)
pub static NODE_RIGHT_SHIFT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(779, vec![ValueInt::def(), ValueInt::def()], ValueInt::def())
});

/// 按位与(ID 780)
pub static NODE_BITWISE_AND: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(780, vec![ValueInt::def(), ValueInt::def()], ValueInt::def())
});

/// 按位或(ID 781)
pub static NODE_BITWISE_OR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(781, vec![ValueInt::def(), ValueInt::def()], ValueInt::def())
});

/// 按位异或(ID 782)
pub static NODE_BITWISE_XOR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(782, vec![ValueInt::def(), ValueInt::def()], ValueInt::def())
});

/// 按位非(ID 783)
pub static NODE_BITWISE_NOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(783, vec![ValueInt::def()], ValueInt::def())
});

/// 写入位(ID 784):value 的第 bit 位置为 bit_value
pub static NODE_WRITE_BIT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(784, vec![ValueInt::def(), ValueInt::def(), ValueInt::def()], ValueInt::def())
});

/// 读取位(ID 785):取出 value 的第 bit 位
pub static NODE_READ_BIT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(785, vec![ValueInt::def(), ValueInt::def()], ValueInt::def())
});

// ========================================================================
// 时间
// ========================================================================

/// 时间戳转时间(ID 752):timestamp → 年月日时分秒
pub static NODE_TIMESTAMP_TO_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(
        752,
        0,
        0,
        vec![ValueInt::def()],
        vec![
            ValueInt::def(), // 年
            ValueInt::def(), // 月
            ValueInt::def(), // 日
            ValueInt::def(), // 时
            ValueInt::def(), // 分
            ValueInt::def(), // 秒
        ],
    )
});

/// 时间转时间戳(ID 753):年月日时分秒 → timestamp
pub static NODE_TIME_TO_TIMESTAMP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(
        753,
        0,
        0,
        vec![
            ValueInt::def(), // 年
            ValueInt::def(), // 月
            ValueInt::def(), // 日
            ValueInt::def(), // 时
            ValueInt::def(), // 分
            ValueInt::def(), // 秒
        ],
        vec![ValueInt::def()],
    )
});

/// 时间戳转星期(ID 754)
pub static NODE_TIMESTAMP_TO_WEEKDAY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(754, vec![ValueInt::def()], ValueInt::def())
});

// ========================================================================
// 列表 / 字典 / 类型转换
// ========================================================================

/// 组装列表(ID 169,泛型变体):shell 固定 169,kernel 随元素类型(Int→169、
/// Str→170、Ety→171、Gid→172、Flt→173、Vec→174、Bol→175、Cfg→568、Pfb→569);
/// 输出 L<R<T>>(按元素类型返回列表占位)。
pub fn node_assemble_list(ty: AnyValue) -> NodeKind {
    // 元素输入动态添加,这里按 1 个元素占位;实际编译期按需扩展 values_in_types
    let mut result = NodeKind::expr(169, vec![ty.clone()], ty.clone());
    let (selected, kernel) = match ty.get_server_type() {
        ServerTypeId::SInt => (0, 169),
        ServerTypeId::SString => (1, 170),
        ServerTypeId::SEntity => (2, 171),
        ServerTypeId::SGuid => (3, 172),
        ServerTypeId::SFloat => (4, 173),
        ServerTypeId::SVector => (5, 174),
        ServerTypeId::SBoolean => (6, 175),
        ServerTypeId::SConfig => (7, 568),
        ServerTypeId::SPrefab => (8, 569),
        other => panic!("Arithmetic.General.Assemble_List does not support type {other:?}"),
    };
    result.kernel_id = kernel;
    result.selectors_in[0] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 类型转换(ID 180,泛型变体):K 类型值转为 V 类型值;
/// shell 固定 180,kernel 随 (K,V) 组合(11 种,见特判);输出 R<V>。
pub fn node_convert_type(from_ty: AnyValue, to_ty: AnyValue) -> NodeKind {
    let mut result = NodeKind::expr(180, vec![from_ty.clone()], to_ty.clone());
    // kernel 由 (K,V) 组合决定(11 种)
    result.kernel_id = match (from_ty.get_server_type(), to_ty.get_server_type()) {
        (ServerTypeId::SInt, ServerTypeId::SBoolean) => 180,
        (ServerTypeId::SInt, ServerTypeId::SFloat) => 181,
        (ServerTypeId::SInt, ServerTypeId::SString) => 182,
        (ServerTypeId::SEntity, ServerTypeId::SString) => 183,
        (ServerTypeId::SGuid, ServerTypeId::SString) => 184,
        (ServerTypeId::SBoolean, ServerTypeId::SInt) => 185,
        (ServerTypeId::SBoolean, ServerTypeId::SString) => 186,
        (ServerTypeId::SFloat, ServerTypeId::SInt) => 187,
        (ServerTypeId::SFloat, ServerTypeId::SString) => 188,
        (ServerTypeId::SVector, ServerTypeId::SString) => 189,
        (from, to) => panic!("Arithmetic.General.Convert_Type does not support {from:?} -> {to:?}"),
    };
    // 输入 R<K> 的 selected index(Int→0、Ety→1、Gid→2、Bol→3、Flt→4、Vec→5)
    let selected_in = match from_ty.get_server_type() {
        ServerTypeId::SInt => 0,
        ServerTypeId::SEntity => 1,
        ServerTypeId::SGuid => 2,
        ServerTypeId::SBoolean => 3,
        ServerTypeId::SFloat => 4,
        ServerTypeId::SVector => 5,
        other => panic!("Unsupported type: {other:?}"),
    };
    // 输出 R<V> 的 selected index(Bol→0、Flt→1、Str→2、Int→3)
    let selected_out = match to_ty.get_server_type() {
        ServerTypeId::SBoolean => 0,
        ServerTypeId::SFloat => 1,
        ServerTypeId::SString => 2,
        ServerTypeId::SInt => 3,
        other => panic!("Unsupported type: {other:?}"),
    };
    result.selectors_in[0] = selected_in.into();
    result.selectors_out[0] = selected_out.into();
    result
}

/// 创建字典(ID 1088):key 列表 + value 列表 → 字典
pub static NODE_CREATE_DICTIONARY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1088, vec![ValueIntList::def(), ValueIntList::def()], ValueDict::new(ValueInt::default(), ValueInt::default()).into())
});

/// 组装字典(ID 1788):若干 K/V 对拼成字典,键值对数量动态
pub static NODE_ASSEMBLE_DICTIONARY: LazyLock<NodeKind> = LazyLock::new(|| {
    // 键值对动态添加,这里按 1 对占位
    NodeKind::expr(1788, vec![ValueInt::def(), ValueInt::def()], ValueDict::new(ValueInt::default(), ValueInt::default()).into())
});

/// 拼装结构体(ID 300002):字段值 → 结构体
pub static NODE_ASSEMBLE_STRUCT: LazyLock<NodeKind> = LazyLock::new(|| {
    // 字段值输入动态添加,按结构体定义顺序
    NodeKind::expr(300002, vec![], ValueStruct::new(0, vec![]).into())
});

/// 拆分结构体(ID 300003):结构体 → 字段值(动态)
pub static NODE_SPLIT_STRUCT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(300003, 0, 0, vec![ValueStruct::new(0, vec![]).into()], vec![])
});
