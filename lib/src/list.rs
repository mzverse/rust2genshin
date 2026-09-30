use core::marker::PhantomData;
use rust2genshin_lib_internal::native;

#[repr(transparent)]
#[native("List")]
pub struct List<T>(&'static ListInternal, PhantomData<T>);

unsafe extern "Rust" {
    type ListInternal;
}

// TODO
