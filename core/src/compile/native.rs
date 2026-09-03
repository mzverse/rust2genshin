use super::Result;
use crate::asset::node_graph::NodeKind;
use crate::asset::node_graph::arithmetic::node_divide;
use crate::asset::node_graph::execution::NODE_LOG;
use crate::asset::value::{AnyValue, ValueDefault, ValueInt};
use crate::compile::{CompilingFn, WithTcx, get_expn_macro_attr};
use rustc_attr_ir::LangItem;
use rustc_middle::query::QueryKey;
use rustc_middle::ty::{Instance, InstanceKind};
use rustc_span::Span;
use syn::{LitInt, LitStr, Meta, MetaList};

impl<'tcx> CompilingFn<'tcx, '_> {
    pub fn compile_native_call(&self, span: Span, func: Instance, params: Vec<AnyValue>, ret: Vec<AnyValue>) -> Option<Result<NodeKind>> {
        if Some(func.def_id()) == self.tcx.lang_items().get(LangItem::Panic) {
            self.tcx.dcx().span_warn(span, "Ignored panic");
            return Ok(NODE_LOG.clone()).into();
        }
        if let InstanceKind::Intrinsic(def_id) = func.def {
            return match self.tcx.intrinsic(def_id).unwrap().name.as_str() {
                "black_box" => todo!(),
                other => todo!("intrinsic: {other}") as Result<_>,
            }.into();
        }
        let def = func.default_span(self.tcx);
        let expn = def.ctxt().outer_expn().expn_data().call_site;
        match get_expn_macro_attr(self.tcx, def)?.meta {
            Meta::List(MetaList { path, tokens, .. }) => {
                match path.get_ident()?.to_string().as_str() {
                    "native" => {
                        let id = match syn::parse2::<LitStr>(tokens) {
                            Ok(id) => id.value(),
                            Err(e) => return Some(self.span_err(expn, e.to_string())),
                        };
                        match id.as_str() {
                            "divide" => Ok(node_divide(ValueInt::def())).into(),
                            _ => self.span_err(expn, format!("Unknown intrinsic {}", id)).into(),
                        }
                    },
                    ident if ident == "native_calc" || ident == "native_exec" => {
                        let control = ident == "native_exec";
                        let id = match syn::parse2::<LitInt>(tokens) {
                            Ok(id) => id,
                            Err(e) => return Some(self.span_err(expn, e.to_string())),
                        };
                        Ok(NodeKind::new(
                            match id.base10_parse::<i64>() {
                                Ok(x) => x,
                                Err(e) => return Some(self.span_err(expn, e.to_string())),
                            },
                            control as usize,
                            control as usize,
                            params,
                            ret,
                        )).into()
                    },
                    _ => None
                }
            }
            _ => None,
        }
    }
}
