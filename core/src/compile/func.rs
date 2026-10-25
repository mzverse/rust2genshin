use super::*;
use crate::compile::ir::{IrKind, IrNodeId, NodeKindIr, node_ir_assemble, node_ir_black_box, node_ir_copy, node_ir_destructure, node_ir_field_mut, node_ir_list_assemble, node_ir_modify_struct, node_ir_switch, node_ir_unreachable, node_ir_unsupported, struct_fields};
use crate::compile::native::native_const;
use crate::compile::place::{CompiledLocal, CompiledPlace, LocalRef, can_read_into, copy_until};
use crate::node::arithmetic::{NODE_AND, NODE_BITWISE_AND, NODE_BITWISE_NOT, NODE_BITWISE_OR, NODE_BITWISE_XOR, NODE_LEFT_SHIFT, NODE_NOT, NODE_OR, NODE_REM, NODE_XOR, node_add, node_cast, node_divide, node_equal, node_greater_equal, node_greater_than, node_less_equal, node_less_than, node_multiply, node_subtract};
use crate::node::list::MAX_ASSEMBLE_ELEMENTS;
use crate::node::{Link, Lower, NativeNodeId, ValueIn};
use crate::unwrap;
use crate::value::{NativeKind, NativeValue};
use either::Either;
use panic_context::panic_context;
use rustc_abi::{FieldIdx, Integer, IntegerType, Size, TagEncoding, VariantIdx, Variants};
use rustc_attr_ir::LangItem;
use rustc_index::{Idx, IndexVec};
use rustc_middle::mir::interpret::{AllocRange, GlobalAlloc, GlobalId, Scalar};
use rustc_middle::mir::{AggregateKind, BasicBlock, BinOp, BorrowKind, Const, ConstOperand, ConstValue, MutBorrowKind, NonDivergingIntrinsic, Operand, Place, PlaceTy, ProjectionElem, Rvalue, Statement, StatementKind, Terminator, TerminatorKind, UnOp, WithRetag};
use rustc_middle::ty::layout::{LayoutCx, PrimitiveExt, TyAndLayout};
use rustc_middle::ty::{AdtDef, FloatTy, InstanceKind, IntTy, PseudoCanonicalInput, ScalarInt, ShimKind, TyKind, TypingEnv, ValTree, Value};
use rustc_span::{DUMMY_SP, Span};
use tap::Pipe;

pub type NodeGraphIr = NodeGraph<IrNodeId, IrKind>;

pub struct CompilingFn<'tcx, 'a> {
    pub tcx: TyCtxt<'tcx>,
    pub func: Instance<'tcx>,
    pub compiler: &'a mut Compiler<'tcx>,
    pub graph: &'a mut NodeGraphIr,
    pub body: &'a Body<'tcx>,
    pub locals: &'a IndexVec<Local, CompiledLocal<LocalRef>>,
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
                StatementKind::SetDiscriminant { place, variant_index } => {
                    let ty = self.mono(place.ty(&self.body.local_decls, self.tcx).ty);
                    match ty.kind() {
                        TyKind::Adt(_, _) => self.tcx.dcx().span_fatal(statement.source_info.span, "setting discriminant of enum"),
                        TyKind::Coroutine(_, _) => {
                            let kind = self.compiler.compile_ty(statement.source_info.span, ty)?;
                            let node = self.graph.insert(node_ir_modify_struct(&self.compiler.target, &kind));
                            let st = self.compile_operand(&Operand::Copy(**place), statement.source_info.span)?;
                            let state = self.compiler.target.adts[unwrap!(&kind, IrKind::Adt)].discriminant.unwrap();
                            self.graph.set_value_in(Link::node(node, 0), st);
                            self.graph.set_value_in(Link::node(node, 2 + state * 2), ValueIn::value((variant_index.index() as i32).into()));
                            self.graph.set_value_in(Link::node(node, 3 + state * 2), ValueIn::value(true.into()));
                            Block::singleton(node, 0).into()
                        },
                        _ => panic!(),
                    }
                },
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
                if matches!(ty0.kind(), TyKind::Uint(..)) {
                    panic!("{ty0:?}");
                }
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
                match op {
                    | BinOp::AddWithOverflow
                    | BinOp::SubWithOverflow
                    | BinOp::MulWithOverflow
                    => {
                        let tu = self.compiler.compile_ty(span, Ty::new_tup(self.tcx, &[ty0, self.tcx.types.bool]))?;
                        let n_ass = self.graph.insert(node_ir_assemble(&self.compiler.target, &tu));
                        self.graph.link_value(Link::node(node, 0), Link::node(n_ass, 0));
                        node = n_ass;
                    },
                    BinOp::Ne => {
                        // ! (a == b) — invert the equal node's bool output
                        let n_not = self.graph.insert(node_ir_native(NODE_NOT.clone()));
                        self.graph.link_value(Link::node(node, 0), Link::node(n_not, 0));
                        node = n_not;
                    }
                    _ => (),
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
            Rvalue::Ref(_region, BorrowKind::Mut { .. }, place) => match self.compile_place(*place, span)? {
                CompiledPlace::Local(CompiledLocal::Singleton(LocalRef { setter, getter, .. })) => {
                    let kind = self.compiler.compile_ty(span, ty)?;
                    let result = self.graph.insert(node_ir_assemble(&self.compiler.target, &kind));
                    self.graph.set_value_in(Link::node(result, 0), ValueIn::link(setter));
                    self.graph.set_value_in(Link::node(result, 1), ValueIn::link(getter));
                    ValueIn::link(Link::node(result, 0))
                },
                CompiledPlace::Field(owner, adt, idx) => {
                    let node = self.graph.insert(node_ir_field_mut(&self.compiler.target, &adt, idx));
                    let v = owner.getter(
                        &self.compiler.target,
                        self.graph,
                        &IrKind::Adt(adt),
                    );
                    self.graph.set_value_in(Link::node(node, 0), v);
                    ValueIn::link(Link::node(node, 0))
                },
                _ => {
                    self.tcx.dcx().span_err(span, "`&mut` 只支持整个局部变量或结构体字段,不支持列表下标 / 扁平引用");
                    todo!()
                },
            },
            Rvalue::Ref(_region, _, place) => {
                // 共享借用:类型是普通值就 Copy 一份,是可变视图就原样给
                // (`&T` 在 `T` 非 `Freeze` 时就是 `PartialMut`)。
                let ty = self.mono(value.ty(&self.body.local_decls, self.tcx));
                let kind = self.compiler.compile_ty(span, ty)?;
                let shape = self.compiler.compile_ty(span, self.mono(place.ty(&self.body.local_decls, self.tcx).ty))?;
                let place = self.compile_place(*place, span)?;
                self.value_of(&place, &shape, &kind, span)?
            },
            Rvalue::RawPtr(_, p) => {
                let Some(ProjectionElem::Deref) = p.projection.last() else {
                    return self.helper().span_err(span, format!("RawPtr rvalue is still unsupported: {p:?}"))?;
                };
                return self.compile_rvalue(&Rvalue::Use(Operand::Copy(Place { local: p.local, projection: self.tcx.mk_place_elems(&p.projection[0..p.projection.len() - 1]) }), WithRetag::No), span);
            },
            Rvalue::Cast(kind, op, target_ty) => {
                // FIXME: f32 to i32
                let from_ty = self.mono(op.ty(&self.body.local_decls, self.tcx));
                let from_kind = self.compiler.compile_ty(span, from_ty)?;
                let to_kind = self.compiler.compile_ty(span, *target_ty)?;
                if from_kind == to_kind {
                    // No-op cast (e.g. i32 as isize, or identity casts inside expressions).
                    self.compile_operand(op, span)?
                } else {
                    let from_kind = from_kind.to_native().unwrap_or_else(|| self.tcx.dcx().span_fatal(span, format!("{from_ty:?}")));
                    let to_kind = to_kind.to_native().unwrap_or_else(|| self.tcx.dcx().span_fatal(span, format!("{target_ty:?}")));
                    let Some(node) = node_cast(from_kind.clone(), to_kind.clone()) else {
                        return self.helper().span_err(span, format!("Unsupported cast ({kind:?}) {from_kind:?} → {to_kind:?}"));
                    };
                    let node = self.graph.insert(node_ir_native(node));
                    let v = self.compile_operand(op, span)?;
                    self.graph.set_value_in(Link::node(node, 0), v);
                    ValueIn::link(Link::node(node, 0))
                }
            }
            Rvalue::Aggregate(kind, fields) => match **kind {
                AggregateKind::Array(_) => {
                    // `[a, b, c]`:元素个数 + 每个元素 → 拼装列表节点。
                    let TyKind::Array(elem_ty, _) = ty.kind() else {
                        return self.helper().span_err(span, format!("Array aggregate without array type: {ty:?}"));
                    };
                    let elem = self.compiler.compile_ty(span, *elem_ty)?;
                    if fields.len() > MAX_ASSEMBLE_ELEMENTS {
                        return self.helper().span_err(span, format!("Array of {} elements exceeds the {MAX_ASSEMBLE_ELEMENTS}-element limit of the assemble-list node", fields.len()));
                    }
                    let node = self.graph.insert(node_ir_list_assemble(elem, fields.len()));
                    for (i, x) in fields.iter().enumerate() {
                        let v = self.compile_operand(x, span)?;
                        self.graph.set_value_in(Link::node(node, i), v);
                    }
                    ValueIn::link(Link::node(node, 0))
                },
                AggregateKind::Adt(..) if unwrap!(ty.kind(), TyKind::Adt(d, _) => d).repr().transparent() =>
                    self.compile_operand(&fields[FieldIdx::new(0)], span)?,
                AggregateKind::Adt(_, id, _, _, _) if let (d, a) = unwrap!(ty.kind(), TyKind::Adt(d, a) => (d, a)) && d.is_enum() => {
                    if self.compiler.get_default_some(*d, a)?.is_some() {
                        self.compile_operand(&fields[FieldIdx::new(0)], span)?
                    } else {
                        let kind = self.compiler.compile_ty(span, ty)?;
                        let node = self.graph.insert(node_ir_assemble(&self.compiler.target, &kind));
                        self.graph.set_default(Link::node(node, 0), Some((id.index() as i32).into()));
                        for (i, x) in self.compiler.target.adts[&unwrap!(kind, IrKind::Adt)].variants[id.index()].clone().into_iter().enumerate() {
                            let v = self.compile_operand(&fields[FieldIdx::new(i)], span)?;
                            self.graph.set_value_in(Link::node(node, x), v);
                        }
                        ValueIn::link(Link::node(node, 0))
                    }
                }
                AggregateKind::CoroutineClosure(_, _) => todo!(),
                AggregateKind::RawPtr(_, _) => todo!(),
                _ => {
                    let kind = self.compiler.compile_ty(span, ty)?;
                    let node = self.graph.insert(node_ir_assemble(&self.compiler.target, &kind));
                    for (i, x) in fields.iter().enumerate() {
                        let v = self.compile_operand(x, span)?;
                        self.graph.set_value_in(Link::node(node, i), v);
                    }
                    ValueIn::link(Link::node(node, 0))
                }
            },
            Rvalue::Discriminant(from) => {
                let ty = self.place_ty(*from).ty;
                if let TyKind::Adt(d, a) = ty.kind() {
                    if d.repr().int == Some(IntegerType::Fixed(Integer::I32, true)) {
                        return self.compile_operand(&Operand::Copy(*from), span);
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
                        return Ok(ValueIn::link(Link::node(node_cast, 0)));
                    }
                }
                let ty = self.mono(from.ty(&self.body.local_decls, self.tcx).ty);
                let kind = self.compiler.compile_ty(span, ty)?;
                let node = self.graph.insert(node_ir_destructure(&self.compiler.target, &kind));
                let v = self.compile_operand(&Operand::Copy(*from), span)?;
                self.graph.set_value_in(Link::node(node, 0), v);
                let state = self.compiler.target.adts[unwrap!(&kind, IrKind::Adt)].discriminant.unwrap();
                ValueIn::link(Link::node(node, state))
            },
            Rvalue::Repeat(value, len) => {
                // `[x; N]`:同一个值接到拼装列表的 N 个元素引脚上。
                let TyKind::Array(elem_ty, _) = ty.kind() else {
                    return self.helper().span_err(span, format!("Repeat rvalue without array type: {ty:?}"));
                };
                let Some(n) = len.try_to_target_usize(self.tcx) else {
                    self.tcx.dcx().span_fatal(span, format!("Repeat with unknown length: {ty:?}"));
                };
                let n = n as usize;
                if n > MAX_ASSEMBLE_ELEMENTS {
                    return self.helper().span_err(span, format!("Array of {n} elements exceeds the {MAX_ASSEMBLE_ELEMENTS}-element limit of the assemble-list node"));
                }
                let elem = self.compiler.compile_ty(span, *elem_ty)?;
                let v = self.compile_operand(value, span)?;
                let node = self.graph.insert(node_ir_list_assemble(elem, n));
                for i in 0..n {
                    self.graph.set_value_in(Link::node(node, i), v.clone());
                }
                ValueIn::link(Link::node(node, 0))
            },
            Rvalue::ThreadLocalRef(_)
            | Rvalue::CopyForDeref(_)
            | Rvalue::WrapUnsafeBinder(_, _)
            => todo!("{:?}", value),
        })
    }

    fn compile_assign_rvalue(&mut self, place: Place<'tcx>, value: &Rvalue<'tcx>, span: Span) -> Result<Block> {
        let value_in = self.compile_rvalue(value, span)?;
        self.compile_assign(place, span, value_in)
    }

    /// 把 MIR 里的常量求值成 valtree。
    ///
    /// valtree 是 rustc 自带的「结构化常量值」(`rustc_middle::ty::consts::valtree`):
    /// 标量是叶子,tuple / struct / 数组 / enum 是分支,分支里是各字段的 `ty::Const`。
    /// 结构和类型一一对应,不用再按布局偏移自己去拆 `ConstValue`。
    /// 它由 `rustc_const_eval` 的常量求值器产生,和 miri 没有关系。
    fn const_to_value(&mut self, c: Const<'tcx>, span: Span) -> Result<Value<'tcx>> {
        let typing_env = TypingEnv::fully_monomorphized();
        match c {
            Const::Ty(ty, ct) => match ct.try_to_value() {
                Some(v) => Ok(Value { ty, valtree: v.valtree }),
                None => self.helper().span_err(span, format!("Unevaluated const in MIR: {ct:?}")),
            },
            Const::Unevaluated(u, ty) => {
                let cid = match Instance::try_resolve(self.tcx, typing_env, u.def, u.args) {
                    Ok(Some(instance)) => GlobalId { instance, promoted: u.promoted },
                    _ => return self.helper().span_err(span, format!("Unresolved const: {u:?}")),
                };
                match self.tcx.const_eval_global_id_for_typeck(typing_env, cid, span) {
                    Ok(Ok(valtree)) => Ok(Value { ty, valtree }),
                    Ok(Err(no_valtree)) => self.helper().span_err(span, format!("Const of type {no_valtree:?} has no valtree representation")),
                    Err(e) => self.helper().span_err(span, format!("Unsupported const eval {u:?}: {e:?}")),
                }
            },
            // MIR 优化(const-prop、常量提升)会把已经算好的值直接塞成内存布局,
            // 没有现成的 valtree,只能按类型结构把内存重新解释回来。
            Const::Val(v, ty) => Ok(Value { ty, valtree: self.const_value_to_valtree(ty, v, span)? }),
        }
    }

    /// 从内存布局还原 valtree,只用于 `Const::Val`(见 [`Self::const_to_value`])。
    /// 这里只做「内存 → 树」的翻译,节点构造仍然由 [`Self::compile_const`] 按结构统一处理。
    fn const_value_to_valtree(&mut self, ty: Ty<'tcx>, v: ConstValue, span: Span) -> Result<ValTree<'tcx>> {
        /// 聚合值在 `ConstValue` 里是「一整块内存 + 起点偏移」,字段就是往后挪一点。
        /// 但只有一个非 ZST 字段的结构体/元组会被 rustc 压成单个标量(见
        /// `rustc_const_eval` 的 `valtree_to_const_value` 快速路径),全 ZST 的聚合则是
        /// `ZeroSized` —— 这两种情况整个值就是字段自己的值,原样传下去。
        fn field_value(v: ConstValue, offset: Size) -> ConstValue {
            match v {
                ConstValue::Indirect { alloc_id, offset: base } => ConstValue::Indirect { alloc_id, offset: base + offset },
                ConstValue::Scalar(_) |
                ConstValue::ZeroSized => v,
                _ => panic!("unexpected aggregate const: {v:?}"),
            }
        }
        let typing_env = TypingEnv::fully_monomorphized();
        let tcx = self.tcx;
        let ty_layout = tcx.layout_of(PseudoCanonicalInput {
            typing_env,
            value: ty,
        }).unwrap();
        let layout = ty_layout.layout;
        let read_scalar = |v: ConstValue| match v {
            ConstValue::Scalar(s) => s.try_to_scalar_int().ok(),
            ConstValue::Indirect { alloc_id, offset } => {
                let alloc = tcx.global_alloc(alloc_id).unwrap_memory().inner();
                alloc.read_scalar(&tcx, AllocRange {
                    start: offset,
                    size: layout.size,
                }, false).ok()?.try_to_scalar_int().ok()
            },
            _ => None,
        };
        Ok(match ty.kind() {
            | TyKind::Bool
            | TyKind::Int(_)
            | TyKind::Uint(_)
            | TyKind::Float(_)
            | TyKind::Char
            => ValTree::from_scalar_int(tcx, read_scalar(v).expect("scalar const")),
            // `&str` 和 `&[u8]` 的 valtree 就是一串 u8 分支(和 rustc 的 `try_to_raw_bytes` 对应)
            TyKind::Ref(_, e, _) if e.is_str() || matches!(e.kind(), TyKind::Slice(_)) =>
                ValTree::from_raw_bytes(tcx, v.try_get_slice_bytes_for_diagnostics(tcx).expect("slice const")),
            // `NonZero<T>` 在这版 rustc 里是 pattern type,valtree 和底层类型一样
            TyKind::Pat(e, _) => return self.const_value_to_valtree(*e, v, span),
            // 透明包装(`Guid(NonZero<i64>)` 之类):valtree 仍是单字段分支,内存就是字段自己的内存
            TyKind::Adt(d, a) if d.repr().transparent() => {
                let inner = tcx.normalize_erasing_regions(typing_env, d.non_enum_variant().single_field().ty(tcx, a));
                self.const_value_branch(span, vec![inner], |_| v)?
            },
            // 枚举的 valtree 是 `[variant 下标(u32)] + 该 variant 的字段`,
            // 下标得从内存里的 tag 反推(见 [`Self::const_variant`])。
            TyKind::Adt(d, a) if d.is_enum() => {
                let variant = self.const_variant(ty, &ty_layout, v, *d, span)?;
                let variant_layout = ty_layout.for_variant(&LayoutCx::new(tcx, typing_env), variant);
                let tys = d.variant(variant).fields.iter().map(|x| tcx.normalize_erasing_regions(typing_env, x.ty(tcx, a))).collect::<Vec<_>>();
                let mut fields = vec![rustc_middle::ty::Const::new_value(tcx, ValTree::from_scalar_int(tcx, variant.as_u32().into()), tcx.types.u32)];
                for (i, x) in tys.into_iter().enumerate() {
                    let v = self.const_value_to_valtree(x, field_value(v, variant_layout.fields.offset(i)), span)?;
                    fields.push(rustc_middle::ty::Const::new_value(tcx, v, x));
                }
                ValTree::from_branches(tcx, fields)
            },
            TyKind::Adt(d, a) => {
                let fields = d.non_enum_variant().fields.iter().map(|x| tcx.normalize_erasing_regions(typing_env, x.ty(tcx, a))).collect::<Vec<_>>();
                self.const_value_branch(span, fields, |i| field_value(v, layout.fields.offset(i)))?
            },
            TyKind::Tuple(tys) => {
                let tys = tys.into_iter().collect::<Vec<_>>();
                self.const_value_branch(span, tys, |i| field_value(v, layout.fields.offset(i)))?
            },
            // `[元素; N]`:元素都紧挨着放,步长就是元素大小
            TyKind::Array(ele, len) => {
                let Some(n) = len.try_to_target_usize(tcx) else {
                    self.tcx.dcx().span_fatal(span, format!("Array const with unknown length: {ty:?}"));
                };
                let stride = tcx.layout_of(PseudoCanonicalInput {
                    typing_env,
                    value: *ele,
                }).unwrap().layout.size;
                self.const_value_branch(span, vec![*ele; n as usize], |i| field_value(v, stride * i as u64))?
            },
            _ => return self.helper().span_err(span, format!("Unsupported const value: {ty:?} = {v:?}")),
        })
    }

    fn const_value_branch(&mut self, span: Span, tys: Vec<Ty<'tcx>>, value: impl Fn(usize) -> ConstValue) -> Result<ValTree<'tcx>> {
        let fields = tys.into_iter().enumerate().map(|(i, x)| {
            let v = self.const_value_to_valtree(x, value(i), span)?;
            Ok(rustc_middle::ty::Const::new_value(self.tcx, v, x))
        }).collect::<Result<Vec<_>>>()?;
        Ok(ValTree::from_branches(self.tcx, fields))
    }

    /// 从值的布局里读出 enum 的 variant 下标,和 rustc 的 `read_discriminant` 同一套逻辑:
    ///
    /// - `TagEncoding::Direct`:tag 就是判别式的位模式,直接对判别式表;
    /// - `TagEncoding::Niche`:判别式藏在别的字段的 niche 里,按
    ///   `tag = (variant_index - niche_variants.start).wrapping_add(niche_start)` 反推,
    ///   落在 niche 范围外就是没有被 niche 编码的那个 variant。
    fn const_variant(&self, ty: Ty<'tcx>, layout: &TyAndLayout<'tcx>, v: ConstValue, d: AdtDef<'tcx>, span: Span) -> Result<VariantIdx> {
        /// 按 `size` 的位宽截断(回绕减法要在 tag 自己的位宽里做)。
        fn truncate(x: u128, size: Size) -> u128 {
            match size.bits() {
                128 => x,
                bits => x & ((1u128 << bits) - 1),
            }
        }
        let tcx = self.tcx;
        let (tag, tag_encoding, tag_field) = match layout.variants {
            Variants::Single { index } => return Ok(index),
            Variants::Multiple { tag, ref tag_encoding, tag_field, .. } => (tag, tag_encoding, tag_field),
            Variants::Empty => return self.helper().span_err(span, format!("Uninhabited enum const: {ty:?}")),
        };
        let tag_layout = tcx.layout_of(PseudoCanonicalInput {
            typing_env: TypingEnv::fully_monomorphized(),
            value: tag.primitive().to_int_ty(tcx),
        }).unwrap().layout;
        let tag_bits = match v {
            // 整个枚举就是一个标量时,这个标量本身就是 tag
            ConstValue::Scalar(s) => s.try_to_scalar_int().ok(),
            ConstValue::Indirect { alloc_id, offset } => {
                let alloc = tcx.global_alloc(alloc_id).unwrap_memory().inner();
                alloc.read_scalar(&tcx, AllocRange {
                    start: offset + layout.fields.offset(tag_field.index()),
                    size: tag_layout.size,
                }, false).ok().and_then(|x| x.try_to_scalar_int().ok())
            },
            _ => None,
        };
        let Some(bits) = tag_bits.map(|x| x.to_bits(tag_layout.size)) else {
            return self.helper().span_err(span, format!("Cannot read enum tag: {ty:?} = {v:?}"));
        };
        let variant = match *tag_encoding {
            // 判别式按位模式存,`-1i8` 就是 `0xFF`(见 `Discr::val`)
            TagEncoding::Direct => d.discriminants(tcx).find(|(_, x)| x.val == bits).map(|(i, _)| i),
            TagEncoding::Niche { untagged_variant, niche_variants, niche_start } => {
                let relative = truncate(bits.wrapping_sub(niche_start), tag_layout.size);
                let last = niche_variants.last.as_u32() - niche_variants.start.as_u32();
                Some(if relative <= last as u128 {
                    VariantIdx::from_u32(niche_variants.start.as_u32() + relative as u32)
                } else {
                    untagged_variant
                })
            },
        };
        match variant {
            Some(x) => Ok(x),
            None => self.helper().span_err(span, format!("Invalid enum tag {bits:#x}: {ty:?} = {v:?}")),
        }
    }

    fn compile_const(&mut self, ty: Ty<'tcx>, kind: &IrKind, v: Value<'tcx>, span: Span) -> Result<ValueIn> {
        let typing_env = TypingEnv::fully_monomorphized();
        // 透明包装(`Guid(NonZero<i64>)` 之类)在 valtree 里是单字段分支,先剥到真正的
        // 标量再取叶子;IR 的 kind 一路保持不变。
        if let TyKind::Adt(d, a) = ty.kind() && d.repr().transparent() {
            let inner = self.tcx.normalize_erasing_regions(typing_env, d.non_enum_variant().single_field().ty(self.tcx, a));
            let field = v.to_branch()[0].try_to_value().unwrap();
            return self.compile_const(inner, kind, Value { ty: inner, valtree: field.valtree }, span);
        }
        let leaf = v.try_to_leaf();
        if let IrKind::Native(kind) = kind && let Some(r) = leaf.and_then(|x| native_const(kind, x)) {
            return Ok(ValueIn::value(r));
        }
        Ok(ValueIn::value(match ty.kind() {
            TyKind::Bool => v.try_to_bool().unwrap().into(),
            TyKind::Int(t) => match t {
                IntTy::I32 => leaf.unwrap().to_i32().into(),
                IntTy::Isize => (leaf.unwrap().to_int(self.tcx.data_layout.pointer_size()) as i32).into(),
                IntTy::I8 |
                IntTy::I16 |
                IntTy::I64 |
                IntTy::I128 => return self.helper().span_err(span, format!("Unsupported const int: {:?}", ty)),
            },
            TyKind::Float(t) => match t {
                FloatTy::F32 => f32::from_bits(leaf.unwrap().to_bits(Size::from_bits(32)) as u32).into(),
                FloatTy::F16 |
                FloatTy::F64 |
                FloatTy::F128 => return self.helper().span_err(span, format!("Unsupported const float: {:?}", ty)),
            },
            TyKind::Str => panic!(),
            TyKind::Ref(_, e, _) => {
                if e.is_str() {
                    str::from_utf8(v.try_to_raw_bytes(self.tcx).unwrap()).unwrap().to_string().into()
                } else {
                    return self.helper().span_err(span, format!("Unsupported const ref: {:?}", ty));
                }
            },
            TyKind::Adt(d, a) if self.compiler.get_default_some(*d, a)?.is_some() => {
                let destructured = v.destructure_adt_const();
                let none = d.variant_index_with_id(self.tcx.lang_items().get(LangItem::OptionNone).unwrap());
                if destructured.variant == none {
                    return Ok(ValueIn::default());
                }
                let field = destructured.fields[0].try_to_value().unwrap();
                return self.compile_const(field.ty, kind, field, span);
            },
            // `#[repr(i32)]` / `#[native_enum]` 枚举在节点图里就是整数,常量取判别式。
            // valtree 里只有 variant 下标,判别式得回 ADT 定义查。
            TyKind::Adt(d, _a) if d.is_enum() => {
                let variant = v.destructure_adt_const().variant;
                let discr = d.discriminant_for_variant(self.tcx, variant);
                if let IrKind::Native(kind) = kind
                    && let Some(r) = native_const(kind, ScalarInt::try_from_uint(discr.val, discr.ty.primitive_size(self.tcx)).unwrap())
                {
                    return Ok(ValueIn::value(r));
                }
                return self.helper().span_err(span, "Enum const is still unsupported, see `black_box`"); // TODO
            },
            TyKind::Adt(..) | TyKind::Tuple(..) => {
                let ele: Vec<_> = match ty.kind() {
                    TyKind::Adt(d, a) => d.non_enum_variant().fields.iter().map(|x| self.tcx.normalize_erasing_regions(typing_env, x.ty(self.tcx, a))).collect(),
                    TyKind::Tuple(ele) => ele.into_iter().collect(),
                    _ => unreachable!(),
                };
                return Ok(match ele.len() {
                    0 => ValueIn::default(),
                    _ => {
                        let fields = v.to_branch();
                        let node = self.graph.insert(node_ir_assemble(&self.compiler.target, kind));
                        for (i, x) in ele.into_iter().enumerate() {
                            let k = self.compiler.compile_ty(span, x)?;
                            let f = fields[i].try_to_value().unwrap();
                            let v = self.compile_const(x, &k, f, span)?;
                            self.graph.set_value_in(Link::node(node, i), v);
                        }
                        ValueIn::link(Link::node(node, 0))
                    },
                });
            },
            // `[元素; N]` 这种常量数组(`let a = [0; 3]`)。
            TyKind::Array(ele, len) => {
                let Some(n) = len.try_to_target_usize(self.tcx) else {
                    self.tcx.dcx().span_fatal(span, format!("Array const with unknown length: {ty:?}"));
                };
                let n = n as usize;
                if n > MAX_ASSEMBLE_ELEMENTS {
                    return self.helper().span_err(span, format!("Array of {n} elements exceeds the {MAX_ASSEMBLE_ELEMENTS}-element limit of the assemble-list node"));
                }
                let elem = self.compiler.compile_ty(span, *ele)?;
                let fields = v.to_branch();
                let node = self.graph.insert(node_ir_list_assemble(elem.clone(), n));
                for i in 0..n {
                    let f = fields[i].try_to_value().unwrap();
                    let v = self.compile_const(*ele, &elem, f, span)?;
                    self.graph.set_value_in(Link::node(node, i), v);
                }
                return Ok(ValueIn::link(Link::node(node, 0)));
            },
            TyKind::Slice(_) |
            TyKind::Foreign(_) |
            TyKind::Char |
            TyKind::Uint(_) |
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
                let place = self.compile_place(*p, span)?;
                // Move / Copy 是「读值」:局部变量给的是可变视图,取值要过 `Copy`。
                self.value_of(&place, &kind, &kind, span)?
            }
            Operand::Constant(co) => {
                let v = self.const_to_value(co.const_, co.span)?;
                self.compile_const(ty, &kind, v, span)?
            }
            Operand::RuntimeChecks(_) => ValueIn::value(false.into()),
        })
    }

    /// 取这个 place 的值，类型是 `value`。
    ///
    /// `shape` 是 place 自己的类型（存储形态）。getter 交出它的原始形态，
    /// 这里统一读到 `value`：视图进、值出，多一层视图就多读一次。
    fn value_of(
        &mut self,
        place: &CompiledPlace,
        shape: &IrKind,
        value: &IrKind,
        span: Span,
    ) -> Result<ValueIn> {
        let got = IrKind::partial_mut(shape.clone());
        if !can_read_into(&got, value) {
            return self.helper().span_err(span, format!("place 交出 {got},读不成 {value}"));
        }
        let raw = place.getter(&self.compiler.target, self.graph, shape);
        Ok(copy_until(self.graph, raw, got, value))
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
            TerminatorKind::Goto { target, .. } => {
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
                let ty = self.mono(discr.ty(&self.body.local_decls, self.tcx));
                let span = terminator.source_info.span;
                let kind = self.compiler.compile_ty(span, ty)?;
                let values = targets.all_values().iter().map(|x| {
                    let leaf = ScalarInt::try_from_uint(x.get(), ty.primitive_size(self.tcx)).unwrap();
                    let v = Value { ty, valtree: ValTree::from_scalar_int(self.tcx, leaf) };
                    Ok(self.compile_const(ty, &kind, v, span)?.default.unwrap())
                }).collect::<Result<_>>()?;
                let node = self.graph.insert(node_ir_switch(kind, values));
                let v = self.compile_operand(discr, terminator.source_info.span)?;
                self.graph.set_value_in(Link::node(node, 0), v);
                for (i, &x) in targets.all_targets().iter().enumerate() {
                    self.graph.link_control(Link::node(node, i), blocks[x].begin);
                }
                Link::node(node, 0)
            },
            TerminatorKind::Unreachable => {
                let node = self.graph.insert(node_ir_unreachable());
                Link::node(node, 0)
            },
            TerminatorKind::Drop { place, target, drop, .. } => {
                assert!(drop.is_none());
                let arg = self.compile_rvalue(&Rvalue::Ref(self.tcx.lifetimes.re_erased, BorrowKind::Mut { kind: MutBorrowKind::Default }, *place), terminator.source_info.span)?;
                let result = self.compile_call(terminator.source_info.span,
                                               Instance::resolve_drop_glue(self.tcx, self.mono(place.ty(&self.body.local_decls, self.tcx).ty)),
                                               vec![arg],
                                               None)?;
                self.graph.link_control(result.end, blocks.get(*target).unwrap().begin);
                result.begin
            },
            TerminatorKind::UnwindResume => panic!(),
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

    fn compile_call(&mut self, span: Span, mut func: Instance<'tcx>, args: Vec<ValueIn>, destination: Option<Place<'tcx>>) -> Result<Block> {
        panic_context!("Compiling calling: {func:?}");
        panic_context!("args: {args:?}");
        if Some(func.def_id()) == self.tcx.lang_items().get(LangItem::Panic) {
            self.tcx.dcx().span_note(span, "Ignored panic");
            return Ok(Block::nop(self.graph));
        }
        // 原生类型(句柄)的析构是空操作:它的字段在节点图里并不存在,
        // 析构胶水去读字段只会读到不存在的东西。
        if let InstanceKind::Shim(ShimKind::DropGlue(..)) = func.def
            && let Some(ty) = func.args.first().and_then(|x| x.as_type())
            && self.compiler.is_native_ty(ty) {
            return Ok(Block::nop(self.graph));
        }
        let sig = self.helper().fn_sig(func);
        let params: Vec<IrKind> = self.helper().fn_params(func, sig).iter().map(|x| self.compiler.compile_ty(span, *x)).collect::<Result<_>>()?;
        // `PartialMut` 的内建不走 `compile_native_call`：它们碰的是 `&mut T` 的
        // 元组形态（LocalSetter + 读槽），值并不「直通」。
        if let Some(id) = self.compiler.native_call_id(func) {
            let result = match id.as_str() {
                // `PartialMut::from(&mut T)`：把元组拆开，取第 1 个字段（读槽）。
                "partial_mut_from" => {
                    let Some(IrKind::Mut(inner)) = params.first().cloned() else {
                        return self.helper().span_err(span, "partial_mut_from 的参数应当是 &mut T");
                    };
                    let node = self.graph.insert(node_ir_destructure(&self.compiler.target, &IrKind::Mut(inner)));
                    self.graph.set_value_in(Link::node(node, 0), args[0].clone());
                    Some(ValueIn::link(Link::node(node, 1)))
                },
                // `Deref`：可变视图的值就是 `T` 的值。
                // 要的还是视图(`T` 非 `Freeze`)就原样给;要值就 `Copy` 一份。
                "partial_mut_deref" => {
                    let out = self.compiler.compile_ty(span, sig.output())?;
                    if out == params[0] {
                        Some(args[0].clone())
                    } else {
                        let node = self.graph.insert(node_ir_copy(out));
                        self.graph.set_value_in(Link::node(node, 0), args[0].clone());
                        Some(ValueIn::link(Link::node(node, 0)))
                    }
                },
                // `DerefMut`：`&mut PartialMut<T>` -> `&mut T`。
                //
                // 输入名义上是 `(LocalSetter, PartialMut<PartialMut<T>>)`，输出是
                // `(LocalSetter, PartialMut<T>)`：第 0 个字段原样拿过来，第 1 个
                // 字段要把视图再拆一层 —— 这一步表示不了，给 `Unsupported`。
                "partial_mut_deref_mut" => {
                    let Some(IrKind::Mut(pm)) = params.first().cloned() else {
                        return self.helper().span_err(span, "partial_mut_deref_mut 的参数应当是 &mut PartialMut<T>");
                    };
                    let out = self.compiler.compile_ty(span, sig.output())?;
                    // 输入的元组：拆开，第 1 个字段就是那个视图。
                    let de = self.graph.insert(node_ir_destructure(&self.compiler.target, &IrKind::Mut(pm)));
                    self.graph.set_value_in(Link::node(de, 0), args[0].clone());
                    // 输出的第 0 个字段是 `LocalSetter` —— 视图里没有 setter 槽，
                    // 造不出来，占 `Unsupported`。
                    let fields = struct_fields(&self.compiler.target, &out);
                    let unsupported = self.graph.insert(node_ir_unsupported(fields[0].kind.clone()));
                    let asm = self.graph.insert(node_ir_assemble(&self.compiler.target, &out));
                    self.graph.set_value_in(Link::node(asm, 0), ValueIn::link(Link::node(unsupported, 0)));
                    self.graph.set_value_in(Link::node(asm, 1), ValueIn::link(Link::node(de, 1)));
                    Some(ValueIn::link(Link::node(asm, 0)))
                },
                _ => None,
            };
            if let Some(value) = result {
                let mut block = Block::nop(self.graph);
                if let Some(destination) = destination {
                    let b = self.compile_assign(destination, span, value)?;
                    block.extend(self.graph, b);
                }
                return Ok(block);
            }
        }
        let node_kind = loop {
            if let InstanceKind::Intrinsic(def_id) = func.def {
                let int = self.tcx.intrinsic(def_id).unwrap();
                match int.name.as_str() {
                    "black_box" => break node_ir_black_box(params[0].clone()),
                    other if int.must_be_overridden => todo!("intrinsic: {other}"),
                    _ => func = Instance::new_raw(func.def_id(), func.args),
                }
            }
            break if params.iter().all(|x| x.to_native().is_some())
                && let Some(native) = self.compiler.compile_native_call(span, func, sig, &params)? {
                native
            } else {
                let ret = self.compiler.compile_ty(span, sig.output())?;
                NodeKind::new(
                    IrNodeId::Fn(self.compiler.touch_fn(func)?),
                    1, 1,
                    params.iter().cloned().map(Some).collect(),
                    vec![ret],
                )
            };
        };
        let control = match (node_kind.controls_in_num, node_kind.controls_out_num) {
            (0, 0) => false,
            (1, 1) => true,
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
