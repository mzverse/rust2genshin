use crate::asset::{AssetBundle, GameMode, Identifier};
use crate::compile::func::{FnDecl, NodeGraphIr};
use crate::compile::ir::{AdtInfo, FnInfo, IrKind, IrNodeId, Optimizer};
use crate::node::composite::CompositeNodeGraph;
use crate::node::{MainNodeGraph, NodeGraphStatic, NodeKind};
use crate::structure::StructRef;
use panic_context::panic_context;
use std::collections::{HashMap, HashSet};
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
        // 主图要声明它用到的全部节点图变量:自己直接用的,以及它(间接)
        // 调用的那些函数用的。
        //
        // 收集得排在最前面:一是 lower 会把函数引用换成复合节点调用,图里就
        // 再也看不出调用关系;二是 `touch_fn` 会把函数从 `target.functions`
        // 里取走,之后就查不到它声明了哪些变量了。
        let mut variables = HashMap::new();
        if let Some(main) = &self.target.main {
            let mut visited = HashSet::new();
            if let Err(e) = self.collect_variables(main, &mut variables, &mut visited) {
                panic!("节点图变量合并失败: {e}");
            }
        }
        // 哪些函数是**导出的**(`#[unsafe(no_mangle)]` 之类),要单独记下来。
        //
        // 不能在这一轮里现查:`touch_fn` 会把函数从 `target.functions` 里**取走**
        // (`remove`),而一个导出函数很可能先作为别的函数的依赖被 touch 到 ——
        // 那时它就已经不在表里了,`exported` 字段也一起没了。于是它永远不会被
        // 标成 primary,产物里就少一条 primary 资产。
        //
        // 记下 key 而不是记 `exported` 本身:key 在 `self.functions` 里,
        // 那个 map 直到最后都在。
        for x in exported_functions(&self.target) {
            // `touch_fn` 对已经 touch 过的会直接返回缓存 —— 所以先被当成依赖
            // touch 到的导出函数,这里照样能拿到它的 id 并标成 primary。
            let id = self.touch_fn(&x).id;
            self.assets.set_primary(id);
        }
        if let Some(main) = self.target.main.take() {
            let statics: Vec<NodeGraphStatic> = variables.into_iter().map(|(name, kind)| {
                let kind = kind.lower(&mut self);
                NodeGraphStatic::new(name, kind, None)
            }).collect();
            let mut graph = Optimizer {
                graph: main,
                decl: Default::default(),
            }.lower(&mut self).0;
            crate::node::layout::layout(&mut graph);
            let mut main_graph = MainNodeGraph::new(graph);
            main_graph.statics = statics;
            let id = main_graph.apply(&mut self.assets);
            self.assets.set_primary(id);
        }
        if self.assets.primary.is_empty() {
            eprintln!("No primary assets, will not be able to import");
        }
        self.assets.save(&self.output).expect("Failed to save assets");
    }

    /// 把 `graph` 里(间接)用到的节点图变量并进 `out`。
    ///
    /// 同名变量指的是同一个变量,类型必须一致 —— 不一致说明两个函数对同名
    /// 变量的用法对不上,主图的变量表没法同时满足它们。
    ///
    /// 变量直接从**图里现扫** `GetGraphVariable` / `SetGraphVariable` 节点,
    /// 不看 `FnDecl::variables` —— 那个字段要等 `lower_ir` 才填,而这里必须
    /// 在任何函数 lower 之前跑完(见 `link` 里的注释)。
    fn collect_variables(
        &self,
        graph: &NodeGraphIr,
        out: &mut HashMap<String, IrKind>,
        visited: &mut HashSet<String>,
    ) -> Result<(), String> {
        // 图自己直接用的变量。
        for node in graph.get_nodes() {
            let kind = &graph.get_node(node).kind;
            match &kind.id {
                IrNodeId::GetGraphVariable(name) => {
                    if let Some(k) = kind.values_out_types.first() {
                        merge_variable(out, name, k)?;
                    }
                }
                IrNodeId::SetGraphVariable(name) => {
                    if let Some(Some(k)) = kind.values_in_types.get(1) {
                        merge_variable(out, name, k)?;
                    }
                }
                _ => (),
            }
        }
        // 再用到它调用的函数。
        for node in graph.get_nodes() {
            let IrNodeId::Fn(key) = &graph.get_node(node).kind.id else {
                continue;
            };
            if !visited.insert(key.clone()) {
                continue;
            }
            let Some(f) = self.target.functions.get(key) else {
                // 已经 lower 掉的函数(它的图进了 `self.functions`),它的变量
                // 在被调用时就已经并进来了。
                continue;
            };
            for (name, kind) in &f.decl.variables {
                merge_variable(out, name, kind)?;
            }
            self.collect_variables(&f.graph, out, visited)?;
        }
        Ok(())
    }
}

/// 导出函数(要单独成 primary 资产)的那批 key。
///
/// 必须在**任何 `touch_fn` 之前**取好 —— `touch_fn` 会把函数从
/// `target.functions` 里 `remove` 掉,一个导出函数如果先作为别的函数的
/// 依赖被 touch 到,它的 `exported` 字段就跟着函数一起没了。事后查表会
/// 漏掉它,产物里就少一条 primary 资产。
fn exported_functions(target: &Target) -> Vec<String> {
    let mut out: Vec<String> = target
        .functions
        .iter()
        .filter(|(_, f)| f.exported)
        .map(|(k, _)| k.clone())
        .collect();
    // `HashMap` 迭代顺序不稳 —— 排一下,让产物可复现。
    out.sort();
    out
}

/// 把一个节点图变量并进变量表。
///
/// 同名变量指的是同一个变量,类型必须一致 —— 不一致说明两个函数对同名
/// 变量的用法对不上,主图的变量表没法同时满足它们。
fn merge_variable(out: &mut HashMap<String, IrKind>, name: &str, kind: &IrKind) -> Result<(), String> {
    match out.get(name) {
        Some(old) if old != kind => {
            Err(format!("变量 {name} 的类型不一致: {old} 与 {kind}"))
        },
        _ => {
            out.insert(name.to_string(), kind.clone());
            Ok(())
        },
    }
}

// lower
impl Linker {
    pub fn touch_fn(&mut self, key: &str) -> &CompiledFn {
        if let Some(r) = self.functions.get(key) {
            return r;
        }
        panic_context!("Linking: {key}");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::NativeKind;

    #[test]
    fn 同名同类型的变量合并成一条() {
        let mut out = HashMap::new();
        merge_variable(&mut out, "x", &IrKind::Native(NativeKind::Int)).unwrap();
        merge_variable(&mut out, "x", &IrKind::Native(NativeKind::Int)).unwrap();
        merge_variable(&mut out, "y", &IrKind::Native(NativeKind::Float)).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out["x"], IrKind::Native(NativeKind::Int));
    }

    #[test]
    fn 同名不同类型的变量报错() {
        let mut out = HashMap::new();
        merge_variable(&mut out, "x", &IrKind::Native(NativeKind::Int)).unwrap();
        let e = merge_variable(&mut out, "x", &IrKind::Native(NativeKind::Float)).unwrap_err();
        assert!(e.contains("x"), "{e}");
    }

    /// 导出函数的清单要在**任何 touch 之前**取好。
    ///
    /// `touch_fn` 会把函数从 `target.functions` 里 `remove` 掉 —— 一个导出
    /// 函数如果先作为别的函数的依赖被 touch 到,事后再查表就找不到它了,
    /// 它永远不会被标成 primary。
    #[test]
    fn 导出函数的清单与函数是否已被取走无关() {
        let fn_info = |exported: bool| FnInfo {
            description: String::new(),
            graph: crate::node::NodeGraph::new(
                crate::node::NodeGraphKind::ServerEntity,
                "test",
            ),
            decl: Default::default(),
            exported,
        };
        let mut target = Target::default();
        target.functions.insert("exported".into(), fn_info(true));
        target.functions.insert("plain".into(), fn_info(false));

        // 清单在**任何 touch 之前**取好。
        let before = exported_functions(&target);
        assert_eq!(before, vec!["exported".to_string()]);

        // `touch_fn` 把导出函数取走了 —— 表里没有了,事后再查就查不到。
        target.functions.remove("exported");
        assert!(exported_functions(&target).is_empty());
        // 但清单已经拿到了 —— `link` 拿它去标 primary,不受影响。
        assert_eq!(before, vec!["exported".to_string()]);
    }
}
