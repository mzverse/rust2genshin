use crate::asset::{AssetBundle, GameMode, Identifier};
use crate::compile::func::{FnDecl, NodeGraphIr};
use crate::compile::ir::{AdtInfo, FnInfo, Optimizer};
use crate::node::composite::CompositeNodeGraph;
use crate::node::{MainNodeGraph, NodeKind};
use crate::structure::StructRef;
use std::collections::HashMap;
use std::iter::Sum;
use std::ops::AddAssign;
use std::path::PathBuf;
use tap::Tap;

#[derive(Default, Clone)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Target {
    pub main: Option<NodeGraphIr>,
    pub functions: HashMap<String, FnInfo>,
    pub adts: HashMap<String, AdtInfo>,
}
impl AddAssign for Target {
    fn add_assign(&mut self, rhs: Self) {
        if let Some(main) = rhs.main {
            if self.main.is_some() {
                panic!();
            }
            self.main = Some(main);
        }
        self.functions.extend(rhs.functions);
        self.adts.extend(rhs.adts);
    }
}
impl Sum for Target {
    fn sum<I: Iterator<Item=Self>>(iter: I) -> Self {
        iter.reduce(|mut acc, other| {
            acc += other;
            acc
        }).unwrap_or_else(Default::default)
    }
}

#[derive(Clone)]
pub struct CompiledFn {
    pub id: Identifier,
    pub node: NodeKind,
    pub decl: FnDecl,
}
pub struct Linker {
    pub target: Target,
    output: PathBuf,
    pub assets: AssetBundle,
    pub adts: HashMap<String, StructRef>,
    pub functions: HashMap<String, CompiledFn>,
}

impl Linker {
    pub fn new(mode: GameMode, target: Target, output: PathBuf) -> Self {
        Self {
            target,
            output,
            assets: AssetBundle::new(mode),
            adts: Default::default(),
            functions: Default::default(),
        }
    }

    pub fn link(mut self) {
        for x in self.target.functions.keys().cloned().collect::<Vec<_>>() {
            if self.functions.contains_key(&x) {
                continue;
            }
            if self.target.functions.get(&x).expect(&x).exported {
                let id = self.touch_fn(&x).id;
                self.assets.set_primary(id);
            }
        }
        if let Some(main) = self.target.main.take() {
            let mut graph = Optimizer {
                graph: main,
                decl: Default::default(),
            }.lower(&mut self).0;
            crate::node::layout::layout(&mut graph);
            let id = MainNodeGraph::new(graph).apply(&mut self.assets);
            self.assets.set_primary(id);
        }
        if self.assets.primary.is_empty() {
            eprintln!("No primary assets, will not be able to import");
        }
        self.assets.save(&self.output).expect("Failed to save assets");
    }
}

// lower
impl Linker {
    pub fn touch_fn(&mut self, key: &str) -> &CompiledFn {
        if let Some(r) = self.functions.get(key) {
            return r;
        }
        let f = self.target.functions.remove(key).expect(key);
        let (mut graph, decl) = Optimizer {
            graph: f.graph,
            decl: f.decl,
        }.lower(self);
        crate::node::layout::layout(&mut graph);
        let (id, node) = CompositeNodeGraph::new(graph).tap_mut(|graph| graph.description = f.description).apply(&mut self.assets);
        self.functions.entry(key.to_string()).or_insert(CompiledFn { id, node, decl })
    }
}
