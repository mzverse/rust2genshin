use core::marker::{PhantomData, Unsize};
use core::ops::{CoerceUnsized, Deref, DerefMut, LegacyReceiver};

#[repr(C)]
pub struct Box<T: ?Sized> {
    pointer: *mut T,
    _marker: PhantomData<T>,
}
impl<T: ?Sized> LegacyReceiver for Box<T> {
}
impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<Box<U>> for Box<T> {
}
impl<T: ?Sized> Deref for Box<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.pointer }
    }
}
impl<T: ?Sized> DerefMut for Box<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.pointer }
    }
}
impl<T: ?Sized> Drop for Box<T> {
    fn drop(&mut self) {
        unsafe { self.pointer.drop_in_place(); }
        todo!()
    }
}
impl<T> From<T> for Box<T> {
    #[inline(always)]
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

pub fn unbox<T>(boxed: Box<T>) -> T {
    Box::into_inner(boxed)
}
impl<T> Box<T> {
    pub fn new(value: T) -> Self {
        todo!()
    }
    #[allow(clippy::wrong_self_convention)]
    pub fn into_inner(boxed: Self) -> T {
        todo!()
    }
}
