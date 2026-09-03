use crate::asset::value::{ValueBool, ValueFloat, ValueInt, ValueIntList, ValueString};

use super::*;
use crate::asset::node_graph::ValueIn;
use crate::asset::node_graph::arithmetic::{NODE_AND, NODE_BITWISE_AND, NODE_BITWISE_NOT, NODE_BITWISE_OR, NODE_BITWISE_XOR, NODE_LEFT_SHIFT, NODE_MODULO, NODE_NOT, NODE_OR, NODE_XOR, node_add, node_divide, node_equal, node_greater_equal, node_greater_than, node_less_equal, node_less_than, node_multiply, node_subtract};
use crate::asset::node_graph::composite::node_composite;
use crate::asset::node_graph::control::node_switch;
use crate::asset::node_graph::execution::node_set_local;
use rustc_abi::Size;
use rustc_index::IndexVec;
use rustc_middle::mir::interpret::{AllocRange, GlobalAlloc, Scalar};
use rustc_middle::mir::{BasicBlock, BinOp, Const, ConstOperand, ConstValue, NonDivergingIntrinsic, Operand, Place, ProjectionElem, Rvalue, Statement, StatementKind, Terminator, TerminatorKind, UnOp, WithRetag};
use rustc_middle::ty::{FloatTy, IntTy, ScalarInt, TyKind, TypingEnv};
use rustc_span::{DUMMY_SP, Span, Spanned, dummy_spanned};
use tap::Pipe;


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

    fn compile_assign(&mut self, place: Place, value: ValueIn) -> Result<Block> {
        if !place.projection.is_empty() {
            todo!()
        }
        let decl = self.body.local_decls.get(place.local).unwrap();
        let node = self.graph.insert(Node::new(node_set_local(compile_ty(self.compiler, decl.source_info.span, decl.ty)?)));
        self.graph.connect_value(Connection(*self.locals.get(place.local).unwrap(), 0), Connection(node, 0));
        self.graph.set_value_in(Connection(node, 1), value);
        Ok(Block::singleton(node, 0))
    }

    fn compile_assign_rvalue(&mut self, place: Place<'tcx>, value: &Rvalue<'tcx>, span: Span) -> Result<Block> {
        let ty = value.ty(&self.body.local_decls, self.tcx);
        let value_in = match value {
            Rvalue::Use(op, _) => self.compile_operand(op, span)?,
            Rvalue::BinaryOp(op, v) => {
                let ty0 = v.0.ty(&self.body.local_decls, self.tcx);
                let kind0 = compile_ty(self.compiler, v.0.span(&self.body.local_decls), ty0)?;
                let node = self.graph.insert(Node::new(match op {
                    BinOp::Add | BinOp::AddUnchecked | BinOp::AddWithOverflow =>
                        node_add(kind0),
                    BinOp::Sub | BinOp::SubUnchecked | BinOp::SubWithOverflow =>
                        node_subtract(kind0),
                    BinOp::Mul | BinOp::MulUnchecked | BinOp::MulWithOverflow =>
                        node_multiply(kind0),
                    BinOp::Div => {
                        if kind0.is::<ValueInt>() {
                            self.tcx.dcx().span_warn(span, "Div is slow and big, see `divide`");
                        }
                        node_divide(kind0)
                    },
                    BinOp::Rem => NODE_MODULO.clone(),
                    BinOp::BitXor => if ty.is_bool() { NODE_XOR.clone() } else { NODE_BITWISE_XOR.clone() },
                    BinOp::BitAnd => if ty.is_bool() { NODE_AND.clone() } else { NODE_BITWISE_AND.clone() },
                    BinOp::BitOr => if ty.is_bool() { NODE_OR.clone() } else { NODE_BITWISE_OR.clone() },
                    BinOp::Eq => node_equal(kind0),
                    BinOp::Shl | BinOp::ShlUnchecked => NODE_LEFT_SHIFT.clone(),
                    BinOp::Shr | BinOp::ShrUnchecked => return self.compile_call(span, find_lib_fn(self.compiler, "<i32 as rust2genshin_lib::math::I32>::shr")?, &[dummy_spanned(v.0.clone()), dummy_spanned(v.1.clone())], place),
                    BinOp::Lt => node_less_than(kind0),
                    BinOp::Le => node_less_equal(kind0),
                    BinOp::Ne => todo!(),
                    BinOp::Ge => node_greater_equal(kind0),
                    BinOp::Gt => node_greater_than(kind0),
                    | BinOp::Cmp
                    | BinOp::Offset => todo!("{:?}", op),
                }));
                let a = self.compile_operand(&v.0, span)?;
                let b = self.compile_operand(&v.1, span)?;
                self.graph.set_value_in(Connection(node, 0), a);
                self.graph.set_value_in(Connection(node, 1), b);
                ValueIn::link(Connection(node, 0).into())
            }
            Rvalue::UnaryOp(op, v) => {
                let ty = v.ty(&self.body.local_decls, self.tcx);
                let _kind = compile_ty(self.compiler, v.span(&self.body.local_decls), ty)?;
                let node = self.graph.insert(Node::new(match op {
                    UnOp::Not => if ty.is_bool() {
                        NODE_NOT.clone()
                    } else {
                        NODE_BITWISE_NOT.clone()
                    },
                    UnOp::Neg => return self.compile_assign_rvalue(place, &Rvalue::BinaryOp(BinOp::Sub, (Operand::Constant(ConstOperand {
                            span: DUMMY_SP,
                            user_ty: None,
                            const_: Const::Val(ConstValue::Scalar(Scalar::from_i32(0)), if ty.is_floating_point() { self.tcx.types.f32 } else { self.tcx.types.i32 }),
                        }.into()), v.clone()).into()), span),
                    UnOp::PtrMetadata => todo!(),
                }));
                let value = self.compile_operand(v, span)?;
                self.graph.set_value_in(Connection(node, 0), value);
                ValueIn::link(Connection(node, 0).into())
            },
            Rvalue::Reborrow(t, _, p) => return if t.is_str() {
                self.compile_assign_rvalue(place, &Rvalue::Use(Operand::Copy(*p), WithRetag::No), span)
            } else {
                self.span_err(span, "Reborrow from raw ptr is unsupported")
            },
            Rvalue::Ref(_, _, _) => panic!(),
            Rvalue::RawPtr(_, p) => {
                let Some(ProjectionElem::Deref) = p.projection.last() else {
                    return self.span_err(span, format!("RawPtr rvalue is unsupported: {p:?}"))?;
                };
                return self.compile_assign_rvalue(place, &Rvalue::Use(Operand::Copy(Place { local: p.local, projection: self.tcx.mk_place_elems(&p.projection[0..p.projection.len() - 1]) }), WithRetag::No), span);
            },
            Rvalue::Repeat(_, _)
            | Rvalue::ThreadLocalRef(_)
            | Rvalue::Cast(_, _, _)
            | Rvalue::Discriminant(_)
            | Rvalue::Aggregate(_, _)
            | Rvalue::CopyForDeref(_)
            | Rvalue::WrapUnsafeBinder(_, _)
                => todo!("{:?}", value),
        };
        self.compile_assign(place, value_in)
    }

    fn compile_operand(&mut self, op: &Operand<'tcx>, span: Span) -> Result<ValueIn> {
        Ok(match op {
            Operand::Copy(p) | // TODO
            Operand::Move(p) => {
                if !p.projection.is_empty() {
                    todo!("struct is still unsupported")
                }
                ValueIn::link(Connection(*self.locals.get(p.local).unwrap(), 1).into())
            }

            Operand::Constant(co) => {
                let ty = co.ty();
                let v = co.const_.eval(self.tcx, TypingEnv::fully_monomorphized(), co.span).map_err(|_| self.get_tcx().dcx().span_err(co.span, format!("Unsupported const eval: {:?}", co.const_)))?;
                ValueIn::value(match &ty.kind() {
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
                        TyKind::Ref(_, e, _) => {
                            if e.is_str() {
                                let ConstValue::Slice { alloc_id, meta: len } = v else { panic!("{v:?}") };
                                ValueString(match str::from_utf8(self.tcx.global_alloc(alloc_id).unwrap_memory().0.get_bytes_unchecked(AllocRange { start: Size::ZERO, size: Size::from_bytes(len) })) {
                                    Ok(s) => s.to_string(),
                                    Err(e) => return self.span_err(co.span, e.to_string()),
                                }).into()
                            } else {
                                return self.span_err(co.span, format!("Unsupported const ref: {:?}", ty));
                            }
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
                        TyKind::Tuple(_) |
                        TyKind::Alias(_, _) |
                        TyKind::Param(_) |
                        TyKind::Bound(_, _) |
                        TyKind::Placeholder(_) |
                        TyKind::Infer(_) |
                        TyKind::Error(_) => return self.span_err(co.span, format!("Unsupported const: {:?} = {:?}", ty, v)),
                    })
            }
            Operand::RuntimeChecks(_) => ValueIn::value(ValueBool(false).into()),
        })
    }

    pub(crate) fn compile_terminator(
        &mut self,
        blocks: &IndexVec<BasicBlock, Block>,
        terminator: &Terminator<'tcx>,
    ) -> Result<Connection> {
        Ok(match &terminator.kind {
            TerminatorKind::Return => Block::nop(self.graph).pipe(|block| {
                self.graph.export_control_out(block.end, 0);
                block.begin
            }),
            TerminatorKind::Goto { target } => {
                let result = Block::nop(self.graph);
                self.graph.connect_control(result.end, blocks.get(*target).unwrap().begin);
                result.begin
            },
            TerminatorKind::Assert { target, .. } => {
                self.tcx.dcx().span_note(terminator.source_info.span, "Ignored assert");
                let result = Block::nop(self.graph); // same as goto
                self.graph.connect_control(result.end, blocks.get(*target).unwrap().begin);
                result.begin
            },
            TerminatorKind::Call { func, target, args, destination, .. } => {
                let result = self.compile_call(terminator.source_info.span, self.find_fn(func)?, args, *destination)?;
                if let Some(target) = target {
                    self.graph.connect_control(result.end, blocks.get(*target).unwrap().begin);
                }
                result.begin
            }
            TerminatorKind::TailCall { func, args, .. } => self.compile_call(terminator.source_info.span, self.find_fn(func)?, args, Place::return_place())?.pipe(|block| {
                self.graph.export_control_out(block.end, 0);
                block.begin
            }),
            TerminatorKind::SwitchInt { discr, targets, .. } => {
                let node = if discr.ty(&self.body.local_decls, self.tcx).is_bool() {
                    let node = self.graph.insert(Node::new(NODE_IF.clone()));
                    self.graph.connect_control(Connection(node, 0), blocks[targets.target_for_value(1u128)].begin);
                    self.graph.connect_control(Connection(node, 1), blocks[targets.target_for_value(0u128)].begin);
                    node
                } else {
                    if targets.all_targets().len() > 100 { // limited by Genshin Impact
                        return self.span_err(terminator.source_info.span, format!("Too many cases: {}", targets.all_targets().len()));
                    }
                    let node = self.graph.insert(Node::new(node_switch(ValueInt::def(), targets.all_values().len())));
                    self.graph.set_default(Connection(node, 1), ValueIntList(targets.all_values().iter().map(|x| ScalarInt::try_from_int(x.0 as u64, Size::from_bytes(4)).unwrap().to_i32()).collect()).into());
                    self.graph.connect_control(Connection(node, 0), blocks[targets.otherwise()].begin);
                    for i in 0..targets.all_values().len() {
                        self.graph.connect_control(Connection(node, 1 + i), blocks[targets.all_targets()[i]].begin);
                    }
                    node
                };
                let value = self.compile_operand(discr, terminator.source_info.span)?;
                self.graph.set_value_in(Connection(node, 0), value);
                Connection(node, 0)
            },
            TerminatorKind::Drop { .. } => todo!(),
            other => return self.span_err(
                terminator.source_info.span,
                format!("Unsupported terminator: {}", other.name()),
            ),
        })
    }

    fn find_fn(&self, operand: &Operand<'tcx>) -> Result<Instance<'tcx>> {
        Ok(match operand {
            Operand::Copy(_) | Operand::Move(_) | Operand::RuntimeChecks(_) =>
                return self.span_err(operand.span(&self.body.local_decls), format!("Unsupported call: {:?}", operand)),
            Operand::Constant(func) => {
                match func.const_ {
                    Const::Ty(_, _) |
                    Const::Unevaluated(_, _) =>
                        match func.const_.eval(self.tcx, TypingEnv::fully_monomorphized(), func.span).map_err(|_| self.get_tcx().dcx().span_err(func.span, format!("Unsupported call const: {:?}", func.const_)))? {
                            ConstValue::Scalar(sc) => match sc {
                                Scalar::Int(_) => panic!("int"),
                                Scalar::Ptr(p, _) => {
                                    let (p, o) = p.into_raw_parts();
                                    if o != Size::ZERO {
                                        return self.span_err(func.span, format!("Unsupported call const Indirect offset: {:?}", sc));
                                    }
                                    match self.tcx.global_alloc(p.alloc_id()) {
                                        GlobalAlloc::Function { instance } => instance,
                                        other => return self.span_err(func.span, format!("Unsupported call const Indirect: {:?}", other)),
                                    }
                                },
                            },
                            other => panic!("{:?}", other),
                        },
                    Const::Val(val, ty) => {
                        match val {
                            ConstValue::Scalar(_) |
                            ConstValue::Slice { .. } => return self.span_err(func.span, format!("Unsupported call const val: {:?}", val)),
                            ConstValue::ZeroSized => {
                                match ty.kind() {
                                    TyKind::FnDef(def_id, b) => Instance::try_resolve(self.tcx, TypingEnv::fully_monomorphized(), *def_id, self.tcx.normalize_erasing_late_bound_regions(TypingEnv::fully_monomorphized(), *b))?.unwrap(),
                                    _ => return self.span_err(func.span, format!("Unsupported call const ty: {:?}", ty)),
                                }
                            }
                            ConstValue::Indirect { alloc_id, offset } => {
                                self.tcx.dcx().note("ConstValue::Indirect");
                                if offset != Size::ZERO {
                                    return self.span_err(func.span, format!("Unsupported call const Indirect offset: {:?}", offset));
                                }
                                match self.tcx.global_alloc(alloc_id) {
                                    GlobalAlloc::Function { instance } => instance,
                                    other => return self.span_err(func.span, format!("Unsupported call const Indirect: {:?}", other)),
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    fn compile_call(&mut self, span: Span, func: Instance<'tcx>, args: &[Spanned<Operand<'tcx>>], destination: Place<'tcx>) -> Result<Block> {
        let sig = self.tcx.normalize_erasing_late_bound_regions(TypingEnv::fully_monomorphized(), self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), self.tcx.fn_sig(func.def_id()).instantiate(self.tcx, func.args)));
        let params: Vec<AnyValue> = sig.inputs().iter().map(|x| compile_ty(self.compiler, span, *x)).collect::<Result<_>>()?;
        let ret: Vec<AnyValue> = Some(sig.output()).filter(|x| !is_unit(*x)).iter().map(|x| compile_ty(self.compiler, span, *x)).collect::<Result<_>>()?;
        let node = self.graph.insert(Node::new(if let Some(native) = self.compile_native_call(span, func, params.clone(), ret.clone()) {
            native?
        } else {
            node_composite(self.compiler.touch_fn(func)?, 1, 1, params, ret)
        }));
        for (i, Spanned { node: a, span }) in args.iter().enumerate() {
            let value = self.compile_operand(a, *span)?;
            self.graph.set_value_in(Connection(node, i), value);
        }
        let mut block = match self.graph.get_node(node).kind.controls_in_num {
            0 => Block::nop(self.graph),
            1 => Block::singleton(node, 0),
            other => return self.span_err(span, format!("Unsupported call controls: {other}")),
        };
        if !is_unit(sig.output()) {
            let value = self.compile_assign(destination, ValueIn::link(Connection(node, 0).into()))?;
            block.extend(self.graph, value);
        }
        Ok(block)
    }
}
