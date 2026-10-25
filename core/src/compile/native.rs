use panic_context::panic_context;
use super::{Compiler, Result};
use crate::compile::func::node_ir_native;
use crate::compile::get_expn_macro_attr;
use crate::compile::ir::{IrKind, IrNodeId, NodeKindIr, node_ir_list_get, node_ir_list_get_length, node_ir_list_insert, node_ir_list_set};
use crate::node::NodeKind;
use crate::node::arithmetic::{node_cast, node_enum_equal, node_power};
use crate::value::{NativeKind, NativeValue};
use rustc_middle::query::QueryKey;
use rustc_middle::ty::{FnSig, GenericArgsRef, Instance, ScalarInt, Ty, TyKind};
use rustc_span::Span;
use syn::{LitInt, LitStr, Meta, MetaList};

/// 类型定义上的原生标记。
enum NativeMark {
    /// `#[native("名字")]`
    Named(String),
    /// `#[native_enum(N)]`
    Enum(i32),
}

/// 泛型实参里的类型参数（跳过生命周期和 const 参数）。
///
/// `GenericArgsRef::type_at` 是按原始下标取的 —— `PartialMut<'a, T>` 的 0 号
/// 是生命周期，取不到类型。
fn type_args<'tcx>(args: GenericArgsRef<'tcx>) -> Vec<Ty<'tcx>> {
    args.iter().filter_map(|x| x.as_type()).collect()
}

pub fn native_const(kind: &NativeKind, v: ScalarInt) -> Option<Box<dyn NativeValue>> {    Some(match kind {
        NativeKind::Int => v.to_i32().into(),
        NativeKind::Guid |
        NativeKind::Faction |
        NativeKind::Config |
        NativeKind::Prefab => v.to_i64().into(),
        NativeKind::Enum(_) => v.to_i32().into(),
        _ => return None,
    })
}

impl<'tcx> Compiler<'tcx> {
    /// 类型定义上的原生标记(不看泛型实参)。
    fn native_mark(&self, ty: Ty<'tcx>) -> Option<Result<NativeMark>> {
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
                            Err(e) => return Some(self.helper().span_err(expn, e.to_string())),
                        };
                        Some(Ok(NativeMark::Named(id)))
                    },
                    "native_enum" => {
                        let id = match syn::parse2::<LitInt>(tokens).and_then(|x| x.base10_parse::<i32>()) {
                            Ok(id) => id,
                            Err(e) => return Some(self.helper().span_err(expn, e.to_string())),
                        };
                        Some(Ok(NativeMark::Enum(id)))
                    },
                    _ => None,
                }
            },
            _ => None,
        }
    }

    /// 类型上有没有原生标记(`#[native]` / `#[native_enum]`)。
    pub fn is_native_ty(&self, ty: Ty<'tcx>) -> bool {
        self.native_mark(ty).is_some()
    }

    /// 原生类型的 IR 类型。
    ///
    /// 泛型原生类型(`List<T>`)在这里把泛型实参解析出来,元素类型落到
    /// `IrKind::List` 里 —— 这样 `[T; N]` 和 `List<T>` 是同一个类型,
    /// 两边之间的转换就是空转换。
    pub fn compile_native_ty(&mut self, span: Span, ty: Ty<'tcx>) -> Option<Result<IrKind>> {
        let mark = match self.native_mark(ty)? {
            Ok(mark) => mark,
            Err(e) => return Some(Err(e)),
        };
        let args = match ty.kind() {
            TyKind::Adt(_, a) => Some(*a),
            _ => None,
        };
        Some(match mark {
            NativeMark::Enum(id) => Ok(IrKind::Native(NativeKind::Enum(id))),
            NativeMark::Named(id) => match id.as_str() {
                "Guid" => Ok(IrKind::Native(NativeKind::Guid)),
                "Entity" => Ok(IrKind::Native(NativeKind::Entity)),
                "Faction" => Ok(IrKind::Native(NativeKind::Faction)),
                "Config" => Ok(IrKind::Native(NativeKind::Config)),
                "Prefab" => Ok(IrKind::Native(NativeKind::Prefab)),
                "Vec3" => Ok(IrKind::Native(NativeKind::Vec3)),
                "PartialMut" => {
                    // `PartialMut<'a, T>`:可变视图。元素类型就是 `T`。
                    let Some(inner) = args.as_ref().and_then(|args| type_args(*args).into_iter().next()) else {
                        return Some(self.helper().span_err(span, "PartialMut 原生类型缺少泛型实参"));
                    };
                    let inner = match self.compile_ty(span, inner) {
                        Ok(inner) => inner,
                        Err(e) => return Some(Err(e)),
                    };
                    Ok(IrKind::partial_mut(inner))
                },
                "List" => {
                    let Some(elem) = args.as_ref().and_then(|args| type_args(*args).into_iter().next()) else {
                        return Some(self.helper().span_err(span, "List 原生类型缺少泛型实参"));
                    };
                    let elem = match self.compile_ty(span, elem) {
                        Ok(elem) => elem,
                        Err(e) => return Some(Err(e)),
                    };
                    Ok(IrKind::list(elem))
                },
                "Dict" => {
                    let Some(args) = args.as_ref().map(|args| type_args(*args)) else {
                        return Some(self.helper().span_err(span, "Dict 原生类型缺少泛型实参"));
                    };
                    let (Some(key_ty), Some(value_ty)) = (args.first(), args.get(1)) else {
                        return Some(self.helper().span_err(span, "Dict 原生类型缺少泛型实参"));
                    };
                    let key = match self.compile_ty(span, *key_ty) {
                        Ok(key) => key,
                        Err(e) => return Some(Err(e)),
                    };
                    let value = match self.compile_ty(span, *value_ty) {
                        Ok(value) => value,
                        Err(e) => return Some(Err(e)),
                    };
                    let (Some(key), Some(value)) = (key.as_native(), value.as_native()) else {
                        return Some(self.helper().span_err(span, format!("Unsupported dict type: {key} / {value}")));
                    };
                    Ok(IrKind::Native(NativeKind::dict(key.clone(), value.clone())))
                },
                _ => return Some(self.helper().span_err(span, format!("Unknown native kind: {id}"))),
            },
        })
    }

    /// 列表内建的第一个参数是 `&List<T>`。
    ///
    /// `T` 是否 `Freeze` 决定它编成视图 `PartialMut<List<T>>` 还是值
    /// `List<T>` —— 两种都可能，返回 `(第一个引脚的实际类型, 元素类型)`。
    fn expect_list(&self, expn: Span, name: &str, kind: &IrKind) -> Result<(IrKind, IrKind)> {
        if let IrKind::List(elem) = kind {
            return Ok((kind.clone(), (**elem).clone()));
        }
        if let Some(inner) = kind.partial_mut_inner()
            && let Some(elem) = inner.list_elem()
        {
            // 引脚类型保持原样（视图还是视图），只把元素类型取出来。
            return Ok((kind.clone(), elem));
        }
        self.helper().span_err(expn, format!("{name} 的第一个参数应当是 &List<T>,实际是 {kind}"))
    }

    /// 原生调用的名字(`#[native("名字")]` 里的那个名字)。
    pub fn native_call_id(&self, func: Instance<'tcx>) -> Option<String> {
        let def = func.default_span(self.tcx);
        let attr = get_expn_macro_attr(self.tcx, def)?;
        let Meta::List(MetaList { path, tokens, .. }) = &attr.meta else {
            return None;
        };
        if path.get_ident()?.to_string() != "native" {
            return None;
        }
        Some(syn::parse2::<LitStr>(tokens.clone()).ok()?.value())
    }

    pub fn compile_native_call(&mut self, span: Span, func: Instance<'tcx>, sig: FnSig<'tcx>, params: &[IrKind]) -> Result<Option<NodeKindIr>> {
        panic_context!("Compiling native call: {func:?}");
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
                            "power" => node_ir_native(node_power(params[0].as_native().unwrap().clone())),
                            "to_string" => node_ir_native(node_cast(params[0].as_native().unwrap().clone(), NativeKind::String).unwrap()),
                            "enum_eq" => node_ir_native(node_enum_equal(match params[0].as_native().unwrap() {
                                NativeKind::Enum(id) => *id,
                                _ => panic!(),
                            })),
                            "free" => {
                                // 目标平台没有手动内存管理,释放是空操作。
                                // 引脚照样接上,免得调用点的连线无处可去。
                                NodeKind::new(
                                    IrNodeId::Nop,
                                    1,
                                    1,
                                    params.iter().map(|x| Some(x.clone())).collect(),
                                    vec![],
                                )
                            },
                            "list_get" => {
                                let (list, elem) = self.expect_list(expn, "list_get", &params[0])?;
                                node_ir_list_get(list, elem)
                            },
                            "list_set" => {
                                let (list, elem) = self.expect_list(expn, "list_set", &params[0])?;
                                node_ir_list_set(list, elem)
                            },
                            "list_insert" => {
                                let (list, elem) = self.expect_list(expn, "list_insert", &params[0])?;
                                node_ir_list_insert(list, elem)
                            },
                            "list_len" => {
                                let (list, _) = self.expect_list(expn, "list_len", &params[0])?;
                                node_ir_list_get_length(list)
                            },
                            _ => return self.helper().span_err(expn, format!("Unknown intrinsic {}", id)),
                        }.into()
                    },
                    ident if ident == "native_calc" || ident == "native_exec" => {
                        let control = ident == "native_exec";
                        let id = match syn::parse2::<LitInt>(tokens).and_then(|x| x.base10_parse::<i64>()) {
                            Ok(id) => id,
                            Err(e) => return self.helper().span_err(expn, e.to_string()),
                        };
                        node_ir_native(NodeKind::simple(
                            id,
                            control as usize,
                            control as usize,
                            params.iter().map(IrKind::to_native).map(Option::unwrap).collect(),
                            ret_kinds.iter().map(IrKind::to_native).map(Option::unwrap).collect(),
                        )).into()
                    },
                    _ => None
                }
            }
            _ => None,
        })
    }
}
