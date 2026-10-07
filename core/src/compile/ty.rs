use crate::compile::ir::{AdtInfo, FieldInfo, IrKind};
use crate::compile::{Compiler, Result};
use crate::value::NativeKind;
use rustc_abi::{FIRST_VARIANT, FieldIdx, Integer, IntegerType};
use rustc_ast::{FloatTy, IntTy};
use rustc_attr_ir::LangItem;
use rustc_index::Idx;
use rustc_middle::infer::canonical::ir::GenericArgKind;
use rustc_middle::mir::Mutability;
use rustc_middle::ty;
use rustc_middle::ty::print::with_no_trimmed_paths;
use rustc_middle::ty::{AdtDef, Const, GenericArg, GenericArgsRef, Instance, Ty, TyKind, TypeVisitableExt, TypingEnv};
use rustc_span::def_id::{DefId, LOCAL_CRATE};
use rustc_span::{DUMMY_SP, Span};
use std::collections::HashSet;

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
            | TyKind::Closure(d, a)
            | TyKind::Coroutine(d, a)
            => Self::mangle_adt(*d, a),
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

    pub fn touch_adt(&mut self, ty: Ty<'tcx>) -> Result<IrKind> {
        if let TyKind::Adt(d, a) = ty.kind() && d.is_enum() {
            if d.repr().int == Some(IntegerType::Fixed(Integer::I32, true)) {
                return Ok(IrKind::Native(NativeKind::Int));
            } else if let Some(r) = self.get_default_some(*d, a)? {
                return Ok(IrKind::Native(r));
            }
        }
        let key = Self::mangle_ty(ty);
        if !self.target.adts.contains_key(&key) {
            let mut info;
            match ty.kind() {
                TyKind::Adt(d, a) => {
                    if d.is_enum() {
                        info = AdtInfo::new(vec![FieldInfo::new("discriminant".into(), IrKind::Native(NativeKind::Int))]);
                        info.discriminant = 0.into();
                        for x in d.variants() {
                            let mut vec = vec![];
                            let mut used = HashSet::new();
                            used.insert(0); // discriminant
                            'label: for field in &x.fields {
                                let kind = self.compile_ty(DUMMY_SP, self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), field.ty(self.tcx, a)))?;
                                for (i, f) in info.fields.iter().enumerate() {
                                    if used.contains(&i) {
                                        continue;
                                    }
                                    if kind == f.kind {
                                        used.insert(i);
                                        vec.push(i);
                                        info.fields[i].name += &format!(" | {}.{}", x.name, field.name);
                                        continue 'label;
                                    }
                                }
                                let i = info.fields.len();
                                used.insert(i);
                                vec.push(i);
                                info.fields.push(FieldInfo::new(format!("{}.{}", x.name, field.name), kind));
                            }
                            info.variants.push(vec);
                        }
                    } else if d.is_struct() {
                        info = AdtInfo::new(d.variant(FIRST_VARIANT).fields.iter().map(|x| Ok(FieldInfo::new(x.name.to_string(), self.compile_ty(DUMMY_SP, self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), x.ty(self.tcx, a)))?))).collect::<Result<_>>()?)
                    } else {
                        panic!()
                    }
                },
                TyKind::Coroutine(d, a) => {
                    let layout = self.tcx.coroutine_layout(*d, a).unwrap();
                    info = AdtInfo::new(a.as_coroutine().upvar_tys().iter().enumerate().map(|(i, x)| FieldInfo::new(i.to_string(), self.compile_ty(DUMMY_SP, x).unwrap())).collect());
                    info.discriminant = info.fields.len().into();
                    info.fields.push(FieldInfo::new("__state".into(), IrKind::Native(NativeKind::Int)));
                    let begin = info.fields.len();
                    info.fields.extend(layout.field_tys.iter_enumerated().map(|(i, x)| Ok(FieldInfo::new(x.debuginfo_name.as_ref().map(ToString::to_string).unwrap_or_else(|| i.index().to_string()), self.compile_ty(DUMMY_SP, x.ty)?))).collect::<Result<Vec<_>>>()?);
                    for x in &layout.variant_fields {
                        info.variants.push(x.iter().map(|i| begin + i.index()).collect());
                    }
                },
                | TyKind::Closure(..)
                | TyKind::Tuple(..)
                => info = AdtInfo::new(match ty.kind() {
                    TyKind::Closure(_d, a) => a.as_closure().upvar_tys(),
                    TyKind::Tuple(es) => es,
                    _ => unreachable!(),
                }.iter().enumerate().map(|(i, x)| FieldInfo::new(i.to_string(), self.compile_ty(DUMMY_SP, x).unwrap())).collect()),
                other => panic!("{other:?}"),
            }
            self.target.adts.insert(key.clone(), info);
        }
        Ok(IrKind::Adt(key))
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
                match e.kind() {
                    | TyKind::Slice(e)
                    | TyKind::Array(e, _)
                    => IrKind::Native(NativeKind::List(match self.compile_ty(span, *e)? {
                        IrKind::Native(x) => x,
                        other => todo!("{other:?}"),
                    }.into())),
                    _ => IrKind::Unsupported(format!("*{e:?}")),
                }
            },
            TyKind::Str => IrKind::Native(NativeKind::String),
            TyKind::Ref(_, e, m) =>
                if m.is_mut() {
                    IrKind::Mut(self.compile_ty(span, *e)?.into())
                } else {
                    self.compile_ty(span, *e)?
                },
            TyKind::Adt(d, a) if d.repr().transparent() =>
                self.compile_ty(span, self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), d.non_enum_variant().fields[FieldIdx::new(0)].ty(self.tcx, a)))?,
            | TyKind::Adt(..)
            | TyKind::Tuple(..)
            | TyKind::Closure(..)
            | TyKind::Coroutine(..)
            => self.touch_adt(ty)?,
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
