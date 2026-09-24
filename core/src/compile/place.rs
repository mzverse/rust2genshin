use super::{Result, get_expn_macro_attr};
use crate::asset::node_graph::{CompositeNodeGraph, Connection, Link, Node, NodeGraph, NodeRef, ValueIn};
use crate::asset::structure::{StructField, node_modify_struct};
use crate::asset::value::{AnyValue, ValueBool};
use crate::compile::func::CompilingFn;
use crate::compile::optimize::{node_ir_assemble, node_ir_destructure, node_ir_local, node_ir_set_local, struct_fields};
use crate::compile::{Block, Compiler};
use rustc_abi::FieldIdx;
use rustc_ast::Mutability;
use rustc_index::IndexVec;
use rustc_middle::mir::{Place, PlaceTy};
use rustc_middle::query::QueryKey;
use rustc_middle::ty::{Ty, TyKind, TypingEnv};
use rustc_span::Span;
use tap::Tap;

#[derive(Clone, Debug)]
pub enum CompiledLocal<T> {
    Singleton(T),
    Flat(IndexVec<FieldIdx, CompiledLocal<T>>),
}
#[derive(Clone, Copy)]
pub struct LocalRef<'tcx> {
    pub ty: Ty<'tcx>,
    pub setter: Link,
    pub getter: Link,
}
impl<'tcx> LocalRef<'tcx> {
    pub fn node(ty: Ty<'tcx>, node: NodeRef) -> Self {
        Self {
            ty,
            setter: Connection(node, 0).into(),
            getter: Connection(node, 1).into(),
        }
    }
}
#[derive(Clone)]
pub enum CompiledPlace<'tcx> {
    Local(CompiledLocal<LocalRef<'tcx>>),
    Field(Box<CompiledPlace<'tcx>>, AnyValue, FieldIdx),
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
    pub fn solve_local(&mut self, ty: Ty<'tcx>, k: LocalKind, name: String, span: Span) -> Result<(CompiledLocal<()>, CompiledLocal<LocalRef<'tcx>>)> {
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
            TyKind::Closure(_d, a) => self.solve_local(Ty::new_tup(self.compiler.tcx, a.as_closure().upvar_tys()), k, name, span)?,
            TyKind::Adt(d, a) if let Some(r) = {
                let def = d.did().default_span(self.compiler.tcx);
                if let Some(attr) = get_expn_macro_attr(self.compiler.tcx, def) {
                    if let Some(ident) = attr.meta.path().get_ident() {
                        match ident.to_string().as_str() {
                            "event" => {
                                let mut fsk = IndexVec::new();
                                let mut fs = IndexVec::new();
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
            TyKind::Ref(_, e, Mutability::Not) if !e.is_never() => {
                self.solve_local(*e, k, name, span)?
            },
            TyKind::Ref(r, e, Mutability::Mut) => {
                let tcx = self.compiler.tcx;
                self.solve_local(Ty::new_tup(tcx, &[Ty::new_imm_ref(tcx, *r, tcx.types.never), *e]), k, name, span)?
            },
            _ => {
                let kind = self.compiler.compile_ty(span, ty)?;
                let local = self.graph.graph.insert(Node::new(node_ir_local(self.compiler, &kind, ty)));
                // if kind.encode_storage(Side::Server).is_some() {
                //     self.graph.graph.set_default(Connection(local, 0), kind.clone());
                // }
                let r = CompiledLocal::Singleton(LocalRef::node(ty, local).tap(|l| {
                    match k {
                        LocalKind::Ret => {
                            self.graph.pins.get_mut(&crate::asset::generated::pin_signature::Kind::OutValue).unwrap().push(name);
                            self.graph.graph.export_value_out(l.getter.connection().unwrap(), self.rets);
                            self.rets += 1;
                        }
                        LocalKind::Arg => {
                            self.graph.pins.get_mut(&crate::asset::generated::pin_signature::Kind::InValue).unwrap().push(name);
                            let block = l.setter(self.compiler, &mut self.graph.graph, &kind, ValueIn::link(Link::Export(self.params)));
                            self.block.extend(&mut self.graph.graph, block);
                            self.params += 1;
                        }
                        LocalKind::Other => (),
                    }
                }));
                (CompiledLocal::Singleton(()), r)
            },
        })
    }
}

pub trait LocalAssembleCtx<'tcx, T> {
    fn leaf(&mut self, content: &T) -> ValueIn;
    fn dfs(&mut self, compiler: &mut Compiler, local: &CompiledLocal<T>, graph: &mut NodeGraph, kind: &AnyValue) -> ValueIn;
}

pub trait LocalDestructureCtx<'tcx, T> {
    fn leaf(&mut self, compiler: &mut Compiler<'tcx>, graph: &mut NodeGraph, kind: &AnyValue, content: &T, value: ValueIn);
    fn dfs(&mut self, compiler: &mut Compiler<'tcx>, local: &CompiledLocal<T>, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn);
}

impl<T> CompiledLocal<T> {
    fn get_fields(compiler: &mut Compiler, kind: &AnyValue) -> Vec<AnyValue> {
        struct_fields(compiler, kind).into_iter().map(|StructField { value, .. }| value).collect()
    }

    pub fn assemble<'tcx>(&self, compiler: &mut Compiler, graph: &mut NodeGraph, kind: &AnyValue, ctx: &mut impl LocalAssembleCtx<'tcx, T>) -> ValueIn {
        match self {
            CompiledLocal::Singleton(v) => ctx.leaf(v),
            CompiledLocal::Flat(elements) => {
                let node_ref = graph.insert(node_ir_assemble(compiler, kind).into());
                let fields = Self::get_fields(compiler, kind);
                for (i, x) in elements.iter().enumerate() {
                    let v = ctx.dfs(compiler, x, graph, &fields[i]);
                    graph.set_value_in(Connection(node_ref, i), v);
                }
                ValueIn::link(Connection(node_ref, 0).into())
            }
        }
    }
    pub fn assemble_all<I: Iterator<Item = ValueIn>>(&self, compiler: &mut Compiler, graph: &mut NodeGraph, kind: &AnyValue, values: &mut I) -> ValueIn {
        struct Ctx<'a, I: Iterator<Item = ValueIn>>(&'a mut I);
        impl<'tcx, T, I: Iterator<Item = ValueIn>> LocalAssembleCtx<'tcx, T> for Ctx<'_, I> {
            fn leaf(&mut self, _content: &T) -> ValueIn {
                self.0.next().unwrap()
            }
            fn dfs(&mut self, compiler: &mut Compiler, local: &CompiledLocal<T>, graph: &mut NodeGraph, kind: &AnyValue) -> ValueIn {
                local.assemble_all(compiler, graph, kind, self.0)
            }
        }
        self.assemble(compiler, graph, kind, &mut Ctx(values))
    }

    pub fn destructure<'tcx>(&self, compiler: &mut Compiler<'tcx>, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn, ctx: &mut impl LocalDestructureCtx<'tcx, T>) {
        match self {
            CompiledLocal::Singleton(v) => ctx.leaf(compiler, graph, kind, v, value),
            CompiledLocal::Flat(elements) => {
                let node_ref = graph.insert(node_ir_destructure(compiler, kind).into());
                let fields = Self::get_fields(compiler, kind);
                graph.set_value_in(Connection(node_ref, 0), value);
                for (i, field) in elements.iter().enumerate() {
                    ctx.dfs(compiler, field, graph, &fields[i], ValueIn::link(Connection(node_ref, i).into()));
                }
            }
        }
    }
    pub fn destructure_all(&self, compiler: &mut Compiler, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn) -> Vec<ValueIn> {
        struct Ctx(Vec<ValueIn>);
        impl<T> LocalDestructureCtx<'_, T> for Ctx {
            fn leaf(&mut self, _compiler: &mut Compiler, _graph: &mut NodeGraph, _kind: &AnyValue, _content: &T, value: ValueIn) {
                self.0.push(value);
            }
            fn dfs(&mut self, compiler: &mut Compiler, local: &CompiledLocal<T>, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn) {
                self.0.extend(local.destructure_all(compiler, graph, kind, value));
            }
        }
        let mut result = Ctx(vec![]);
        self.destructure(compiler, graph, kind, value, &mut result);
        result.0
    }
}

impl<'tcx> LocalRef<'tcx> {
    #[must_use]
    pub fn setter(&self, compiler: &mut Compiler<'tcx>, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn) -> Block {
        let node = graph.insert(node_ir_set_local(compiler, kind, self.ty).into());
        graph.set_value_in(Connection(node, 0), ValueIn::link(self.setter));
        graph.set_value_in(Connection(node, 1), value);
        Block::singleton(node, 0)
    }
}

impl<'tcx> CompiledPlace<'tcx> {
    pub fn getter(&self, compiler: &mut Compiler, graph: &mut NodeGraph, kind: &AnyValue) -> ValueIn {
        match self {
            CompiledPlace::Local(local) => {
                struct Ctx;
                impl<'tcx> LocalAssembleCtx<'tcx, LocalRef<'tcx>> for Ctx {
                    fn leaf(&mut self, content: &LocalRef<'tcx>) -> ValueIn {
                        ValueIn::link(content.getter)
                    }
                    fn dfs(&mut self, compiler: &mut Compiler, local: &CompiledLocal<LocalRef<'tcx>>, graph: &mut NodeGraph, kind: &AnyValue) -> ValueIn {
                        local.assemble(compiler, graph, kind, self)
                    }
                }
                Ctx.dfs(compiler, local, graph, kind)
            },
            CompiledPlace::Field(owner, owner_kind, ele) => {
                let node = graph.insert(node_ir_destructure(compiler, owner_kind).into());
                let v = owner.getter(compiler, graph, owner_kind);
                graph.set_value_in(Connection(node, 0), v);
                ValueIn::link(Connection(node, ele.index()).into())
            },
        }
    }

    pub fn setter(&self, compiler: &mut Compiler<'tcx>, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn) -> Block {
        match self {
            CompiledPlace::Local(local) => {
                struct Ctx(Block);
                impl<'tcx> LocalDestructureCtx<'tcx, LocalRef<'tcx>> for Ctx {
                    fn leaf(&mut self, compiler: &mut Compiler<'tcx>, graph: &mut NodeGraph, kind: &AnyValue, content: &LocalRef<'tcx>, value: ValueIn) {
                        let block = content.setter(compiler, graph, kind, value);
                        self.0.extend(graph, block)
                    }

                    fn dfs(&mut self, compiler: &mut Compiler<'tcx>, local: &CompiledLocal<LocalRef<'tcx>>, graph: &mut NodeGraph, kind: &AnyValue, value: ValueIn) {
                        local.destructure(compiler, graph, kind, value, self)
                    }
                }
                Ctx(Block::nop(graph)).tap_mut(|result| result.dfs(compiler, local, graph, kind, value)).0
            },
            CompiledPlace::Field(owner, owner_kind, ele) => {
                let node = graph.insert(node_modify_struct(owner_kind.downcast_ref().unwrap()).into());
                let v = owner.getter(compiler, graph, owner_kind);
                graph.set_value_in(Connection(node, 0), v);
                graph.set_value_in(Connection(node, 2 + ele.index() * 2), value);
                graph.set_value_in(Connection(node, 3 + ele.index() * 2), ValueIn::value(ValueBool(true).into()));
                Block::singleton(node, 0)
            },
        }
    }
}

impl<'tcx> CompilingFn<'tcx, '_> {
    pub fn compile_place(&mut self, place: Place<'tcx>, span: Span) -> Result<CompiledPlace<'tcx>> {
        let mut result = CompiledPlace::Local(self.locals.get(place.local).unwrap().clone());
        let mut ty = PlaceTy::from_ty(self.mono(self.body.local_decls.get(place.local).unwrap().ty));
        for x in place.projection {
            use rustc_middle::mir::ProjectionElem::*;
            match x {
                Field(i, _) => {
                    match result {
                        CompiledPlace::Local(CompiledLocal::Flat(v)) => result = CompiledPlace::Local(v.into_iter().nth(i.index()).unwrap()),
                        other => result = CompiledPlace::Field(other.into(), self.compiler.compile_ty(span, ty.ty)?, i),
                    }
                },
                Deref => if ty.ty.ref_mutability().unwrap() == Mutability::Mut { // TODO: raw ptr
                    let kind = self.compiler.compile_ty(span, ty.ty)?;
                    let de = self.graph.graph.insert(node_ir_destructure(self.compiler, &kind).into());
                    let getter = result.getter(self.compiler, &mut self.graph.graph, &kind);
                    self.graph.graph.set_value_in(Connection(de, 0), getter);
                    result = CompiledPlace::Local(CompiledLocal::Singleton(LocalRef::node(ty.ty, de)));
                }, // else nop
                other => todo!("{other:?}")
            }
            ty = ty.projection_ty(self.tcx, x);
        }
        Ok(result)
    }
}
