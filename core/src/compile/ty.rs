use rustc_abi::{FieldIdx, Integer, IntegerType};
use crate::asset::value::NativeKind;
use crate::compile::ir::{AdtInfo, FieldInfo, IrKind};
use crate::compile::{Compiler, Result};
use rustc_ast::{FloatTy, IntTy};
use rustc_attr_ir::LangItem;
use rustc_index::Idx;
use rustc_middle::infer::canonical::ir::GenericArgKind;
use rustc_middle::mir::Mutability;
use rustc_middle::ty;
use rustc_middle::ty::print::with_no_trimmed_paths;
use rustc_middle::ty::{AdtDef, AdtKind, Const, GenericArg, GenericArgsRef, Instance, Ty, TyKind, TypeVisitableExt, TypingEnv};
use rustc_span::{Span, DUMMY_SP};
use rustc_span::def_id::{DefId, LOCAL_CRATE};


impl<'tcx> Compiler<'tcx> {
    pub fn get_default_some(&mut self, d: AdtDef<'tcx>, a: GenericArgsRef<'tcx>) -> Result<Option<NativeKind>> {
        let opt = self.tcx.lang_items().get(LangItem::Option);
        if Some(d.did()) != opt {
            return Ok(None);
        }
        let ele = a.type_at(0);
        if ele.ty_adt_def().map(AdtDef::did) != opt {
            let Some(ele) = self.compile_ty(DUMMY_SP, ele)?.into_native() else {
                return Ok(None);
            };
            match ele {
                | NativeKind::Guid
                | NativeKind::Entity
                | NativeKind::Prefab
                | NativeKind::Config
                | NativeKind::Faction
                => return Ok(Some(ele)),
                _ => (),
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
            let result = with_no_trimmed_paths!(tcx.def_path_str(def));
            if def.is_local() {
                format!("{}::{result}", tcx.crate_name(LOCAL_CRATE))
            } else {
                result
            }
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

    pub fn touch_adt(&mut self, ty: Ty<'tcx>) -> IrKind {
        let key = Self::mangle_ty(ty);
        if !self.target.adts.contains_key(&key) {
            let info = AdtInfo::new(match ty.kind() {
                TyKind::Adt(d, a) => {
                    if !d.is_struct() {
                        todo!();
                    }
                    d.non_enum_variant().fields.iter().map(|x| FieldInfo::new(x.name.to_string(), self.compile_ty(DUMMY_SP, self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), x.ty(self.tcx, a))).unwrap())).collect()
                },
                TyKind::Closure(..) | TyKind::Tuple(..) =>
                    match ty.kind() {
                        TyKind::Closure(_d, a) => a.as_closure().upvar_tys(),
                        TyKind::Tuple(es) => es,
                        _ => unreachable!(),
                    }.iter().enumerate().map(|(i, x)| FieldInfo::new(i.to_string(), self.compile_ty(DUMMY_SP, x).unwrap())).collect(),
                other => panic!("{other:?}"),
            });
            self.target.adts.insert(key.clone(), info);
        }
        IrKind::Adt(key)
    }

    pub fn compile_ty(&mut self, span: Span, ty: Ty<'tcx>) -> Result<IrKind> {
        assert!(!ty.has_param());
        if let Some(n) = self.compile_native_ty(ty) {
            return Ok(IrKind::Native(n?));
        }
        Ok(match ty.kind() {
            TyKind::Bool => IrKind::Native(NativeKind::Bool),
            TyKind::Char => return self.helper().span_err(span, "Char is unsupported"),
            TyKind::Int(ty) => match ty {
                IntTy::I8 | IntTy::I16 | IntTy::I64 | IntTy::I128 =>
                    return self.helper().span_err(span, format!("Unsupported int: {}", ty.name())),
                IntTy::Isize | IntTy::I32 => IrKind::Native(NativeKind::Int),
            },
            TyKind::Uint(e) => IrKind::Unsupported(e.name_str().to_string()),
            TyKind::Float(ty) => match ty {
                FloatTy::F16 |
                FloatTy::F64 |
                FloatTy::F128 =>
                    return self.helper().span_err(span, format!("Unsupported float: {}", ty.name())),
                FloatTy::F32 => IrKind::Native(NativeKind::Float),
            },
            TyKind::RawPtr(e, _) => if e.is_str() { IrKind::Native(NativeKind::String) } else {
                IrKind::Unsupported(format!("*{e:?}"))
            },
            TyKind::Str => IrKind::Native(NativeKind::String),
            TyKind::Ref(_, e, m) =>
                if m.is_mut() {
                    IrKind::Mut(self.compile_ty(span, *e)?.into())
                } else {
                    if e.is_never() { // &! write only FIXME
                        IrKind::Native(NativeKind::LocalVarRef)
                    } else {
                        self.compile_ty(span, *e)?
                    }
                },
            TyKind::Adt(d, a) if d.repr().transparent() =>
                self.compile_ty(span, self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), d.non_enum_variant().fields[FieldIdx::new(0)].ty(self.tcx, a)))?,
            TyKind::Adt(d, a) if d.adt_kind() == AdtKind::Enum => {
                if d.repr().int == Some(IntegerType::Fixed(Integer::I32, true)) {
                    IrKind::Native(NativeKind::Int)
                } else if let Some(r) = self.get_default_some(*d, a)? {
                    IrKind::Native(r)
                } else {
                    todo!("{d:?} {a:?}")
                }
            },
            TyKind::Adt(..) |
            TyKind::Tuple(..) |
            TyKind::Closure(..) => self.touch_adt(ty),
            TyKind::Never => IrKind::Never,
            TyKind::Foreign(id) => todo!("{id:?}"),
            TyKind::Array(_, _) => todo!(),
            TyKind::Pat(e, _) => self.compile_ty(span, *e)?, // TODO?
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
                self.tcx.dcx().span_fatal(span, format!("Unsupported type: {:?}", ty.kind()));
            }
        })
    }
}
