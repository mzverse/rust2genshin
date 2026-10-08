use core::marker::PhantomData;
use core::ops::Deref;
use core::ptr::NonNull;
use crate::boxed::Box;

pub struct Sc<T> {
    ptr: NonNull<ScInner<T>>,
    _marker: PhantomData<ScInner<T>>,
}

struct ScInner<T> {
    count: usize,
    value: T,
}

impl<T> Drop for Sc<T> {
    fn drop(&mut self) {
        unsafe {
            let inner = self.ptr.as_ptr();
            (*inner).count -= 1;
            if (*inner).count == 0 {
                drop(Box::from_non_null(self.ptr));
            }
        }
    }
}

impl<T> Clone for Sc<T> {
    fn clone(&self) -> Self {
        unsafe {
            (*self.ptr.as_ptr()).count += 1;
        }
        Self {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }
}

impl<T> Deref for Sc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &self.ptr.as_ref().value
        }
    }
}

impl<T> Sc<T> {
    pub fn new(value: T) -> Self {
        Self {
            ptr: Box::into_non_null(ScInner {
                count: 1,
                value,
            }.into()),
            _marker: PhantomData,
        }
    }
}
