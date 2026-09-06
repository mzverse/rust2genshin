use rust2genshin_lib_internal::native_calc;

pub struct Vec3 { // TODO
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

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
}
unsafe impl F32 for f32 {
    #[native_calc(221)]
    fn sqrt(self) -> Self;

    #[native_calc(215)]
    fn log(self, base: Self) -> Self;

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
