use crate::Guid;
use rust2genshin_lib_internal::native_exec;
use crate::list::List;

/// A Gc ref of an entity
#[derive(Clone, Copy)]
pub struct Entity(()); // TODO

impl Entity {
    #[native_exec(70)]
    pub fn spawn_preset(id: Guid, list: List<i32>);
}
