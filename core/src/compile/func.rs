use crate::asset::value::{AnyValue, ValueBool, ValueFloat, ValueInt, ValueIntList, ValueString, ValueStruct};

use super::*;
use crate::asset::node_graph::ValueIn;
use crate::asset::node_graph::arithmetic::{node_add, node_convert_type, node_divide, node_equal, node_greater_equal, node_greater_than, node_less_equal, node_less_than, node_multiply, node_subtract, NODE_AND, NODE_ASSEMBLE_STRUCT, NODE_BITWISE_AND, NODE_BITWISE_NOT, NODE_BITWISE_OR, NODE_BITWISE_XOR, NODE_LEFT_SHIFT, NODE_MODULO, NODE_NOT, NODE_OR, NODE_SPLIT_STRUCT, NODE_XOR};
use crate::asset::node_graph::composite::node_composite;
use crate::asset::node_graph::control::node_switch;
use crate::asset::node_graph::execution::node_set_local;
use rustc_abi::Size;
use rustc_index::IndexVec;
use rustc_middle::mir::interpret::{AllocRange, GlobalAlloc, Scalar};
use rustc_middle::mir::{AggregateKind, BasicBlock, BinOp, Const, ConstOperand, ConstValue, NonDivergingIntrinsic, Operand, Place, ProjectionElem, Rvalue, Statement, StatementKind, Terminator, TerminatorKind, UnOp, WithRetag};
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
            // For write paths, we don't currently support tuple field writes
            // (would require STRUCT_MODIFY rather than STRUCT_SPLIT). Defer.
            let span = self.body.local_decls[place.local].source_info.span;
            return self.span_err(
                span,
                format!("Write into place projection is unsupported: {:?}", place),
            );
        }
        let decl = self.body.local_decls.get(place.local).unwrap();
        let node = self.graph.insert(Node::new(node_set_local(self.compiler.compile_ty(decl.source_info.span, decl.ty)?)));
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
                let kind0 = self.compiler.compile_ty(v.0.span(&self.body.local_decls), ty0)?;
                let mut node = self.graph.insert(Node::new(match op {
                    BinOp::Add | BinOp::AddUnchecked | BinOp::AddWithOverflow =>
                        node_add(kind0),
                    BinOp::Sub | BinOp::SubUnchecked | BinOp::SubWithOverflow =>
                        node_subtract(kind0),
                    BinOp::Mul | BinOp::MulUnchecked | BinOp::MulWithOverflow =>
                        node_multiply(kind0),
                    BinOp::Div => {
                        if kind0.is::<ValueInt>() {
                            self.tcx.dcx().span_warn(span, "Div of i32 is slow and big, see `divide`");
                        }
                        node_divide(kind0)
                    },
                    BinOp::Rem => NODE_MODULO.clone(),
                    BinOp::BitXor => if ty.is_bool() { NODE_XOR.clone() } else { NODE_BITWISE_XOR.clone() },
                    BinOp::BitAnd => if ty.is_bool() { NODE_AND.clone() } else { NODE_BITWISE_AND.clone() },
                    BinOp::BitOr => if ty.is_bool() { NODE_OR.clone() } else { NODE_BITWISE_OR.clone() },
                    BinOp::Eq | BinOp::Ne => node_equal(kind0),
                    BinOp::Shl | BinOp::ShlUnchecked => NODE_LEFT_SHIFT.clone(),
                    BinOp::Shr | BinOp::ShrUnchecked => return self.compile_call(span, self.compiler.find_lib_fn("<i32 as rust2genshin_lib::math::I32>::shr")?, &[dummy_spanned(v.0.clone()), dummy_spanned(v.1.clone())], place),
                    BinOp::Lt => node_less_than(kind0),
                    BinOp::Le => node_less_equal(kind0),
                    BinOp::Ge => node_greater_equal(kind0),
                    BinOp::Gt => node_greater_than(kind0),
                    | BinOp::Cmp
                    | BinOp::Offset => todo!("{:?}", op),
                }));
                let a = self.compile_operand(&v.0, span)?;
                let b = self.compile_operand(&v.1, span)?;
                self.graph.set_value_in(Connection(node, 0), a);
                self.graph.set_value_in(Connection(node, 1), b);
                if matches!(op, BinOp::Ne) {
                    // ! (a == b) — invert the equal node's bool output
                    let not_node = self.graph.insert(Node::new(NODE_NOT.clone()));
                    self.graph.connect_value(Connection(node, 0), Connection(not_node, 0));
                    node = not_node;
                }
                ValueIn::link(Connection(node, 0).into())
            }
            Rvalue::UnaryOp(op, v) => {
                let ty = v.ty(&self.body.local_decls, self.tcx);
                let _kind = self.compiler.compile_ty(v.span(&self.body.local_decls), ty)?;
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
            Rvalue::Cast(kind, op, target_ty) => {
                let from_ty = op.ty(&self.body.local_decls, self.tcx);
                let from_kind = self.compiler.compile_ty(span, from_ty)?;
                let to_kind = self.compiler.compile_ty(span, *target_ty)?;
                if from_kind.is_instance(&to_kind) {
                    // No-op cast (e.g. i32 as isize, or identity casts inside expressions).
                    self.compile_operand(op, span)?
                } else {
                    let Some(node) = node_convert_type(from_kind, to_kind) else {
                        return self.span_err(span, format!("Unsupported cast ({kind:?}) {from_ty:?} → {target_ty:?}"));
                    };
                    let node = self.graph.insert(Node::new(node));
                    let v = self.compile_operand(op, span)?;
                    self.graph.set_value_in(Connection(node, 0), v);
                    ValueIn::link(Connection(node, 0).into())
                }
            }
            Rvalue::Aggregate(kind, fields) if matches!(**kind, AggregateKind::Tuple) => {
                let struct_id = self.compiler.intern_tuple_schema(span, ty)?.0;
                // Resolve each field's type up front. Reusing these AnyValues
                // for both the output placeholder and the input pins avoids
                // duplicate `compile_ty` calls and keeps the input/output type
                // lists in lock-step.
                let field_kinds: Vec<AnyValue> = fields
                    .iter()
                    .map(|f| {
                        self.compiler
                            .compile_ty(span, f.ty(&self.body.local_decls, self.tcx))
                    })
                    .collect::<Result<_>>()?;
                // Build a ValueStruct for the output type so STRUCT_ASSEMBLY's
                // return pin carries the right struct_id + field-type list.
                let placeholder: AnyValue =
                    ValueStruct::new(struct_id, field_kinds.clone()).into();
                // Clone the static STRUCT_ASSEMBLY template and resize its
                // dynamic input/output Vecs. STRUCT_ASSEMBLY's contract:
                //   - input pin 0 = struct_id selector (ValueInt)
                //   - input pins 1..N = field values (one per tuple element)
                //   - output pin 0 = the assembled struct (ValueStruct)
                // The static starts with empty values_in_types / values_out_types
                // (it's a "dynamic-pin" node), so we resize both `kind.*` and
                // `node.*` Vecs in lock-step, matching the pattern used by
                // NODE_SPLIT_STRUCT for dynamic-output nodes.
                let mut node_kind = NODE_ASSEMBLE_STRUCT.clone();
                let struct_id_selector: AnyValue = ValueInt(struct_id as i32).into();
                let mut values_in_types = Vec::with_capacity(1 + field_kinds.len());
                values_in_types.push(struct_id_selector.clone());
                values_in_types.extend(field_kinds.iter().cloned());
                node_kind.values_in_types = values_in_types;
                node_kind.selectors_in = vec![None; node_kind.values_in_types.len()];
                node_kind.values_out_types = vec![placeholder];
                node_kind.selectors_out = vec![None; node_kind.values_out_types.len()];
                let mut node = Node::new(node_kind);
                // Resize values_in / values_out after Node::new (which sizes
                // them from the empty Vecs in the static template).
                node.values_in
                    .resize(node.kind.values_in_types.len(), ValueIn::default());
                node.values_out
                    .resize(node.kind.values_out_types.len(), Vec::new());
                let node_ref = self.graph.insert(node);
                // Wire the struct_id selector at input pin 0.
                self.graph.set_value_in(
                    Connection(node_ref, 0),
                    ValueIn::value(ValueInt(struct_id as i32).into()),
                );
                // Wire the field operands to dynamic input pins 1..N+1.
                for (i, field) in fields.iter().enumerate() {
                    let v = self.compile_operand(field, span)?;
                    self.graph.set_value_in(Connection(node_ref, i + 1), v);
                }
                ValueIn::link(Connection(node_ref, 0).into())
            }
            Rvalue::Repeat(_, _)
            | Rvalue::ThreadLocalRef(_)
            | Rvalue::Discriminant(_)
            | Rvalue::Aggregate(_, _) // non-Tuple AggregateKind still panics
            | Rvalue::CopyForDeref(_)
            | Rvalue::WrapUnsafeBinder(_, _)
                => todo!("{:?}", value),
        };
        self.compile_assign(place, value_in)
    }

    fn compile_operand(&mut self, op: &Operand<'tcx>, span: Span) -> Result<ValueIn> {
        Ok(match op {
            Operand::Copy(p) |
            Operand::Move(p) => {
                if !p.projection.is_empty() {
                    return self.compile_operand_projection(*p, span);
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
                        TyKind::Adt(d, a) => {
                            if d.did().krate != self.compiler.lib {
                                return self.span_err(span, format!("Adt Const is still unsupported: {d:?} {a:?}"));
                            } else {
                                match self.tcx.def_path_str(d.did()).as_str() {
                                    "rust2genshin_lib::Guid" => ValueGuid(v.try_to_scalar_int().unwrap().to_i64()).into(),
                                    _ => panic!("adt: {d:?} {a:?}"),
                                }
                            }
                        },
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

    /// Resolve a Place with a non-empty projection chain to a `ValueIn`,
    /// inserting `STRUCT_SPLIT` nodes as needed.
    ///
    /// Only chains consisting entirely of `ProjectionElem::Field(i, _)` are
    /// supported. Other projection kinds (Deref, Index, ConstantIndex, etc.)
    /// trigger `span_err` and defer to follow-up sub-projects.
    fn compile_operand_projection(&mut self, place: Place<'tcx>, span: Span) -> Result<ValueIn> {
        use rustc_middle::mir::ProjectionElem;
        let local_ref = *self.locals.get(place.local).unwrap();
        let base_kind = self.compiler.compile_ty(
            span,
            self.body.local_decls[place.local].ty,
        )?;
        // The base local's output pin is pin 1 (per node_local's contract).
        let mut current_input = ValueIn::link(Connection(local_ref, 1).into());
        let mut current_kind: AnyValue = base_kind;
        for elem in place.projection {
            match elem {
                ProjectionElem::Field(field_idx, _) => {
                    // Build a fresh STRUCT_SPLIT node per field access. The engine
                    // treats pins 0..N-1 as the field outputs (one per struct field),
                    // even though NODE_SPLIT_STRUCT declares values_out_types=[] in
                    // the Rust `NodeKind` (dynamic-output node). We resize the
                    // node's `values_out` Vec to match the struct's field count
                    // so `connect_value`/`set_value_in` can write to pin `field_idx`.
                    let struct_field_count = match current_kind.downcast_ref::<ValueStruct>() {
                        Ok(s) => s.fields.len(),
                        Err(_) => return self.span_err(
                            span,
                            format!("Field access into non-struct: {:?}", current_kind),
                        ),
                    };
                    if field_idx.as_usize() >= struct_field_count {
                        return self.span_err(
                            span,
                            format!(
                                "Field index {} out of bounds for struct with {} field(s)",
                                field_idx.as_usize(),
                                struct_field_count
                            ),
                        );
                    }
                    let mut node = Node::new(NODE_SPLIT_STRUCT.clone());
                    // NODE_SPLIT_STRUCT declares `values_out_types=[]` (dynamic-output
                    // node). Populate `kind.values_out_types` with the struct's actual
                    // field types so downstream nodes can read `kind.values_out_types[pin]`
                    // to determine each output pin's type (e.g. `connect_value`'s type
                    // assertion at line 289 of node_graph/mod.rs).
                    node.kind.values_out_types = match current_kind.downcast_ref::<ValueStruct>() {
                        Ok(s) => s.fields.clone(),
                        Err(_) => return self.span_err(
                            span,
                            format!("Field access into non-struct: {:?}", current_kind),
                        ),
                    };
                    node.values_out.resize(struct_field_count, Vec::new());
                    let node_ref = self.graph.insert(node);
                    self.graph.set_value_in(Connection(node_ref, 0), current_input);
                    current_input = ValueIn::link(Connection(node_ref, field_idx.as_usize()).into());
                    // Advance the type tracker to the field's type.
                    current_kind = match current_kind.downcast_ref::<ValueStruct>() {
                        Ok(s) => s.fields[field_idx.as_usize()].clone(),
                        Err(_) => return self.span_err(
                            span,
                            format!("Field access into non-struct: {:?}", current_kind),
                        ),
                    };
                }
                other => return self.span_err(
                    span,
                    format!("Unsupported projection element in field access: {:?}", other),
                ),
            }
        }
        Ok(current_input)
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
        let params: Vec<AnyValue> = sig.inputs().iter().map(|x| self.compiler.compile_ty(span, *x)).collect::<Result<_>>()?;
        let ret: Vec<AnyValue> = Some(sig.output()).filter(|x| !is_unit(*x)).iter().map(|x| self.compiler.compile_ty(span, *x)).collect::<Result<_>>()?;
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
