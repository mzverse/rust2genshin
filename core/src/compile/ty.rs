use rustc_abi::{FieldIdx, Integer, IntegerType};
use crate::asset::value::{AnyValue, ValueBool, ValueConfig, ValueDefault, ValueEntity, ValueFaction, ValueFloat, ValueGuid, ValueInt, ValueLocalVarRef, ValuePrefab, ValueString};
use crate::compile::optimize::{ValueIrAdt, ValueIrMut, ValueIrNever};
use crate::compile::{Compiler, Result, WithTcx};
use rustc_ast::{FloatTy, IntTy};
use rustc_attr_ir::LangItem;
use rustc_index::Idx;
use rustc_middle::infer::canonical::ir::GenericArgKind;
use rustc_middle::mir::Mutability;
use rustc_middle::ty;
use rustc_middle::ty::print::with_no_trimmed_paths;
use rustc_middle::ty::{AdtDef, AdtKind, Const, GenericArg, GenericArgsRef, Instance, Ty, TyKind, TypeVisitableExt, TypingEnv};
use rustc_span::{Span, DUMMY_SP};
use rustc_span::def_id::DefId;


impl<'tcx> Compiler<'tcx> {
    pub fn get_default_some(&mut self, d: AdtDef<'tcx>, a: GenericArgsRef<'tcx>) -> Result<Option<AnyValue>> {
        let opt = self.tcx.lang_items().get(LangItem::Option);
        if Some(d.did()) != opt {
            return Ok(None);
        }
        let ele = a.type_at(0);
        if ele.ty_adt_def().map(AdtDef::did) != opt {
            let ele = self.compile_ty(DUMMY_SP, ele)?;
            if ele.is::<ValueGuid>() || ele.is::<ValueEntity>() || ele.is::<ValuePrefab>() || ele.is::<ValueConfig>() || ele.is::<ValueFaction>() {
                return Ok(Some(ele));
            }
        }
        Ok(None)
    }

    fn mangle_ty(ty: Ty) -> String {
        assert!(!ty.has_param());
        match ty.kind() {
            TyKind::Tuple(tys) => Self::mangle_tuple(tys),
            TyKind::Adt(d, a) => Self::mangle_adt(d.did(), a),
            TyKind::Closure(d, a) => Self::mangle_adt(*d, a),
            TyKind::Str => "String".into(),
            TyKind::Ref(_, e, Mutability::Not) => format!("&{}", Self::mangle_ty(*e)),
            TyKind::Ref(_, e, Mutability::Mut) => format!("&mut {}", Self::mangle_ty(*e)),
            TyKind::Bound(..) | TyKind::UnsafeBinder(..) | TyKind::Param(..) => panic!(),
            TyKind::Infer(..) => panic!(),
            TyKind::Foreign(..) => panic!(),
            TyKind::Placeholder(..) => panic!(),
            TyKind::Pat(..) => panic!(),
            TyKind::Alias(..) => panic!(),
            TyKind::Error(..) => panic!(),
            TyKind::FnDef(..) => panic!(),
            TyKind::FnPtr(b, _) => {
                let s = b.skip_binder();
                let result = format!("fn{}", Self::mangle_tuple(s.inputs()));
                if s.output().is_unit() {
                    result
                } else {
                    format!("{result}->{}", Self::mangle_ty(s.output()))
                }
            },
            other => format!("{:?}", other), // TODO
        }
    }

    fn mangle_const(c: Const) -> String {
        c.to_string()
    }

    fn mangle_tuple(tys: &[Ty]) -> String {
        format!("({})", tys.iter().copied().map(Self::mangle_ty).collect::<Vec<_>>().join(","))
    }

    fn mangle_def(def: DefId) -> String {
        ty::tls::with(|tcx| {
            with_no_trimmed_paths!(tcx.def_path_str(def))
        })
    }

    pub fn mangle_func(&self, func: Instance<'tcx>) -> String {
        if self.tcx.codegen_fn_attrs(func.def_id()).contains_extern_indicator() {
            self.tcx.symbol_name(func).to_string()
        } else {
            Self::mangle_adt(func.def_id(), func.args)
        }
    }

    fn mangle_adt(def: DefId, s: GenericArgsRef) -> String {
        let result = Self::mangle_def(def);
        let s = s.iter().map(GenericArg::kind).filter(|x| !matches!(x, GenericArgKind::Lifetime(..))).collect::<Vec<_>>();
        if s.is_empty() {
            result
        } else {
            format!("{}<{}>", result, s.into_iter().map(|x| match x {
                GenericArgKind::Type(t) => Self::mangle_ty(t),
                GenericArgKind::Const(c) => Self::mangle_const(c),
                _ => unreachable!(),
            }).collect::<Vec<_>>().join(","))
        }
    }

    pub fn compile_ty(&mut self, span: Span, ty: Ty<'tcx>) -> Result<AnyValue> {
        assert!(!ty.has_param());
        if let Some(n) = self.compile_native_ty(ty) {
            return n;
        }
        Ok(match ty.kind() {
            TyKind::Bool => ValueBool::def(),
            TyKind::Char => return self.span_err(span, "Char is unsupported"),
            TyKind::Int(ty) => match ty {
                IntTy::I8 | IntTy::I16 | IntTy::I64 | IntTy::I128 =>
                    return self.span_err(span, format!("Unsupported int: {}", ty.name())),
                IntTy::Isize | IntTy::I32 => ValueInt::def(),
            },
            TyKind::Uint(_) => todo!(),
            TyKind::Float(ty) => match ty {
                FloatTy::F16 |
                FloatTy::F64 |
                FloatTy::F128 =>
                    return self.span_err(span, format!("Unsupported float: {}", ty.name())),
                FloatTy::F32 => ValueFloat::def(),
            },
            TyKind::RawPtr(e, _) => if e.is_str() { ValueString::def() } else {
                return self.span_err(span, format!("RawPtr is unsupported: {e:?}"));
            },
            TyKind::Str => ValueString::def(),
            TyKind::Ref(_, e, m) => if e.is_str() { ValueString::def() } else {
                if m.is_mut() {
                    ValueIrMut(self.compile_ty(span, *e)?).into()
                } else {
                    if e.is_never() { // write only
                        ValueLocalVarRef::def()
                    } else {
                        self.compile_ty(span, *e)?
                    }
                }
            },
            TyKind::Adt(d, a) if d.repr().transparent() =>
                self.compile_ty(span, self.get_tcx().normalize_erasing_regions(TypingEnv::fully_monomorphized(), d.non_enum_variant().fields[FieldIdx::new(0)].ty(self.tcx, a)))?,
            TyKind::Adt(d, a) if d.adt_kind() == AdtKind::Enum => {
                if d.repr().int == Some(IntegerType::Fixed(Integer::I32, true)) {
                    ValueInt::def()
                } else if let Some(r) = self.get_default_some(*d, a)? {
                    r
                } else {
                    todo!("{d:?} {a:?}")
                }
            },
            TyKind::Adt(..) |
            TyKind::Tuple(..) |
            TyKind::Closure(..) => {
                let key = Self::mangle_ty(ty);
                self.interned_adts.insert(key.clone(), ty);
                ValueIrAdt(key).into()
            },
            TyKind::Never => ValueIrNever::def(),
            TyKind::Foreign(_) => todo!(),
            TyKind::Array(_, _) => todo!(),
            TyKind::Pat(_, _) => todo!(),
            TyKind::Slice(_) => todo!(),
            TyKind::FnDef(_, _) => todo!(),
            TyKind::FnPtr(_, _) => todo!(),
            TyKind::Alias(_, _) => todo!(),
            TyKind::Dynamic(_, _)
            | TyKind::CoroutineClosure(_, _)
            | TyKind::Coroutine(_, _)
            | TyKind::CoroutineWitness(_, _)
            | TyKind::Param(_)
            | TyKind::Bound(_, _)
            | TyKind::Placeholder(_)
            | TyKind::Infer(_)
            | TyKind::Error(_)
            | TyKind::UnsafeBinder(_) => {
                let _ = self.span_err::<()>(span, format!("Unsupported type: {:?}", ty.kind())).ok();
                panic!("Unsupported type: {:?}", ty.kind());
            }
        })
    }
}
