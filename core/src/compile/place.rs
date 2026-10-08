use panic_context::panic_context;
use super::{Result, get_expn_macro_attr};
use crate::compile::func::{CompilingFn, NodeGraphIr};
use crate::compile::ir::{node_ir_assemble, node_ir_destructure, node_ir_local, node_ir_modify_struct, node_ir_set_local, struct_fields, AdtKey, FieldInfo, IrKind};
use crate::compile::link::Target;
use crate::compile::{Block, Compiler};
use crate::node::{ExportDecl, Link, NodeRef, ValueIn};
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

#[derive(Clone, Copy)]
pub struct LocalRef {
    pub setter: Link,
    pub getter: Link,
}
impl<'tcx> LocalRef {
    pub fn node(node: NodeRef) -> Self {
        Self {
            setter: Link::node(node, 0),
            getter: Link::node(node, 1),
        }
    }
}
#[derive(Clone)]
pub enum CompiledPlace {
    Local(CompiledLocal<LocalRef>),
    Field(Box<CompiledPlace>, AdtKey, usize),
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
            let local = s.graph.insert(node_ir_local(&kind));
            // if kind.encode_storage(Side::Server).is_some() {
            //     self.graph.graph.set_default(Connection(local, 0), kind.clone());
            // }
            let r = CompiledLocal::Singleton(LocalRef::node(local).tap(|l| {
                match k {
                    LocalKind::Ret => {
                        s.graph.push_export_value_out(ExportDecl::new(name, kind.into()));
                        s.graph.link_value(l.getter, Link::export(s.rets));
                        s.rets += 1;
                    }
                    LocalKind::Arg => {
                        s.graph.push_export_value_in(ExportDecl::new(name, kind.clone().into()));
                        let block = l.setter(s.graph, &kind, ValueIn::link(Link::export(s.params)));
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
            TyKind::Adt(d, a) if d.repr().transparent() && self.compiler.compile_native_ty(ty).is_none() =>
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
            TyKind::Ref(_, e, Mutability::Not) if !e.is_never() =>
                self.solve_local(*e, k, name, span)?,
            TyKind::Ref(_, e, Mutability::Mut) => {
                let mut fsk = Vec::new();
                let mut fs = Vec::new();
                let kind = self.compiler.compile_ty(span, *e)?;
                let (k1, r) = f(self, IrKind::LocalRef(kind.clone().into()), k, format!("{name}.write"));
                fsk.push(k1);
                fs.push(r);
                let (k1, r) = f(self, IrKind::partial_mut(kind.clone()), k, format!("{name}.read"));
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

pub trait LocalAssembleCtx<T> {
    fn leaf(&mut self, content: &T) -> ValueIn;
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
            CompiledLocal::Singleton(v) => ctx.leaf(v),
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
            fn leaf(&mut self, _content: &T) -> ValueIn {
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
    pub fn setter(&self, graph: &mut NodeGraphIr, kind: &IrKind, value: ValueIn) -> Block {
        let node = graph.insert(node_ir_set_local(kind));
        graph.set_value_in(Link::node(node, 0), ValueIn::link(self.setter));
        graph.set_value_in(Link::node(node, 1), value);
        Block::singleton(node, 0)
    }
}

impl CompiledPlace {
    pub fn getter(&self, target: &Target, graph: &mut NodeGraphIr, kind: &IrKind) -> ValueIn {
        match self {
            CompiledPlace::Local(local) => {
                struct Ctx;
                impl<'tcx> LocalAssembleCtx<LocalRef> for Ctx {
                    fn leaf(&mut self, content: &LocalRef) -> ValueIn {
                        ValueIn::link(content.getter)
                    }
                    fn dfs(&mut self, target: &Target, local: &CompiledLocal<LocalRef>, graph: &mut NodeGraphIr, kind: &IrKind) -> ValueIn {
                        local.assemble(target, graph, kind, self)
                    }
                }
                Ctx.dfs(target, local, graph, kind)
            },
            CompiledPlace::Field(owner, owner_kind, ele) => {
                let node = graph.insert(node_ir_destructure(target, &IrKind::Adt(owner_kind.clone())));
                let v = owner.getter(target, graph, &IrKind::Adt(owner_kind.clone()));
                graph.set_value_in(Link::node(node, 0), v);
                ValueIn::link(Link::node(node, *ele))
            },
        }
    }

    pub fn setter(&self, target: &mut Target, graph: &mut NodeGraphIr, kind: &IrKind, value: ValueIn) -> Block {
        match self {
            CompiledPlace::Local(local) => {
                struct Ctx(Block);
                impl LocalDestructureCtx<LocalRef> for Ctx {
                    fn leaf(&mut self, graph: &mut NodeGraphIr, kind: &IrKind, content: &LocalRef, value: ValueIn) {
                        let block = content.setter(graph, kind, value);
                        self.0.extend(graph, block)
                    }

                    fn dfs(&mut self, target: &Target, local: &CompiledLocal<LocalRef>, graph: &mut NodeGraphIr, kind: &IrKind, value: ValueIn) {
                        local.destructure(target, graph, kind, value, self)
                    }
                }
                Ctx(Block::nop(graph)).tap_mut(|result| result.dfs(target, local, graph, kind, value)).0
            },
            CompiledPlace::Field(owner, owner_kind, ele) => {
                let node = graph.insert(node_ir_modify_struct(target, &IrKind::Adt(owner_kind.clone())));
                let v = owner.getter(target, graph, &IrKind::Adt(owner_kind.clone()));
                graph.set_value_in(Link::node(node, 0), v);
                let index = 2 + *ele * 2;
                graph.set_value_in(Link::node(node, index), value);
                let index = 3 + *ele * 2;
                graph.set_value_in(Link::node(node, index), ValueIn::value(true.into()));
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
                Deref => if ty.ty.ref_mutability().unwrap() == Mutability::Mut { // TODO: raw ptr
                    let kind = self.compiler.compile_ty(span, ty.ty)?;
                    let de = self.graph.insert(node_ir_destructure(&mut self.compiler.target, &kind));
                    let getter = result.getter(&mut self.compiler.target, self.graph, &kind);
                    self.graph.set_value_in(Link::node(de, 0), getter);
                    result = CompiledPlace::Local(CompiledLocal::Singleton(LocalRef::node(de)));
                }, // else nop
                Downcast(_, _id) => {
                    // do nothing
                }
                other => todo!("{other:?}")
            }
            ty = ty.projection_ty(self.tcx, x);
        }
        Ok(result)
    }
}
