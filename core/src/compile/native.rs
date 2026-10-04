use super::{Compiler, Result};
use crate::compile::get_expn_macro_attr;
use crate::compile::ir::IrKind;
use crate::node::NodeKind;
use crate::node::arithmetic::{node_cast, node_enum_equal, node_power};
use crate::value::{NativeKind, NativeValue};
use rustc_middle::mir::interpret::Scalar;
use rustc_middle::query::QueryKey;
use rustc_middle::ty::{FnSig, Instance, Ty, TyKind};
use rustc_span::Span;
use syn::{LitInt, LitStr, Meta, MetaList};

pub fn native_const(kind: &NativeKind, get_scalar: impl FnOnce() -> Scalar) -> Option<Box<dyn NativeValue>> {
    Some(match kind {
        NativeKind::Int => get_scalar().to_i32().unwrap().into(),
        NativeKind::Guid |
        NativeKind::Faction |
        NativeKind::Config |
        NativeKind::Prefab => get_scalar().to_i64().unwrap().into(),
        NativeKind::Enum(_) => get_scalar().to_i32().unwrap().into(),
        _ => return None,
    })
}

impl<'tcx> Compiler<'tcx> {
    pub fn compile_native_ty(&self, ty: Ty<'tcx>) -> Option<Result<NativeKind>> {
        let def = match ty.kind() {
            TyKind::Foreign(d) => *d,
            TyKind::Adt(d, _a) =>
                d.did(),
            _ => return None,
        }.default_span(self.tcx);
        let expn = def.ctxt().outer_expn().expn_data().call_site;
        match get_expn_macro_attr(self.tcx, def)?.meta {
            Meta::List(MetaList { path, tokens, .. }) => {
                match path.get_ident()?.to_string().as_str() {
                    "native" => {
                        let id = match syn::parse2::<LitStr>(tokens) {
                            Ok(id) => id.value(),
                            Err(e) => return self.helper().span_err(expn, e.to_string()).into(),
                        };
                        Ok(match id.as_str() {
                            "Guid" => NativeKind::Guid,
                            "Entity" => NativeKind::Entity,
                            "Faction" => NativeKind::Faction,
                            "Config" => NativeKind::Config,
                            "Prefab" => NativeKind::Prefab,
                            "Vec3" => NativeKind::Vec3,
                            "List" => NativeKind::List(NativeKind::LocalVarRef.into()), // TODO
                            "Dict" => NativeKind::dict(NativeKind::LocalVarRef, NativeKind::LocalVarRef), // TODO
                            _ => return self.helper().span_err(expn, format!("Unknown native kind: {id}")).into(),
                        }).into()
                    },
                    "native_enum" => {
                        let id = match syn::parse2::<LitInt>(tokens).and_then(|x| x.base10_parse::<i32>()) {
                            Ok(id) => id,
                            Err(e) => return self.helper().span_err(expn, e.to_string()).into(),
                        };
                        Ok(NativeKind::Enum(id)).into()
                    },
                    _ => None,
                }
            },
            _ => None,
        }
    }

    pub fn compile_native_call(&mut self, span: Span, func: Instance<'tcx>, sig: FnSig<'tcx>, params: &[IrKind]) -> Result<Option<NodeKind>> {
        let ret = sig.output();
        let ret_kinds = if let TyKind::Tuple(ele) = ret.kind() {
            ele.iter().collect()
        } else {
            vec![ret]
        }.iter().map(|x| self.compile_ty(span, *x)).collect::<Result<Vec<_>>>()?;
        let def = func.default_span(self.tcx);
        let expn = def.ctxt().outer_expn().expn_data().call_site;
        Ok(match if let Some(x) = get_expn_macro_attr(self.tcx, def) { x } else { return Ok(None) }.meta {
            Meta::List(MetaList { path, tokens, .. }) => {
                match if let Some(x) = path.get_ident() { x } else { return Ok(None) }.to_string().as_str() {
                    "native" => {
                        let id = match syn::parse2::<LitStr>(tokens) {
                            Ok(id) => id.value(),
                            Err(e) => return self.helper().span_err(expn, e.to_string()),
                        };
                        match id.as_str() {
                            "power" => node_power(params[0].as_native().unwrap().clone()),
                            "to_string" => node_cast(params[0].as_native().unwrap().clone(), NativeKind::String).unwrap(),
                            "enum_eq" => node_enum_equal(match params[0].as_native().unwrap() {
                                NativeKind::Enum(id) => *id,
                                _ => panic!(),
                            }),
                            _ => return self.helper().span_err(expn, format!("Unknown intrinsic {}", id)),
                        }.into()
                    },
                    ident if ident == "native_calc" || ident == "native_exec" => {
                        let control = ident == "native_exec";
                        let id = match syn::parse2::<LitInt>(tokens).and_then(|x| x.base10_parse::<i64>()) {
                            Ok(id) => id,
                            Err(e) => return self.helper().span_err(expn, e.to_string()),
                        };
                        NodeKind::simple(
                            id,
                            control as usize,
                            control as usize,
                            params.iter().map(IrKind::as_native).map(Option::unwrap).cloned().collect(),
                            ret_kinds.iter().map(IrKind::as_native).map(Option::unwrap).cloned().collect(),
                        ).into()
                    },
                    _ => None
                }
            }
            _ => None,
        })
    }
}
