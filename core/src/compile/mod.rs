use crate::asset::AssetBundle;
use crate::asset::node_graph::control::NODE_IF;
use crate::asset::node_graph::query::node_local;
use crate::asset::node_graph::{Connection, Node, NodeGraph, NodeGraphClass, NodeGraphComposite, NodeGraphExtra, NodeGraphStatic, NodeRef};
use crate::asset::value::{AnyValue, ValueBool, ValueDefault, ValueFloat, ValueInt, ValueString};
use proc_macro2::TokenStream;
use rustc_attr_ir::{Attribute, AttributeKind};
use rustc_hir as hir;
use rustc_hir::intravisit::Visitor;
use rustc_hir::{ImplItem, intravisit};
use rustc_index::IndexVec;
use rustc_middle::hir::nested_filter;
use rustc_middle::mir::{BasicBlock, Body, Local};
use rustc_middle::query::QueryKey;
use rustc_middle::ty::{FloatTy, Instance, IntTy, Ty, TyCtxt, TyKind};
use rustc_middle::{mir, ty};
use rustc_span::def_id::{CrateNum, LOCAL_CRATE, LocalDefId};
use rustc_span::{ErrorGuaranteed, ExpnKind, Ident, MacroKind, Span};
use rustc_structures::CrateType;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use rustc_span::sym::panic;

mod func;
mod native;
mod optimize;

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
    pub fn nop(graph: &mut NodeGraph<impl NodeGraphExtra>) -> Self {
        let node = graph.insert(Node::new(NODE_IF.clone()));
        graph.set_default(Connection(node, 0), ValueBool(true).into());
        Self::singleton(node, 0)
    }
    pub fn extend(&mut self, graph: &mut NodeGraph<impl NodeGraphExtra>, other: Block) {
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
}

pub(crate) struct CompilingFn<'tcx, 'a> {
    pub tcx: TyCtxt<'tcx>,
    pub compiler: &'a mut Compiler<'tcx>,
    pub graph: &'a mut NodeGraph<NodeGraphComposite>,
    pub body: &'a Body<'tcx>,
    pub locals: &'a IndexVec<Local, NodeRef>,
}
impl<'tcx> WithTcx<'tcx> for CompilingFn<'tcx, '_> {
    fn get_tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }
}

pub(crate) fn is_unit(ty: Ty) -> bool {
    if ty.is_never() {
        return true;
    }
    if let TyKind::Tuple(cs) = ty.kind() {
        cs.len() == 0
    } else {
        false
    }
}

pub(crate) fn compile_ty<'tcx>(tcx: &impl WithTcx<'tcx>, span: Span, ty: Ty) -> Result<AnyValue> {
    Ok(match ty.kind() {
        TyKind::Bool => ValueBool::def(),
        TyKind::Char => return tcx.span_err(span, "Char is unsupported"),
        TyKind::Int(ty) => match ty {
            IntTy::I8 | IntTy::I16 | IntTy::I64 | IntTy::I128 =>
                return tcx.span_err(span, format!("Unsupported int: {}", ty.name())),
            IntTy::Isize | IntTy::I32 => ValueInt::def(),
        },
        TyKind::Uint(_) => todo!(),
        TyKind::Float(ty) => match ty {
            FloatTy::F16 |
            FloatTy::F64 |
            FloatTy::F128 =>
                return tcx.span_err(span, format!("Unsupported float: {}", ty.name())),
            FloatTy::F32 => ValueFloat::def(),
        },
        TyKind::Str => ValueString::def(),
        TyKind::Ref(_, e, _) => if e.is_str() { ValueString::def() } else {
            todo!()
        },
        TyKind::Adt(d, a) => {
            return tcx.span_err(span, format!("Adt: {d:?} = {a:?}"))
        },
        TyKind::Foreign(_) => todo!(),
        TyKind::Array(_, _) => todo!(),
        TyKind::Pat(_, _) => todo!(),
        TyKind::Slice(_) => todo!(),
        TyKind::RawPtr(_, _) => todo!(),
        TyKind::FnDef(_, _) => todo!(),
        TyKind::FnPtr(_, _) => todo!(),
        TyKind::Tuple(tys) => todo!("Todo tuple: {:?}", tys),
        TyKind::Closure(_, _) => todo!(),
        TyKind::Alias(_, _) => todo!(),
        TyKind::Dynamic(_, _)
        | TyKind::CoroutineClosure(_, _)
        | TyKind::Coroutine(_, _)
        | TyKind::CoroutineWitness(_, _)
        | TyKind::Never
        | TyKind::Param(_)
        | TyKind::Bound(_, _)
        | TyKind::Placeholder(_)
        | TyKind::Infer(_)
        | TyKind::Error(_)
        | TyKind::UnsafeBinder(_) => {
            tcx.span_err::<()>(span, format!("Unsupported type: {:?}", ty.kind()));
            panic!();
        }
    })
}

const LIB_NAME: &str = "rust2genshin_lib";
pub(crate) struct Compiler<'tcx> {
    tcx: TyCtxt<'tcx>,
    lib: CrateNum,
    assets: AssetBundle,
    compiling: HashSet<Instance<'tcx>>,
    compiled: HashMap<Instance<'tcx>, i64>,
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
        })
    }
    fn save(&self, out_dir: &Path) {
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
        for x in c.out {
            self.touch_fn(Instance::mono(self.tcx, x.to_def_id()))?;
            if let Some(_it) = get_expn_macro_attr(self.tcx, x.default_span(self.tcx)) {

                // TODO: manage entrypoint (event_handler)
            }
        }
        let main = NodeGraph::<Vec<NodeGraphStatic>>::new(NodeGraphClass::Entity, self.tcx.crate_name(LOCAL_CRATE).to_string(), Default::default());
        // for (i, _) in &self.assets.assets {
        //     main.insert(
        //         NodeComposite {
        //             id: AssetBundle::ID_BEGIN + i as i64,
        //             controls_in: 0,
        //             controls_out: vec![],
        //             values_in: vec![],
        //             values_out: vec![],
        //         }
        //         .into(),
        //     );
        // }
        // TODO
        if !main.is_empty() {
            let main_id = self.assets.insert(main.into());
            self.assets.display.push(main_id);
        }
        if self.assets.display.is_empty() {
            self.tcx.dcx().warn("No primary assets, may be not able to import");
        }
        Ok(())
    }

    pub(crate) fn touch_fn(&mut self, func: Instance<'tcx>) -> Result<i64> {
        if let Some(asset_id) = self.compiled.get(&func) {
            return Ok(*asset_id);
        }
        if !self.compiling.insert(func) {
            return self.span_err(func.default_span(self.tcx), "Recursive call");
        }
        let asset_id = self.compile_fn(func)?;
        self.compiling.remove(&func);
        self.compiled.insert(func, asset_id);
        Ok(asset_id)
    }

    fn monomorphize(&self, body: &Body<'tcx>, func: Instance<'tcx>, ty: Ty<'tcx>) -> Ty<'tcx> {
        func.instantiate_mir_and_normalize_erasing_regions(self.tcx, body.typing_env(self.tcx), ty::EarlyBinder::bind(self.tcx, ty))
    }

    fn compile_fn(&mut self, func: Instance<'tcx>) -> Result<i64> {
        self.tcx.dcx().span_note(func.default_span(self.tcx), format!("Compiling fn: {:?}", func));
        // let name = self.tcx.def_path_str(id);
        let mut graph = NodeGraph::new(NodeGraphClass::Entity, self.tcx.symbol_name(func).to_string(), NodeGraphComposite::new());
        let body = self.tcx.instance_mir(func.def);
        graph.extra.description = self.tcx.sess.source_map().span_to_snippet(body.span).unwrap();
        let mut locals = IndexVec::<Local, NodeRef>::new(); // TODO: adapt for struct, struct list and map
        for x in &body.local_decls {
            if is_unit(x.ty) {
                locals.push(NodeRef::from(usize::MAX));
                continue;
            }
            let kind = compile_ty(self, x.source_info.span, self.monomorphize(body, func, x.ty))?;
            let local = graph.insert(Node::new(node_local(kind.clone())));
            graph.set_default(Connection(local, 0), kind);
            locals.push(local);
        }
        let mut blocks = IndexVec::<BasicBlock, Block>::new();
        for (k, result) in {
            let mut compiling = CompilingFn {
                tcx: self.tcx,
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
            graph.connect_control(blocks.get(k).unwrap().end, result?);
        }
        graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::InControl).unwrap().push("".into());
        graph.export_control_in(blocks.get(mir::START_BLOCK).unwrap().begin, 0);
        graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::OutControl).unwrap().push("".into());
        for (i, param) in self.tcx.fn_arg_idents(func.def_id()).iter().enumerate() {
            graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::InValue).unwrap().push(param.as_ref().map(Ident::to_string).unwrap_or_else(|| format!("arg{}", i).to_string()));
            let node = *locals.get(Local::arg(i)).unwrap();
            graph.export_value_in(Connection(node, 0), i);
        }
        if !is_unit(body.return_ty()) {
            graph.extra.pins.get_mut(&crate::asset::generated::pin_signature::Kind::OutValue).unwrap().push("".into());
            let node = *locals.get(mir::RETURN_PLACE).unwrap();
            graph.export_value_out(Connection(node, 1), 0);
        }
        let asset_id = self.assets.insert(graph.into());
        if self.tcx.codegen_fn_attrs(func.def_id()).contains_extern_indicator() {
            self.assets.display.push(asset_id + NodeGraphComposite::DECL_OFFSET);
        }
        Ok(asset_id)
    }

    #[allow(dead_code)]
    fn touch_struct(&mut self) {
        todo!()
    }
}
