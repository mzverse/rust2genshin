use panic_context::panic_context;
use super::{Result, get_expn_macro_attr};
use crate::compile::func::{CompilingFn, NodeGraphIr};
use crate::compile::ir::{node_ir_assemble, node_ir_copy, node_ir_destructure, node_ir_list_get, node_ir_list_set, node_ir_local, node_ir_modify_struct, node_ir_set_local, node_ir_to_partial_mut, partial_mut_list, struct_fields, AdtKey, FieldInfo, IrKind};
use crate::compile::link::Target;
use crate::compile::{Block, Compiler};
use crate::node::{ExportDecl, Link, NodeRef, ValueIn};
use crate::value::NativeKind;
use rustc_abi::FieldIdx;
use rustc_ast::Mutability;
use rustc_attr_ir::LangItem;
use rustc_index::Idx;
use rustc_middle::mir::{Place, PlaceTy};
use rustc_middle::query::QueryKey;
use rustc_middle::ty::{Ty, TyKind, TypingEnv};
use rustc_span::Span;
use tap::Tap;
use crate::unwrap;

#[derive(Clone, Debug)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum CompiledLocal<T> {
    Singleton(T),
    Flat(Vec<CompiledLocal<T>>),
}
impl<T: Default> Default for CompiledLocal<T> {
    fn default() -> Self {
        CompiledLocal::Singleton(T::default())
    }
}

#[derive(Clone)]
pub struct LocalRef {
    pub setter: Link,
    pub getter: Link,
    /// 这个槽存储的类型。值出口是它的视图（`partial_mut(kind)`）。
    pub kind: IrKind,
}
impl LocalRef {
    pub fn new(node: NodeRef, kind: IrKind) -> Self {
        Self {
            setter: Link::node(node, 0),
            getter: Link::node(node, 1),
            kind,
        }
    }
}
#[derive(Clone)]
pub enum CompiledPlace {
    Local(CompiledLocal<LocalRef>),
    Field(Box<CompiledPlace>, AdtKey, usize),
    /// 列表下标:`owner[index]`,元素类型记在最后。
    Index(Box<CompiledPlace>, Box<CompiledIndex>, IrKind),
}

/// 列表下标:可以是某个局部变量,也可以是编译期常量(`a[1]` 这种)。
#[derive(Clone)]
pub enum CompiledIndex {
    Place(Box<CompiledPlace>),
    Const(i32),
}
impl CompiledIndex {
    fn getter(&self, target: &Target, graph: &mut NodeGraphIr) -> ValueIn {
        match self {
            CompiledIndex::Place(place) => {
                let int = IrKind::Native(NativeKind::Int);
                let raw = place.getter(target, graph, &int);
                copy_until(graph, raw, IrKind::partial_mut(int.clone()), &int)
            }
            CompiledIndex::Const(i) => ValueIn::value((*i).into()),
        }
    }
}
#[derive(Clone, Copy)]
pub enum LocalKind {
    Ret,
    Arg,
    Other,
}
pub struct CompilingLocals<'a, 'tcx> {
    pub compiler: &'a mut Compiler<'tcx>,
    pub graph: &'a mut NodeGraphIr,
    pub block: Block,
    pub params: usize,
    pub rets: usize,
}

impl<'tcx> CompilingLocals<'_, 'tcx> {
    pub fn solve_local(&mut self, ty: Ty<'tcx>, k: LocalKind, name: String, span: Span) -> Result<(CompiledLocal<()>, CompiledLocal<LocalRef>)> {
        fn f(s: &mut CompilingLocals, kind: IrKind, k: LocalKind, name: String) -> (CompiledLocal<()>, CompiledLocal<LocalRef>) {
            let local = s.graph.insert(node_ir_local(kind.clone()));
            s.graph.set_comment(local, if name.is_empty() { None } else { name.clone().into() });
            // if kind.encode_storage(Side::Server).is_some() {
            //     self.graph.graph.set_default(Connection(local, 0), kind.clone());
            // }
            let r = CompiledLocal::Singleton(LocalRef::new(local, kind.clone()).tap(|l| {
                match k {
                    LocalKind::Ret => {
                        s.graph.push_export_value_out(ExportDecl::new(name, kind.clone().into()));
                        // 局部变量交出来的是视图;返回值要普通值,读到底。
                        let value = copy_until(
                            s.graph,
                            ValueIn::link(l.getter),
                            IrKind::partial_mut(kind.clone()),
                            &kind,
                        );
                        s.graph.link_value(value.link.unwrap(), Link::export(s.rets));
                        s.rets += 1;
                    }
                    LocalKind::Arg => {
                        s.graph.push_export_value_in(ExportDecl::new(name, kind.clone().into()));
                        let block = l.setter(s.graph, kind, ValueIn::link(Link::export(s.params)));
                        s.block.extend(s.graph, block);
                        s.params += 1;
                    }
                    LocalKind::Other => (),
                }
            }));
            (CompiledLocal::Singleton(()), r)
        }
        Ok(match ty.kind() {
            TyKind::Never => (CompiledLocal::Flat(vec![]), CompiledLocal::Flat(vec![])),
            TyKind::Tuple(es) => {
                let mut fsk = Vec::new();
                let mut fs = Vec::new();
                for (i, t) in es.iter().enumerate() {
                    let (k, r) = self.solve_local(t, k, format!("{name}.{i}"), span)?;
                    fsk.push(k);
                    fs.push(r);
                }
                (CompiledLocal::Flat(fsk), CompiledLocal::Flat(fs))
            },
            TyKind::Closure(_d, a) => self.solve_local(Ty::new_tup(self.compiler.tcx, a.as_closure().upvar_tys()), k, name, span)?,
            TyKind::Adt(d, a) if d.repr().transparent() && !self.compiler.is_native_ty(ty) =>
                self.solve_local(self.compiler.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), d.non_enum_variant().fields[FieldIdx::new(0)].ty(self.compiler.tcx, a)), k, name, span)?,
            TyKind::Adt(d, a) if let Some(r) = {
                let def = d.did().default_span(self.compiler.tcx);
                if let Some(attr) = get_expn_macro_attr(self.compiler.tcx, def) {
                    if let Some(ident) = attr.meta.path().get_ident() {
                        match ident.to_string().as_str() {
                            "event" => {
                                let mut fsk = Vec::new();
                                let mut fs = Vec::new();
                                for f in d.non_enum_variant().fields.iter() {
                                    let (k, r) = self.solve_local(self.compiler.tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), f.ty(self.compiler.tcx, a)), k, format!("{name}.{}", f.name), span)?;
                                    fsk.push(k);
                                    fs.push(r);
                                }
                                (CompiledLocal::Flat(fsk), CompiledLocal::Flat(fs)).into()
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } => r,
            // `&T` 不单独处理:`compile_ty` 已经给出正确形态(`Freeze` 时是 `T`,
            // 否则是 `PartialMut<T>`),兜底分支照着它建局部变量就行。
            TyKind::Ref(_, e, Mutability::Mut) => {
                let mut fsk = Vec::new();
                let mut fs = Vec::new();
                let kind = self.compiler.compile_ty(span, *e)?;
                let (k1, r) = f(self, IrKind::LocalRef(kind.clone().into()), k, format!("{name}.write"));
                fsk.push(k1);
                fs.push(r);
                // 读槽永远是可变视图:取值要经过 `Copy`。
                let (k1, r) = f(self, IrKind::PartialMut(kind.into()), k, format!("{name}.read"));
                fsk.push(k1);
                fs.push(r);
                (CompiledLocal::Flat(fsk), CompiledLocal::Flat(fs))
            },
            _ => {
                let kind = self.compiler.compile_ty(span, ty)?;
                f(self, kind, k, name)
            },
        })
    }
}

/// `got` 一直读下去能不能得到 `want`。
pub fn can_read_into(got: &IrKind, want: &IrKind) -> bool {
    let mut got = got.clone();
    loop {
        if got == *want {
            return true;
        }
        match got.partial_mut_inner() {
            Some(inner) => got = inner,
            None => return false,
        }
    }
}

/// 把值从 `got` 读到 `want`:每层视图插一个 `Copy`。
///
/// 局部变量的值出口永远是视图，读值没有例外；视图变量会多出几层，
/// 这里统一剥到底。
pub fn copy_until(graph: &mut NodeGraphIr, value: ValueIn, mut got: IrKind, want: &IrKind) -> ValueIn {
    let mut value = value;
    while got != *want {
        let Some(inner) = got.partial_mut_inner() else {
            panic!("类型读不到底：{got} → {want}（place 的形态和调用点的期望对不上）");
        };
        let node = graph.insert(node_ir_copy(inner.clone()));
        graph.set_value_in(Link::node(node, 0), value);
        value = ValueIn::link(Link::node(node, 0));
        got = inner;
    }
    value
}

pub trait LocalAssembleCtx<T> {
    fn leaf(&mut self, graph: &mut NodeGraphIr, kind: &IrKind, content: &T) -> ValueIn;
    fn dfs(&mut self, target: &Target, local: &CompiledLocal<T>, graph: &mut NodeGraphIr, kind: &IrKind) -> ValueIn;
}

pub trait LocalDestructureCtx<T> {
    fn leaf(&mut self, graph: &mut NodeGraphIr, kind: &IrKind, content: &T, value: ValueIn);
    fn dfs(&mut self, target: &Target, local: &CompiledLocal<T>, graph: &mut NodeGraphIr, kind: &IrKind, value: ValueIn);
}

impl<T> CompiledLocal<T> {
    fn get_fields(target: &Target, kind: &IrKind) -> Vec<IrKind> {
        struct_fields(target, kind).into_iter().map(|FieldInfo { kind, .. }| kind).collect()
    }

    pub fn assemble(&self, target: &Target, graph: &mut NodeGraphIr, kind: &IrKind, ctx: &mut impl LocalAssembleCtx<T>) -> ValueIn {
        match self {
            CompiledLocal::Singleton(v) => ctx.leaf(graph, kind, v),
            CompiledLocal::Flat(elements) => {
                let node_ref = graph.insert(node_ir_assemble(target, kind));
                let fields = Self::get_fields(target, kind);
                for (i, x) in elements.iter().enumerate() {
                    let v = ctx.dfs(target, x, graph, &fields[i]);
                    graph.set_value_in(Link::node(node_ref, i), v);
                }
                ValueIn::link(Link::node(node_ref, 0))
            }
        }
    }
    pub fn assemble_all<I: Iterator<Item = ValueIn>>(&self, target: &Target, graph: &mut NodeGraphIr, kind: &IrKind, values: &mut I) -> ValueIn {
        struct Ctx<'a, I: Iterator<Item = ValueIn>>(&'a mut I);
        impl<T, I: Iterator<Item = ValueIn>> LocalAssembleCtx<T> for Ctx<'_, I> {
            fn leaf(&mut self, _graph: &mut NodeGraphIr, _kind: &IrKind, _content: &T) -> ValueIn {
                self.0.next().unwrap()
            }
            fn dfs(&mut self, target: &Target, local: &CompiledLocal<T>, graph: &mut NodeGraphIr, kind: &IrKind) -> ValueIn {
                local.assemble_all(target, graph, kind, self.0)
            }
        }
        self.assemble(target, graph, kind, &mut Ctx(values))
    }

    pub fn destructure(&self, target: &Target, graph: &mut NodeGraphIr, kind: &IrKind, value: ValueIn, ctx: &mut impl LocalDestructureCtx<T>) {
        panic_context!("kind: {kind:?}");
        match self {
            CompiledLocal::Singleton(v) => ctx.leaf(graph, kind, v, value),
            CompiledLocal::Flat(elements) => {
                let node_ref = graph.insert(node_ir_destructure(target, kind));
                let fields = Self::get_fields(target, kind);
                graph.set_value_in(Link::node(node_ref, 0), value);
                for (i, field) in elements.iter().enumerate() {
                    ctx.dfs(target, field, graph, &fields[i], ValueIn::link(Link::node(node_ref, i)));
                }
            }
        }
    }
    pub fn destructure_all(&self, target: &Target, graph: &mut NodeGraphIr, kind: &IrKind, value: ValueIn) -> Vec<ValueIn> {
        struct Ctx(Vec<ValueIn>);
        impl<T> LocalDestructureCtx<T> for Ctx {
            fn leaf(&mut self, _graph: &mut NodeGraphIr, _kind: &IrKind, _content: &T, value: ValueIn) {
                self.0.push(value);
            }
            fn dfs(&mut self, target: &Target, local: &CompiledLocal<T>, graph: &mut NodeGraphIr, kind: &IrKind, value: ValueIn) {
                self.0.extend(local.destructure_all(target, graph, kind, value));
            }
        }
        let mut result = Ctx(vec![]);
        self.destructure(target, graph, kind, value, &mut result);
        result.0
    }
}

impl LocalRef {
    pub fn setter(&self, graph: &mut NodeGraphIr, kind: IrKind, value: ValueIn) -> Block {
        let node = graph.insert(node_ir_set_local(kind));
        graph.set_value_in(Link::node(node, 0), ValueIn::link(self.setter));
        graph.set_value_in(Link::node(node, 1), value);
        Block::singleton(node, 0)
    }
}

/// 结构体视图的类型。
fn field_view(owner_kind: &AdtKey) -> IrKind {
    IrKind::partial_mut(IrKind::Adt(owner_kind.clone()))
}

impl CompiledPlace {
    /// 取这个 place 的值。交出的一定是 `partial_mut(place 的类型)` —— 视图。
    ///
    /// 要普通值由调用方 `copy_until` 读一次。
    pub fn getter(&self, target: &Target, graph: &mut NodeGraphIr, shape: &IrKind) -> ValueIn {
        match self {
            // 单槽：出口就是变量值的视图。
            CompiledPlace::Local(CompiledLocal::Singleton(content)) => {
                ValueIn::link(content.getter)
            },
            // 多槽：先把各槽拼成整体，再标成视图。
            CompiledPlace::Local(local @ CompiledLocal::Flat(_)) => {
                struct Ctx;
                impl LocalAssembleCtx<LocalRef> for Ctx {
                    fn leaf(&mut self, graph: &mut NodeGraphIr, kind: &IrKind, content: &LocalRef) -> ValueIn {
                        // 拼装要的是字段类型；把每个槽从它的视图读到字段。
                        copy_until(
                            graph,
                            ValueIn::link(content.getter),
                            IrKind::partial_mut(content.kind.clone()),
                            kind,
                        )
                    }
                    fn dfs(&mut self, target: &Target, local: &CompiledLocal<LocalRef>, graph: &mut NodeGraphIr, kind: &IrKind) -> ValueIn {
                        local.assemble(target, graph, kind, self)
                    }
                }
                let raw = Ctx.dfs(target, local, graph, shape);
                let view = graph.insert(node_ir_to_partial_mut(shape.clone()));
                graph.set_value_in(Link::node(view, 0), raw);
                ValueIn::link(Link::node(view, 0))
            },
            CompiledPlace::Field(owner, owner_kind, ele) => {
                // 解构的是结构体的视图:取出来的字段也是视图。
                let view = field_view(owner_kind);
                let node = graph.insert(node_ir_destructure(target, &view));
                let v = owner.getter(target, graph, &IrKind::Adt(owner_kind.clone()));
                graph.set_value_in(Link::node(node, 0), v);
                ValueIn::link(Link::node(node, *ele))
            },
            CompiledPlace::Index(owner, index, elem) => {
                // 视图穿透:owner 交出来的是列表的视图,元素也按视图给。
                let node = graph.insert(node_ir_list_get(
                    partial_mut_list(elem),
                    IrKind::partial_mut(elem.clone()),
                ));
                let v = owner.getter(target, graph, &IrKind::list(elem.clone()));
                graph.set_value_in(Link::node(node, 0), v);
                let i = index.getter(target, graph);
                graph.set_value_in(Link::node(node, 1), i);
                ValueIn::link(Link::node(node, 0))
            },
        }
    }

    pub fn setter(&self, target: &mut Target, graph: &mut NodeGraphIr, kind: &IrKind, value: ValueIn) -> Block {
        match self {
            CompiledPlace::Local(local) => {
                struct Ctx(Block);
                impl LocalDestructureCtx<LocalRef> for Ctx {
                    fn leaf(&mut self, graph: &mut NodeGraphIr, kind: &IrKind, content: &LocalRef, value: ValueIn) {
                        let block = content.setter(graph, kind.clone(), value);
                        self.0.extend(graph, block)
                    }

                    fn dfs(&mut self, target: &Target, local: &CompiledLocal<LocalRef>, graph: &mut NodeGraphIr, kind: &IrKind, value: ValueIn) {
                        local.destructure(target, graph, kind, value, self)
                    }
                }
                Ctx(Block::nop(graph)).tap_mut(|result| result.dfs(target, local, graph, kind, value)).0
            },
            CompiledPlace::Field(owner, owner_kind, ele) => {
                let kind = field_view(owner_kind);
                let node = graph.insert(node_ir_modify_struct(target, &kind));
                let v = owner.getter(target, graph, &IrKind::Adt(owner_kind.clone()));
                graph.set_value_in(Link::node(node, 0), v);
                let index = 2 + *ele * 2;
                graph.set_value_in(Link::node(node, index), value);
                let index = 3 + *ele * 2;
                graph.set_value_in(Link::node(node, index), ValueIn::value(true.into()));
                Block::singleton(node, 0)
            },
            CompiledPlace::Index(owner, index, elem) => {
                // 列表是句柄,按下标改值就地生效,不需要写回 owner。
                let node = graph.insert(node_ir_list_set(partial_mut_list(elem), elem.clone()));
                let v = owner.getter(target, graph, &IrKind::list(elem.clone()));
                graph.set_value_in(Link::node(node, 0), v);
                let i = index.getter(target, graph);
                graph.set_value_in(Link::node(node, 1), i);
                graph.set_value_in(Link::node(node, 2), value);
                Block::singleton(node, 0)
            },
        }
    }
}

impl<'tcx> CompilingFn<'tcx, '_> {
    pub fn compile_place(&mut self, place: Place<'tcx>, span: Span) -> Result<CompiledPlace> {
        let mut result = CompiledPlace::Local(self.locals.get(place.local).unwrap().clone());
        let mut ty = PlaceTy::from_ty(self.mono(self.body.local_decls.get(place.local).unwrap().ty));
        for x in place.projection {
            use rustc_middle::mir::ProjectionElem::*;
            match x {
                Field(i, _) => {
                    match ty.ty.kind() {
                        TyKind::Adt(d, _a) if d.repr().transparent() => {
                            assert_eq!(i, FieldIdx::new(0));
                            // do nothing
                        },
                        TyKind::Adt(d, a) if self.compiler.get_default_some(*d, a)?.is_some() => {
                            assert_eq!(d.variant(ty.variant_index.unwrap()).def_id, self.tcx.lang_items().get(LangItem::OptionSome).unwrap());
                            assert_eq!(i, FieldIdx::new(0));
                            // do nothing
                        },
                        _ if let Some(variant_index) = ty.variant_index => {
                            let kind = unwrap!(self.compiler.compile_ty(span, ty.ty)?, IrKind::Adt);
                            let i = self.compiler.target.adts[&kind].variants[variant_index.index()][i.index()];
                            result = CompiledPlace::Field(result.into(), kind, i);
                        },
                        _ => match result {
                            CompiledPlace::Local(CompiledLocal::Flat(v)) => result = CompiledPlace::Local(v.into_iter().nth(i.index()).unwrap()),
                            other => result = CompiledPlace::Field(other.into(), unwrap!(self.compiler.compile_ty(span, ty.ty)?, IrKind::Adt), i.index()),
                        },
                    }
                },
                Deref => match ty.ty.ref_mutability() {
                    // 原始指针的解引用在节点图里没有对应表示(类型已经落成
                    // List / 标量),按值直通,不额外造节点。
                    None => (),
                    Some(Mutability::Mut) => {
                        let kind = self.compiler.compile_ty(span, ty.ty)?;
                        let IrKind::Mut(inner) = kind.clone() else {
                            return self.helper().span_err(span, format!("Deref of non-mut reference: {kind}"));
                        };
                        let de = self.graph.insert(node_ir_destructure(&self.compiler.target, &kind));
                        let raw = result.getter(&self.compiler.target, self.graph, &kind);
                        let getter = copy_until(self.graph, raw, IrKind::partial_mut(kind.clone()), &kind);
                        self.graph.set_value_in(Link::node(de, 0), getter);
                        result = CompiledPlace::Local(CompiledLocal::Singleton(LocalRef::new(de, *inner)));
                    },
                    // 不可变引用:读值直接沿用原 place,不需要节点。
                    Some(Mutability::Not) => (),
                },
                Downcast(_, _id) => {
                    // do nothing
                }
                Index(i) => {
                    let elem = match ty.ty.kind() {
                        TyKind::Array(e, _) => self.compiler.compile_ty(span, *e)?,
                        other => return self.helper().span_err(span, format!("Unsupported index base type: {other:?}")),
                    };
                    let index = CompiledIndex::Place(Box::new(CompiledPlace::Local(self.locals.get(i).unwrap().clone())));
                    result = CompiledPlace::Index(result.into(), index.into(), elem);
                }
                // `a[1]` 这种常量下标:rustc 直接用 `ConstantIndex`,不经过局部变量。
                ConstantIndex { offset, from_end, .. } => {
                    if from_end {
                        // 数组不会从末尾数,这一档只出现在切片模式里
                        return self.helper().span_err(span, "Unsupported index from end");
                    }
                    let elem = match ty.ty.kind() {
                        TyKind::Array(e, _) => self.compiler.compile_ty(span, *e)?,
                        other => return self.helper().span_err(span, format!("Unsupported index base type: {other:?}")),
                    };
                    result = CompiledPlace::Index(result.into(), CompiledIndex::Const(offset as i32).into(), elem);
                }
                other => todo!("{other:?}")
            }
            ty = ty.projection_ty(self.tcx, x);
        }
        Ok(result)
    }
}
