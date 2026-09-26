use rust2genshin_lib_internal::{native, native_calc};
use crate::{String, ToString};

/// 三维向量 (Server 类型 ID 12,Vec3)
#[repr(C)]
#[derive(Copy, Clone)]
#[native("Vec3")]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// 向量加法
    #[native_calc(10)]
    pub fn add(self, other: Self) -> Self;

    /// 向量减法
    #[native_calc(11)]
    pub fn sub(self, other: Self) -> Self;

    /// 向量缩放
    #[native_calc(12)]
    pub fn scale(self, factor: f32) -> Self;

    /// 归一化
    #[native_calc(74)]
    pub fn normalize(self) -> Self;

    /// 向量长度
    #[native_calc(220)]
    pub fn length(self) -> f32;

    /// 与另一向量的距离
    #[native_calc(244)]
    pub fn distance(self, other: Self) -> f32;

    /// 向量点积
    #[native_calc(505)]
    pub fn dot(self, other: Self) -> f32;

    /// 向量叉积
    #[native_calc(506)]
    pub fn cross(self, other: Self) -> Self;

    /// 两向量夹角(度)
    #[native_calc(13)]
    pub fn angle(self, other: Self) -> f32;

    /// 按旋转量旋转向量
    #[native_calc(474)]
    pub fn rotate(self, rotation: Self) -> Self;
}

impl ToString for bool {
    #[native("to_string")]
    fn to_string(&self) -> String;
}

// ========================================================================
// 自由函数式数学 / 逻辑
// ========================================================================

/// 整数取模
#[native_calc(208)]
pub fn modulo(a: i32, b: i32) -> i32;

/// 布尔与
#[native_calc(226)]
pub fn and(a: bool, b: bool) -> bool;

/// 布尔或
#[native_calc(227)]
pub fn or(a: bool, b: bool) -> bool;

/// 布尔异或
#[native_calc(228)]
pub fn xor(a: bool, b: bool) -> bool;

/// 布尔非
#[native_calc(229)]
pub fn not(a: bool) -> bool;

/// 圆周率
#[native_calc(191)]
pub fn pi() -> f32;

/// 弧度转角度
#[native_calc(321)]
pub fn rad_to_deg(radians: f32) -> f32;

/// 角度转弧度
#[native_calc(322)]
pub fn deg_to_rad(degrees: f32) -> f32;

/// 区间 [lower, upper] 内的随机整数
#[native_calc(257)]
pub fn random_int(lower: i32, upper: i32) -> i32;

/// 区间 [lower, upper) 内的随机浮点
#[native_calc(7)]
pub fn random_float(lower: f32, upper: f32) -> f32;

/// 当前时间戳(秒)
#[native_calc(755)]
pub fn timestamp() -> i32;

/// 当前时区(秒偏移)
#[native_calc(756)]
pub fn timezone() -> i32;

/// 时间戳转星期(0=周日,1=周一,...6=周六)
#[native_calc(754)]
pub fn timestamp_to_weekday(timestamp: i32) -> i32;

/// 年月日时分秒 → 时间戳
#[native_calc(753)]
pub fn time_to_timestamp(year: i32, month: i32, day: i32, hour: i32, minute: i32, second: i32) -> i32;

/// 左移
#[native_calc(778)]
pub fn left_shift(value: i32, shift: i32) -> i32;

/// 位与
#[native_calc(780)]
pub fn bit_and(a: i32, b: i32) -> i32;

/// 位或
#[native_calc(781)]
pub fn bit_or(a: i32, b: i32) -> i32;

/// 位异或
#[native_calc(782)]
pub fn bit_xor(a: i32, b: i32) -> i32;

/// 位非(按位取反)
#[native_calc(783)]
pub fn bit_not(value: i32) -> i32;

/// 写一个 bit(返回写入后的新值)
#[native_calc(784)]
pub fn write_bit(value: i32, bit_index: i32, set: bool) -> i32;

/// 读一个 bit
#[native_calc(785)]
pub fn read_bit(value: i32, bit_index: i32) -> bool;

/// Integer math helpers backed by genshin node-graph kernel operations.
///
/// # Safety
///
/// This trait must only be implemented for primitive integer types whose
/// genshin node-graph kernel IDs match those in the `#[native_calc(N)]`
/// attributes on each method. The current implementation targets `i32` and
/// uses kernel `779` (ushr) plus an inlined shr. Implementing this trait
/// for any other type would dispatch to wrong kernel IDs and produce a
/// corrupt node graph.
pub unsafe trait I32 {
    fn ushr(self, rhs: Self) -> Self;
    fn shr(self, rhs: Self) -> Self;
    fn ipow(self, rhs: Self) -> Self;
}

/// Float math helpers backed by genshin node-graph kernel operations.
///
/// # Safety
///
/// This trait must only be implemented for primitive float types whose
/// genshin node-graph kernel IDs match those in the `#[native_calc(N)]`
/// attributes on each method. The current implementation targets `f32`
/// and uses kernels `221` (sqrt), `215` (log), `291-296` (trig). Implementing
/// this trait for any other type would dispatch to wrong kernel IDs and
/// produce a corrupt node graph.
pub unsafe trait F32 {
    fn sqrt(self) -> Self;

    fn log(self, base: Self) -> Self;
    fn pow(self, exp: Self) -> Self;

    fn sin(self) -> Self;
    fn cos(self) -> Self;
    fn tan(self) -> Self;

    fn asin(self) -> Self;
    fn acos(self) -> Self;
    fn atan(self) -> Self;
}

unsafe impl I32 for i32 {
    #[native_calc(779)]
    fn ushr(self, rhs: Self) -> Self;

    #[inline(always)]
    fn shr(self, rhs: Self) -> Self {
        if rhs == 0 {
            return self;
        }
        let result = self.ushr(rhs);
        if self < 0 {
            result | !(1i32 << (32 - rhs)).wrapping_sub(1)
        } else {
            result
        }
    }

    #[native("power")]
    fn ipow(self, rhs: Self) -> Self;
}
impl ToString for i32 {
    #[native("to_string")]
    fn to_string(&self) -> String;
}

unsafe impl F32 for f32 {
    #[native_calc(221)]
    fn sqrt(self) -> Self;

    #[native_calc(215)]
    fn log(self, base: Self) -> Self;

    #[native("power")]
    fn pow(self, exp: Self) -> Self;

    #[native_calc(291)]
    fn sin(self) -> Self;

    #[native_calc(292)]
    fn cos(self) -> Self;

    #[native_calc(293)]
    fn tan(self) -> Self;

    #[native_calc(294)]
    fn asin(self) -> Self;

    #[native_calc(295)]
    fn acos(self) -> Self;

    #[native_calc(296)]
    fn atan(self) -> Self;
}
impl ToString for f32 {
    #[native("to_string")]
    fn to_string(&self) -> String;
}
