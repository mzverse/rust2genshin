pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/rust2genshin.rs"));
}

pub mod composite;
pub mod raw_node_graph;
pub mod node_graph;
pub mod value;

use enum_dispatch::enum_dispatch;
use generated::*;
use prost::Message;
use composite::CompositeNode;
use raw_node_graph::{RawNodeGraph, StructureDefinition};
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[enum_dispatch]
pub enum Asset {
    RawNodeGraph,
    StructureDefinition,
    CompositeNode,
}

#[enum_dispatch(Asset)]
pub trait IAsset {
    fn encode(&self, id: i64) -> AssetData;
}

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum FileType {
    Project = 1,
    Level = 2,
    AssetBundle = 3, // .gia
    Runtime = 4, //
}
pub enum GameMode {
    Overlimit,
    Classic,
}
pub struct AssetBundle {
    mode: GameMode,
    assets: Vec<Asset>,
    display: Vec<usize>,
}

const ENGINE_VERSION: &str = "6.3.0";

impl AssetBundle {
    pub fn new(mode: GameMode, assets: Vec<Asset>, display: Vec<usize>) -> Self {
        Self { mode, assets, display }
    }

    pub fn encode(&self) -> AssetBundleData {
        // asset_guid 分配(对齐真实导出 export.gia 的 ID 空间):
        //   主入口资源(display 下标)→ 结构体 ID 空间 0x40400001 起
        //   依赖资源 → 节点声明 ID 空间 0x60000001 起
        // 注:真实 ID 分配器在游戏侧是全局自增的;此处为占位策略,可后续替换。
        const PRIMARY_GUID_BASE: i64 = 0x4040_0001;
        const DEP_GUID_BASE: i64 = 0x6000_0001;
        let mut assets = Vec::new();
        let mut dependencies = Vec::new();
        for (i, asset) in self.assets.iter().enumerate() {
            let is_primary = self.display.contains(&i);
            let guid = if is_primary {
                PRIMARY_GUID_BASE + i as i64
            } else {
                DEP_GUID_BASE + i as i64
            };
            let data = asset.encode(guid);
            if is_primary {
                assets.push(data);
            } else {
                dependencies.push(data);
            }
        }
        AssetBundleData {
            assets,
            dependencies,
            // export_tag 格式参考真实导出:{UID}-{TIME}-{FILE_ID}-\{EXPORT_FILE_NAME}.gia
            export_info: "0-0-0-\\rust2genshin.gia".to_string(), // TODO: 传真实文件名
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

