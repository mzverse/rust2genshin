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
        let mut x = $x;
        loop {
            match {
                use $crate::asynchronous::Resume;
                x.resume()
            } {
                ::core::ops::CoroutineState::Yielded(x) => yield x,
                ::core::ops::CoroutineState::Complete(x) => break x,
            }
        }
    }
}

pub trait Resume {
    type Return;

    fn resume(&mut self) -> CoroutineState<f32, Self::Return>;
}

impl<T: Coroutine<Yield = f32>> Resume for T {
    type Return = T::Return;

    fn resume(&mut self) -> CoroutineState<f32, Self::Return> {
        unsafe { Pin::new_unchecked(self) }.resume(())
    }
}
