use rust2genshin_lib_internal::native_calc;
use crate::Guid;

// TODO: rename
#[native_calc(751)]
pub fn get_player_id_by_guid(guid: Guid) -> i32;
