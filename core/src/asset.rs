pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/rust2genshin.rs"));
}

pub mod raw_node_graph;
mod node_graph;
mod value;

use enum_dispatch::enum_dispatch;
use generated::*;
use prost::Message;
use raw_node_graph::{RawNodeGraph, StructureDefinition};
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[enum_dispatch]
pub enum Asset {
    RawNodeGraph,
    StructureDefinition,
}

#[enum_dispatch(Asset)]
pub trait IAsset {
    fn encode(&self) -> AssetData;
}

#[repr(i32)]
#[derive(Clone, Copy)]
pub enum AssetScope {
    Project = 1,
    Level = 2,
    Assets = 3, // TODO: rename
    Runtime = 4, // TODO: rename
}
pub enum GameMode {
    Overlimit,
    Classic,
}
pub struct AssetBundle {
    scope: AssetScope,
    mode: GameMode,
    assets: Vec<Asset>,
    dependencies: Vec<Asset>,
}

const ENGINE_VERSION: &str = "6.3.0";

impl AssetBundle {
    pub fn encode(&self) -> AssetBundleData {
        AssetBundleData {
            assets: self.assets.iter().map(IAsset::encode).collect(),
            dependencies: self.dependencies.iter().map(IAsset::encode).collect(),
            export_info: "by mz".to_string(), // TODO
            mode: match self.mode {
                GameMode::Overlimit => asset_bundle_data::Mode::Overlimit,
                GameMode::Classic => asset_bundle_data::Mode::Classic,
            } as i32,
            engine_version: ENGINE_VERSION.to_string(),
        }
    }

    /// .gia Genshin Impact Assets
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        let data = self.encode().encode_to_vec();
        file.write_all(&(data.len() + 20).to_be_bytes())?;
        file.write_all(&1i32.to_be_bytes())?; // schema_version
        file.write_all(&0x0326i32.to_be_bytes())?; // head_tag
        file.write_all(&(self.scope as i32).to_be_bytes())?;
        file.write_all(&data.len().to_be_bytes())?;
        file.write_all(data.as_ref())?;
        file.write_all(&0x0679i32.to_be_bytes())?;
        file.flush()?;
        Ok(())
    }
}

