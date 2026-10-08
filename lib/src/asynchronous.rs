use core::ops::{Coroutine, CoroutineState};
use core::pin::Pin;
pub use rust2genshin_lib_internal::{asynchronous, async_loop};

pub macro sleep($x:expr) {
    yield $x
}

#[must_use]
#[inline(always)]
// #[native("async_point")]
pub fn async_point() -> f32 {
    0.
}

#[must_use]
#[inline(always)]
// #[native("async_jump")]
pub fn async_jump() -> f32 {
    0.
}

pub macro async_continue($($x:lifetime)?) {
    {
        yield $crate::asynchronous::async_jump();
        continue $($x)?;
    }
}

pub macro awa($x:expr) {
    {
        let mut x = ::core::pin::pin!($x);
        $crate::asynchronous::async_loop! {
            match ::core::ops::Coroutine::resume(::core::pin::Pin::new(&mut x), ()) {
                ::core::ops::CoroutineState::Yielded(x) => yield x,
                ::core::ops::CoroutineState::Complete(x) => break x,
            }
        }
    }
}

pub fn resume_coroutine<T: Coroutine>(co: &mut T) -> CoroutineState<T::Yield, T::Return> {
    unsafe { Pin::new_unchecked(co) }.resume(())
}
