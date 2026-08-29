use crate::asset::AssetBundle;
use crate::asset::node_graph::arithmetic::{
    NodeAdd, NodeBitwiseAnd, NodeBitwiseOr, NodeBitwiseXor, NodeDivide, NodeModulo, NodeMultiply,
    NodeSubtract, NodeXor,
};
use crate::asset::node_graph::control::NodeIf;
use crate::asset::node_graph::execution::NodeSetLocal;
use crate::asset::node_graph::query::NodeLocal;
use crate::asset::node_graph::{
    Link, Node, NodeComposite, NodeGraph, NodeGraphComposite, NodeGraphMain, NodeRef, ValueIn,
};
use crate::asset::raw_node_graph::NodeGraphClass;
use crate::asset::value::{AnyValue, ValueBool, ValueDefault, ValueFloat, ValueInt, ValueString};
use proc_macro2::TokenStream;
use rustc_attr_ir::{Attribute, AttributeKind};
use rustc_hir as hir;
use rustc_hir::intravisit::Visitor;
use rustc_hir::{ImplItem, intravisit};
use rustc_index::IndexVec;
use rustc_middle::hir::nested_filter;
use rustc_middle::mir;
use rustc_middle::mir::{
    BasicBlock, BinOp, Const, Local, LocalDecl, Location, NonDivergingIntrinsic, Operand, Rvalue,
    Statement, StatementKind, Terminator, TerminatorKind,
};
use rustc_middle::query::QueryKey;
use rustc_middle::ty::inherent::SliceLike;
use rustc_middle::ty::{FloatTy, IntTy, Ty, TyCtxt, TyKind};
use rustc_span::def_id::{DefId, LOCAL_CRATE, LocalDefId};
use rustc_span::sym::rustc_abi;
use rustc_span::{ErrorGuaranteed, ExpnKind, Ident, MacroKind, Span};
use rustc_structures::CrateType;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;

type Result<T> = core::result::Result<T, ErrorGuaranteed>;

pub fn resolved_out_dir() -> PathBuf {
    if let Ok(td) = env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(td);
    }
    PathBuf::from("target")
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
    let mut compiler = Compiler::new(tcx);
    compiler.run()?;
    compiler.save(&out_dir);
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Block {
    begin: NodeRef,
    end: Link,
}
impl Block {
    pub fn singleton(node: NodeRef, out: i32) -> Self {
        Self {
            begin: node,
            end: Link::new(node, out),
        }
    }
    pub fn nop(graph: &mut NodeGraphComposite) -> Self {
        let mut node = NodeIf::default();
        node.condition = ValueIn {
            value: ValueBool(true).into(),
            has_default: true,
            link: None,
        };
        Self::singleton(graph.insert(node.into()), 0)
    }
    pub fn extend(&mut self, graph: &mut NodeGraphComposite, other: Block) {
        graph.connect_control(self.end, other.begin);
        *self = Self {
            begin: self.begin,
            end: other.end,
        };
    }
}

struct Compiler<'tcx> {
    tcx: TyCtxt<'tcx>,
    assets: AssetBundle,
    compiling: HashSet<DefId>,
    compiled: HashMap<DefId, i64>,
}
impl<'tcx> Compiler<'tcx> {
    fn new(tcx: TyCtxt<'tcx>) -> Self {
        let mut result = Self {
            tcx,
            assets: AssetBundle::new(crate::asset::GameMode::Overlimit),
            compiling: HashSet::new(),
            compiled: HashMap::new(),
        };
        result
    }
    fn save(&self, out_dir: &PathBuf) {
        // eprintln!("{:?}", self.tcx.output_filenames(()).with_extension("gia")); // TODO
        let path = out_dir.join(format!(
            "{}.gia",
            self.tcx.crate_name(LOCAL_CRATE).to_string()
        ));
        self.assets.save(&path).expect("encode error");
    }

    fn err<T>(&self, msg: impl Into<rustc_errors::DiagMessage>) -> Result<T> {
        Err(self.tcx.dcx().err(msg.into()))
    }

    fn span_err<T>(&self, span: Span, msg: impl Into<rustc_errors::DiagMessage>) -> Result<T> {
        Err(self.tcx.dcx().span_err(span, msg.into()))
    }

    fn get_expn_macro_attr(&self, span: Span) -> Option<syn::Attribute> {
        let ex = span.ctxt().outer_expn().expn_data();
        if matches!(ex.kind, ExpnKind::Macro(MacroKind::Attr, _)) {
            let tokens = syn::parse_str::<TokenStream>(
                &self
                    .tcx
                    .sess
                    .source_map()
                    .span_to_snippet(ex.call_site.data().span())
                    .unwrap(),
            )
            .unwrap();
            use syn::parse::Parser;
            let result = syn::Attribute::parse_outer.parse2(tokens).unwrap();
            assert_eq!(result.len(), 1);
            Some(result.into_iter().next().unwrap())
        } else {
            None
        }
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
            self.touch_fn(x.to_def_id())?;
            if let Some(it) = self.get_expn_macro_attr(x.default_span(self.tcx)) {

                // TODO: manage entrypoint (event_handler)
            }
        }
        let mut main = NodeGraphMain::new(
            NodeGraphClass::Entity,
            self.tcx.crate_name(LOCAL_CRATE).to_string(),
        );
        for (i, _) in &self.assets.assets {
            main.insert(
                NodeComposite {
                    id: AssetBundle::ID_BEGIN + i as i64,
                    controls_in: 0,
                    controls_out: vec![],
                    values_in: vec![],
                    values_out: vec![],
                }
                .into(),
            );
        }
        // TODO
        let main_id = self.assets.insert(main.into());
        self.assets.display.push(main_id);
        Ok(())
    }

    fn touch_fn(&mut self, id: DefId) -> Result<()> {
        if self.compiled.contains_key(&id) {
            return Ok(());
        }
        if !self.compiling.insert(id) {
            return self.span_err(id.default_span(self.tcx), "Recursive call");
        }
        self.compile_fn(id)?;
        self.compiling.remove(&id);
        Ok(())
    }

    fn compile_fn(&mut self, id: DefId) -> Result<()> {
        let mut graph = NodeGraphComposite::new(NodeGraphClass::Entity, self.tcx.def_path_str(id));
        let body = self.tcx.optimized_mir(id);
        let mut locals = IndexVec::<Local, NodeRef>::new(); // TODO: adapt for struct, struct list and map
        for x in &body.local_decls {
            let mut local = NodeLocal::new(self.compile_ty(x.source_info.span, x.ty)?);
            local.initial.has_default = true;
            locals.push(graph.insert(local.into()));
        }
        let mut blocks = IndexVec::<BasicBlock, Block>::new();
        for x in body.basic_blocks.iter() {
            blocks.push(self.compile_basic_block(
                &mut graph,
                &body.local_decls,
                &locals,
                &x.statements,
            )?);
        }
        let mut returns = Vec::new();
        for (k, v) in body.basic_blocks.iter_enumerated() {
            let (begin, end) =
                self.compile_terminator(&mut graph, &blocks, v.terminator.as_ref().unwrap())?;
            graph.connect_control(blocks.get(k).unwrap().end, begin);
            if let Some(end) = end {
                returns.push(end);
            }
        }
        graph
            .pins
            .get_mut(&crate::asset::generated::pin_signature::Kind::InFlow)
            .unwrap()
            .push((
                "".into(),
                vec![Link::new(blocks.get(mir::START_BLOCK).unwrap().begin, 0)],
            ));
        graph
            .pins
            .get_mut(&crate::asset::generated::pin_signature::Kind::OutFlow)
            .unwrap()
            .push(("".into(), returns));
        graph
            .pins
            .get_mut(&crate::asset::generated::pin_signature::Kind::InParam)
            .unwrap()
            .extend(self.tcx.fn_arg_idents(id).iter().enumerate().map(|(i, n)| {
                (
                    n.as_ref().map(Ident::to_string).unwrap_or_else(|| format!("arg{}", i).to_string()),
                    vec![Link::new(*locals.get(Local::arg(i)).unwrap(), 0)],
                )
            }));
        graph
            .pins
            .get_mut(&crate::asset::generated::pin_signature::Kind::OutParam)
            .unwrap()
            .push((
                "return".into(),
                vec![Link::new(*locals.get(mir::RETURN_PLACE).unwrap(), 1)],
            ));
        self.compiled.insert(id, self.assets.insert(graph.into()));
        Ok(())
    }

    fn compile_basic_block(
        &self,
        graph: &mut NodeGraphComposite,
        local_decls: &IndexVec<Local, LocalDecl<'tcx>>,
        locals: &IndexVec<Local, NodeRef>,
        statements: &Vec<Statement<'tcx>>,
    ) -> Result<Block> {
        let mut block: Option<Block> = None;
        for statement in statements {
            if let Some(nb) = match &statement.kind {
                StatementKind::Nop
                | StatementKind::StorageLive(_)
                | StatementKind::StorageDead(_)
                | StatementKind::FakeRead(_)
                | StatementKind::BackwardIncompatibleDropHint { .. } => None,
                StatementKind::Intrinsic(intrinsic) => match intrinsic.as_ref() {
                    NonDivergingIntrinsic::Assume(_) => None,
                    NonDivergingIntrinsic::CopyNonOverlapping(_) => todo!(),
                },
                StatementKind::Assign(a) => {
                    let (p, r) = a.as_ref();
                    if !p.projection.is_empty() {
                        todo!()
                    }
                    let mut node = NodeSetLocal::default();
                    node.local.link = Some(Link::new(*locals.get(p.local).unwrap(), 0));
                    node.value = self.compile_rvalue(
                        graph,
                        local_decls,
                        locals,
                        r,
                        statement.source_info.span,
                    )?;
                    Some(Block::singleton(graph.insert(node.into()), 0))
                }
                StatementKind::SetDiscriminant { .. } => todo!(),
                StatementKind::PlaceMention(_) => todo!(),
                StatementKind::AscribeUserType(_, _) => todo!(),
                StatementKind::Coverage(_) => todo!(),
                StatementKind::ConstEvalCounter => todo!(),
            } {
                match block.as_mut() {
                    None => block = Some(nb),
                    Some(block) => block.extend(graph, nb),
                }
            }
        }
        Ok(block.unwrap_or_else(|| Block::nop(graph)))
    }

    fn compile_rvalue(
        &self,
        graph: &mut NodeGraphComposite,
        local_decls: &IndexVec<Local, LocalDecl<'tcx>>,
        locals: &IndexVec<Local, NodeRef>,
        value: &Rvalue<'tcx>,
        span: Span,
    ) -> Result<ValueIn> {
        let ty = value.ty(local_decls, self.tcx);
        Ok(match value {
            Rvalue::Use(op, _) => self.compile_operand(local_decls, locals, op)?,
            Rvalue::BinaryOp(op, v) => {
                let a = self.compile_operand(local_decls, locals, &v.0)?;
                let b = self.compile_operand(local_decls, locals, &v.1)?;
                let node: Box<dyn Node> = match op {
                    BinOp::Add | BinOp::AddUnchecked | BinOp::AddWithOverflow => {
                        NodeAdd { a, b }.into()
                    }
                    BinOp::Sub | BinOp::SubUnchecked | BinOp::SubWithOverflow => {
                        NodeSubtract { a, b }.into()
                    }
                    BinOp::Mul | BinOp::MulUnchecked | BinOp::MulWithOverflow => {
                        NodeMultiply { a, b }.into()
                    }
                    BinOp::Div => NodeDivide { a, b }.into(),
                    BinOp::Rem => NodeModulo { a, b }.into(), // FIXME
                    BinOp::BitXor => NodeBitwiseXor { a, b }.into(),
                    BinOp::BitAnd => NodeBitwiseAnd { a, b }.into(),
                    BinOp::BitOr => NodeBitwiseOr { a, b }.into(),
                    BinOp::Shl
                    | BinOp::ShlUnchecked
                    | BinOp::Shr
                    | BinOp::ShrUnchecked
                    | BinOp::Eq
                    | BinOp::Lt
                    | BinOp::Le
                    | BinOp::Ne
                    | BinOp::Ge
                    | BinOp::Gt
                    | BinOp::Cmp
                    | BinOp::Offset => todo!("{:?}", op),
                };
                ValueIn {
                    value: self.compile_ty(span, ty)?,
                    has_default: false,
                    link: Link::new(graph.insert(node), 0).into(),
                }
            }
            Rvalue::Repeat(_, _)
            | Rvalue::Ref(_, _, _)
            | Rvalue::ThreadLocalRef(_)
            | Rvalue::RawPtr(_, _)
            | Rvalue::Cast(_, _, _)
            | Rvalue::UnaryOp(_, _)
            | Rvalue::Discriminant(_)
            | Rvalue::Aggregate(_, _)
            | Rvalue::CopyForDeref(_)
            | Rvalue::WrapUnsafeBinder(_, _)
            | Rvalue::Reborrow(_, _, _) => todo!("{:?}", value),
        })
    }

    fn compile_operand(
        &self,
        local_decls: &IndexVec<Local, LocalDecl<'tcx>>,
        locals: &IndexVec<Local, NodeRef>,
        op: &Operand<'tcx>,
    ) -> Result<ValueIn> {
        Ok(match op {
            Operand::Copy(p) | // TODO
            Operand::Move(p) => {
                if !p.projection.is_empty() {
                    todo!()
                }
                ValueIn {
                    value: self.compile_ty(op.span(local_decls), local_decls.get(p.local).unwrap().ty)?,
                    has_default: false,
                    link: Link::new(*locals.get(p.local).unwrap(), 1).into(),
                }
            }
            Operand::Constant(co) => match co.const_ {
                Const::Val(v, t) => {
                    ValueIn {
                        has_default: true,
                        link: None,
                        value: match &t.kind() {
                            TyKind::Bool => ValueBool(v.try_to_bool().unwrap()).into(),
                            TyKind::Int(t) => match t {
                                IntTy::I32 |
                                IntTy::Isize => ValueInt(v.try_to_scalar_int().unwrap().to_i32()).into(),
                                IntTy::I8 |
                                IntTy::I16 |
                                IntTy::I64 |
                                IntTy::I128 => return self.span_err(co.span, format!("Unsupported const: {:?}", co.const_)),
                            },
                            TyKind::Float(t) => match t {
                                FloatTy::F32 => ValueFloat(f32::from_bits(v.try_to_scalar_int().unwrap().to_bits(rustc_abi::Size::from_bits(32)) as u32)).into(),
                                FloatTy::F16 |
                                FloatTy::F64 |
                                FloatTy::F128 => return self.span_err(co.span, format!("Unsupported const: {:?}", co.const_)),
                            },
                            TyKind::Adt(_, _) => todo!(),
                            TyKind::Str => ValueString(str::from_utf8(v.try_get_slice_bytes_for_diagnostics(self.tcx).unwrap()).unwrap().to_string()).into(),
                            TyKind::Foreign(_) |
                            TyKind::Char |
                            TyKind::Uint(_) |
                            TyKind::Array(_, _) |
                            TyKind::Pat(_, _) |
                            TyKind::Slice(_) |
                            TyKind::RawPtr(_, _) |
                            TyKind::Ref(_, _, _) |
                            TyKind::FnDef(_, _) |
                            TyKind::FnPtr(_, _) |
                            TyKind::UnsafeBinder(_) |
                            TyKind::Dynamic(_, _) |
                            TyKind::Closure(_, _) |
                            TyKind::CoroutineClosure(_, _) |
                            TyKind::Coroutine(_, _) |
                            TyKind::CoroutineWitness(_, _) |
                            TyKind::Never |
                            TyKind::Tuple(_) |
                            TyKind::Alias(_, _) |
                            TyKind::Param(_) |
                            TyKind::Bound(_, _) |
                            TyKind::Placeholder(_) |
                            TyKind::Infer(_) |
                            TyKind::Error(_) => return self.span_err(co.span, format!("Unsupported const: {:?}", co.const_)),
                        },
                    }
                }
                Const::Ty(_, _) |
                Const::Unevaluated(_, _) => return self.span_err(co.span, format!("Unsupported const: {:?}", co.const_)),
            }
            Operand::RuntimeChecks(_) => todo!(),
        })
    }

    fn compile_terminator(
        &self,
        graph: &mut NodeGraphComposite,
        blocks: &IndexVec<BasicBlock, Block>,
        terminator: &Terminator,
    ) -> Result<(NodeRef, Option<Link>)> {
        match &terminator.kind {
            TerminatorKind::Return => match Block::nop(graph) {
                Block { begin, end } => Ok((begin, Some(end))),
            },
            TerminatorKind::Goto { target } => {
                let result = Block::nop(graph);
                graph.connect_control(result.end, blocks.get(*target).unwrap().begin);
                Ok((result.begin, None))
            }
            TerminatorKind::Call { func, target, .. } => {
                let result = self.compile_call(func)?;
                if let Some(target) = target {
                    graph.connect_control(result.end, blocks.get(*target).unwrap().begin);
                } else {
                    return self.span_err(terminator.source_info.span, "Panic is unsupported");
                }
                Ok((result.begin, None))
            }
            TerminatorKind::TailCall { func, .. } => match self.compile_call(func)? {
                Block { begin, end } => Ok((begin, Some(end))),
            },
            TerminatorKind::SwitchInt { .. } => todo!(),
            TerminatorKind::Drop { .. } => todo!(),
            other => self.span_err(
                terminator.source_info.span,
                format!("Unsupported terminator: {}", other.name()),
            ),
        }
    }

    fn compile_ty(&self, span: Span, ty: Ty) -> Result<AnyValue> {
        Ok(match ty.kind() {
            TyKind::Bool => ValueBool::def(),
            TyKind::Char => return self.span_err(span, "Char is unsupported"),
            TyKind::Int(ty) => match ty {
                IntTy::I8 | IntTy::I16 | IntTy::I64 | IntTy::I128 => {
                    return self.span_err(span, format!("{} is unsupported", ty.name()));
                }
                IntTy::Isize | IntTy::I32 => ValueInt::def(),
            },
            TyKind::Uint(_) => todo!(),
            TyKind::Float(_) => todo!(),
            TyKind::Adt(_, _) => todo!(),
            TyKind::Foreign(_) => todo!(),
            TyKind::Str => ValueString::def(),
            TyKind::Array(_, _) => todo!(),
            TyKind::Pat(_, _) => todo!(),
            TyKind::Slice(_) => todo!(),
            TyKind::RawPtr(_, _) => todo!(),
            TyKind::Ref(_, _, _) => todo!(),
            TyKind::FnDef(_, _) => todo!(),
            TyKind::FnPtr(_, _) => todo!(),
            TyKind::Tuple(_) => todo!(),
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
                return self.span_err(span, format!("{:?} is unsupported", ty));
            }
        })
    }

    fn compile_call(&self, func: &Operand) -> Result<Block> {
        todo!()
    }

    fn touch_struct(&mut self) {
        todo!()
    }
}
