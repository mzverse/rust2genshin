use super::Result;
use crate::asset::node_graph::arithmetic::NodeDivide;
use crate::asset::node_graph::execution::NodeLog;
use crate::asset::node_graph::{ControlOut, Node, NodeRef, Simulation, ValueIn};
use crate::asset::raw_node_graph::NodeType;
use crate::asset::value::AnyValue;
use crate::compile::{CompilingFn, WithTcx, get_expn_macro_attr};
use rustc_attr_ir::LangItem;
use rustc_middle::query::QueryKey;
use rustc_middle::ty::{Instance, InstanceKind};
use rustc_span::Span;
use syn::{LitInt, LitStr, Meta, MetaList};

struct NodeNativeExec {
    id: i64,
    controls_in: i32,
    controls_out: Vec<ControlOut>,
    values_in: Vec<ValueIn>,
    values_out: Vec<AnyValue>,
}
impl Node for NodeNativeExec {
    fn get_controls_in(&self) -> i32 {
        self.controls_in
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        self.controls_out.clone()
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        self.values_in.clone()
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        self.values_out.clone()
    }

    fn execute(&mut self, context: &mut Simulation) -> anyhow::Result<Vec<NodeRef>> {
        panic!("Unknown logic")
    }
    fn get_value(&self, index: i32, context: &Simulation) -> anyhow::Result<AnyValue> {
        panic!("Unknown logic")
    }

    fn get_type(&self) -> NodeType {
        NodeType::simple(self.id)
    }

    fn get_controls_out_mut(&mut self) -> Vec<&mut ControlOut> {
        self.controls_out.iter_mut().collect()
    }
}

impl<'tcx> CompilingFn<'tcx, '_> {
    pub fn compile_native_call(&self, span: Span, func: Instance, values_in: Vec<ValueIn>, values_out: Vec<AnyValue>) -> Option<Result<Box<dyn Node>>> {
        if Some(func.def_id()) == self.tcx.lang_items().get(LangItem::Panic) {
            self.tcx.dcx().span_warn(span, "Ignored panic");
            return Some(Ok(NodeLog { value: values_in.into_iter().next().unwrap(), next: ControlOut::new() }.into()));
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
                        let mut values_in = values_in.into_iter();
                        match id.as_str() {
                            "divide" => Ok(NodeDivide { a: values_in.next().unwrap(), b: values_in.next().unwrap() }.into()).into(),
                            _ => return Some(self.span_err(expn, format!("Unknown intrinsic {}", id))),
                        }
                    },
                    ident if ident == "native_calc" || ident == "native_exec" => {
                        let control = ident == "native_exec";
                        let id = match syn::parse2::<LitInt>(tokens) {
                            Ok(id) => id,
                            Err(e) => return Some(self.span_err(expn, e.to_string())),
                        };
                        Some(Ok(NodeNativeExec {
                            id: match id.base10_parse::<i64>() {
                                Ok(x) => x,
                                Err(e) => return Some(self.span_err(expn, e.to_string())),
                            },
                            controls_in: control as i32,
                            controls_out: vec![ControlOut::new(); control as usize],
                            values_in,
                            values_out,
                        }.into()))
                    },
                    _ => None
                }
            }
            _ => None,
        }
    }
}
