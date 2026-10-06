use crate::boxed::Box;
use rust2genshin_lib_internal::native;

pub type List<T> = Box<[T]>;

impl<T, const N: usize> From<[T; N]> for List<T> {
    fn from(value: [T; N]) -> Self {
        Box::new(value)
    }
}
impl<T> Default for List<T> {
    fn default() -> Self {
        [].into()
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
