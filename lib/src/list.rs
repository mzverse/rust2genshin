use crate::boxed::Box;
use core::intrinsics::transmute_unchecked;
use core::ops::Index;
use rust2genshin_lib_internal::native;
use crate::PartialMut;

#[native("List")]
pub struct List<T> {
    _internal: Box<[T]>,
    _marker: [T; 0], // 阻止递归包含
}

impl<T, const N: usize> From<[T; N]> for List<T> {
    #[inline(always)]
    fn from(value: [T; N]) -> Self {
        unsafe {
            transmute_unchecked(value)
        }
    }
}
impl<T> Default for List<T> {
    #[inline(always)]
    fn default() -> Self {
        [].into()
    }
}
impl<T> Index<usize> for List<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index)
    }
}

// impl<T> Deref for List<T> {
//     type Target = [T];
//
//     #[inline(always)]
//     fn deref(&self) -> &Self::Target {
//         unsafe {
//             transmute_unchecked(self)
//         }
//     }
// }
// impl<T> DerefMut for List<T> {
//     #[inline(always)]
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         unsafe {
//             transmute_unchecked(self)
//         }
//     }
// }

pub macro list {
    [] => {
        $crate::list::List::default()
    },
    [$($x:expr),+ $(,)?] => {
        $crate::list::List::from([$($x),+])
    },
    [$elem:expr; $n:expr] => {
        from_elem(&$elem, $n)
    },
}

fn from_elem<T: Clone>(elem: &T, n: usize) -> List<T> {
    let mut result = List::default();
    for _ in 0..n {
        result.insert(0, elem.clone());
    }
    result
}

// TODO
impl<T> List<T> {
    /// # Safety
    #[inline(always)]
    pub unsafe fn into_array<const N: usize>(self) -> [T; N] {
        unsafe {
            transmute_unchecked(self)
        }
    }

    #[native("list_get")]
    pub fn get(&self, index: usize) -> &T;

    #[inline(always)]
    pub fn set(&mut self, index: usize, value: T) {
        #[native("list_get")]
        fn get0<T>(s: &List<T>, index: usize) -> T;
        #[native("list_set")]
        fn set0<T>(s: PartialMut<List<T>>, index: usize, value: T);
        drop(get0(self, index));
        set0(self.into(), index, value);
    }

    #[native("list_len")]
    pub fn len(&self) -> usize;

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub fn insert(&mut self, index: usize, value: T) {
        #[native("list_insert")]
        fn insert_<T>(list: PartialMut<List<T>>, index: usize, value: T);
        insert_(self.into(), index, value);
    }

    #[inline(always)]
    pub fn push(&mut self, value: T) {
        self.insert(self.len(), value);
    }

    // #[asynchronous]
    // #[must_use]
    // pub fn drop_async(mut self) {
    //     #[async_loop]
    //     for x in &mut *self {
    //         unsafe {
    //             (x as *mut T).drop_in_place();
    //         }
    //     }
    //     mem::forget(self);
    // }
}
