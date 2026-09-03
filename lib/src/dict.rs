use crate::entity::Entity;
use crate::{Guid, String};
use core::marker::PhantomData;

#[repr(C)]
pub struct Dict<K: DictKey, V>(i32, PhantomData<(K, V)>);

/// # Safety
/// never impl this trait manually
pub unsafe trait DictKey {
}

unsafe impl DictKey for i32 {
}
unsafe impl DictKey for String {
}
unsafe impl DictKey for Guid {
}
unsafe impl DictKey for Entity {
}
// TODO
