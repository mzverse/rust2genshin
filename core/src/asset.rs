pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/rust2genshin.rs"));
}

pub mod raw_node_graph;
pub mod node_graph;
pub mod value;

use generated::*;
use prost::Message;
use std::fs::File;
use std::io::Write;
use std::ops::Sub;
use std::path::Path;
use slab::Slab;

pub trait Asset: 'static {
    fn encode(&self, id: i64) -> Vec<AssetData>;
}
impl<T: Asset> From<T> for Box<dyn Asset> {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum FileType {
    Project = 1, // .gip
    Level = 2, // 
    AssetBundle = 3, // .gia
    Runtime = 4, //
}
pub use asset_bundle_data::Mode as GameMode;
pub struct AssetBundle {
    pub(crate) mode: GameMode,
    pub(crate) assets: Slab<Box<dyn Asset>>,
    pub(crate) display: Vec<i64>,
}

const ENGINE_VERSION: &str = "7.0.0";

impl AssetBundle {
    pub const ID_BEGIN: i64 = 0x40000000;

    pub fn new(mode: GameMode) -> Self {
        let assets = Slab::new();
        Self {
            mode,
            assets,
            display: Vec::new(),
        }
    }

    pub fn insert(&mut self, asset: Box<dyn Asset>) -> i64 {
        Self::ID_BEGIN + self.assets.insert(asset) as i64
    }

    pub fn remove(&mut self, id: i64) -> Box<dyn Asset> {
        self.assets.remove(id.sub(Self::ID_BEGIN) as usize)
    }

    pub fn encode(&self) -> AssetBundleData {
        let mut primary = Vec::new();
        let mut dependencies = Vec::new();
        for (i, asset) in &self.assets {
            for data in asset.encode(Self::ID_BEGIN + i as i64) {
                if self.display.contains(&data.id.unwrap().guid) {
                    primary.push(data);
                } else {
                    dependencies.push(data);
                }
            }
        }
        AssetBundleData {
            assets: primary,
            dependencies,
            export_info: "by mz".to_string(),
            mode: self.mode as i32,
            engine_version: ENGINE_VERSION.to_string(),
        }
    }

    /// .gia Genshin Impact Assets
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        let data = self.encode().encode_to_vec();
        // GIA 文件头:5 × u32 大端,共 20 字节
        // (权威格式见 GIA 项目 utils/protobuf/decode.ts 的 unwrap_gia/wrap_gia):
        //   [0x00] 文件总大小 - 4(= proto 长度 + 20)
        //   [0x04] schema 版本 = 1
        //   [0x08] 头部标记 0x0326(加载器严格校验)
        //   [0x0C] 文件类型 = 3(加载器严格校验,GIA = 3,固定值)
        //   [0x10] proto 数据长度(严格校验 = 文件总大小 - 24)
        // 尾部 4 字节:0x0679(严格校验)
        file.write_all(&((data.len() + 20) as u32).to_be_bytes())?;
        file.write_all(&1u32.to_be_bytes())?;
        file.write_all(&0x0326u32.to_be_bytes())?;
        file.write_all(&(FileType::AssetBundle as u32).to_be_bytes())?;
        file.write_all(&(data.len() as u32).to_be_bytes())?;
        file.write_all(data.as_ref())?;
        file.write_all(&0x0679u32.to_be_bytes())?;
        file.flush()?;
        Ok(())
    }
}

