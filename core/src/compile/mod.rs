use crate::asset::node_graph::control::NODE_IF;
use crate::asset::node_graph::{CompositeNodeGraph, Connection, MainNodeGraph, Node, NodeGraph, NodeGraphKind, NodeKind, NodeRef, ValueIn};
use crate::asset::structure::StructureDefinition;
use crate::asset::value::{ValueBool, ValueDefault, ValueGuid};
use crate::asset::{Asset, AssetBundle, AssetRef};
use crate::compile::func::{CompilingFn, FnDecl};
use crate::compile::optimize::Optimizer;
use crate::compile::place::{CompiledLocal, CompilingLocals, LocalKind, LocalRef};
use proc_macro2::{Ident, TokenStream};
use rustc_attr_ir::{Attribute, AttributeKind};
use rustc_hir as hir;
use rustc_hir::intravisit::Visitor;
use rustc_hir::{ImplItem, intravisit};
use rustc_index::{Idx, IndexVec};
use rustc_middle::hir::nested_filter;
use rustc_middle::middle::exported_symbols::ExportedSymbol;
use rustc_middle::mir;
use rustc_middle::mir::{BasicBlock, Body, Local, RETURN_PLACE};
use rustc_middle::query::QueryKey;
use rustc_middle::ty::inherent::SliceLike;
use rustc_middle::ty::{EarlyBinder, Instance, Ty, TyCtxt, TyKind, TypingEnv};
use rustc_span::def_id::{CrateNum, LOCAL_CRATE, LocalDefId};
use rustc_span::{ErrorGuaranteed, ExpnKind, MacroKind, Span};
use rustc_structures::CrateType;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use syn::{LitInt, Meta, MetaList};
use crate::asset::node_graph::composite::node_composite;

pub mod func;
pub mod native;
pub mod optimize;
pub mod place;
pub mod ty;

pub type Result<T> = core::result::Result<T, ErrorGuaranteed>;

pub fn resolved_out_dir() -> PathBuf {
    if let Ok(td) = env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(td);
    }
    PathBuf::from("target")
}

pub fn get_expn_macro_attr(tcx: TyCtxt, span: Span) -> Option<syn::Attribute> {
    let ex = span.ctxt().outer_expn().expn_data();
    if matches!(ex.kind, ExpnKind::Macro(MacroKind::Attr, _)) {
        let tokens = syn::parse_str::<TokenStream>(
            &tcx.sess
                .source_map()
                .span_to_snippet(ex.call_site.data().span())
                .unwrap()
        ).unwrap();
        use syn::parse::Parser;
        let result = syn::Attribute::parse_outer.parse2(tokens).unwrap();
        assert_eq!(result.len(), 1);
        Some(result.into_iter().next().unwrap())
    } else {
        None
    }
}

pub fn compile(tcx: TyCtxt<'_>) -> Result<()> {
    if !tcx
        .hir_krate_attrs()
        .iter()
        .any(|attr| matches!(attr, Attribute::Parsed(AttributeKind::NoStd)))
    {
        let is_proc_macro = tcx
            .crate_types()
            .iter()
            .any(|t| matches!(t, CrateType::ProcMacro));
        if is_proc_macro || tcx.sess.opts.test {
            return Ok(());
        }
        let create_name = tcx.crate_name(LOCAL_CRATE).to_string();
        tcx.dcx().warn(format!(
            "rust2genshin: crate `{}` is not `#![no_std]`; \
             the target project has no memory layout and cannot support std/core \
             assembly-level features",
            create_name
        ));
    }
    let out_dir = resolved_out_dir();
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        return Err(tcx.dcx().err(format!(
            "rust2genshin: cannot create {}: {e}",
            out_dir.display()
        )));
    }
    let mut compiler = Compiler::new(tcx)?;
    compiler.run()?;
    compiler.save(&out_dir);
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Block {
    pub(crate) begin: Connection,
    pub(crate) end: Connection,
}
impl Block {
    pub fn singleton(node: NodeRef, out: usize) -> Self {
        Self {
            begin: Connection(node, 0),
            end: Connection(node, out),
        }
    }
    pub fn nop(graph: &mut NodeGraph) -> Self {
        let node = graph.insert(Node::new(NODE_IF.clone()));
        graph.set_default(Connection(node, 0), ValueBool(true).into());
        Self::singleton(node, 0)
    }
    pub fn extend(&mut self, graph: &mut NodeGraph, other: Block) {
        graph.connect_control(self.end, other.begin);
        *self = Self {
            begin: self.begin,
            end: other.end,
        };
    }
}

pub(crate) trait WithTcx<'tcx> {
    fn get_tcx(&self) -> TyCtxt<'tcx>;

    #[allow(dead_code)]
    fn err<T>(&self, msg: impl Into<rustc_errors::DiagMessage>) -> Result<T> {
        Err(self.get_tcx().dcx().err(msg.into()))
    }

    fn span_err<T>(&self, span: Span, msg: impl Into<rustc_errors::DiagMessage>) -> Result<T> {
        Err(self.get_tcx().dcx().span_err(span, msg.into()))
    }

    fn monomorphize(&self, func: Instance<'tcx>, ty: Ty<'tcx>) -> Ty<'tcx> {
        func.instantiate_mir_and_normalize_erasing_regions(self.get_tcx(), TypingEnv::fully_monomorphized(), EarlyBinder::bind(self.get_tcx(), ty))
    }
}

impl<'tcx> Compiler<'tcx> {
    fn find_lib_fn(&self, name: &str) -> Result<Instance<'tcx>> {
        for (s, _) in self.tcx.exported_non_generic_symbols(self.lib) {
            if let ExportedSymbol::NonGeneric(id) = *s {
                self.tcx.dcx().note(self.tcx.def_path_str(id));
                if self.tcx.def_path_str(id) == name {
                    return Ok(Instance::mono(self.tcx, id));
                }
            } else {
                panic!();
            }
        }
        self.err(format!("Lib fn not found: {name}"))
    }
}

const LIB_NAME: &str = "rust2genshin_lib";

pub struct Compiler<'tcx> {
    tcx: TyCtxt<'tcx>,
    lib: CrateNum,
    assets: AssetBundle,
    compiling: HashSet<Instance<'tcx>>,
    compiled: HashMap<Instance<'tcx>, (AssetRef<CompositeNodeGraph>, FnDecl)>,
    structs: HashMap<String, AssetRef<StructureDefinition>>,
}
impl<'tcx> WithTcx<'tcx> for Compiler<'tcx> {
    fn get_tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }
}
impl<'tcx> Compiler<'tcx> {

    fn new(tcx: TyCtxt<'tcx>) -> Result<Self> {
        let mut lib = None;
        for x in tcx.crates(()) {
            if tcx.crate_name(*x).as_str() == LIB_NAME {
                lib = Some(x);
                break;
            }
        };
        let Some(&lib) = lib else {
            return Err(tcx.dcx().err(format!("Must depend {LIB_NAME}")));
        };
        Ok(Self {
            tcx, lib,
            assets: AssetBundle::new(crate::asset::GameMode::Overlimit),
            compiling: HashSet::new(),
            compiled: HashMap::new(),
            structs: HashMap::new(),
        })
    }
    fn save(self, out_dir: &Path) {
        // eprintln!("{:?}", self.tcx.output_filenames(()).with_extension("gia")); // TODO
        let path = out_dir.join(format!(
            "{}.gia",
            self.tcx.crate_name(LOCAL_CRATE)
        ));
        self.assets.save(&path).expect("encode error");
    }

    fn run(&mut self) -> Result<()> {
        struct Collector<'tcx> {
            tcx: TyCtxt<'tcx>,
            out: Vec<LocalDefId>,
        }
        impl<'tcx> Visitor<'tcx> for Collector<'tcx> {
            type NestedFilter = nested_filter::All;
            fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
                self.tcx
            }
            fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) {
                if let hir::ItemKind::Fn { .. } = &item.kind {
                    self.out.push(item.owner_id.def_id);
                }
                intravisit::walk_item(self, item);
            }
            fn visit_impl_item(&mut self, ii: &'tcx ImplItem<'tcx>) -> Self::Result {
                if let hir::ImplItemKind::Fn { .. } = &ii.kind {
                    self.out.push(ii.owner_id.def_id);
                }
                intravisit::walk_impl_item(self, ii);
            }
        }
        let mut c = Collector {
            tcx: self.tcx,
            out: Vec::new(),
        };
        self.tcx.hir_walk_toplevel_module(&mut c);
        let mut main = MainNodeGraph::new(NodeGraph::new(NodeGraphKind::ServerEntity, self.tcx.crate_name(LOCAL_CRATE).to_string()));
        for x in c.out {
            let func = Instance::mono(self.tcx, x.to_def_id());
            _ = self.touch_fn(func)?;
            if let Some(attr) = get_expn_macro_attr(self.tcx, x.default_span(self.tcx)) {
                #[allow(clippy::single_match)]
                match attr.meta.path().get_ident().map(Ident::to_string).unwrap_or_default().as_str() {
                    "event_listener" => {
                        let event = self.monomorphize(func, self.tcx.instance_mir(func.def).local_decls.get(Local::arg(0)).unwrap().ty);
                        let TyKind::Adt(d, a) = event.kind() else {
                            panic!();
                        };
                        let def = d.did().default_span(self.tcx);
                        let expn = def.ctxt().outer_expn().expn_data().call_site;
                        if let Some(attr) = get_expn_macro_attr(self.tcx, def) {
                            match attr.meta {
                                Meta::List(MetaList { path, tokens, .. }) => {
                                    if let Some(ident) = path.get_ident() {
                                        match ident.to_string().as_str() {
                                            "event" => {
                                                let id = match syn::parse2::<LitInt>(tokens) {
                                                    Ok(id) => id,
                                                    Err(e) => return self.span_err(expn, e.to_string()),
                                                };
                                                let id = match id.base10_parse::<i64>() {
                                                    Ok(x) => x,
                                                    Err(e) => return self.span_err(expn, e.to_string()),
                                                };
                                                let node = main.graph.insert(NodeKind::trigger(id, d.non_enum_variant().fields.iter().map(|f| self.compile_ty(f.did.default_span(self.tcx), self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), f.ty(self.tcx, a)))).collect::<Result<Vec<_>>>()?).into());
                                                let (r, decl) = self.touch_fn(func)?;
                                                let com = main.graph.insert(node_composite(r).into());
                                                let block = CompilingFn::compile_call0(&mut main.graph, com, decl, &(0..d.non_enum_variant().fields.len()).map(|x| ValueIn::link(Connection(node, x).into())).collect::<Vec<_>>());
                                                main.graph.connect_control(Connection(node, 0), block.begin);
                                            }
                                            _ => (),
                                        }
                                    }
                                }
                                _ => (),
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
        if !main.graph.is_empty() {
            let main = main.apply(&mut self.assets);
            self.assets.set_primary(&main);
        }
        if self.assets.primary.is_empty() {
            self.tcx.dcx().warn("No primary assets, may be not able to import");
        }
        Ok(())
    }

    pub fn touch_fn(&mut self, func: Instance<'tcx>) -> Result<&(AssetRef<CompositeNodeGraph>, FnDecl)> {
        if let Some(result) = self.compiled.get(&func) {
            return Ok(result);
        }
        if !self.compiling.insert(func) {
            return self.span_err(func.default_span(self.tcx), "Recursive call");
        }
        let result = self.compile_fn(func)?;
        self.compiling.remove(&func);
        Ok(self.compiled.entry(func).or_insert(result))
    }

    fn compile_fn(&mut self, func: Instance<'tcx>) -> Result<(AssetRef<CompositeNodeGraph>, FnDecl)> {
        // self.tcx.dcx().span_note(func.default_span(self.tcx), format!("Compiling fn: {:?}", func));
        let mut graph = CompositeNodeGraph::new(NodeGraph::new(NodeGraphKind::ServerEntity, self.tcx.symbol_name(func).to_string()));
        let body = self.tcx.instance_mir(func.def);
        graph.description = self.tcx.sess.source_map().span_to_snippet(body.span).unwrap();
        let mut locals = IndexVec::<Local, CompiledLocal<LocalRef>>::new(); // TODO: adapt for struct, struct list and map
        let args = self.tcx.fn_arg_idents(func.def_id());
        let mut compiling_locals = CompilingLocals {
            compiler: self,
            block: Block::nop(&mut graph.graph),
            graph: &mut graph,
            params: 0,
            rets: 0,
        };
        let Some(ret_decl) = body.local_decls.get(RETURN_PLACE) else {
            unreachable!();
        };
        let (ret, r) = compiling_locals.solve_local(if ret_decl.ty.is_never() { compiling_locals.compiler.tcx.types.unit } else { compiling_locals.compiler.monomorphize(func, ret_decl.ty) }, LocalKind::Ret, "".into(), ret_decl.source_info.span)?;
        locals.push(r);
        let mut params = vec![];
        for i in 0..args.len() {
            let decl = body.local_decls.get(Local::arg(i)).unwrap();
            let (k, r) = compiling_locals.solve_local(compiling_locals.compiler.monomorphize(func, decl.ty), LocalKind::Arg, args.get(i).unwrap().as_ref().map(rustc_span::Ident::to_string).unwrap_or_else(|| format!("arg{}", i.index() - 1).to_string()), decl.source_info.span)?;
            locals.push(r);
            params.push(k);
        }
        let mut fn_decl = FnDecl {
            control: true,
            params,
            ret,
            proxies_in: (0..compiling_locals.params).collect(),
            proxies_out: vec![None; compiling_locals.rets],
        };
        for x in body.local_decls.iter().skip(1 + args.len()) { // other locals
            locals.push(compiling_locals.solve_local(compiling_locals.compiler.monomorphize(func, x.ty), LocalKind::Other, "".to_string(), x.source_info.span)?.1);
        }
        let CompilingLocals { mut block, .. } = compiling_locals;
        let mut blocks = IndexVec::<BasicBlock, Block>::new();
        graph.graph.export_control_in(block.begin, 0);
        for (k, result) in {
            let mut compiling = CompilingFn {
                tcx: self.tcx,
                func,
                compiler: self,
                graph: &mut graph,
                body,
                locals: &locals,
            };
            for x in body.basic_blocks.iter() {
                blocks.push(compiling.compile_basic_block(&x.statements)?);
            }
            body.basic_blocks.iter_enumerated().map(|(k, v)| (k, compiling.compile_terminator(&blocks, v.terminator.as_ref().unwrap()))).collect::<Vec<_>>()
        } {
            graph.graph.connect_control(blocks.get(k).unwrap().end, result?);
        }
        block.extend(&mut graph.graph, blocks.get(mir::START_BLOCK).unwrap().clone());
        let mut optimizer = Optimizer {
            graph: &mut graph,
            decl: &mut fn_decl,
        };
        optimizer.lower();
        if fn_decl.control {
            graph.pins.get_mut(&crate::asset::generated::pin_signature::Kind::InControl).unwrap().push("".into());
            graph.pins.get_mut(&crate::asset::generated::pin_signature::Kind::OutControl).unwrap().push("".into());
        }
        let asset_id = graph.apply(&mut self.assets);
        if self.tcx.codegen_fn_attrs(func.def_id()).contains_extern_indicator() {
            self.assets.set_primary(&asset_id);
        }
        Ok((asset_id, fn_decl))
    }
}
