// The generated bindings contain a `Payload` oneof enum whose largest
// variant (`InterfaceData`) is ~424 bytes — clippy flags it as `large_enum_variant`.
// Boxing the variant is not an option for prost-generated code without
// restructuring the .proto schema, so we allow the lint at the include site.
// This is the standard pattern for generated code that triggers clippy lints.
#[allow(clippy::large_enum_variant)]
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/rust2genshin.rs"));
}

pub mod node_graph;
pub mod value;
pub mod structure;

pub use generated::Identifier;
pub use asset_bundle_data::Mode as GameMode;

use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter, Write};
use std::hash::{Hash, Hasher};
use generated::*;
use prost::Message;
use std::path::Path;
use tap::Tap;

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum FileType {
    Project = 1, // .gip
    Level = 2, // .gil
    AssetBundle = 3, // .gia
    Runtime = 4, // ?
}

#[derive(Clone, Copy)]
pub enum Side {
    Server,
    Client,
}

pub struct AssetRef<T: Asset + ?Sized> {
    pub root: Identifier,
    data: T::RefData,
}
impl<T: Asset> AssetRef<T> {
    pub fn new(root: Identifier, extra: T::RefData) -> Self {
        Self {
            root,
            data: extra,
        }
    }
}
impl<T: Asset> Clone for AssetRef<T>
where
    T::RefData: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.root, self.data.clone())
    }
}
impl<T: Asset> Debug for AssetRef<T>
where
    T::RefData: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.root.fmt(f)?;
        f.write_char(' ')?;
        self.data.fmt(f)?;
        Ok(())
    }
}
impl<T: Asset> PartialEq for AssetRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}
impl<T: Asset> Eq for AssetRef<T> {
}
impl<T: Asset> Hash for AssetRef<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.root.hash(state);
    }
}

pub trait Asset {
    type RefData: Sized;

    #[must_use]
    fn apply(self, bundle: &mut AssetBundle) -> AssetRef<Self>;
}

pub struct AssetBundle {
    pub mode: GameMode,
    pub allocators: HashMap<identifier::Category, i64>,
    pub assets: Vec<AssetData>,
    pub primary: HashSet<Identifier>,
}

const ENGINE_VERSION: &str = "7.0.0";

impl AssetBundle {
    pub fn new(mode: GameMode) -> Self {
        let mut allocators = HashMap::default();
        allocators.insert(identifier::Category::ServerNodeGraph, 0x40000001);
        allocators.insert(identifier::Category::Default, 0x40400001);
        allocators.insert(identifier::Category::NodeDecl, 0x60000001);
        Self {
            mode,
            allocators,
            assets: Default::default(),
            primary: Default::default(),
        }
    }
    
    pub fn get(&self, key: Identifier) -> Option<&AssetData> {
        self.assets.iter().find(|x| x.id == Some(key))
    }

    pub fn alloc(&mut self, cat: identifier::Category, kind: identifier::AssetKind) -> Identifier {
        let allocator = self.allocators.get_mut(&cat).unwrap_or_else(|| panic!("{cat:?}"));
        Identifier {
            source: 0,
            category: cat as i32,
            kind: kind as i32,
            guid: *allocator,
            runtime_id: 0,
        }.tap(|_| *allocator += 1)
    }

    pub fn push(&mut self, asset: AssetData) {
        self.assets.push(asset);
    }

    pub fn set_primary(&mut self, r: &AssetRef<impl Asset + ?Sized>) {
        self.primary.insert(r.root);
    }

    pub fn encode(self) -> AssetBundleData {
        let mut primary = Vec::new();
        let mut dependencies = Vec::new();
        for asset in self.assets {
            if self.primary.contains(&asset.id.unwrap()) {
                primary.push(asset);
            } else {
                dependencies.push(asset);
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
    ///
    /// Skips the write when the existing file already has the same bytes.
    /// This preserves the file's mtime across rebuilds with no inputs
    /// changed, which lets `demo/build.rs` track the .gia via
    /// `cargo:rerun-if-changed` without triggering an infinite loop on
    /// every successful build. External modifications (different bytes)
    /// are still detected — they advance the mtime and force one extra
    /// rebuild to overwrite the externally written content.
    pub fn save(self, path: &Path) -> std::io::Result<()> {
        let data = self.encode().encode_to_vec();
        // GIA 文件头:5 × u32 大端,共 20 字节
        // (权威格式见 GIA 项目 utils/protobuf/decode.ts 的 unwrap_gia/wrap_gia):
        //   [0x00] 文件总大小 - 4(= proto 长度 + 20)
        //   [0x04] schema 版本 = 1
        //   [0x08] 头部标记 0x0326(加载器严格校验)
        //   [0x0C] 文件类型 = 3(加载器严格校验,GIA = 3,固定值)
        //   [0x10] proto 数据长度(严格校验 = 文件总大小 - 24)
        // 尾部 4 字节:0x0679(严格校验)
        let mut new_bytes = Vec::with_capacity(data.len() + 24);
        new_bytes.extend_from_slice(&((data.len() + 20) as u32).to_be_bytes());
        new_bytes.extend_from_slice(&1u32.to_be_bytes());
        new_bytes.extend_from_slice(&0x0326u32.to_be_bytes());
        new_bytes.extend_from_slice(&(FileType::AssetBundle as u32).to_be_bytes());
        new_bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        new_bytes.extend_from_slice(data.as_ref());
        new_bytes.extend_from_slice(&0x0679u32.to_be_bytes());

        if let Ok(existing) = std::fs::read(path)
            && existing == new_bytes {
                // Content unchanged — preserve mtime so the next build
                // doesn't re-trigger due to this artifact.
                return Ok(());
            }

        std::fs::write(path, &new_bytes)?;
        Ok(())
    }
}

