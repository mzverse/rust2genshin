use crate::compile::func::{CompilingFn, FnDecl, NodeGraphIr, node_ir_native};
use crate::compile::helper::Helper;
use crate::compile::ir::{node_ir_assemble, FnInfo, IrNodeId, Optimizer};
use crate::compile::link::Target;
use crate::compile::place::{CompiledLocal, CompilingLocals, LocalKind, LocalRef};
use crate::node::control::NODE_IF;
use crate::node::{ExportDecl, Link, NodeGraph, NodeGraphKind, NodeKind, NodeRef};
use proc_macro2::{Ident, TokenStream};
use rustc_attr_ir::{Attribute, AttributeKind};
use rustc_codegen_ssa::{CompiledModule, ModuleKind};
use rustc_hir::def::DefKind;
use rustc_index::IndexVec;
use rustc_middle::middle::exported_symbols::ExportedSymbol;
use rustc_middle::mir;
use rustc_middle::mir::{BasicBlock, Body, Local, RETURN_PLACE};
use rustc_middle::query::QueryKey;
use rustc_middle::ty::inherent::SliceLike;
use rustc_middle::ty::{Instance, Mutability, Ty, TyCtxt, TyKind, TypingEnv};
use rustc_span::def_id::{CrateNum, LOCAL_CRATE};
use rustc_span::{DUMMY_SP, ErrorGuaranteed, ExpnKind, MacroKind, Span};
use rustc_structures::CrateType;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use syn::{LitInt, Meta, MetaList};

pub mod func;
pub mod native;
pub mod ir;
pub mod place;
pub mod ty;
pub mod helper;
pub mod backend;
pub mod link;

pub type Result<T> = core::result::Result<T, ErrorGuaranteed>;

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

pub fn compile(tcx: TyCtxt<'_>) -> Result<Vec<CompiledModule>> {
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
            return Ok(vec![]);
        }
        let create_name = tcx.crate_name(LOCAL_CRATE).to_string();
        tcx.dcx().warn(format!(
            "rust2genshin: crate `{}` is not `#![no_std]`; \
             the target project has no memory layout and cannot support std/core \
             assembly-level features",
            create_name
        ));
    }
    let mut compiler = Compiler::new(tcx)?;
    compiler.run()?;
    Ok(vec![(CompiledModule {
        name: tcx.crate_name(LOCAL_CRATE).to_string(),
        kind: ModuleKind::Regular,
        object: compiler.save().expect("saving").into(),
        global_asm_object: None,
        dwarf_object: None,
        bytecode: None,
        assembly: None,
        llvm_ir: None,
    })])
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Block {
    pub begin: Link,
    pub end: Link,
}
impl Block {
    pub fn singleton(node: NodeRef, out: usize) -> Self {
        Self {
            begin: Link::node(node, 0),
            end: Link::node(node, out),
        }
    }
    pub fn nop(graph: &mut NodeGraphIr) -> Self {
        let node = graph.insert(node_ir_native(NODE_IF.clone()));
        graph.set_default(Link::node(node, 0), Some(true.into()));
        Self::singleton(node, 0)
    }
    pub fn extend(&mut self, graph: &mut NodeGraphIr, other: Block) {
        graph.link_control(self.end, other.begin);
        *self = Self {
            begin: self.begin,
            end: other.end,
        };
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
        self.helper().err(format!("Lib fn not found: {name}"))
    }
}

const LIB_NAME: &str = "rust2genshin_lib";


pub struct Compiler<'tcx> {
    tcx: TyCtxt<'tcx>,
    lib: CrateNum,
    target: Target,
}
impl<'tcx> Compiler<'tcx> {
    pub fn helper(&self) -> Helper<'tcx> {
        Helper(self.tcx)
    }

    fn new(tcx: TyCtxt<'tcx>) -> Result<Self> {
        let mut lib = None;
        for x in tcx.crates(()).iter().copied().chain(vec![LOCAL_CRATE]) {
            if tcx.crate_name(x).as_str() == LIB_NAME {
                lib = Some(x);
                break;
            }
        };
        let Some(lib) = lib else {
            return Err(tcx.dcx().err(format!("Must depend {LIB_NAME}")));
        };
        Ok(Self {
            tcx, lib,
            target: Default::default(),
        })
    }
    fn save(&self) -> std::result::Result<PathBuf, Box<dyn std::error::Error>> {
        let path = self.tcx.output_filenames(()).with_extension(concat!("o", "gia"));
        let mut writer = BufWriter::new(File::create(&path)?);
        bincode::serde::encode_into_std_write(&self.target, &mut writer, bincode::config::standard())?;
        Ok(path)
    }

    fn run(&mut self) -> Result<()> {
        let mut main = NodeGraphIr::new(NodeGraphKind::ServerEntity, self.tcx.crate_name(LOCAL_CRATE).to_string());
        let instance = self.tcx.exported_generic_symbols(LOCAL_CRATE).iter().chain(self.tcx.exported_non_generic_symbols(LOCAL_CRATE)).map(|(x, _)| match x {
            ExportedSymbol::NonGeneric(d) => Instance::mono(self.tcx, *d),
            ExportedSymbol::Generic(d, a) => Instance::try_resolve(self.tcx, TypingEnv::fully_monomorphized(), *d, a).unwrap().unwrap(),
            _ => todo!("{x:?}"),
        }).collect::<Vec<_>>();
        for func in instance {
            let sig = self.helper().fn_sig(func);
            let params = self.helper().fn_params(func, sig).iter().map(|x| self.compile_ty(DUMMY_SP, *x)).collect::<Result<Vec<_>>>()?;
            if self.compile_native_call(func.default_span(self.tcx), func, sig, &params)?.is_some() {
                continue;
            }
            _ = self.touch_fn(func)?;
            if let Some(attr) = get_expn_macro_attr(self.tcx, func.default_span(self.tcx)) {
                #[allow(clippy::single_match)]
                match attr.meta.path().get_ident().map(Ident::to_string).unwrap_or_default().as_str() {
                    "event_listener" => {
                        let event = self.helper().monomorphize(func, self.tcx.instance_mir(func.def).local_decls.get(Local::arg(0)).unwrap().ty);
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
                                                let id = match syn::parse2::<LitInt>(tokens).and_then(|id| id.base10_parse::<i64>()) {
                                                    Ok(id) => id,
                                                    Err(e) => return self.helper().span_err(expn, e.to_string()),
                                                };
                                                let node = main.insert(node_ir_native(NodeKind::trigger(id, d.non_enum_variant().fields.iter().map(|f| Ok(self.compile_ty(f.did.default_span(self.tcx), self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), f.ty(self.tcx, a)))?.into_native().unwrap())).collect::<Result<Vec<_>>>()?)));
                                                let event = self.compile_ty(DUMMY_SP, event)?;
                                                let na = node_ir_assemble(&mut self.target, &event);
                                                let com = main.insert(NodeKind::new(
                                                    IrNodeId::Fn(self.touch_fn(func)?),
                                                    1, 1,
                                                    vec![event.into()],
                                                    vec![self.compile_ty(DUMMY_SP, self.tcx.types.unit)?],
                                                ));
                                                let fields = na.values_in_types.len();
                                                let na = main.insert(na);
                                                for i in 0..fields {
                                                    main.link_value(Link::node(node, i), Link::node(na, i));
                                                }
                                                main.link_value(Link::node(na, 0), Link::node(com, 0));
                                                main.link_control(Link::node(node, 0), Link::node(com, 0));
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
        if !main.is_empty() {
            self.target.main = main.into();
        }
        Ok(())
    }

    pub fn touch_fn(&mut self, func: Instance<'tcx>) -> Result<String> {
        let mangle = self.mangle_func(func);
        if !self.tcx.is_mir_available(func.def_id()) || self.target.functions.contains_key(&mangle) {
            return Ok(mangle);
        }
        // self.tcx.dcx().span_note(func.default_span(self.tcx), format!("Compiling fn: {mangle}"));
        let result = self.compile_fn(func)?;
        self.target.functions.insert(mangle.clone(), result);
        Ok(mangle)
    }

    fn compile_fn(&mut self, func: Instance<'tcx>) -> Result<FnInfo> {
        let tcx = self.tcx;
        let is_closure = matches!(tcx.def_kind(func.def_id()), DefKind::Closure);
        let mut graph = NodeGraph::new(NodeGraphKind::ServerEntity, self.mangle_func(func));
        let body = tcx.instance_mir(func.def);
        // graph.description = self.tcx.sess.source_map().span_to_snippet(body.span).unwrap();
        let mut locals = IndexVec::<Local, CompiledLocal<LocalRef>>::new(); // TODO: adapt for struct, struct list and map
        let args = tcx.fn_arg_idents(func.def_id()).iter().enumerate().map(|(i, x)| x.as_ref().map(rustc_span::Ident::to_string).unwrap_or_else(|| format!("arg{i}"))).collect::<Vec<_>>();
        let mut compiling_locals = CompilingLocals {
            compiler: self,
            block: Block::nop(&mut graph),
            graph: &mut graph,
            params: 0,
            rets: 0,
        };
        let Some(ret_decl) = body.local_decls.get(RETURN_PLACE) else {
            unreachable!();
        };
        let (ret, r) = compiling_locals.solve_local(if ret_decl.ty.is_never() { compiling_locals.compiler.tcx.types.unit } else { compiling_locals.compiler.helper().monomorphize(func, ret_decl.ty) }, LocalKind::Ret, "".into(), ret_decl.source_info.span)?;
        locals.push(r);
        let mut params = vec![];
        if is_closure {
            let ty = func.instantiate_mir_and_normalize_erasing_regions(tcx, TypingEnv::fully_monomorphized(), tcx.type_of(func.def_id()));
            match ty.kind() {
                TyKind::Closure(..) => {
                    let arg0 = body.local_decls.get(Local::arg(0)).unwrap();
                    let (k, r) = compiling_locals.solve_local(compiling_locals.compiler.helper().monomorphize(func, arg0.ty), LocalKind::Arg, "#closure".to_string(), arg0.source_info.span)?;
                    locals.push(r);
                    params.push(k);
                    let mut p1 = vec![];
                    for (i, name) in args.iter().enumerate() {
                        let decl = body.local_decls.get(Local::arg(1 + i)).unwrap();
                        let (k, r) = compiling_locals.solve_local(compiling_locals.compiler.helper().monomorphize(func, decl.ty), LocalKind::Arg, name.clone(), decl.source_info.span)?;
                        p1.push(k);
                        locals.push(r);
                    }
                    params.push(CompiledLocal::Flat(p1));
                },
                TyKind::Coroutine(_d, a) => {
                    let (k, r) = compiling_locals.solve_local(Ty::new_pinned_ref(tcx, tcx.lifetimes.re_erased, ty, Mutability::Mut), LocalKind::Arg, "self".to_string(), DUMMY_SP)?;
                    locals.push(r);
                    params.push(k);
                    let (k, r) = compiling_locals.solve_local(a.as_coroutine().resume_ty(), LocalKind::Arg, "arg".to_string(), DUMMY_SP)?;
                    locals.push(r);
                    params.push(k);
                },
                _ => panic!(),
            }
        } else {
            for (i, x) in args.iter().enumerate() {
                let decl = body.local_decls.get(Local::arg(i)).unwrap();
                let (k, r) = compiling_locals.solve_local(compiling_locals.compiler.helper().monomorphize(func, decl.ty), LocalKind::Arg, x.clone(), decl.source_info.span)?;
                locals.push(r);
                params.push(k);
            }
        }
        let fn_decl = FnDecl {
            control: true,
            params,
            ret,
            proxies_in: (0..compiling_locals.params).collect(),
            proxies_out: vec![None; compiling_locals.rets],
        };
        for x in body.local_decls.iter().skip(locals.len()) { // other locals
            locals.push(compiling_locals.solve_local(compiling_locals.compiler.helper().monomorphize(func, x.ty), LocalKind::Other, "".to_string(), x.source_info.span)?.1);
        }
        let CompilingLocals { mut block, .. } = compiling_locals;
        graph.push_export_control_in(ExportDecl::new("".into(), None));
        graph.push_export_control_out(ExportDecl::new("".into(), None));
        let mut blocks = IndexVec::<BasicBlock, Block>::new();
        graph.link_control(Link::export(0), block.begin);
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
            graph.link_control(blocks.get(k).unwrap().end, result?);
        }
        block.extend(&mut graph, blocks.get(mir::START_BLOCK).unwrap().clone());
        let optimizer = Optimizer {
            graph,
            decl: fn_decl,
        };
        // optimizer.optimize();
        optimizer.verify(self.helper())?;
        Ok(FnInfo {
            description: "".to_string(), // TODO
            graph: optimizer.graph,
            decl: optimizer.decl,
            exported: self.tcx.codegen_fn_attrs(func.def_id()).contains_extern_indicator(),
        })
    }
}
