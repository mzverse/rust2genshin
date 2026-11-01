use panic_context::panic_context;
use super::{Compiler, Result};
use crate::compile::func::node_ir_native;
use crate::compile::ir::{IrKind, IrNodeId, NodeKindIr, node_ir_list_get, node_ir_list_get_length, node_ir_list_insert, node_ir_list_set};
use crate::node::NodeKind;
use crate::node::arithmetic::{node_cast, node_enum_equal, node_power};
use crate::value::{NativeKind, NativeValue};
use rustc_attr_ir::{AttrArgs, Attribute};
use rustc_middle::query::QueryKey;
use rustc_middle::ty::{FnSig, GenericArgsRef, Instance, ScalarInt, Ty, TyCtxt, TyKind};
use rustc_span::def_id::DefId;
use rustc_span::Span;

/// tool attribute 的名字 —— 与 `lib-internal` 的 `TOOL` 一致。
const TOOL: &str = "rust2genshin";

/// 一条 tool attribute 的内容。
///
/// `register_tool` 注册的属性编译器不消耗,原样留在 HIR 里,后端用
/// `tcx.get_all_attrs()` 直接读 —— 不用回源码里抠 span。
pub enum ToolMark {
    /// `#[rust2genshin::call("名字")]`
    Call(String),
    /// `#[rust2genshin::calc(N)]`
    Calc(i64),
    /// `#[rust2genshin::exec(N)]`
    Exec(i64),
    /// `#[rust2genshin::ty("名字")]`
    Ty(String),
    /// `#[rust2genshin::enumty(N)]` —— 叫 `enumty` 而不是 `enum`,`enum` 是关键字
    Enum(i64),
    /// `#[rust2genshin::event(N)]`
    Event(i64),
    /// `#[rust2genshin::listener]` —— 无参数
    Listener,
}

/// 在一个定义上找 tool attribute。
///
/// 找到第一个 `rust2genshin::*` 的就返回 —— 一个定义上只该有一条。
///
/// 解析失败时返回 `(错误说明, 出错的 span)`,由调用方用 `span_err` 报出来 ——
/// 这里拿不到 `Compiler`(只是个自由函数),没法直接发诊断。
fn tool_mark(tcx: TyCtxt<'_>, def: DefId) -> Option<std::result::Result<ToolMark, (String, Span)>> {
    for attr in tcx.get_all_attrs(def).iter() {
        // tool attribute 是 `Unparsed` 的 —— 编译器不认得它,原样留着。
        let Attribute::Unparsed(item) = attr else {
            continue;
        };
        let segments = item.path.segments.as_ref();
        if segments.first().map(|x| x.as_str()) != Some(TOOL) {
            continue;
        }
        let span = item.span;
        let Some(kind) = segments.last().map(|x| x.as_str()) else {
            return Some(Err(("tool attribute 缺少名字".to_string(), span)));
        };
        // `listener` 没有参数。
        if kind == "listener" {
            return Some(Ok(ToolMark::Listener));
        }
        // 参数是 `(...)` 括起来的 token 流。参数总是**一个字面量**
        // (字符串或整数),用 rustc 自己的 AST 解析。
        let AttrArgs::Delimited(args) = &item.args else {
            return Some(Err((format!("`{kind}` 需要括号里的字面量参数"), span)));
        };
        let Some(lit) = rustc_ast::ast::MetaItemKind::list_from_tokens(args.tokens.clone()).and_then(|list| {
            list.into_iter().next().and_then(|x| match x {
                rustc_ast::ast::MetaItemInner::Lit(lit) => Some(lit),
                rustc_ast::ast::MetaItemInner::MetaItem(_) => None,
            })
        }) else {
            return Some(Err((format!("`{kind}` 的参数应当是一个字面量"), span)));
        };
        // `symbol` 是字面量的**原文** —— 字符串不带引号,整数是十进制。
        let text = lit.symbol.to_string();
        let num = || match text.parse::<i64>() {
            Ok(id) => Ok(id),
            Err(_) => Err((format!("`{kind}` 的参数应当是整数,实际是 {text}"), span)),
        };
        return Some(match kind {
            "call" => Ok(ToolMark::Call(text)),
            "ty" => Ok(ToolMark::Ty(text)),
            "calc" => num().map(ToolMark::Calc),
            "exec" => num().map(ToolMark::Exec),
            "enumty" => num().map(ToolMark::Enum),
            "event" => num().map(ToolMark::Event),
            _ => Err((format!("未知的 tool attribute `{kind}`"), span)),
        });
    }
    None
}

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
    /// 一个定义上的 tool attribute。
    pub fn tool_mark(&self, def: DefId) -> Option<std::result::Result<ToolMark, (String, Span)>> {
        tool_mark(self.tcx, def)
    }

    /// 类型定义上的原生标记(不看泛型实参)。
    fn native_mark(&self, ty: Ty<'tcx>) -> Option<Result<NativeMark>> {
        let def = match ty.kind() {
            TyKind::Foreign(d) => *d,
            TyKind::Adt(d, _a) =>
                d.did(),
            _ => return None,
        };
        let expn = def.default_span(self.tcx);
        let mark = match tool_mark(self.tcx, def)? {
            Ok(mark) => mark,
            Err((msg, span)) => return Some(self.helper().span_err(span, msg)),
        };
        Some(match mark {
            ToolMark::Ty(id) => Ok(NativeMark::Named(id)),
            ToolMark::Enum(id) => Ok(NativeMark::Enum(id as i32)),
            // 别的 tool attribute(`event` / `listener` …)不代表「这是原生类型」,
            // 它们挂在普通结构体 / 函数上 —— 这里返回「不是原生类型」,
            // 让 `compile_ty` 走普通 ADT 那条路。
            _ => return None,
        })
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
    ///
    /// `&mut List<T>`(`Mut` 两槽元组)不在这里处理:拆元组要拿到参数**值**,
    /// 而这里只拿得到类型。`List::remove` 那条走 `compile_call` 的特判。
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
    ///
    /// 只认 `call` 档 —— `calc` / `exec` 走的是节点 ID 那条路。
    pub fn native_call_id(&self, func: Instance<'tcx>) -> Option<String> {
        match tool_mark(self.tcx, func.def_id())? {
            Ok(ToolMark::Call(id)) => Some(id),
            _ => None,
        }
    }

    pub fn compile_native_call(&mut self, span: Span, func: Instance<'tcx>, sig: FnSig<'tcx>, params: &[IrKind]) -> Result<Option<NodeKindIr>> {
        panic_context!("Compiling native call: {func:?}");
        let ret = sig.output();
        let ret_kinds = if let TyKind::Tuple(ele) = ret.kind() {
            ele.iter().collect()
        } else {
            vec![ret]
        }.iter().map(|x| self.compile_ty(span, *x)).collect::<Result<Vec<_>>>()?;
        let expn = func.def_id().default_span(self.tcx);
        let mark = match tool_mark(self.tcx, func.def_id()) {
            Some(mark) => match mark {
                Ok(mark) => mark,
                Err((msg, span)) => return self.helper().span_err(span, msg),
            },
            None => return Ok(None),
        };
        Ok(match mark {
            ToolMark::Call(id) => {
                match id.as_str() {
                            // 这几个要求参数是原生类型;拿到视图说明用法不对,
                            // 交给普通函数那条路(它会报更清楚的错)。
                            "power" => match params[0].as_native() {
                                Some(kind) => node_ir_native(node_power(kind.clone())),
                                None => return Ok(None),
                            },
                            "to_string" => match params[0].as_native().and_then(|x| node_cast(x.clone(), NativeKind::String)) {
                                Some(node) => node_ir_native(node),
                                None => return Ok(None),
                            },
                            "enum_eq" => match params[0].as_native() {
                                Some(NativeKind::Enum(id)) => node_ir_native(node_enum_equal(*id)),
                                _ => return Ok(None),
                            },
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
                                // 取到的可能是值(`&T`)也可能是视图(`PartialMut<T>`),
                                // 以声明的返回类型为准。
                                let out = match ret_kinds.first() {
                                    Some(out @ IrKind::PartialMut(inner)) if **inner == elem => out.clone(),
                                    _ => elem,
                                };
                                node_ir_list_get(list, out)
                            },
                            "list_set" => {
                                let (list, elem) = self.expect_list(expn, "list_set", &params[0])?;
                                node_ir_list_set(list, elem)
                            },
                            "list_insert" => {
                                let (list, elem) = self.expect_list(expn, "list_insert", &params[0])?;
                                node_ir_list_insert(list, elem)
                            },                            "list_len" => {
                                let (list, _) = self.expect_list(expn, "list_len", &params[0])?;
                                node_ir_list_get_length(list)
                            },
                            // 图变量读写。名字由调用点在 `compile_call` 里从
                            // 常量参数取出来填进 IR 节点,这里只给形状。
                            // 图变量读写要**变量名的字面量**,那个只有调用点的
                            // 常量实参里才有,这里拿不到 —— 交给 `compile_call`
                            // 特判。
                            | "graph_variable_get"
                            | "graph_variable_set"
                            => return Ok(None),
                            // 有些内建要拆 `&mut` 的元组、要拿到参数**值**,
                            // 这里只拿得到类型 —— 它们在 `compile_call` 里特判
                            // (`partial_mut_*` 系、`list_remove`),这里认出来
                            // 交给后面,不要报错。
                            | "partial_mut_from"
                            | "partial_mut_into_mut"
                            | "partial_mut_deref"
                            | "partial_mut_into_ref"
                            | "partial_mut_deref_mut"
                            | "list_remove"
                            | "identity"
                            // `Box` 的堆访问:变量名要按 `T` 拼,还得拿到
                            // 泛型实参,`compile_native_call` 都没有。
                            | "box_heap_get"
                            | "box_heap_set"
                            | "box_free_get"
                            | "box_free_set"
                            | "box_free"
                            => return Ok(None),
                            _ => return self.helper().span_err(expn, format!("Unknown intrinsic {}", id)),
                        }.into()
                    },
            ToolMark::Calc(id) | ToolMark::Exec(id) => {
                        let control = matches!(mark, ToolMark::Exec(_));
                        // 引脚的标称类型必须是原生类型:有视图 / 结构体混进来
                        // 就说明这个原生调用不该在这里合成。
                        let (Some(params), Some(rets)) = (
                            params.iter().map(IrKind::to_native).collect::<Option<Vec<_>>>(),
                            ret_kinds.iter().map(IrKind::to_native).collect::<Option<Vec<_>>>(),
                        ) else {
                            return Ok(None);
                        };
                        node_ir_native(NodeKind::simple(
                            id,
                            control as usize,
                            control as usize,
                            params,
                            rets,
                        )).into()
                    },
            _ => None,
        })
    }
}
