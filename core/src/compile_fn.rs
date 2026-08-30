use crate::asset::node_graph::arithmetic::{NodeAdd, NodeBitwiseAnd, NodeBitwiseOr, NodeBitwiseXor, NodeDivide, NodeModulo, NodeMultiply, NodeSubtract};
use crate::asset::node_graph::execution::NodeSetLocal;
use crate::asset::node_graph::{ControlOut, Link, Node, NodeComposite, NodeGraph, NodeGraphComposite, NodeRef, ValueIn};
use crate::asset::value::{ValueBool, ValueFloat, ValueInt, ValueString};
use crate::compile::*;
use rustc_abi::Size;
use rustc_index::IndexVec;
use rustc_middle::mir::interpret::{GlobalAlloc, Scalar};
use rustc_middle::mir::{BasicBlock, BinOp, Body, Const, ConstValue, Local, NonDivergingIntrinsic, Operand, Place, Rvalue, Statement, StatementKind, Terminator, TerminatorKind};
use rustc_middle::ty::{FloatTy, IntTy, TyCtxt, TyKind};
use rustc_span::{Span, Spanned};

pub(crate) struct CompilingFn<'tcx, 'a> {
    pub tcx: TyCtxt<'tcx>,
    pub compiler: &'a mut Compiler<'tcx>,
    pub graph: &'a mut NodeGraphComposite,
    pub body: &'a Body<'tcx>,
    pub locals: &'a IndexVec<Local, NodeRef>,
}
impl<'tcx> WithTyCtxt<'tcx> for CompilingFn<'tcx, '_> {
    fn get_tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }
}
impl<'tcx, 'a> CompilingFn<'tcx, 'a> {
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
                    let value = self.compile_rvalue(r, statement.source_info.span)?;
                    Some(self.compile_assign(*p, value))
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

    fn compile_assign(&mut self, place: Place, value: ValueIn) -> Block {
        if !place.projection.is_empty() {
            todo!()
        }
        let mut node = NodeSetLocal::default();
        node.local.link = Some(Link::new(*self.locals.get(place.local).unwrap(), 0));
        node.value = value;
        Block::singleton(self.graph.insert(node.into()), 0)
    }

    fn compile_rvalue(&mut self, value: &Rvalue<'tcx>, span: Span) -> Result<ValueIn> {
        let ty = value.ty(&self.body.local_decls, self.tcx);
        Ok(match value {
            Rvalue::Use(op, _) => self.compile_operand(op)?,
            Rvalue::BinaryOp(op, v) => {
                let a = self.compile_operand(&v.0)?;
                let b = self.compile_operand(&v.1)?;
                let node: Box<dyn Node> = match op {
                    BinOp::Add | BinOp::AddUnchecked | BinOp::AddWithOverflow =>
                        NodeAdd { a, b }.into(),
                    BinOp::Sub | BinOp::SubUnchecked | BinOp::SubWithOverflow =>
                        NodeSubtract { a, b }.into(),
                    BinOp::Mul | BinOp::MulUnchecked | BinOp::MulWithOverflow =>
                        NodeMultiply { a, b }.into(),
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
                    value: compile_ty(self, span, ty)?,
                    has_default: false,
                    link: Link::new(self.graph.insert(node), 0).into(),
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

    fn compile_operand(&mut self, op: &Operand<'tcx>) -> Result<ValueIn> {
        Ok(match op {
            Operand::Copy(p) | // TODO
            Operand::Move(p) => {
                if !p.projection.is_empty() {
                    todo!()
                }
                ValueIn {
                    value: compile_ty(self, op.span(&self.body.local_decls), self.body.local_decls.get(p.local).unwrap().ty)?,
                    has_default: false,
                    link: Link::new(*self.locals.get(p.local).unwrap(), 1).into(),
                }
            }

            Operand::Constant(co) => {
                let ty = co.ty();
                let v = co.const_.eval(self.tcx, self.body.typing_env(self.tcx), co.span).map_err(|_| self.get_tcx().dcx().span_err(co.span, format!("Unsupported const: {:?}", co.const_)))?;
                ValueIn {
                    has_default: true,
                    link: None,
                    value: match &ty.kind() {
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
                            FloatTy::F32 => ValueFloat(f32::from_bits(v.try_to_scalar_int().unwrap().to_bits(Size::from_bits(32)) as u32)).into(),
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
            Operand::RuntimeChecks(_) => todo!(),
        })
    }

    pub(crate) fn compile_terminator(
        &mut self,
        blocks: &IndexVec<BasicBlock, Block>,
        terminator: &Terminator<'tcx>,
    ) -> Result<(NodeRef, Option<Link>)> {
        match &terminator.kind {
            TerminatorKind::Return => match Block::nop(self.graph) {
                Block { begin, end } => Ok((begin, Some(end))),
            },
            TerminatorKind::Goto { target } => {
                let result = Block::nop(self.graph);
                self.graph.connect_control(result.end, blocks.get(*target).unwrap().begin);
                Ok((result.begin, None))
            }
            TerminatorKind::Call { func, target, args, destination, .. } => {
                let result = self.compile_call(func, args, *destination)?;
                if let Some(target) = target {
                    self.graph.connect_control(result.end, blocks.get(*target).unwrap().begin);
                } else {
                    return self.span_err(terminator.source_info.span, "Panic is unsupported");
                }
                Ok((result.begin, None))
            }
            TerminatorKind::TailCall { func, args, .. } => match self.compile_call(func, args, Place::return_place())? {
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

    fn compile_call(&mut self, func: &Operand<'tcx>, args: &[Spanned<Operand<'tcx>>], destination: Place) -> Result<Block> {
        let func = match func {
            Operand::Copy(_) | Operand::Move(_) | Operand::RuntimeChecks(_) =>
                return self.span_err(func.span(&self.body.local_decls), format!("Unsupported call: {:?}", func)),
            Operand::Constant(func) => {
                match func.const_ {
                    Const::Ty(_, _) |
                    Const::Unevaluated(_, _) =>
                        match func.const_.eval(self.tcx, self.body.typing_env(self.tcx), func.span).map_err(|_| self.get_tcx().dcx().span_err(func.span, format!("Unsupported call const: {:?}", func.const_)))? {
                            ConstValue::Scalar(sc) => match sc {
                                Scalar::Int(_) => panic!("int"),
                                Scalar::Ptr(p, _) => {
                                    let (p, o) = p.into_raw_parts();
                                    if o !=Size::ZERO {
                                        return self.span_err(func.span, format!("Unsupported call const Indirect offset: {:?}", sc));
                                    }
                                    match self.tcx.global_alloc(p.alloc_id()) {
                                        GlobalAlloc::Function { instance } => instance.def_id(),
                                        other => return self.span_err(func.span, format!("Unsupported call const Indirect: {:?}", other)),
                                    }
                                },
                            },
                            it => panic!("{:?}", it),
                        },
                    Const::Val(val, ty) => {
                        match val {
                            ConstValue::Scalar(_) |
                            ConstValue::Slice { .. } => return self.span_err(func.span, format!("Unsupported call const val: {:?}", val)),
                            ConstValue::ZeroSized => {
                                match ty.kind() {
                                    TyKind::FnDef(func, _) => *func,
                                    _ => return self.span_err(func.span, format!("Unsupported call const ty: {:?}", ty)),
                                }
                            }
                            ConstValue::Indirect { alloc_id, offset } => {
                                self.tcx.dcx().note("ConstValue::Indirect");
                                if offset != Size::ZERO {
                                    return self.span_err(func.span, format!("Unsupported call const Indirect offset: {:?}", offset));
                                }
                                match self.tcx.global_alloc(alloc_id) {
                                    GlobalAlloc::Function { instance } => instance.def_id(),
                                    other => return self.span_err(func.span, format!("Unsupported call const Indirect: {:?}", other)),
                                }
                            }
                        }
                    }
                }
            }
        };
        let ret = compile_ty(self, self.tcx.def_span(func), self.tcx.optimized_mir(func).return_ty())?;
        let node = NodeComposite {
            id: self.compiler.touch_fn(func)?,
            controls_in: 1,
            controls_out: vec![ControlOut::new()],
            values_in: args.iter().map(|x| &x.node).map(|x| self.compile_operand(x)).collect::<Result<Vec<_>>>()?,
            values_out: vec![ret.clone()], // FIXME: ()
        };
        let node_call = self.graph.insert(node.into());
        let mut block = Block::singleton(node_call, 0);
        let block1 = self.compile_assign(destination, ValueIn {
            value: ret,
            has_default: false,
            link: Link::new(node_call, 0).into(),
        });
        block.extend(self.graph, block1);
        Ok(block)
    }
}
