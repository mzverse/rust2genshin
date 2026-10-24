use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use rust2genshin_lib_internal::native;
use crate::boxed::Box;

#[native("List")]
pub struct List<T> {
    internal: Box<[T]>,
    _marker: UnsafeCell<[T; 0]>, // 阻止递归包含；阻止储存&List<T>
}

impl<T, const N: usize> From<[T; N]> for List<T> {
    #[native("list_from_array")]
    fn from(value: [T; N]) -> Self;
}
impl<T> Default for List<T> {
    fn default() -> Self {
        [].into()
    }
}
impl<T> Deref for List<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.internal
    }
}
impl<T> DerefMut for List<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.internal
    }
}

pub macro list {
    [] => {
        $crate::list::List::default()
    },
    [$($x:expr),+ $(,)?] => {
        $crate::list::List::from([$($x),+])
    },
    [$elem:expr; $n:expr] => {
        const { todo!() }
    },
}

// TODO
impl<T> List<T> {


    #[inline(always)]
    pub fn insert(&mut self, index: usize, value: T) {
        #[native("list_insert")]
        fn insert_<T>(list: &List<T>, index: usize, value: T);
        insert_(self, index, value);
    }

    #[inline(always)]
    pub fn push(&mut self, value: T) {
        self.insert(self.len(), value);
    }
}
