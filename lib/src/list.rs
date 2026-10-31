use crate::{drop_glue, PartialMut};
use core::ops::{Index, IndexMut};
use rust2genshin_lib_internal::native;

#[native("List")]
pub struct List<T> {
    _internal: *mut [T],
    _marker: [T; 0], // 阻止递归包含
}

impl<T, const N: usize> From<[T; N]> for List<T> {
    #[native("identity")]
    fn from(value: [T; N]) -> Self;
}
impl<T> Default for List<T> {
    #[inline(always)]
    fn default() -> Self {
        [].into()
    }
}
impl<T> Index<usize> for List<T> {
    type Output = T;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index)
    }
}
impl<T> IndexMut<usize> for List<T> {
    /// 必须 `#[inline(always)]`:节点图里写元素是「写视图」,这个 `&mut` 的
    /// 写句柄是假的(见 [`crate::PartialMut::into_mut`])。不内联的话它就是个
    /// 独立函数,假句柄会留在函数体里降级时报 `Unsupported`。
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        PartialMut::into_mut(self.get_mut(index))
    }
}
impl<T> Drop for List<T> {
    #[inline]
    fn drop(&mut self) {
        for i in 0..self.len() {
            unsafe {
                drop_glue(&mut *self.get_mut(i));
            }
        }
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
    #[native("identity")]
    pub unsafe fn into_array<const N: usize>(self) -> [T; N];

    #[native("list_get")]
    pub fn get(&self, index: usize) -> &T;

    pub fn get_mut(&mut self, index: usize) -> PartialMut<'_, T> {
        #[native("list_get")]
        fn get_mut_<T>(list: PartialMut<'_, List<T>>, index: usize) -> PartialMut<'_, T>;
        get_mut_(PartialMut::from_mut(self), index)
    }

    #[native("list_set")]
    pub unsafe fn set_via_move(list: PartialMut<Self>, index: usize, value: T);

    #[native("list_get")]
    pub unsafe fn get_via_move(list: &Self, index: usize) -> T;

    #[inline(always)]
    pub fn set(&mut self, index: usize, value: T) {
        unsafe {
            drop(Self::get_via_move(self, index));
            Self::set_via_move(PartialMut::from_mut(self), index, value);
        }
    }

    #[native("list_len")]
    pub fn len(&self) -> usize;

    /// 移除下标处的元素,后面的元素前移。
    #[native("list_remove")]
    pub fn remove(&mut self, index: usize);

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub fn insert(&mut self, index: usize, value: T) {
        #[native("list_insert")]
        fn insert_<T>(list: PartialMut<List<T>>, index: usize, value: T);
        insert_(PartialMut::from_mut(self), index, value);
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
