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
        match &self.id {
            IrNodeId::Local => false,
            IrNodeId::Native(native) =>
                *native != NODE_RANDOM_INT.id && *native != NODE_RANDOM_FLOAT.id,
            _ => true,
        }
    }
}

pub type AdtKey = String;

#[derive(Clone, Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IrKind {
    Native(NativeKind),
    /// 列表 `List<T>`。元素类型是 IR 类型,降级时才落成 `NativeKind::List`。
    List(Box<IrKind>),
    Never,
    LocalRef(Box<IrKind>),
    Mut(Box<IrKind>),
    Adt(AdtKey),
    PartialMut(Box<IrKind>),
    Unsupported(String),
}
impl Display for IrKind {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            IrKind::Native(native) => write!(f, "{native}"),
            IrKind::List(elem) => write!(f, "List<{elem}>"),
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
            // FIXME
            // 列表能存进局部变量,前提是元素类型在节点图里有列表变体。
            IrKind::List(elem) => elem
                .to_native()
                .is_some_and(|n| node_local(&NativeKind::List(n.into())).is_some()),
            IrKind::Adt(..) => true,
            // 可变视图的值就是被视图对象的值,能不能存跟它一样。
            IrKind::PartialMut(kind) => kind.is_storable(),
            _ => false,
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

    /// 节点图里的原生类型。列表的元素也必须是原生类型 —— 元素是结构体的
    /// 列表要到 lower 之后才有 `NativeKind::Struct`。
    pub fn to_native(&self) -> Option<NativeKind> {
        match self {
            IrKind::Native(native) => Some(native.clone()),
            IrKind::List(elem) => elem.to_native().map(|n| NativeKind::List(n.into())),
            // 节点图里没有引用:句柄类型的借用(`&List<T>` 是非 `Freeze` 的,
            // 会被编成 `PartialMut`)就是它本身。
            IrKind::PartialMut(kind) => kind.to_native(),
            _ => None,
        }
    }

    /// 列表类型 `List<elem>`。
    pub fn list(elem: IrKind) -> IrKind {
        IrKind::List(Box::new(elem))
    }

    /// `LocalRef` 里装的类型(局部变量的实际类型);不是 `LocalRef` 时为 `None`。
    ///
    /// 局部变量的类型一律从这里取,不要用引脚的另一个值出口 —— 那个出口带
    /// `PartialMut` 包装,表达式可变性不同,形态也不一样。
    pub fn local_ref(&self) -> Option<IrKind> {
        match self {
            IrKind::LocalRef(kind) => Some((**kind).clone()),
            _ => None,
        }
    }

    /// 「可变视图」:`kind` 的值,但标记着「这是通过引用拿到的」。
    ///
    /// 节点图里没有引用 —— 写它就是写原处。`lower` 之后和 `kind` 等同。
    pub fn mutable_view(kind: Self) -> Self {
        IrKind::PartialMut(kind.into())
    }

    /// 视图里的类型;不是视图时为 `None`。
    pub fn view_inner(&self) -> Option<IrKind> {
        match self {
            IrKind::PartialMut(kind) => Some((**kind).clone()),
            _ => None,
        }
    }

    /// 列表的元素类型;不是列表时为 `None`。
    pub fn list_elem(&self) -> Option<IrKind> {
        match self {
            // 只认列表本身。`PartialMut<List<T>>` 是视图,不是列表 ——
            // 要元素类型先显式取出里面的 `List`。
            IrKind::List(elem) => Some((**elem).clone()),
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
            IrKind::List(elem) => NativeKind::List(elem.lower(linker).into()),
            IrKind::Adt(kind) => Self::touch_adt(linker, kind.clone()),
            IrKind::PartialMut(kind) => kind.lower(linker),
            // `&mut T` 在节点图里是「引用结构体」:setter 槽(写) + getter 槽(读)。
            // 注册成结构体,assemble / destructure 节点才有形态可用。
            IrKind::Mut(inner) => {
                let key = format!("&mut {inner}");
                linker.target.adts.entry(key.clone()).or_insert_with(|| {
                    let kind = (**inner).clone();
                    AdtInfo::new(vec![
                        FieldInfo::new("0".into(), IrKind::LocalRef(kind.clone().into())),
                        FieldInfo::new("1".into(), IrKind::mutable_view(kind)),
                    ])
                });
                Self::touch_adt(linker, key)
            }
            IrKind::Never => panic!(),
            IrKind::LocalRef(kind) => {
                if !kind.is_storable() {
                    panic!("Cannot store {kind}");
                }
                if let Some(native) = kind.to_native()
                    && node_local(&native).is_some()
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
    /// 对结构体字段取 `&mut`:输出一个「能读能写」的二元组。setter 那头是虚的
    /// (节点图里没有字段句柄),优化期必须把它消费掉的每一处写入改写成 `Modify`
    /// —— 见 `Optimizer::eliminate_field_mut`。
    ///
    /// `fields` 是 owner 结构体的字段类型,带着它是为了让改写不必再回头查 `Target`。
    FieldMut {
        idx: usize,
        fields: Vec<IrKind>,
    },
    BlackBox,
    /// 未实现:只有一个出引脚,类型随便填。降级时直接 panic ——
    /// 让「还没支持的东西」在编译期被明确标出来,而不是静默走错路。
    Unsupported,
    /// 读值:`PartialMut<T>` 进,`T` 出,没有控制流。
    /// 局部变量给的是可变视图,MIR 的 Copy / Move 读值时要经过它;
    /// 降级时等价于值直通(节点图里读局部变量本来就是取值)。
    Copy,
    /// 写回时把值标成视图:`T` 进,`PartialMut<T>` 出,没有控制流。
    ///
    /// 优化器把「读局部变量」改接到写入值时,目标引脚要的是视图,
    /// 中间就得有这么一层。它和 `Copy` 正好互逆,`Copy(ToView(x))`
    /// 会被 `eliminate_copy` 消成直连。
    ToView,
    /// 把若干元素拼成一个列表,对应 `[T; N]` 字面量 / 数组聚合。
    /// 引脚只有元素本身;原节点前面那个「元素个数」引脚在 lower 时补。
    ListAssemble,
    /// 按下标取值,对应 `[T; N]` / 列表的下标读。
    ListGet,
    /// 按下标改值,对应 `[T; N]` / 列表的下标写。
    ListSet,
    /// 往列表里插入元素,对应 `List::insert`。
    ListInsert,
    /// 取列表长度,对应 `List::len`。
    ListGetLength,
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
        // 值的出口永远是可变视图 —— 读值要经过 `Copy`。
        // 句柄类型(`&mut` 的 setter 槽)本身就是句柄,没有再套视图的说法。
        vec![IrKind::LocalRef(kind.clone().into()), match &kind {
            // 已经是视图 / 句柄的,不再套一层。
            IrKind::PartialMut(_) | IrKind::LocalRef(_) => kind.clone(),
            _ => IrKind::mutable_view(kind),
        }],
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

/// 未实现的节点:只有一个出引脚(类型随便填),没有入参。
/// 降级时 panic —— 它只用来占住一个「还没实现」的值。
pub fn node_ir_unsupported(out_kind: IrKind) -> NodeKindIr {
    NodeKind::new(IrNodeId::Unsupported, 0, 0, vec![], vec![out_kind])
}

/// 把值标成视图:`T` 进,`PartialMut<T>` 出。
///
/// `kind` 已经是视图 / 句柄时它就是恒等的 —— 调用方应当直接连线,
/// 不必插一个什么都不做的节点。
pub fn node_ir_to_view(kind: IrKind) -> NodeKindIr {
    let out = match &kind {
        IrKind::PartialMut(_) | IrKind::LocalRef(_) => kind.clone(),
        _ => IrKind::mutable_view(kind.clone()),
    };
    NodeKind::new(IrNodeId::ToView, 0, 0, vec![kind.into()], vec![out])
}

/// 读值:`PartialMut<T>` 进,`T` 出。
pub fn node_ir_copy(kind: IrKind) -> NodeKindIr {
    let kind1 = kind.clone();
    NodeKind::new(
        IrNodeId::Copy,
        0,
        0,
        vec![IrKind::PartialMut(kind1.into()).into()],
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
        IrKind::Mut(e) => {
            let kind1 = e.as_ref().clone();
            vec![
                // setter 槽的类型和 `Local` 的句柄出口一致,必须是 `LocalRef`。
                FieldInfo::new(0.to_string(), IrKind::LocalRef(kind1.clone().into())),
                FieldInfo::new(1.to_string(), IrKind::mutable_view(kind1)),
            ]
        }
        IrKind::PartialMut(e) => struct_fields(target, e)
            .into_iter()
            .map(|mut x| {
                x.kind = IrKind::PartialMut(x.kind.into());
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

/// 拼装列表(`[T; N]` 字面量):`count` 个元素进,`List<T>` 出。
///
/// 这里**不带**原节点的「元素个数」引脚 —— IR 只管元素,link 期按
/// `values_in_types.len()` 补上那个引脚和它的默认值。
pub fn node_ir_list_assemble(elem: IrKind, count: usize) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::ListAssemble,
        0,
        0,
        vec![Some(elem.clone()); count],
        vec![IrKind::list(elem)],
    )
}

/// 按下标取列表元素，结果当**视图**用（place 的 getter）：取值要过 `Copy`。
/// 和 [`node_ir_list_get`] 是同一个节点，只有 IR 层的出口类型不同。
pub fn node_ir_list_get_view(elem: IrKind) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::ListGet,
        0,
        0,
        vec![
            Some(IrKind::mutable_view(IrKind::list(elem.clone()))),
            Some(IrKind::Native(NativeKind::Int)),
        ],
        vec![IrKind::mutable_view(elem)],
    )
}

/// 按下标取列表元素。
pub fn node_ir_list_get(elem: IrKind) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::ListGet,
        0,
        0,
        vec![
            Some(IrKind::mutable_view(IrKind::list(elem.clone()))),
            Some(IrKind::Native(NativeKind::Int)),
        ],
        vec![elem],
    )
}

/// 按下标改列表元素(就地修改,没有值输出)。
pub fn node_ir_list_set(elem: IrKind) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::ListSet,
        1,
        1,
        vec![
            Some(IrKind::mutable_view(IrKind::list(elem.clone()))),
            Some(IrKind::Native(NativeKind::Int)),
            Some(elem),
        ],
        vec![],
    )
}

/// 往列表里插入元素:列表 / 下标 / 元素进,没有值输出。
pub fn node_ir_list_insert(elem: IrKind) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::ListInsert,
        1,
        1,
        vec![
            Some(IrKind::mutable_view(IrKind::list(elem.clone()))),
            Some(IrKind::Native(NativeKind::Int)),
            Some(elem),
        ],
        vec![],
    )
}

/// 取列表长度:列表进,长度出。
pub fn node_ir_list_get_length(elem: IrKind) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::ListGetLength,
        0,
        0,
        vec![Some(IrKind::mutable_view(IrKind::list(elem)))],
        vec![IrKind::Native(NativeKind::Int)],
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
    // 第 0 个引脚是结构体本身(可以是视图,原地改),但写进字段的是**值** ——
    // 所以字段类型从 `Adt` 取,不是从视图取(视图的字段也是视图)。
    let adt = match kind {
        IrKind::Adt(key) => IrKind::Adt(key.clone()),
        IrKind::PartialMut(inner) => (**inner).clone(),
        other => panic!("Modify 的目标应当是结构体,实际是 {other}"),
    };
    for x in struct_fields(target, &adt) {
        params.push(x.kind.clone().into());
        params.push(IrKind::Native(NativeKind::Bool).into());
    }
    NodeKind::new(IrNodeId::Modify, 1, 1, params, vec![])
}

pub fn node_ir_field_mut(target: &Target, adt: &AdtKey, idx: usize) -> NodeKindIr {
    let fields = struct_fields(target, &IrKind::Adt(adt.to_string()))
        .into_iter()
        .map(|x| x.kind)
        .collect::<Vec<_>>();
    NodeKind::new(
        IrNodeId::FieldMut {
            idx,
            fields: fields.clone(),
        },
        0,
        0,
        vec![IrKind::PartialMut(IrKind::Adt(adt.to_string()).into()).into()],
        vec![IrKind::Mut(fields[idx].clone().into())],
    )
}

/// 解构一个视图(`PartialMut<Adt>`),取出各字段的视图。
///
/// 和 [`node_ir_destructure`] 的区别:这里不查 `Target`,字段类型直接由
/// `fields`(结构体本身的字段类型)包装出来 —— 优化期消 `FieldMut` 时要用。
pub fn node_ir_destructure_view(view: IrKind, fields: &[IrKind]) -> NodeKindIr {
    NodeKind::new(
        IrNodeId::Destructure,
        0,
        0,
        vec![Some(view)],
        fields
            .iter()
            .map(|x| IrKind::mutable_view(x.clone()))
            .collect(),
    )
}
