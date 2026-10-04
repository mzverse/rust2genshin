//! 算术域节点(Server,Arithmetic)
//!
//! 人工设计,替换自动生成版本:
//! - 引脚按语义命名(a/b/vector/scale/x/y/z 等),不穷举 out_0/out_1
//! - 动态数量结构用 `Vec`(Assemble_List / Assemble_Dictionary 的参数不穷举字段)
//! - 泛型 `R<T>` 数值按 Float 语义;execute/get_value 仅模拟(todo!())

use crate::node::NodeKind;
use crate::value::NativeKind::{self, *};
use std::sync::LazyLock;
// ========================================================================
// 向量运算
// ========================================================================

/// 拆分向量为分量(Arithmetic.Math.Split_Vector,ID 9):Vec → x/y/z
pub static NODE_SPLIT_VECTOR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(9, 0, 0, vec![Vec3], vec![Float, Float, Float])
});

/// 向量加法(ID 10):a + b
pub static NODE_VECTOR_ADD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(10, vec![Vec3, Vec3], Vec3)
});

/// 向量减法(ID 11):a - b
pub static NODE_VECTOR_SUBTRACT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(11, vec![Vec3, Vec3], Vec3)
});

/// 向量缩放(ID 12):vector * scale
pub static NODE_VECTOR_SCALE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(12, vec![Vec3, Float], Vec3)
});

/// 向量夹角(ID 13):a 与 b 的夹角(度)
pub static NODE_VECTOR_ANGLE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(13, vec![Vec3, Vec3], Float)
});

/// 向量归一化(ID 74):长度归一为 1
pub static NODE_VECTOR_NORMALIZE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(74, vec![Vec3], Vec3)
});

/// 向量长度(ID 220):模长
pub static NODE_VECTOR_LENGTH: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(220, vec![Vec3], Float)
});

/// 两点距离(ID 244):a 与 b 的距离
pub static NODE_DISTANCE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(244, vec![Vec3, Vec3], Float)
});

/// 向量旋转(ID 474):按旋转量旋转
pub static NODE_VECTOR_ROTATE: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(474, vec![Vec3, Vec3], Vec3)
});

/// 向量点积(ID 505)
pub static NODE_VECTOR_DOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(505, vec![Vec3, Vec3], Float)
});

/// 向量叉积(ID 506)
pub static NODE_VECTOR_CROSS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(506, vec![Vec3, Vec3], Vec3)
});

/// 向量转旋转(ID 519):由前向/上向量构造旋转
pub static NODE_VECTOR_TO_ROTATION: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(519, vec![Vec3, Vec3], Vec3)
});

/// 创建向量(ID 225):x/y/z 分量
pub static NODE_CREATE_VECTOR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(225, vec![Float, Float, Float], Vec3)
});

// ========================================================================
// 数值二元运算(泛型 R<T>,按 Float 语义)
// ========================================================================

/// 加法(ID 200,泛型变体):shell 固定 200,kernel 随类型(Int→200、Flt→201)。
pub fn node_add(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(200, vec![ty.clone(), ty.clone()], ty.clone());

    let selected = match ty {
        Int => {

            result.id.kernel = 200;
            0

        }
        Float => {

            result.id.kernel = 201;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 减法(ID 202,泛型变体):shell 固定 202,kernel 随类型(Int→202、Flt→203)。
pub fn node_subtract(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(202, vec![ty.clone(), ty.clone()], ty.clone());

    let selected = match ty {
        Int => {

            result.id.kernel = 202;
            0

        }
        Float => {

            result.id.kernel = 203;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 乘法(ID 204,泛型变体):shell 固定 204,kernel 随类型(Int→204、Flt→205)。
pub fn node_multiply(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(204, vec![ty.clone(), ty.clone()], ty.clone());

    let selected = match ty {
        Int => {

            result.id.kernel = 204;
            0

        }
        Float => {

            result.id.kernel = 205;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 除法(ID 206,泛型变体):shell 固定 206,kernel 随类型(Int→206、Flt→207)。
pub fn node_divide(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(206, vec![ty.clone(), ty.clone()], ty.clone());

    let selected = match ty {
        Int => {

            result.id.kernel = 206;
            0

        }
        Float => {

            result.id.kernel = 207;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 幂运算(ID 209,泛型变体):shell 固定 209,kernel 随类型(Int→209、Flt→210)。
pub fn node_power(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(209, vec![ty.clone(), ty.clone()], ty.clone());

    let selected = match ty {
        Int => {

            result.id.kernel = 209;
            0

        }
        Float => {

            result.id.kernel = 210;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 取大值(ID 211,泛型变体):shell 固定 211,kernel 随类型(Int→211、Flt→212)。
pub fn node_max(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(211, vec![ty.clone(), ty.clone()], ty.clone());

    let selected = match ty {
        Int => {

            result.id.kernel = 211;
            0

        }
        Float => {

            result.id.kernel = 212;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 取小值(ID 213,泛型变体):shell 固定 213,kernel 随类型(Int→213、Flt→214)。
pub fn node_min(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(213, vec![ty.clone(), ty.clone()], ty.clone());

    let selected = match ty {
        Int => {

            result.id.kernel = 213;
            0

        }
        Float => {

            result.id.kernel = 214;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 取余(ID 208):整数取余
pub static NODE_REM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(208, vec![Int, Int], Int)
});

// ========================================================================
// 一元运算与夹取
// ========================================================================

/// 绝对值(ID 216,泛型变体):shell 固定 216,kernel 随类型(Int→216、Flt→217)。
pub fn node_abs(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(216, vec![ty.clone()], ty.clone());

    let selected = match ty {
        Int => {

            result.id.kernel = 216;
            0

        }
        Float => {

            result.id.kernel = 217;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 取符号(ID 218,泛型变体):-1 / 0 / 1;shell 固定 218,kernel 随类型(Int→218、Flt→219)。
pub fn node_sign(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(218, vec![ty.clone()], ty.clone());

    let selected = match ty {
        Int => {

            result.id.kernel = 218;
            0

        }
        Float => {

            result.id.kernel = 219;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 夹取(ID 222,泛型变体):value 限制在 [min, max];
/// shell 固定 222,kernel 随类型(Int→222、Flt→223)。
pub fn node_clamp(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(222, vec![ty.clone(), ty.clone(), ty.clone()], ty.clone());

    let selected = match ty {
        Int => {

            result.id.kernel = 222;
            0

        }
        Float => {

            result.id.kernel = 223;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result.selectors_in[2] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 四舍五入(ID 224):value 按舍入模式取整
pub static NODE_ROUND: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(224, vec![Float, Enum(todo!())], Int)
});

/// 平方根(ID 221)
pub static NODE_SQRT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(221, vec![Float], Float)
});

/// 对数(ID 215):log_base(value)
pub static NODE_LOGARITHM: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(215, vec![Float, Float], Float)
});

// ========================================================================
// 三角函数
// ========================================================================

/// 正弦(ID 291)
pub static NODE_SIN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(291, vec![Float], Float)
});

/// 余弦(ID 292)
pub static NODE_COS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(292, vec![Float], Float)
});

/// 正切(ID 293)
pub static NODE_TAN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(293, vec![Float], Float)
});

/// 反正弦(ID 294)
pub static NODE_ASIN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(294, vec![Float], Float)
});

/// 反余弦(ID 295)
pub static NODE_ACOS: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(295, vec![Float], Float)
});

/// 反正切(ID 296)
pub static NODE_ATAN: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(296, vec![Float], Float)
});

/// 弧度转角度(ID 321)
pub static NODE_RAD_TO_DEG: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(321, vec![Float], Float)
});

/// 角度转弧度(ID 322)
pub static NODE_DEG_TO_RAD: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(322, vec![Float], Float)
});

// ========================================================================
// 布尔逻辑
// ========================================================================

/// 与(ID 226)
pub static NODE_AND: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(226, vec![Bool, Bool], Bool)
});

/// 或(ID 227)
pub static NODE_OR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(227, vec![Bool, Bool], Bool)
});

/// 异或(ID 228)
pub static NODE_XOR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(228, vec![Bool, Bool], Bool)
});

/// 非(ID 229)
pub static NODE_NOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(229, vec![Bool], Bool)
});

// ========================================================================
// 比较
// ========================================================================

/// 相等(ID 14,泛型变体):shell 固定 14,kernel 随类型(Str→14、Gid→15、
/// Ety→16、Vec→17、Int→370、Flt→371、Cfg→581、Pfb→582、Bol→786);输出 Bol。
pub fn node_equal(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(14, vec![ty.clone(), ty.clone()], Bool);
    // selected index 按变体顺序(Str,Gid,Ety,Vec,Fct,Int,Flt,Cfg,Pfb,Bol),与 kernel 无关
    let (selected, kernel) = match ty {
        String => (0, 14),
        Guid => (1, 15),
        Entity => (2, 16),
        Vec3 => (3, 17),
        Int => (5, 370),
        Float => (6, 371),
        Config => (7, 581),
        Prefab => (8, 582),
        Bool => (9, 786),
        _ => panic!("Unsupported type: {ty:?}"),
    };
    result.id.kernel = kernel;
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}

/// 枚举相等(ID 475)
pub fn node_enum_equal(id: i32) -> NodeKind {
    let mut result = NodeKind::expr(475, vec![Enum(id), Enum(id)], Bool);
    let (selected, kernel) = match id {
        2..31 => (id - 1, id as i64 + 474),
        31..40 => (id - 1, id as i64 + 3320),
        42..44 => (id - 2, id as i64 + 734),
        200043 => (39, 759),
        _ => panic!(),
    };
    result.id.kernel = kernel;
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}

/// 小于(ID 230,泛型变体):shell 固定 230,kernel 随类型(Int→230、Flt→235);输出 Bol。
pub fn node_less_than(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(230, vec![ty.clone(), ty.clone()], Bool);

    let selected = match ty {
        Int => {

            result.id.kernel = 230;
            0

        }
        Float => {

            result.id.kernel = 235;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}

/// 小于等于(ID 231,泛型变体):shell 固定 231,kernel 随类型(Int→231、Flt→236);输出 Bol。
pub fn node_less_equal(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(231, vec![ty.clone(), ty.clone()], Bool);

    let selected = match ty {
        Int => {

            result.id.kernel = 231;
            0

        }
        Float => {

            result.id.kernel = 236;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}

/// 大于(ID 232,泛型变体):shell 固定 232,kernel 随类型(Int→232、Flt→237);输出 Bol。
pub fn node_greater_than(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(232, vec![ty.clone(), ty.clone()], Bool);

    let selected = match ty {
        Int => {

            result.id.kernel = 232;
            0

        }
        Float => {

            result.id.kernel = 237;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}

/// 大于等于(ID 233,泛型变体):shell 固定 233,kernel 随类型(Int→233、Flt→238);输出 Bol。
pub fn node_greater_equal(ty: NativeKind) -> NodeKind {
    let mut result = NodeKind::expr(233, vec![ty.clone(), ty.clone()], Bool);

    let selected = match ty {
        Int => {

            result.id.kernel = 233;
            0

        }
        Float => {

            result.id.kernel = 238;
            1

        }
        _ => panic!("Unsupported type: {ty:?}")
    };
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}

// ========================================================================
// 位运算
// ========================================================================

/// 左移(ID 778)
pub static NODE_LEFT_SHIFT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(778, vec![Int, Int], Int)
});

/// 右移(ID 779)
pub static NODE_RIGHT_SHIFT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(779, vec![Int, Int], Int)
});

/// 按位与(ID 780)
pub static NODE_BITWISE_AND: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(780, vec![Int, Int], Int)
});

/// 按位或(ID 781)
pub static NODE_BITWISE_OR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(781, vec![Int, Int], Int)
});

/// 按位异或(ID 782)
pub static NODE_BITWISE_XOR: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(782, vec![Int, Int], Int)
});

/// 按位非(ID 783)
pub static NODE_BITWISE_NOT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(783, vec![Int], Int)
});

/// 写入位(ID 784):value 的第 bit 位置为 bit_value
pub static NODE_WRITE_BIT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(784, vec![Int, Int, Int], Int)
});

/// 读取位(ID 785):取出 value 的第 bit 位
pub static NODE_READ_BIT: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(785, vec![Int, Int], Int)
});

// ========================================================================
// 时间
// ========================================================================

/// 时间戳转时间(ID 752):timestamp → 年月日时分秒
pub static NODE_TIMESTAMP_TO_TIME: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(
        752,
        0,
        0,
        vec![Int],
        vec![
            Int, // 年
            Int, // 月
            Int, // 日
            Int, // 时
            Int, // 分
            Int, // 秒
        ],
    )
});

/// 时间转时间戳(ID 753):年月日时分秒 → timestamp
pub static NODE_TIME_TO_TIMESTAMP: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(
        753,
        0,
        0,
        vec![
            Int, // 年
            Int, // 月
            Int, // 日
            Int, // 时
            Int, // 分
            Int, // 秒
        ],
        vec![Int],
    )
});

/// 时间戳转星期(ID 754)
pub static NODE_TIMESTAMP_TO_WEEKDAY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(754, vec![Int], Int)
});

// ========================================================================
// 列表 / 字典 / 类型转换
// ========================================================================

/// 组装列表(ID 169,泛型变体):shell 固定 169,kernel 随元素类型(Int→169、
/// Str→170、Ety→171、Gid→172、Flt→173、Vec→174、Bol→175、Cfg→568、Pfb→569);
/// 输出 L<R<T>>(按元素类型返回列表占位)。
pub fn node_assemble_list(ty: NativeKind) -> NodeKind {
    // 元素输入动态添加,这里按 1 个元素占位;实际编译期按需扩展 values_in_types
    let mut result = NodeKind::expr(169, vec![ty.clone()], ty.clone());
    let (selected, kernel) = match ty {
        Int => (0, 169),
        String => (1, 170),
        Entity => (2, 171),
        Guid => (3, 172),
        Float => (4, 173),
        Vec3 => (5, 174),
        Bool => (6, 175),
        Config => (7, 568),
        Prefab => (8, 569),
        other => panic!("Arithmetic.General.Assemble_List does not support type {other:?}"),
    };
    result.id.kernel = kernel;
    result.selectors_in[0] = selected.into();
    result.selectors_out[0] = selected.into();
    result
}

/// 类型转换(ID 180,泛型变体):K 类型值转为 V 类型值;
/// shell 固定 180,kernel 随 (K,V) 组合(11 种,见特判);输出 R<V>。
pub fn node_cast(from_ty: NativeKind, to_ty: NativeKind) -> Option<NodeKind> {
    let mut result = NodeKind::expr(180, vec![from_ty.clone()], to_ty.clone());
    // kernel 由 (K,V) 组合决定(11 种)
    result.id.kernel = match (&from_ty, &to_ty) {
        (Int, Bool) => 180,
        (Int, Float) => 181,
        (Int, String) => 182,
        (Entity, String) => 183,
        (Guid, String) => 184,
        (Bool, Int) => 185,
        (Bool, String) => 186,
        (Float, Int) => 187,
        (Float, String) => 188,
        (Vec3, String) => 189,
        _ => return None,
    };
    // 输入 R<K> 的 selected index(Int→0、Ety→1、Gid→2、Bol→3、Flt→4、Vec→5)
    let selected_in = match &from_ty {
        Int => 0,
        Entity => 1,
        Guid => 2,
        Bool => 3,
        Float => 4,
        Vec3 => 5,
        _ => unreachable!(),
    };
    // 输出 R<V> 的 selected index(Bol→0、Flt→1、Str→2、Int→3)
    let selected_out = match &to_ty {
        Bool => 0,
        Float => 1,
        String => 2,
        Int => 3,
        _ => unreachable!(),
    };
    result.selectors_in[0] = selected_in.into();
    result.selectors_out[0] = selected_out.into();
    result.into()
}

/// 创建字典(ID 1088):key 列表 + value 列表 → 字典
pub static NODE_CREATE_DICTIONARY: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::expr(1088, vec![List(Int.into()), List(Int.into())], Dict { key: Int.into(), value: Int.into() })
});

/// 组装字典(ID 1788):若干 K/V 对拼成字典,键值对数量动态
pub static NODE_ASSEMBLE_DICTIONARY: LazyLock<NodeKind> = LazyLock::new(|| {
    // 键值对动态添加,这里按 1 对占位
    NodeKind::expr(1788, vec![Int, Int], Dict { key: Int.into(), value: Int.into() })
});
