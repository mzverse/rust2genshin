use core::marker::{CoerceShared, PhantomData, Reborrow, Unsize};
use core::ops::{CoerceUnsized, Deref, DerefMut, LegacyReceiver};
use core::ptr::NonNull;
use rust2genshin_lib_internal::native;

#[repr(transparent)]
pub struct Box<T: ?Sized> {
    pointer: NonNull<T>,
    _marker: PhantomData<T>,
}
impl<T: ?Sized> LegacyReceiver for Box<T> {
}
impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<Box<U>> for Box<T> {
}
impl<T: ?Sized> Drop for Box<T> {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe { self.pointer.drop_in_place(); }
        #[native("free")]
        fn free<T: ?Sized>(pointer: NonNull<T>);
        free(self.pointer);
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
    #![allow(clippy::wrong_self_convention)]

    #[native("box_new")]
    pub fn new(value: T) -> Self;

    #[native("box_into_inner")]
    pub fn into_inner(boxed: Self) -> T;

    pub fn into_non_null(boxed: Self) -> NonNull<T> {
        boxed.pointer
    }

    pub fn into_ptr(boxed: Self) -> *mut T {
        boxed.pointer.as_ptr()
    }
    
    pub unsafe fn from_non_null(pointer: NonNull<T>) -> Box<T> {
        Self {
            pointer,
            _marker: PhantomData,
        }
    }

    /// # Safety
    pub unsafe fn from_ptr(pointer: *mut T) -> Self {
        unsafe {
            Self::from_non_null(NonNull::new_unchecked(pointer))
        }
    }
}

impl<T: ?Sized> Box<T> {
    #[inline(always)]
    pub fn borrow(&self) -> BoxRef<'_, T> {
        BoxRef {
            pointer: self.pointer,
            _marker: PhantomData,
        }
    }
    #[inline(always)]
    pub fn borrow_mut(&mut self) -> BoxRefMut<'_, T> {
        BoxRefMut {
            pointer: self.pointer,
            _marker: PhantomData,
        }
    }
}
impl<T: ?Sized> Deref for Box<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { self.pointer.as_ref() }
    }
}
impl<T: ?Sized> DerefMut for Box<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.pointer.as_mut() }
    }
}
impl<T: ?Sized> AsRef<T> for Box<T> {
    fn as_ref(&self) -> &T {
        self.deref()
    }
}
impl<T: ?Sized> AsMut<T> for Box<T> {
    fn as_mut(&mut self) -> &mut T {
        self.deref_mut()
    }
}

#[repr(transparent)]
pub struct BoxRef<'a, T: ?Sized> {
    pointer: NonNull<T>,
    _marker: PhantomData<&'a T>,
}
impl<T: ?Sized> LegacyReceiver for BoxRef<'_, T> {
}
impl<T: ?Sized> Copy for BoxRef<'_, T> {
}
impl<T: ?Sized> Clone for BoxRef<'_, T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Deref for BoxRef<'_, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { self.pointer.as_ref() }
    }
}
impl<T: ?Sized> AsRef<T> for BoxRef<'_, T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

#[repr(transparent)]
pub struct BoxRefMut<'a, T: ?Sized> {
    pointer: NonNull<T>,
    _marker: PhantomData<&'a mut T>,
}
impl<T: ?Sized> LegacyReceiver for BoxRefMut<'_, T> {
}
impl<'a, T> CoerceShared<BoxRef<'a, T>> for BoxRefMut<'a, T> {
}
impl<T: ?Sized> Reborrow for BoxRefMut<'_, T> {
}
impl<T: ?Sized> Deref for BoxRefMut<'_, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { self.pointer.as_ref() }
    }
}
impl<T: ?Sized> DerefMut for BoxRefMut<'_, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.pointer.as_mut() }
    }
}
impl<T: ?Sized> AsRef<T> for BoxRefMut<'_, T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        self.deref()
    }
}
impl<T: ?Sized> AsMut<T> for BoxRefMut<'_, T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut T {
        self.deref_mut()
    }
}
