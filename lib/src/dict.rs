use crate::entity::Entity;
use crate::{Guid, String};
use core::marker::PhantomData;
use rust2genshin_lib_internal::native;

#[repr(transparent)]
#[native("Dict")]
pub struct Dict<K: DictKey, V>(&'static DictInternal, PhantomData<(K, V)>);

unsafe extern "Rust" {
    type DictInternal;
}

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
