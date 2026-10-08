use core::marker::PhantomData;
use crate::boxed::Box;
use core::ops::Coroutine;
use rust2genshin_lib_internal::native;

pub struct Task<R>(PhantomData<R>);

impl<R> Task<R> {
    #[native("task_spawn")]
    pub fn spawn<T: Coroutine<Yield=f32, Return=R>>(co: Box<T>) -> Self;
}
