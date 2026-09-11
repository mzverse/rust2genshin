use crate::asset::Side;
use crate::asset::node_graph::execution::node_set_local;
use crate::asset::node_graph::query::node_local;
use crate::asset::node_graph::{CompositeNodeGraph, Connection, Link, Node, NodeGraph, NodeRef, ValueIn};
use crate::asset::structure::{ValueStruct, node_assemble_struct, node_destruct_struct};
use crate::asset::value::AnyValue;
use crate::compile::func::CompilingFn;
use crate::compile::{Block, Compiler};
use rustc_abi::FieldIdx;
use rustc_index::IndexVec;
use rustc_middle::mir::{Place, PlaceElem};
use rustc_middle::ty::{Ty, TyKind};
use rustc_span::Span;
use tap::Tap;

#[derive(Clone)]
pub enum CompiledPlace {
    LocalCommon(NodeRef),
    LocalEx {
        node: NodeRef,
        getter: NodeRef,
    },
    Flat(IndexVec<FieldIdx, CompiledPlace>),
    Field(Box<CompiledPlace>, FieldIdx),
}
#[derive(Clone, Copy)]
pub enum LocalVarKind {
    Ret,
    Arg,
    Other,
}
pub struct CompilingLocals<'a, 'tcx> {
    pub compiler: &'a mut Compiler<'tcx>,
    pub graph: &'a mut CompositeNodeGraph,
    pub block: Block,
    pub a: usize,
    pub r: usize,
}
impl<'tcx> CompilingLocals<'_, 'tcx> {
    pub fn solve_local(&mut self, ty: Ty<'tcx>, k: LocalVarKind, name: String, span: Span) -> crate::compile::Result<CompiledPlace> {
        Ok(match ty.kind() {
            TyKind::Tuple(es) => {
                let mut fs = IndexVec::new();
                for (i, t) in es.iter().enumerate() {
                    fs.push(self.solve_local(t, k, format!("{name}.{i}"), span)?);
                }
                CompiledPlace::Flat(fs)
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
                        CompiledPlace::LocalCommon(local).tap(|l| {
                            match k {
                                LocalVarKind::Ret => {
                                    self.graph.pins.get_mut(&crate::asset::generated::pin_signature::Kind::OutValue).unwrap().push(name);
                                    // Ret arm is always LocalVar::Basic; getter returns
                                    // ValueIn::link(Link::Connection(...)), so unwrap both.
                                    let conn = l.getter(&mut self.graph.graph, kind.clone());
                                    self.graph.graph.export_value_out(conn, self.r);
                                    self.r += 1;
                                }
                                LocalVarKind::Arg => {
                                    self.graph.pins.get_mut(&crate::asset::generated::pin_signature::Kind::InValue).unwrap().push(name);
                                    let block = l.setter(&mut self.graph.graph, kind, ValueIn::link(Link::Export(self.a)));
                                    self.block.extend(&mut self.graph.graph, block);
                                    self.a += 1;
                                }
                                LocalVarKind::Other => (),
                            }
                        })
                    }
                }
            },
        })
    }
}

impl CompiledPlace {
    pub fn getter(&self, graph: &mut NodeGraph, kind: AnyValue) -> Connection {
        match self {
            CompiledPlace::LocalCommon(x) => Connection(*x, 1),
            CompiledPlace::LocalEx { getter, .. } => Connection(*getter, 0),
            CompiledPlace::Flat(fields) => {
                let kind = *kind.downcast::<ValueStruct>().expect("Flat::getter called with non-struct kind");
                let node_ref = graph.insert(node_assemble_struct(&kind).into());
                for (i, field) in fields.iter().enumerate() {
                    let v = field.getter(graph, kind.fields[i].clone());
                    graph.connect_value(v, Connection(node_ref, i));
                }
                Connection(node_ref, 0)
            },
            CompiledPlace::Field(field, _) => todo!(),
        }
    }

    pub fn setter(&self, graph: &mut NodeGraph, kind: AnyValue, value: ValueIn) -> Block {
        match self {
            CompiledPlace::LocalCommon(x) => {
                let node = graph.insert(node_set_local(kind).into());
                graph.connect_value(Connection(*x, 0), Connection(node, 0));
                graph.set_value_in(Connection(node, 1), value);
                Block::singleton(node, 0)
            },
            CompiledPlace::LocalEx { .. } => todo!(),
            CompiledPlace::Flat(fields) => {
                // STRUCT_SPLIT (kernel 300003). Pin layout:
                //   - input pin 0 = the struct value (polymorphic ValueStruct)
                //   - output pins 0..N-1 = per-field values (one per tuple element)
                // Pin 0 is dual-purpose: in is the struct, out is the first field.
                // Nested Flat children recurse via their own setter(), so each
                // level only ever sees its own arity.
                let struct_kind = match kind.downcast_ref::<ValueStruct>() {
                    Ok(vs) => vs.clone(),
                    Err(_) => return Block::nop(graph),
                };
                let field_types = struct_kind.fields.clone();
                let node_ref = graph.insert(node_destruct_struct(&struct_kind).into());
                graph.set_value_in(Connection(node_ref, 0), value);
                let mut block = Block::nop(graph);
                for (i, field) in fields.iter().enumerate() {
                    let block_for_field = field.setter(graph, field_types[i].clone(), ValueIn::link(Connection(node_ref, i).into()));
                    block.extend(graph, block_for_field);
                }
                block
            },
            CompiledPlace::Field(..) => todo!(),
        }
    }
}

impl CompilingFn<'_, '_> {
    pub fn compile_place(&self, place: Place) -> CompiledPlace {
        let mut result = self.locals.get(place.local).unwrap().clone();
        for x in place.projection {
            match x {
                PlaceElem::Field(i, _) => {
                    match result {
                        CompiledPlace::Flat(v) => result = v.into_iter().nth(i.index()).unwrap(),
                        other => result = CompiledPlace::Field(other.into(), i),
                    }
                },
                other => todo!("{other:?}")
            }
        }
        result
    }
}
