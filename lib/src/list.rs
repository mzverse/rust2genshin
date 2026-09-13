use core::marker::PhantomData;

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct List<T>(&'static ListInternal, PhantomData<T>);

unsafe extern "Rust" {
    type ListInternal;
}

// TODO
