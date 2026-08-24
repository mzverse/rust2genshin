use crate::asset::{Asset, IAsset};
use crate::asset::generated::*;

pub enum NodeGraphClass {
    Entity,
}

pub struct RawNodeGraph {
    class: NodeGraphClass,
    id: Identifier,
    name: String,
}

impl IAsset for RawNodeGraph {
    fn encode(&self) -> AssetData {
        // TODO
        AssetData {
            id: Some(self.id),
            reference: vec![],
            name: self.name.clone(),
            r#type: match self.class {
                NodeGraphClass::Entity => asset_data::Type::EntityNodeGraph,
            } as i32,
            payload: None,
        }
    }
}



pub struct StructureDefinition {

}

impl IAsset for StructureDefinition {
    #[inline]
    fn encode(&self) -> AssetData {
        // TODO
        AssetData {
            id: None,
            reference: vec![],
            name: "".to_string(),
            r#type: asset_data::Type::Structure as i32,
            payload: None,
        }
    }
}
