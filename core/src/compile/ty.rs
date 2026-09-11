use crate::asset::structure::{StructureDefinition, ValueStruct};
use crate::asset::value::{AnyValue, ValueBool, ValueDefault, ValueEntity, ValueFloat, ValueGuid, ValueInt, ValueString};
use crate::asset::{Asset, AssetRef};
use crate::compile::{Compiler, Result, TupleKey, WithTcx};
use rustc_ast::{FloatTy, IntTy};
use rustc_middle::ty::{List, Ty, TyKind};
use rustc_span::Span;

impl Compiler<'_> {
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

    fn touch_tuple(&mut self, span: Span, tys: &List<Ty>) -> Result<(&AssetRef<StructureDefinition>, Vec<AnyValue>)> {
        if tys.is_empty() {
            return self.span_err(span, "unit tuple () has no struct schema");
        }
        // Resolve element types first (recursively interns nested tuples).
        let elem_kinds: Vec<AnyValue> = tys.iter()
            .map(|t| self.compile_ty(span, t))
            .collect::<Result<_>>()?;
        let key = TupleKey(format!("[{}]", elem_kinds.iter()
            .map(|k| format!("{k:?}"))
            .collect::<Vec<_>>().join(", ")));
        if let Some(id) = self.tuple_schemas.get(&key) {
            return Ok((id, elem_kinds));
        }
        // Build the StructureDefinition and insert it as an asset.
        use crate::asset::structure::{StructField, StructureDefinition};
        let fields: Vec<StructField> = elem_kinds.iter().enumerate()
            .map(|(i, k)| StructField {
                name: format!("{i}"),
                value: k.clone(),
                is_set: false,
            })
            .collect();
        let def = StructureDefinition {
            name: Self::mangle_tuple(tys),
            version: 1,
            fields,
        };
        let id = def.apply(&mut self.assets);
        Ok((self.tuple_schemas.entry(key).or_insert(id), elem_kinds))
    }

    pub fn compile_ty(&mut self, span: Span, ty: Ty) -> Result<AnyValue> {
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
            TyKind::Ref(_, e, _) => if e.is_str() { ValueString::def() } else {
                return self.span_err(span, "Ref is unsupported, see `Box as Deref`".to_string());
            },
            TyKind::Adt(d, a) => {
                if d.did().krate == self.lib {
                    match self.tcx.def_path(d.did()).to_string_no_crate_verbose().as_str() {
                        "::Guid" => return Ok(ValueGuid::def()),
                        "::entity::Entity" => return Ok(ValueEntity::def()),
                        other => panic!("{other}"),
                    }
                }
                return self.span_err(span, format!("Adt: {d:?} = {a:?}"));
            },
            TyKind::Foreign(_) => todo!(),
            TyKind::Array(_, _) => todo!(),
            TyKind::Pat(_, _) => todo!(),
            TyKind::Slice(_) => todo!(),
            TyKind::FnDef(_, _) => todo!(),
            TyKind::FnPtr(_, _) => todo!(),
            TyKind::Tuple(tys) => {
                if tys.is_empty() {
                    panic!();
                }
                let (struct_id, field_kinds) = self.touch_tuple(span, *tys)?;
                ValueStruct::new(struct_id.clone(), field_kinds).into()
            }
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
