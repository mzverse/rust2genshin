pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/rust2genshin.rs"));
}

pub mod raw_node_graph;
mod node_graph;

use enum_dispatch::enum_dispatch;
use generated::*;
use raw_node_graph::{RawNodeGraph, StructureDefinition};

#[enum_dispatch]
pub enum Asset {
    RawNodeGraph,
    StructureDefinition,
}

#[enum_dispatch(Asset)]
pub trait IAsset {
    fn encode(&self) -> AssetData;
}
