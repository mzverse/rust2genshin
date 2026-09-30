use super::*;
use crate::asset::value::{NativeKind, NativeValue};
use crate::compile::ir::{IrKind, IrNodeId, NodeKindIr, node_ir_assemble, node_ir_unreachable};
use crate::compile::native::native_const;
use crate::compile::place::{CompiledLocal, CompiledPlace, LocalRef};
use crate::node::arithmetic::{NODE_AND, NODE_BITWISE_AND, NODE_BITWISE_NOT, NODE_BITWISE_OR, NODE_BITWISE_XOR, NODE_LEFT_SHIFT, NODE_NOT, NODE_OR, NODE_REM, NODE_XOR, node_add, node_cast, node_divide, node_equal, node_greater_equal, node_greater_than, node_less_equal, node_less_than, node_multiply, node_subtract};
use crate::node::control::node_switch;
use crate::node::{Link, Lower, NativeNodeId, ValueIn};
use either::Either;
use rustc_abi::{FieldIdx, Integer, IntegerType, Size};
use rustc_attr_ir::LangItem;
use rustc_index::{Idx, IndexVec};
use rustc_middle::mir::interpret::{AllocRange, GlobalAlloc, Scalar};
use rustc_middle::mir::{AggregateKind, BasicBlock, BinOp, BorrowKind, Const, ConstOperand, ConstValue, MutBorrowKind, NonDivergingIntrinsic, Operand, Place, PlaceTy, ProjectionElem, Rvalue, Statement, StatementKind, Terminator, TerminatorKind, UnOp, WithRetag};
use rustc_middle::ty::{FloatTy, InstanceKind, IntTy, PseudoCanonicalInput, ScalarInt, TyKind, TypingEnv};
use rustc_span::{DUMMY_SP, Span};
use tap::Pipe;

pub type NodeGraphIr = NodeGraph<IrNodeId, IrKind>;

pub struct CompilingFn<'tcx, 'a> {
    pub tcx: TyCtxt<'tcx>,
    pub func: Instance<'tcx>,
    pub compiler: &'a mut Compiler<'tcx>,
    pub graph: &'a mut NodeGraphIr,
    pub body: &'a Body<'tcx>,
    pub locals: &'a IndexVec<Local, CompiledLocal<LocalRef<'tcx>>>,
}

#[derive(Clone, Default)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct FnDecl {
    pub control: bool,
    pub params: Vec<CompiledLocal<()>>,
    pub ret: CompiledLocal<()>,
    pub proxies_in: Vec<usize>,
    pub proxies_out: Vec<Option<Either<usize, Option<Box<dyn NativeValue>>>>>,
}

pub fn node_ir_native(kind: NodeKind) -> NodeKindIr {
    struct Enhancer;
    impl Lower for Enhancer {
        type NodeId0 = NativeNodeId;
        type Kind0 = NativeKind;
        type NodeId1 = IrNodeId;
        type Kind1 = IrKind;

        fn lower_node_id(&mut self, node_id: Self::NodeId0) -> Self::NodeId1 {
            IrNodeId::Native(node_id)
        }

        fn lower_kind(&mut self, kind: Self::Kind0) -> Self::Kind1 {
            IrKind::Native(kind)
        }
    }
    Enhancer.lower_node_kind(kind)
}

impl<'tcx, 'a> CompilingFn<'tcx, 'a> {
    pub fn helper(&self) -> Helper<'tcx> {
        Helper(self.tcx)
    }
    pub fn mono(&self, ty: Ty<'tcx>) -> Ty<'tcx> {
        self.helper().monomorphize(self.func, ty)
    }
    pub fn compile_basic_block(
        &mut self,
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
                    Some(self.compile_assign_rvalue(*p, r, statement.source_info.span)?)
                }
                StatementKind::SetDiscriminant { .. } => todo!(),
                StatementKind::PlaceMention(_) => todo!(),
                StatementKind::AscribeUserType(_, _) => todo!(),
                StatementKind::Coverage(_) => todo!(),
                StatementKind::ConstEvalCounter => todo!(),
            } {
                match block.as_mut() {
                    None => block = Some(nb),
                    Some(block) => block.extend(self.graph, nb),
                }
            }
        }
        Ok(block.unwrap_or_else(|| Block::nop(self.graph)))
    }

    pub fn compile_assign(&mut self, place: Place<'tcx>, span: Span, value: ValueIn) -> Result<Block> {
        let kind = self.compiler.compile_ty(span, self.mono(place.ty(&self.body.local_decls, self.tcx).ty))?;
        Ok(self.compile_place(place, span)?.setter(&mut self.compiler.target, self.graph, &kind, value))
    }

    pub fn place_ty(&self, place: Place<'tcx>) -> PlaceTy<'tcx> {
        let r = place.ty(&self.body.local_decls, self.tcx);
        PlaceTy {
            ty: self.mono(r.ty),
            ..r
        }
    }

    fn compile_rvalue(&mut self, value: &Rvalue<'tcx>, span: Span) -> Result<ValueIn> {
        let ty = self.mono(value.ty(&self.body.local_decls, self.tcx));
        Ok(match value {
            Rvalue::Use(op, _) => self.compile_operand(op, span)?,
            Rvalue::BinaryOp(op, v) => {
                let ty0 = v.0.ty(&self.body.local_decls, self.tcx);
                let kind0 = self.compiler.compile_ty(v.0.span(&self.body.local_decls), ty0)?.into_native().unwrap();
                let mut node = self.graph.insert(node_ir_native(match op {
                    BinOp::Add | BinOp::AddUnchecked | BinOp::AddWithOverflow =>
                        node_add(kind0),
                    BinOp::Sub | BinOp::SubUnchecked | BinOp::SubWithOverflow =>
                        node_subtract(kind0),
                    BinOp::Mul | BinOp::MulUnchecked | BinOp::MulWithOverflow =>
                        node_multiply(kind0),
                    BinOp::Div => node_divide(kind0),
                    BinOp::Rem => NODE_REM.clone(),
                    BinOp::BitXor => if ty.is_bool() { NODE_XOR.clone() } else { NODE_BITWISE_XOR.clone() },
                    BinOp::BitAnd => if ty.is_bool() { NODE_AND.clone() } else { NODE_BITWISE_AND.clone() },
                    BinOp::BitOr => if ty.is_bool() { NODE_OR.clone() } else { NODE_BITWISE_OR.clone() },
                    BinOp::Eq | BinOp::Ne => node_equal(kind0),
                    BinOp::Shl | BinOp::ShlUnchecked => NODE_LEFT_SHIFT.clone(),
                    BinOp::Shr | BinOp::ShrUnchecked => todo!(), //return self.compile_call(span, self.compiler.find_lib_fn("<i32 as rust2genshin_lib::math::I32>::shr")?, &[dummy_spanned(v.0.clone()), dummy_spanned(v.1.clone())], place),
                    BinOp::Lt => node_less_than(kind0),
                    BinOp::Le => node_less_equal(kind0),
                    BinOp::Ge => node_greater_equal(kind0),
                    BinOp::Gt => node_greater_than(kind0),
                    | BinOp::Cmp
                    | BinOp::Offset => todo!("{:?}", op),
                }));
                let a = self.compile_operand(&v.0, span)?;
                let b = self.compile_operand(&v.1, span)?;
                self.graph.set_value_in(Link::node(node, 0), a);
                self.graph.set_value_in(Link::node(node, 1), b);
                if matches!(op, BinOp::Ne) {
                    // ! (a == b) — invert the equal node's bool output
                    let not_node = self.graph.insert(node_ir_native(NODE_NOT.clone()));
                    self.graph.link_value(Link::node(node, 0), Link::node(not_node, 0));
                    node = not_node;
                }
                ValueIn::link(Link::node(node, 0))
            }
            Rvalue::UnaryOp(op, v) => {
                let _kind = self.compiler.compile_ty(v.span(&self.body.local_decls), ty)?;
                let node = self.graph.insert(node_ir_native(match op {
                    UnOp::Not => if ty.is_bool() {
                        NODE_NOT.clone()
                    } else {
                        NODE_BITWISE_NOT.clone()
                    },
                    UnOp::Neg => return self.compile_rvalue(&Rvalue::BinaryOp(BinOp::Sub, (Operand::Constant(ConstOperand {
                        span: DUMMY_SP,
                        user_ty: None,
                        const_: Const::Val(ConstValue::Scalar(Scalar::from_i32(0)), if ty.is_floating_point() { self.tcx.types.f32 } else { self.tcx.types.i32 }),
                    }.into()), v.clone()).into()), span),
                    UnOp::PtrMetadata => todo!(),
                }));
                let value = self.compile_operand(v, span)?;
                self.graph.set_value_in(Link::node(node, 0), value);
                ValueIn::link(Link::node(node, 0))
            },
            Rvalue::Reborrow(t, _, p) => return if t.is_str() {
                self.compile_rvalue(&Rvalue::Use(Operand::Copy(*p), WithRetag::No), span)
            } else {
                self.helper().span_err(span, "Reborrow from raw ptr is still unsupported")
            },
            Rvalue::Ref(_region, k, place) => match k {
                BorrowKind::Mut { .. } => if let CompiledPlace::Local(CompiledLocal::Singleton(LocalRef { setter, getter, .. })) = self.compile_place(*place, span)? {
                    let kind = self.compiler.compile_ty(span, ty)?;
                    let result = self.graph.insert(node_ir_assemble(&mut self.compiler.target, &kind));
                    self.graph.set_value_in(Link::node(result, 0), ValueIn::link(setter));
                    self.graph.set_value_in(Link::node(result, 1), ValueIn::link(getter));
                    ValueIn::link(Link::node(result, 0))
                } else {
                    return self.helper().span_err(span, "Only local singleton can be ref mut");
                },
                _ => self.compile_operand(&Operand::Copy(*place), span)?
            },
            Rvalue::RawPtr(_, p) => {
                let Some(ProjectionElem::Deref) = p.projection.last() else {
                    return self.helper().span_err(span, format!("RawPtr rvalue is still unsupported: {p:?}"))?;
                };
                return self.compile_rvalue(&Rvalue::Use(Operand::Copy(Place { local: p.local, projection: self.tcx.mk_place_elems(&p.projection[0..p.projection.len() - 1]) }), WithRetag::No), span);
            },
            Rvalue::Cast(kind, op, target_ty) => {
                // FIXME: f32 to i32
                let from_ty = op.ty(&self.body.local_decls, self.tcx);
                let from_kind = self.compiler.compile_ty(span, from_ty)?.into_native().unwrap();
                let to_kind = self.compiler.compile_ty(span, *target_ty)?.into_native().unwrap();
                if from_kind == to_kind {
                    // No-op cast (e.g. i32 as isize, or identity casts inside expressions).
                    self.compile_operand(op, span)?
                } else {
                    let Some(node) = node_cast(from_kind, to_kind) else {
                        return self.helper().span_err(span, format!("Unsupported cast ({kind:?}) {from_ty:?} → {target_ty:?}"));
                    };
                    let node = self.graph.insert(node_ir_native(node));
                    let v = self.compile_operand(op, span)?;
                    self.graph.set_value_in(Link::node(node, 0), v);
                    ValueIn::link(Link::node(node, 0))
                }
            }
            Rvalue::Aggregate(kind, fields) if !matches!(**kind, AggregateKind::Array(..)) => {
                match ty.kind() {
                    TyKind::Adt(d, _a) if d.repr().transparent() =>
                        self.compile_operand(&fields[FieldIdx::new(0)], span)?,
                    TyKind::Adt(d, a) if d.is_enum() => {
                        if self.compiler.get_default_some(*d, a)?.is_some() {
                            self.compile_operand(&fields[FieldIdx::new(0)], span)?
                        } else {
                            todo!()
                        }
                    },
                    _ => {
                        let kind = self.compiler.compile_ty(span, ty)?;
                        let node = self.graph.insert(node_ir_assemble(&mut self.compiler.target, &kind));
                        for (i, x) in fields.iter().enumerate() {
                            let v = self.compile_operand(x, span)?;
                            self.graph.set_value_in(Link::node(node, i), v);
                        }
                        ValueIn::link(Link::node(node, 0))
                    }
                }
            },
            Rvalue::Discriminant(from) => {
                let TyKind::Adt(d, a) = self.place_ty(*from).ty.kind() else { unreachable!() };
                if d.repr().int == Some(IntegerType::Fixed(Integer::I32, true)) {
                    self.compile_operand(&Operand::Copy(*from), span)?
                } else if let Some(kind) = self.compiler.get_default_some(*d, a)? {
                    assert_eq!(d.discriminant_for_variant(self.tcx, d.variant_index_with_id(self.tcx.lang_items().get(LangItem::OptionNone).unwrap())).val, 0);
                    assert_eq!(d.discriminant_for_variant(self.tcx, d.variant_index_with_id(self.tcx.lang_items().get(LangItem::OptionSome).unwrap())).val, 1);
                    let node_eq = self.graph.insert(node_ir_native(node_equal(kind)));
                    let v = self.compile_operand(&Operand::Copy(*from), span)?;
                    self.graph.set_value_in(Link::node(node_eq, 0), v);
                    let node_not = self.graph.insert(node_ir_native(NODE_NOT.clone()));
                    self.graph.link_value(Link::node(node_eq, 0), Link::node(node_not, 0));
                    let node_cast = self.graph.insert(node_ir_native(node_cast(NativeKind::Bool, NativeKind::Int).unwrap()));
                    self.graph.link_value(Link::node(node_not, 0), Link::node(node_cast, 0));
                    ValueIn::link(Link::node(node_cast, 0))
                } else {
                    todo!()
                }
            },
            Rvalue::Repeat(_, _)
            | Rvalue::ThreadLocalRef(_)
            | Rvalue::Aggregate(_, _) // non-Tuple AggregateKind still panics
            | Rvalue::CopyForDeref(_)
            | Rvalue::WrapUnsafeBinder(_, _)
            => todo!("{:?}", value),
        })
    }

    fn compile_assign_rvalue(&mut self, place: Place<'tcx>, value: &Rvalue<'tcx>, span: Span) -> Result<Block> {
        let value_in = self.compile_rvalue(value, span)?;
        self.compile_assign(place, span, value_in)
    }

    fn compile_const(&mut self, ty: Ty<'tcx>, kind: IrKind, v: ConstValue, span: Span) -> Result<ValueIn> {
        let layout = self.tcx.layout_of(PseudoCanonicalInput {
            typing_env: TypingEnv::fully_monomorphized(),
            value: ty,
        }).unwrap().layout;
        let get_scalar = || match v {
            ConstValue::Scalar(r) => r,
            ConstValue::Indirect { alloc_id, offset } => {
                let alloc = self.tcx.global_alloc(alloc_id).unwrap_memory().inner();
                alloc.read_scalar(&self.tcx, AllocRange {
                    start: offset,
                    size: layout.size,
                }, false).unwrap()
            },
            ConstValue::ZeroSized |
            ConstValue::Slice { .. } => panic!(),
        };
        if let IrKind::Native(ref kind) = kind && let Some(r) = native_const(kind, get_scalar) {
            return Ok(ValueIn::value(r));
        }
        Ok(ValueIn::value(match ty.kind() {
            TyKind::Bool => get_scalar().to_bool().unwrap().into(),
            TyKind::Int(t) => match t {
                IntTy::I32 => get_scalar().to_i32().unwrap().into(),
                IntTy::Isize => (get_scalar().to_int(self.tcx.data_layout.pointer_size()).unwrap() as i32).into(),
                IntTy::I8 |
                IntTy::I16 |
                IntTy::I64 |
                IntTy::I128 => return self.helper().span_err(span, format!("Unsupported const int: {:?}", v)),
            },
            TyKind::Float(t) => match t {
                FloatTy::F32 => f32::from_bits(get_scalar().to_bits(Size::from_bits(32)).unwrap() as u32).into(),
                FloatTy::F16 |
                FloatTy::F64 |
                FloatTy::F128 => return self.helper().span_err(span, format!("Unsupported const float: {:?}", v)),
            },
            TyKind::Str => panic!(),
            TyKind::Ref(_, e, _) => {
                if e.is_str() {
                    str::from_utf8(v.try_get_slice_bytes_for_diagnostics(self.tcx).unwrap()).unwrap().to_string().into()
                } else {
                    return self.helper().span_err(span, format!("Unsupported const ref: {:?}", ty));
                }
            },
            TyKind::Adt(d, a) if self.compiler.get_default_some(*d, a)?.is_some() => {
                assert_eq!(get_scalar().to_uint(self.tcx.data_layout.pointer_size()).unwrap(), d.discriminant_for_variant(self.tcx, d.variant_index_with_id(self.tcx.lang_items().get(LangItem::OptionNone).unwrap())).val);
                return Ok(ValueIn::default());
            },
            TyKind::Adt(d, a) if d.repr().transparent() =>
                return self.compile_const(self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), d.non_enum_variant().single_field().ty(self.tcx, a)), kind, v, span),
            TyKind::Adt(d, _a) if d.is_enum() && d.variants().len() > 1 =>
                todo!("enum const is still unsupported"),
            TyKind::Adt(..) | TyKind::Tuple(..) => {
                let ele: Vec<_> = match ty.kind() {
                    TyKind::Adt(d, a) => d.non_enum_variant().fields.iter().map(|x| self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), x.ty(self.tcx, a))).collect(),
                    TyKind::Tuple(ele) => ele.into_iter().collect(),
                    _ => unreachable!(),
                };
                return Ok(match ele.len() {
                    0 => ValueIn::default(),
                    _ => {
                        let ConstValue::Indirect { alloc_id, offset } = v else { todo!("{v:?}") };
                        let node = self.graph.insert(node_ir_assemble(&mut self.compiler.target, &kind));
                        for (i, x) in ele.into_iter().enumerate() {
                            let k = self.compiler.compile_ty(span, x)?;
                            let v = self.compile_const(x, k, ConstValue::Indirect {
                                alloc_id,
                                offset: offset + layout.fields.offset(i),
                            }, span)?;
                            self.graph.set_value_in(Link::node(node, i), v);
                        }
                        ValueIn::link(Link::node(node, 0))
                    },
                });
            },
            TyKind::Slice(_) |
            TyKind::Foreign(_) |
            TyKind::Char |
            TyKind::Uint(_) |
            TyKind::Array(_, _) |
            TyKind::Pat(_, _) |
            TyKind::RawPtr(_, _) |
            TyKind::FnDef(_, _) |
            TyKind::FnPtr(_, _) |
            TyKind::UnsafeBinder(_) |
            TyKind::Dynamic(_, _) |
            TyKind::Closure(_, _) |
            TyKind::CoroutineClosure(_, _) |
            TyKind::Coroutine(_, _) |
            TyKind::CoroutineWitness(_, _) |
            TyKind::Never |
            TyKind::Alias(_, _) |
            TyKind::Param(_) |
            TyKind::Bound(_, _) |
            TyKind::Placeholder(_) |
            TyKind::Infer(_) |
            TyKind::Error(_) => return self.helper().span_err(span, format!("Unsupported const: {:?} = {:?}", ty, v)),
        }))
    }

    fn compile_operand(&mut self, op: &Operand<'tcx>, span: Span) -> Result<ValueIn> {
        let ty = self.mono(op.ty(&self.body.local_decls, self.tcx));
        let kind = self.compiler.compile_ty(span, ty)?;
        Ok(match op {
            Operand::Copy(p) |
            Operand::Move(p) => {
                self.compile_place(*p, span)?.getter(&mut self.compiler.target, self.graph, &kind)
            }
            Operand::Constant(co) => {
                let v = co.const_.eval(self.tcx, TypingEnv::fully_monomorphized(), co.span).map_err(|_| self.tcx.dcx().span_err(co.span, format!("Unsupported const eval: {:?}", co.const_)))?;
                self.compile_const(ty, kind, v, span)?
            }
            Operand::RuntimeChecks(_) => ValueIn::value(false.into()),
        })
    }

    pub fn compile_terminator(
        &mut self,
        blocks: &IndexVec<BasicBlock, Block>,
        terminator: &Terminator<'tcx>,
    ) -> Result<Link> {
        Ok(match &terminator.kind {
            TerminatorKind::Return => Block::nop(self.graph).pipe(|block| {
                self.graph.link_control(block.end, Link::export(0));
                block.begin
            }),
            TerminatorKind::Goto { target } => {
                let result = Block::nop(self.graph);
                self.graph.link_control(result.end, blocks.get(*target).unwrap().begin);
                result.begin
            },
            TerminatorKind::Assert { target, .. } => {
                self.tcx.dcx().span_note(terminator.source_info.span, "Ignored assert");
                let result = Block::nop(self.graph); // same as goto
                self.graph.link_control(result.end, blocks.get(*target).unwrap().begin);
                result.begin
            },
            TerminatorKind::Call { func, target, args, destination, .. } => {
                let args = args.iter().map(|x| self.compile_operand(&x.node, x.span)).collect::<Result<_>>()?;
                let result = self.compile_call(terminator.source_info.span, self.find_fn(func)?, args, Some(*destination))?;
                if let Some(target) = target {
                    self.graph.link_control(result.end, blocks.get(*target).unwrap().begin);
                }
                result.begin
            }
            TerminatorKind::TailCall { func, args, .. } => {
                let args = args.iter().map(|x| self.compile_operand(&x.node, x.span)).collect::<Result<_>>()?;
                self.compile_call(terminator.source_info.span, self.find_fn(func)?, args, Place::return_place().into())?.pipe(|block| {
                    self.graph.link_control(block.end, Link::export(0));
                    block.begin
                })
            },
            TerminatorKind::SwitchInt { discr, targets, .. } => {
                let node = if discr.ty(&self.body.local_decls, self.tcx).is_bool() {
                    let node = self.graph.insert(node_ir_native(NODE_IF.clone()));
                    self.graph.link_control(Link::node(node, 0), blocks[targets.target_for_value(1u128)].begin);
                    self.graph.link_control(Link::node(node, 1), blocks[targets.target_for_value(0u128)].begin);
                    node
                } else {
                    if targets.all_targets().len() > 10 { // limited by Genshin Impact
                        self.tcx.dcx().span_err(terminator.source_info.span, format!("Too many cases: {}", targets.all_targets().len()));
                        todo!();
                    }
                    // FIXME: ir_switch
                    let node = self.graph.insert(node_ir_native(node_switch(NativeKind::Int, targets.all_values().len())));
                    self.graph.set_default(Link::node(node, 1), Some(targets.all_values().iter().map(|x| ScalarInt::try_from_int(x.0 as u64, Size::from_bytes(4)).unwrap().to_i32().into()).collect::<Vec<Box<dyn NativeValue>>>().into()));
                    self.graph.link_control(Link::node(node, 0), blocks[targets.otherwise()].begin);
                    for i in 0..targets.all_values().len() {
                        let index = 1 + i;
                        self.graph.link_control(Link::node(node, index), blocks[targets.all_targets()[i]].begin);
                    }
                    node
                };
                let value = self.compile_operand(discr, terminator.source_info.span)?;
                self.graph.set_value_in(Link::node(node, 0), value);
                Link::node(node, 0)
            },
            TerminatorKind::Unreachable => {
                let node = self.graph.insert(node_ir_unreachable());
                Link::node(node, 0)
            } // TODO
            TerminatorKind::Drop { place, target, .. } => {
                let arg = self.compile_rvalue(&Rvalue::Ref(self.tcx.lifetimes.re_erased, BorrowKind::Mut { kind: MutBorrowKind::Default }, *place), terminator.source_info.span)?;
                let result = self.compile_call(terminator.source_info.span,
                                               Instance::resolve_drop_glue(self.tcx, self.mono(place.ty(&self.body.local_decls, self.tcx).ty)),
                                               vec![arg],
                                               None)?;
                self.graph.link_control(result.end, blocks.get(*target).unwrap().begin);
                result.begin
            },
            TerminatorKind::UnwindResume => Block::nop(self.graph).begin,
            other => return self.helper().span_err(
                terminator.source_info.span,
                format!("Unsupported terminator: {}", other.name()),
            ),
        })
    }

    fn find_fn(&self, operand: &Operand<'tcx>) -> Result<Instance<'tcx>> {
        Ok(match operand {
            Operand::Copy(_) | Operand::Move(_) | Operand::RuntimeChecks(_) =>
                return self.helper().span_err(operand.span(&self.body.local_decls), format!("Unsupported call: {:?}", operand)),
            Operand::Constant(func) => {
                match func.const_ {
                    Const::Ty(_, _) |
                    Const::Unevaluated(_, _) =>
                        match func.const_.eval(self.tcx, TypingEnv::fully_monomorphized(), func.span).map_err(|_| self.tcx.dcx().span_err(func.span, format!("Unsupported call const: {:?}", func.const_)))? {
                            ConstValue::Scalar(sc) => match sc {
                                Scalar::Int(_) => panic!("int"),
                                Scalar::Ptr(p, _) => {
                                    let (p, o) = p.into_raw_parts();
                                    if o != Size::ZERO {
                                        return self.helper().span_err(func.span, format!("Unsupported call const Indirect offset: {:?}", sc));
                                    }
                                    match self.tcx.global_alloc(p.alloc_id()) {
                                        GlobalAlloc::Function { instance } => instance,
                                        other => return self.helper().span_err(func.span, format!("Unsupported call const Indirect: {:?}", other)),
                                    }
                                },
                            },
                            other => panic!("{:?}", other),
                        },
                    Const::Val(val, ty) => {
                        match val {
                            ConstValue::Scalar(_) |
                            ConstValue::Slice { .. } => return self.helper().span_err(func.span, format!("Unsupported call const val: {:?}", val)),
                            ConstValue::ZeroSized => {
                                match self.mono(ty).kind() {
                                    TyKind::FnDef(def_id, b) =>
                                        Instance::try_resolve(self.tcx, TypingEnv::fully_monomorphized(), *def_id, self.tcx.normalize_erasing_late_bound_regions(TypingEnv::fully_monomorphized(), *b))?.unwrap(),
                                    _ => return self.helper().span_err(func.span, format!("Unsupported call const ty: {:?}", ty)),
                                }
                            }
                            ConstValue::Indirect { alloc_id, offset } => {
                                self.tcx.dcx().note("ConstValue::Indirect");
                                if offset != Size::ZERO {
                                    return self.helper().span_err(func.span, format!("Unsupported call const Indirect offset: {:?}", offset));
                                }
                                match self.tcx.global_alloc(alloc_id) {
                                    GlobalAlloc::Function { instance } => instance,
                                    other => return self.helper().span_err(func.span, format!("Unsupported call const Indirect: {:?}", other)),
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    fn compile_call(&mut self, span: Span, func: Instance<'tcx>, args: Vec<ValueIn>, destination: Option<Place<'tcx>>) -> Result<Block> {
        if Some(func.def_id()) == self.tcx.lang_items().get(LangItem::Panic) {
            self.tcx.dcx().span_note(span, "Ignored panic");
            return Ok(Block::nop(self.graph));
        }
        if let InstanceKind::Intrinsic(def_id) = func.def {
            return match self.tcx.intrinsic(def_id).unwrap().name.as_str() {
                "black_box" => // only blocked mir, FIXME
                    self.compile_assign(destination.unwrap(), span, args[0].clone()),
                other => todo!("intrinsic: {other}"),
            };
        }
        let sig = self.helper().fn_sig(func);
        let params: Vec<IrKind> = self.helper().fn_params(func, sig).iter().map(|x| self.compiler.compile_ty(span, *x)).collect::<Result<_>>()?;
        let node_kind = if params.iter().all(|x| x.as_native().is_some())
            && let Some(native) = self.compiler.compile_native_call(span, func, sig, &params)? {
            node_ir_native(native)
        } else {
            let ret = self.compiler.compile_ty(span, sig.output())?;
            NodeKind::new(
                IrNodeId::Fn(self.compiler.touch_fn(func)?),
                1, 1,
                params.iter().cloned().map(Some).collect(),
                vec![ret],
            )
        };
        let control = match node_kind.controls_in_num {
            0 => false,
            1 => true,
            _ => panic!()
        };
        let node = self.graph.insert(node_kind.clone());
        let mut block = if control {
            Block::singleton(node, 0)
        } else {
            Block::nop(self.graph)
        };
        for (i, x) in args.into_iter().enumerate() {
            self.graph.set_value_in(Link::node(node, i), x);
        }
        match node_kind.values_out_types.len() {
            0 => (),
            1 => if let Some(destination) = destination {
                let b1 = self.compile_assign(destination, span, ValueIn::link(Link::node(node, 0)))?;
                block.extend(self.graph, b1);
            },
            _ => panic!(),
        }
        Ok(block)
    }
}
