use crate::compile::func::{FnDecl, NodeGraphIr};
use crate::compile::link::{Linker, Target};
use crate::node::query::{NODE_RANDOM_FLOAT, NODE_RANDOM_INT, node_local};
use crate::node::{
    NativeNodeId, NodeKind,
};
use crate::structure::{
    StructField, StructureDefinition,
};
use crate::value::{NativeKind, NativeValue};
use std::fmt::{Display, Formatter};
use tap::Tap;

pub struct Optimizer {
    pub graph: NodeGraphIr,
    pub decl: FnDecl,
}

pub type NodeKindIr = NodeKind<IrNodeId, IrKind>;

impl NodeKindIr {
    pub fn is_calc(&self) -> bool {
        self.controls_in_num == 0 && self.controls_out_num == 0
    }

    pub fn is_pure(&self) -> bool {
        assert!(self.is_calc());
        if let IrNodeId::Native(native) = &self.id {
            *native != NODE_RANDOM_INT.id && *native != NODE_RANDOM_FLOAT.id
        } else {
            true
        }
    }
}

pub type AdtKey = String;

#[derive(Clone, Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrKind {
    Native(NativeKind),
    Never,
    LocalRef(Box<IrKind>),
    Mut(Box<IrKind>),
    Adt(AdtKey),
    PartialMut(AdtKey),
    Unsupported(String),
}
impl Display for IrKind {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            IrKind::Native(native) => write!(f, "{native}"),
            IrKind::Never => write!(f, "Never"),
            IrKind::LocalRef(kind) => write!(f, "Local<{kind}>"),
            IrKind::Adt(key) => write!(f, "{key}"),
            IrKind::Mut(kind) => write!(f, "Mut<{kind}>"),
            IrKind::PartialMut(kind) => write!(f, "PartialMut<{kind}>"),
            IrKind::Unsupported(info) => write!(f, "Unsupported<{info}>"),
        }
    }
}

impl IrKind {
    pub fn is_storable(&self) -> bool {
        match self {
            IrKind::Native(native) => node_local(native).is_some(),
            IrKind::Adt(..) => true,
            IrKind::PartialMut(..) => false,
            _ => false,
        }
    }

    pub fn partial_mut(kind: IrKind) -> IrKind {
        if let Self::Adt(kind) = kind {
            IrKind::PartialMut(kind)
        } else {
            kind
        }
    }

    pub fn as_native(&self) -> Option<&NativeKind> {
        match self {
            IrKind::Native(native) => native.into(),
            _ => None,
        }
    }
    pub fn into_native(self) -> Option<NativeKind> {
        match self {
            IrKind::Native(native) => native.into(),
            _ => None,
        }
    }

    fn touch_adt(linker: &mut Linker, kind: String) -> NativeKind {
        if let Some(r) = linker.adts.get(&kind) {
            return r.to_kind();
        }
        let r = StructureDefinition {
            name: kind.clone(),
            version: 1,
            fields: linker
                .target
                .adts
                .get(&kind)
                .unwrap()
                .fields
                .clone()
                .into_iter()
                .map(|field| StructField {
                    name: field.name,
                    kind: field.kind.lower(linker),
                    default: None,
                })
                .collect(),
        }
        .apply(&mut linker.assets);
        r.to_kind().tap(|_| _ = linker.adts.insert(kind, r))
    }

    pub fn lower(&self, linker: &mut Linker) -> NativeKind {
        match self {
            IrKind::Native(native) => native.clone(),
            IrKind::Adt(kind) | IrKind::PartialMut(kind) => Self::touch_adt(linker, kind.clone()),
            IrKind::Mut(_) => panic!(),
            IrKind::Never => panic!(),
            IrKind::LocalRef(kind) => {
                if let Some(kind) = kind.as_native()
                    && node_local(kind).is_some()
                {
                    NativeKind::LocalVarRef
                } else {
                    let key = self.to_string();
                    linker.target.adts.entry(key.clone()).or_insert_with(|| {
                        AdtInfo::new(vec![FieldInfo::new("value".into(), kind.as_ref().clone())])
                    });
                    IrKind::Adt(key).lower(linker)
                }
            }
            IrKind::Unsupported(info) => panic!("Unsupported: {info}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrNodeId {
    Native(NativeNodeId),
    Nop,
    Unreachable,
    Local,
    SetLocal,
    /// Such as `if` or `match`
    /// The last branch is default
    Switch(Vec<Box<dyn NativeValue>>),
    Fn(String),
    Assemble,
    Destructure,
    Modify,
    FieldMut(usize),
    BlackBox,
}
impl IrNodeId {
    pub fn as_native(&self) -> Option<&NativeNodeId> {
        match self {
            IrNodeId::Native(native) => native.into(),
            _ => None,
        }
    }
    pub fn into_native(self) -> Option<NativeNodeId> {
        match self {
            IrNodeId::Native(native) => native.into(),
            _ => None,
        }
    }
}

pub fn node_ir_nop() -> NodeKindIr {
    NodeKind::new(IrNodeId::Nop, 1, 1, vec![], vec![])
}

pub fn node_ir_unreachable() -> NodeKindIr {
    NodeKind::new(IrNodeId::Unreachable, 1, 0, vec![], vec![])
}

pub fn node_ir_local(kind: IrKind) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::Local,
        0,
        0,
        vec![kind.clone().into()],
        vec![IrKind::LocalRef(kind.clone().into()), kind],
    )
}

pub fn node_ir_set_local(kind: IrKind) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::SetLocal,
        1,
        1,
        vec![IrKind::LocalRef(kind.clone().into()).into(), kind.into()],
        vec![],
    )
}

pub fn node_ir_switch(kind: IrKind, values: Vec<Box<dyn NativeValue>>) -> NodeKindIr {
    let len = values.len();
    NodeKind::new(
        IrNodeId::Switch(values),
        1,
        len + 1,
        vec![kind.into()],
        vec![],
    )
}

pub fn node_ir_black_box(kind: IrKind) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::BlackBox,
        1,
        1,
        vec![kind.clone().into()],
        vec![kind],
    )
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FieldInfo {
    pub name: String,
    pub kind: IrKind,
}
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AdtInfo {
    pub fields: Vec<FieldInfo>,
    pub variants: Vec<Vec<usize>>,
    pub discriminant: Option<usize>,
}
impl AdtInfo {
    pub fn new(fields: Vec<FieldInfo>) -> Self {
        Self {
            fields,
            variants: vec![],
            discriminant: None,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct FnInfo {
    pub description: String,
    pub graph: NodeGraphIr,
    pub decl: FnDecl,
    pub exported: bool,
}
impl FieldInfo {
    pub fn new(name: String, kind: IrKind) -> Self {
        Self { name, kind }
    }
    pub fn encode(&self, linker: &mut Linker) -> StructField {
        StructField {
            name: self.name.clone(),
            kind: self.kind.lower(linker),
            default: None,
        }
    }
}
pub fn struct_fields(target: &Target, kind: &IrKind) -> Vec<FieldInfo> {
    match kind {
        IrKind::Adt(key) => target.adts.get(key).unwrap().fields.clone(),
        IrKind::Mut(e) => vec![
            FieldInfo::new(0.to_string(), IrKind::Native(NativeKind::LocalVarRef)),
            FieldInfo::new(1.to_string(), IrKind::partial_mut(e.as_ref().clone())),
        ],
        IrKind::PartialMut(e) => struct_fields(target, &IrKind::Adt(e.clone()))
            .into_iter()
            .map(|mut x| {
                x.kind = IrKind::partial_mut(x.kind);
                x
            })
            .collect(),
        _ => panic!("{kind:?}"),
    }
}

pub fn node_ir_assemble(target: &Target, kind: &IrKind) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::Assemble,
        0,
        0,
        struct_fields(target, kind)
            .into_iter()
            .map(|FieldInfo { kind, .. }| Some(kind))
            .collect(),
        vec![kind.clone()],
    )
}

pub fn node_ir_destructure(target: &Target, kind: &IrKind) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::Destructure,
        0,
        0,
        vec![kind.clone().into()],
        struct_fields(target, kind)
            .into_iter()
            .map(|FieldInfo { kind, .. }| kind)
            .collect(),
    )
}

pub fn node_ir_modify_struct(target: &Target, kind: &IrKind) -> NodeKindIr {
    let mut params = vec![kind.clone().into()];
    params.push(None);
    for x in struct_fields(target, kind) {
        params.push(x.kind.clone().into());
        params.push(IrKind::Native(NativeKind::Bool).into());
    }
    NodeKind::new(IrNodeId::Modify, 1, 1, params, vec![])
}

pub fn node_ir_field_mut(target: &Target, adt: &AdtKey, idx: usize) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::FieldMut(idx),
        0,
        0,
        vec![IrKind::PartialMut(adt.to_string()).into()],
        vec![IrKind::Mut(
            target.adts.get(adt).unwrap().fields[idx]
                .kind
                .clone()
                .into(),
        )],
    )
}
