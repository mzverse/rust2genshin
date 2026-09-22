use crate::asset::structure::{StructField, StructureDefinition, ValueStruct};
use crate::asset::value::{AnyValue, ValueBool, ValueDefault, ValueFloat, ValueInt, ValueLocalVarRef, ValueString};
use crate::asset::{Asset, AssetRef};
use crate::compile::optimize::ValueIrMut;
use crate::compile::{Compiler, Result, WithTcx};
use rustc_ast::{FloatTy, IntTy};
use rustc_middle::infer::canonical::ir::{GenericArgKind, Unnormalized};
use rustc_middle::mir::Mutability;
use rustc_middle::ty;
use rustc_middle::ty::print::with_no_trimmed_paths;
use rustc_middle::ty::{AdtDef, Const, GenericArg, GenericArgsRef, Instance, List, Ty, TyKind, TypeVisitableExt, TypingEnv};
use rustc_span::Span;
use rustc_span::def_id::DefId;

impl<'tcx> Compiler<'tcx> {
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

    fn touch_adt(&mut self, def: AdtDef<'tcx>, g: GenericArgsRef<'tcx>, span: Span) -> Result<&AssetRef<StructureDefinition>> {
        if !def.is_struct() {
            todo!();
        }
        let key = Self::mangle_adt(def.did(), g);
        if let Some(id) = self.structs.get(&key) {
            return Ok(id);
        }
        let id = StructureDefinition {
            name: key.clone(),
            version: 1,
            fields: def.non_enum_variant().fields.iter().map(|f| Ok(StructField {
                name: f.name.to_string(),
                value: self.compile_ty(span, self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), f.ty(self.tcx, g)))?,
            })).collect::<Result<_>>()?,
        }.apply(&mut self.assets);
        Ok(self.structs.entry(key).or_insert(id))
    }

    fn touch_closure(&mut self, def: DefId, g: GenericArgsRef<'tcx>, span: Span) -> Result<&AssetRef<StructureDefinition>> {
        let key = Self::mangle_adt(def, g);
        if let Some(id) = self.structs.get(&key) {
            return Ok(id);
        }
        let id = StructureDefinition {
            name: key.clone(),
            version: 1,
            fields: g.as_closure().upvar_tys().iter().enumerate().map(|(i, t)| Ok(StructField {
                name: i.to_string(),
                value: self.compile_ty(span, self.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), Unnormalized::new(t)))?,
            })).collect::<Result<_>>()?,
        }.apply(&mut self.assets);
        Ok(self.structs.entry(key).or_insert(id))
    }

    fn touch_tuple(&mut self, span: Span, tys: &List<Ty<'tcx>>) -> Result<&AssetRef<StructureDefinition>> {
        let key = Self::mangle_tuple(tys);
        if let Some(id) = self.structs.get(&key) {
            return Ok(id);
        }
        // Build the StructureDefinition and insert it as an asset.
        use crate::asset::structure::{StructField, StructureDefinition};
        // Resolve element types first (recursively interns nested tuples).
        let elem_kinds: Vec<AnyValue> = tys.iter()
            .map(|t| self.compile_ty(span, t))
            .collect::<Result<_>>()?;
        let fields: Vec<StructField> = elem_kinds.iter().enumerate()
            .map(|(i, k)| StructField {
                name: format!("{i}"),
                value: k.clone(),
            })
            .collect();
        let def = StructureDefinition {
            name: key.clone(),
            version: 1,
            fields,
        };
        let id = def.apply(&mut self.assets);
        Ok(self.structs.entry(key).or_insert(id))
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
            TyKind::Adt(d, a) => ValueStruct::new(self.touch_adt(*d, a, span)?.clone()).into(),
            TyKind::Foreign(_) => todo!(),
            TyKind::Array(_, _) => todo!(),
            TyKind::Pat(_, _) => todo!(),
            TyKind::Slice(_) => todo!(),
            TyKind::FnDef(_, _) => todo!(),
            TyKind::FnPtr(_, _) => todo!(),
            TyKind::Tuple(tys) => ValueStruct::new(self.touch_tuple(span, tys)?.clone()).into(),
            TyKind::Closure(d, a) => ValueStruct::new(self.touch_closure(*d, a, span)?.clone()).into(),
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
                let _ = self.span_err::<()>(span, format!("Unsupported type: {:?}", ty.kind())).ok();
                panic!("Unsupported type: {:?}", ty.kind());
            }
        })
    }
}
