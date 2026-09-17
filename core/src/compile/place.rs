use crate::asset::Side;
use crate::asset::node_graph::execution::node_set_local;
use crate::asset::node_graph::query::node_local;
use crate::asset::node_graph::{CompositeNodeGraph, Connection, Link, Node, NodeGraph, NodeRef, ValueIn};
use crate::asset::structure::{ValueStruct, node_assemble_struct, node_destructure_struct};
use crate::asset::value::AnyValue;
use crate::compile::func::CompilingFn;
use crate::compile::{Block, Compiler};
use rustc_abi::FieldIdx;
use rustc_ast::Mutability;
use rustc_index::IndexVec;
use rustc_middle::mir::{Place, PlaceTy};
use rustc_middle::ty::{Ty, TyKind};
use rustc_span::Span;
use tap::Tap;

#[derive(Clone)]
pub enum CompiledLocal<T> {
    Singleton(T),
    Flat(IndexVec<FieldIdx, CompiledLocal<T>>),
}
#[derive(Clone, Copy)]
pub struct LocalRef(NodeRef);
#[derive(Clone)]
pub enum CompiledPlace {
    Local(CompiledLocal<LocalRef>),
    Field(Box<CompiledPlace>, FieldIdx),
}
#[derive(Clone, Copy)]
pub enum LocalKind {
    Ret,
    Arg,
    Other,
}
pub struct CompilingLocals<'a, 'tcx> {
    pub compiler: &'a mut Compiler<'tcx>,
    pub graph: &'a mut CompositeNodeGraph,
    pub block: Block,
    pub params: usize,
    pub rets: usize,
}
impl<'tcx> CompilingLocals<'_, 'tcx> {
    pub fn solve_local(&mut self, ty: Ty<'tcx>, k: LocalKind, name: String, span: Span) -> crate::compile::Result<(CompiledLocal<()>, CompiledLocal<LocalRef>)> {
        Ok(match ty.kind() {
            TyKind::Tuple(es) => {
                let mut fsk = IndexVec::new();
                let mut fs = IndexVec::new();
                for (i, t) in es.iter().enumerate() {
                    let (k, r) = self.solve_local(t, k, format!("{name}.{i}"), span)?;
                    fsk.push(k);
                    fs.push(r);
                }
                (CompiledLocal::Flat(fsk), CompiledLocal::Flat(fs))
            },
            _ => {
                let kind = self.compiler.compile_ty(span, ty)?;
                match kind.downcast::<ValueStruct>() {
                    Ok(kind) => {
                        todo!("{kind:?}")
                    }
                    Err(kind) => {
                        let kind = kind.into_object();
                        let local = self.graph.graph.insert(Node::new(node_local(kind.clone())));
                        if kind.encode_storage(Side::Server /* locals are server-side; SLocalVarRef has ClientUnknown */).is_some() {
                            self.graph.graph.set_default(Connection(local, 0), kind.clone());
                        }
                        let r = CompiledLocal::Singleton(LocalRef(local).tap(|l| {
                            match k {
                                LocalKind::Ret => {
                                    self.graph.pins.get_mut(&crate::asset::generated::pin_signature::Kind::OutValue).unwrap().push(name);
                                    let conn = l.getter();
                                    self.graph.graph.export_value_out(conn, self.rets);
                                    self.rets += 1;
                                }
                                LocalKind::Arg => {
                                    self.graph.pins.get_mut(&crate::asset::generated::pin_signature::Kind::InValue).unwrap().push(name);
                                    let block = l.setter(&mut self.graph.graph, &kind, ValueIn::link(Link::Export(self.params)));
                                    self.block.extend(&mut self.graph.graph, block);
                                    self.params += 1;
                                }
                                LocalKind::Other => (),
                            }
                        }));
                        (CompiledLocal::Singleton(()), r)
                    }
                }
            },
        })
    }
}

pub trait LocalAssembleCtx<T> {
    fn leaf(&mut self, content: &T) -> ValueIn;
    fn dfs(&mut self, local: &CompiledLocal<T>, graph: &mut NodeGraph, kind: &AnyValue) -> ValueIn;
}

pub trait LocalDestructureCtx<T> {
    fn leaf(&mut self, graph: &mut NodeGraph, kind: &AnyValue, content: &T, value: ValueIn);
    fn dfs(&mut self, local: &CompiledLocal<T>, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn);
}

impl<T> CompiledLocal<T> {
    pub fn assemble(&self, graph: &mut NodeGraph, kind: &AnyValue, ctx: &mut impl LocalAssembleCtx<T>) -> ValueIn {
        match self {
            CompiledLocal::Singleton(v) => ctx.leaf(v),
            CompiledLocal::Flat(elements) => {
                let kind = kind.downcast_ref::<ValueStruct>().unwrap();
                let node_ref = graph.insert(node_assemble_struct(&kind.st).into());
                for (i, x) in elements.iter().enumerate() {
                    let v = ctx.dfs(x, graph, &kind.fields[i]);
                    graph.set_value_in(Connection(node_ref, i), v);
                }
                ValueIn::link(Connection(node_ref, 0).into())
            }
        }
    }
    pub fn assemble_all<I: Iterator<Item = ValueIn>>(&self, graph: &mut NodeGraph, kind: &AnyValue, values: &mut I) -> ValueIn {
        struct Ctx<'a, I: Iterator<Item = ValueIn>>(&'a mut I);
        impl<T, I: Iterator<Item = ValueIn>> LocalAssembleCtx<T> for Ctx<'_, I> {
            fn leaf(&mut self, _content: &T) -> ValueIn {
                self.0.next().unwrap()
            }
            fn dfs(&mut self, local: &CompiledLocal<T>, graph: &mut NodeGraph, kind: &AnyValue) -> ValueIn {
                local.assemble_all(graph, kind, self.0)
            }
        }
        self.assemble(graph, kind, &mut Ctx(values))
    }

    pub fn destructure(&self, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn, ctx: &mut impl LocalDestructureCtx<T>) {
        match self {
            CompiledLocal::Singleton(v) => ctx.leaf(graph, kind, v, value),
            CompiledLocal::Flat(elements) => {
                let kind = kind.downcast_ref::<ValueStruct>().unwrap();
                let node_ref = graph.insert(node_destructure_struct(&kind.st).into());
                graph.set_value_in(Connection(node_ref, 0), value);
                for (i, field) in elements.iter().enumerate() {
                    ctx.dfs(field, graph, &kind.fields[i], ValueIn::link(Connection(node_ref, i).into()));
                }
            }
        }
    }
    pub fn destructure_all(&self, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn) -> Vec<ValueIn> {
        struct Ctx(Vec<ValueIn>);
        impl<T> LocalDestructureCtx<T> for Ctx {
            fn leaf(&mut self, _graph: &mut NodeGraph, _kind: &AnyValue, _content: &T, value: ValueIn) {
                self.0.push(value);
            }
            fn dfs(&mut self, local: &CompiledLocal<T>, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn) {
                self.0.extend(local.destructure_all(graph, kind, value));
            }
        }
        let mut result = Ctx(vec![]);
        self.destructure(graph, kind, value, &mut result);
        result.0
    }
}

impl LocalRef {
    #[must_use]
    pub fn getter(&self) -> Connection {
        Connection(self.0, 1)
    }

    #[must_use]
    pub fn setter(&self, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn) -> Block {
        let node = graph.insert(node_set_local(kind).into());
        graph.connect_value(Connection(self.0, 0), Connection(node, 0));
        graph.set_value_in(Connection(node, 1), value);
        Block::singleton(node, 0)
    }
}

impl CompiledPlace {
    pub fn getter(&self, graph: &mut NodeGraph, kind: &AnyValue) -> ValueIn {
        match self {
            CompiledPlace::Local(local) => {
                struct Ctx;
                impl LocalAssembleCtx<LocalRef> for Ctx {
                    fn leaf(&mut self, content: &LocalRef) -> ValueIn {
                        ValueIn::link(content.getter().into())
                    }
                    fn dfs(&mut self, local: &CompiledLocal<LocalRef>, graph: &mut NodeGraph, kind: &AnyValue) -> ValueIn {
                        local.assemble(graph, kind, self)
                    }
                }
                Ctx.dfs(local, graph, kind)
            },
            CompiledPlace::Field(_, _) => todo!(),
        }
    }

    pub fn setter(&self, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn) -> Block {
        match self {
            CompiledPlace::Local(local) => {
                struct Ctx(Block);
                impl LocalDestructureCtx<LocalRef> for Ctx {
                    fn leaf(&mut self, graph: &mut NodeGraph, kind: &AnyValue, content: &LocalRef, value: ValueIn) {
                        let block = content.setter(graph, kind, value);
                        self.0.extend(graph, block)
                    }

                    fn dfs(&mut self, local: &CompiledLocal<LocalRef>, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn) {
                        local.destructure(graph, kind, value, self)
                    }
                }
                Ctx(Block::nop(graph)).tap_mut(|result| result.dfs(local, graph, kind, value)).0
            },
            CompiledPlace::Field(_, _) => todo!(),
        }
    }
}

impl<'tcx> CompilingFn<'tcx, '_> {
    pub fn compile_place(&self, place: Place<'tcx>) -> CompiledPlace {
        let mut result = CompiledPlace::Local(self.locals.get(place.local).unwrap().clone());
        let mut ty = PlaceTy::from_ty(self.mono(self.body.local_decls.get(place.local).unwrap().ty));
        for x in place.projection {
            use rustc_middle::mir::ProjectionElem::*;
            match x {
                Field(i, _) => {
                    match result {
                        CompiledPlace::Local(CompiledLocal::Flat(v)) => result = CompiledPlace::Local(v.into_iter().nth(i.index()).unwrap()),
                        other => result = CompiledPlace::Field(other.into(), i),
                    }
                },
                Deref => if ty.ty.ref_mutability().unwrap() == Mutability::Mut {
                    todo!()
                }, // else nop
                other => todo!("{other:?}")
            }
            ty = ty.projection_ty(self.tcx, x);
        }
        result
    }
}
