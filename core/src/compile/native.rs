use super::{Compiler, Result};
use crate::asset::node_graph::NodeKind;
use crate::asset::node_graph::arithmetic::{node_cast, node_divide, node_enum_equal, node_power};
use crate::asset::node_graph::execution::NODE_LOG;
use crate::asset::value::{AnyValue, ValueDefault, ValueEntity, ValueEnum, ValueGuid, ValueInt, ValueString};
use crate::compile::{WithTcx, get_expn_macro_attr};
use rustc_attr_ir::LangItem;
use rustc_middle::query::QueryKey;
use rustc_middle::ty::{Instance, InstanceKind, Ty, TyCtxt, TyKind};
use rustc_span::Span;
use syn::{LitInt, LitStr, Meta, MetaList};

impl<'tcx> Compiler<'tcx> {
    pub fn compile_native_ty(&self, ty: Ty<'tcx>) -> Option<Result<AnyValue>> {
        match ty.kind() {
            TyKind::Adt(d, _a) => {
                let def = d.did().default_span(self.tcx);
                let expn = def.ctxt().outer_expn().expn_data().call_site;
                match get_expn_macro_attr(self.tcx, def)?.meta {
                    Meta::List(MetaList { path, tokens, .. }) => {
                        match path.get_ident()?.to_string().as_str() {
                            "native" => {
                                let id = match syn::parse2::<LitStr>(tokens) {
                                    Ok(id) => id.value(),
                                    Err(e) => return self.span_err(expn, e.to_string()).into(),
                                };
                                Ok(match id.as_str() {
                                    "Guid" => ValueGuid::def(),
                                    "Entity" => ValueEntity::def(),
                                    _ => return self.span_err(expn, format!("Unknown intrinsic {}", id)).into(),
                                }).into()
                            },
                            "native_enum" => {
                                let id = match syn::parse2::<LitInt>(tokens).and_then(|x| x.base10_parse::<i32>()) {
                                    Ok(id) => id,
                                    Err(e) => return self.span_err(expn, e.to_string()).into(),
                                };
                                Ok(ValueEnum::new(id, 0).into()).into()
                            },
                            _ => None,
                        }
                    },
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

pub fn compile_native_call(tcx: TyCtxt, span: Span, func: Instance, params: Vec<AnyValue>, ret: Option<AnyValue>) -> Option<Result<NodeKind>> {
    if Some(func.def_id()) == tcx.lang_items().get(LangItem::Panic) {
        tcx.dcx().span_note(span, "Ignored panic");
        return Ok(NODE_LOG.clone()).into();
    }
    if let InstanceKind::Intrinsic(def_id) = func.def {
        let _: Result<NodeKind> = match tcx.intrinsic(def_id).unwrap().name.as_str() {
            "black_box" => todo!(),
            other => todo!("intrinsic: {other}"),
        };
    }
    let def = func.default_span(tcx);
    let expn = def.ctxt().outer_expn().expn_data().call_site;
    match get_expn_macro_attr(tcx, def)?.meta {
        Meta::List(MetaList { path, tokens, .. }) => {
            match path.get_ident()?.to_string().as_str() {
                "native" => {
                    let id = match syn::parse2::<LitStr>(tokens) {
                        Ok(id) => id.value(),
                        Err(e) => return Err(tcx.dcx().span_err(expn, e.to_string())).into(),
                    };
                    Ok(match id.as_str() {
                        "divide" => node_divide(ValueInt::def()),
                        "power" => node_power(params[0].clone()),
                        "to_string" => node_cast(params[0].clone(), ValueString::def()).unwrap(),
                        "enum_eq" => node_enum_equal(params[0].downcast_ref().unwrap()),
                        _ => return Err(tcx.dcx().span_err(expn, format!("Unknown intrinsic {}", id))).into(),
                    }).into()
                },
                ident if ident == "native_calc" || ident == "native_exec" => {
                    let control = ident == "native_exec";
                    let id = match syn::parse2::<LitInt>(tokens).and_then(|x| x.base10_parse::<i64>()) {
                        Ok(id) => id,
                        Err(e) => return Err(tcx.dcx().span_err(expn, e.to_string())).into(),
                    };
                    Ok(NodeKind::new(
                        id,
                        control as usize,
                        control as usize,
                        params,
                        ret.into_iter().collect(),
                    )).into()
                },
                _ => None
            }
        }
        _ => None,
    }
}
