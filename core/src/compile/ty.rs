use crate::asset::structure::{StructField, StructureDefinition, ValueStruct};
use crate::asset::value::{AnyValue, ValueBool, ValueDefault, ValueFloat, ValueInt, ValueString};
use crate::asset::{Asset, AssetRef};
use crate::compile::{Compiler, Result, WithTcx};
use rustc_ast::{FloatTy, IntTy};
use rustc_middle::infer::canonical::ir::GenericArgKind;
use rustc_middle::ty::inherent::SliceLike;
use rustc_middle::ty::{AdtDef, GenericArg, GenericArgsRef, List, Mutability, Ty, TyKind, TypingEnv};
use rustc_span::Span;

impl<'tcx> Compiler<'tcx> {
    fn mangle_ty(ty: Ty) -> String {
        match ty.kind() {
            TyKind::Tuple(tys) => Self::mangle_tuple(tys),
            TyKind::Str => "String".into(),
            other => format!("{:?}", other), // TODO
        }
    }

    fn mangle_tuple(tys: &List<Ty>) -> String {
        format!("({})", tys.iter().map(Self::mangle_ty).collect::<Vec<_>>().join(","))
    }

    fn mangle_adt(def: AdtDef, s: GenericArgsRef) -> String {
        let result = format!("{def:?}");
        if s.is_empty() {
            result
        } else {
            format!("{}<{}>", result, s.iter().map(GenericArg::kind).filter(|x| !matches!(x, GenericArgKind::Lifetime(..))).map(|x| match x {
                GenericArgKind::Type(t) => Self::mangle_ty(t),
                GenericArgKind::Const(c) => format!("{c:?}"),
                _ => unreachable!(),
            }).collect::<Vec<_>>().join(","))
        }
    }

    fn touch_adt(&mut self, def: AdtDef<'tcx>, g: GenericArgsRef<'tcx>, span: Span) -> Result<&AssetRef<StructureDefinition>> {
        if !def.is_struct() {
            todo!();
        }
        let key = Self::mangle_adt(def, g);
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
                if *m == Mutability::Mut {
                    return self.span_err(span, "&mut is still unsupported// , see `<Box as Deref>` or `#[rustc_force_inline] or #[inline(always)]`".to_string());
                }
                return self.compile_ty(span, *e);
            },
            TyKind::Adt(d, a) => ValueStruct::new(self.touch_adt(*d, a, span)?.clone()).into(),
            TyKind::Foreign(_) => todo!(),
            TyKind::Array(_, _) => todo!(),
            TyKind::Pat(_, _) => todo!(),
            TyKind::Slice(_) => todo!(),
            TyKind::FnDef(_, _) => todo!(),
            TyKind::FnPtr(_, _) => todo!(),
            TyKind::Tuple(tys) => ValueStruct::new(self.touch_tuple(span, tys)?.clone()).into(),
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
                let _ = self.span_err::<()>(span, format!("Unsupported type: {:?}", ty.kind())).ok();
                panic!("Unsupported type: {:?}", ty.kind());
            }
        })
    }
}
