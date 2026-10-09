//! 解析 .ogia / .rlib 并展平成可渲染的图
//!
//! .ogia 是 bincode(serde) 序列化的 `Target`,格式定义见 `core/src/linker.rs:50`。

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use gpui::{Point, Size, point};
use rust2genshin::asset::generated::identifier;
use rust2genshin::asset::GameMode;
use rust2genshin::compile::func::{FnDecl, NodeGraphIr};
use rust2genshin::compile::ir::{FnInfo, IrKind, IrNodeId, Optimizer};
use rust2genshin::compile::link::{Linker, Target};
use rust2genshin::node::layout::layout_with;
use rust2genshin::node::{Link, LinkTarget, NativeNodeId, NodeKind, NodeRef, ValueIn};

/// 解析一个 .ogia 文件
pub fn load_ogia(path: &Path) -> Result<Target, String> {
    let f = File::open(path).map_err(|e| format!("打开文件失败 {path:?}: {e}"))?;
    bincode::serde::decode_from_reader(BufReader::new(f), bincode::config::standard())
        .map_err(|e| format!("解析 .ogia 失败 {path:?}: {e}"))
}

/// 解析一个 .rlib,返回其中所有 .ogia entry 解析出的 Target
pub fn load_rlib(path: &Path) -> Result<Vec<Target>, String> {
    let f = File::open(path).map_err(|e| format!("打开文件失败 {path:?}: {e}"))?;
    let mut archive = ar::Archive::new(BufReader::new(f));
    let mut result = Vec::new();
    while let Some(entry) = archive.next_entry() {
        let mut entry = entry.map_err(|e| format!("读取 rlib 条目失败 {path:?}: {e}"))?;
        let name = String::from_utf8(Vec::from(entry.header().identifier()))
            .map_err(|e| format!("条目名非 UTF-8: {e}"))?;
        if !name.ends_with(".ogia") {
            continue;
        }
        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .map_err(|e| format!("读取 {name} 内容失败: {e}"))?;
        let target: Target = bincode::serde::decode_from_reader(
            BufReader::new(&content[..]),
            bincode::config::standard(),
        )
        .map_err(|e| format!("解析 {name} 失败: {e}"))?;
        result.push(target);
    }
    if result.is_empty() {
        return Err(format!("{path:?} 中未找到 .ogia 条目"));
    }
    Ok(result)
}

/// 防御式合并。**不要**用 `Target: AddAssign` —— 它在两边都有 main 时 panic。
pub fn merge(into: &mut Target, from: Target) {
    if into.main.is_none() {
        into.main = from.main;
    }
    into.functions.extend(from.functions);
    into.adts.extend(from.adts);
}

/// 按扩展名分派。
///
/// ⚠️ 走 `.rlib` 分支时,会把归档里所有 `.ogia` 依次 [`merge`],
/// **只有第一个带 `main` 的会被保留**,其余的 `main` 被静默丢弃
/// (和 `Target: AddAssign` 不同,这里不会 panic)。
/// 想要逐个列出归档里的**所有**图,请直接用 [`load_rlib`],它返回未合并的原始 `Vec<Target>`。
pub fn load(path: &Path) -> Result<Target, String> {
    match path.extension().and_then(|x| x.to_str()) {
        Some("ogia") => load_ogia(path),
        Some("rlib") => {
            let mut t = Target::default();
            for x in load_rlib(path)? {
                merge(&mut t, x);
            }
            Ok(t)
        }
        other => Err(format!("不支持的扩展名: {other:?}")),
    }
}

// ------------------------------------------------------------------ 展平层

// 节点几何(宽度估算 / 高度 / pin 间距)由 core 提供,viewer 只做渲染。
// **不要**在这里另起一份:core 的自动整理(`node::layout`)就是按这套尺寸
// 排布坐标的,两份尺寸漂开的那一刻,框、字、间距和坐标就会互相压盖。
pub use rust2genshin::node::layout::{
    BASE_FONT_PX, NODE_H, NODE_W, PIN_GAP, node_height, node_width,
};

/// 节点标签。
///
/// `IrNodeId` 没有 `Display` 但有 `Debug`,而 `Debug` 对**全部六个单元变体**
/// (`Local` / `SetLocal` / `Unreachable` / `Assemble` / `Destructure` / `Modify`)
/// 给出的正是想要的名字。带载荷的变体里:`Fn` 直接用函数名,`Native` 走
/// [`sys_call_label`](常用 sys call 给短名,其它回退到 `syscall#<id>`)。
/// **不要**写八个分支各返回一段字符串常量。
///
/// 类型标注一律用 `Debug` 而不是 `Display`:`impl Display for NativeKind`
/// 在 `Struct` 上是 `panic!`(`core/src/value.rs`),而 viewer 的输入是用户
/// 随手拖进来的任意 `.ogia`,不能假设碰不到那一支。
///
/// 这个字符串也是 [`node_width`] 的依据(展平时就算好宽度),所以它放展平层,
/// 不留在渲染层 —— 两边必须是同一个函数,不然算出来的宽对不上显示的字。
pub fn label(kind: &IrNodeId, type_label: &str) -> String {
    let base = match kind {
        IrNodeId::Native(x) => sys_call_label(x),
        IrNodeId::Fn(s) => s.clone(),
        other => format!("{other:?}"),
    };
    if type_label.is_empty() {
        base
    } else {
        format!("{base}<{type_label}>")
    }
}

/// `IrNodeId::Native` 节点的显示名。常用 sys call(`SysCallStub` 那一组)给
/// 短名(如 `log` / `if`),表里查不到就回退到 `syscall#<id>`,**别**用
/// `SysCallStub#<id>` 那串没意义的资产类型前缀。其它 `AssetKind`(`GeneratedStub`
/// 等 —— 用户声明的复合节点)保持 `{kind:?}#{id}`,那是它们的真实身份,不是
/// 系统调用,viewer 不去给它起名。
fn sys_call_label(x: &NativeNodeId) -> String {
    if x.kind == identifier::AssetKind::SysCallStub {
        match sys_call_name(x.id) {
            Some(name) => name.to_string(),
            None => format!("syscall#{}", x.id),
        }
    } else {
        format!("{:?}#{}", x.kind, x.id)
    }
}

/// 常用 sys call 节点的 id → 短名。id 与 `core/src/node/*.rs` 的
/// `NodeKind::simple(id, ...)` 调用对齐 —— 想加新条目就照着那个 id 填。
/// 走 `Some` 走短名、没命中走 `syscall#<id>`,**不要**把没把握的 id 也填进来
/// 起一个猜的名字 —— 看图的人会被误导。
fn sys_call_name(id: i64) -> Option<&'static str> {
    match id {
        1 => Some("log"), // NODE_LOG
        2 => Some("if"), // NODE_IF
        5 => Some("for"), // NODE_FOR_CLOSED
        6 => Some("break"), // NODE_BREAK
        22 => Some("set_var"), // NODE_SET_VARIABLE
        66 => Some("set_status"), // NODE_SET_STATUS
        69 => Some("destroy_entity"),
        70 => Some("create_entity"),
        77 => Some("settle"), // NODE_SETTLE
        190 => Some("forward_event"),
        _ => None,
    }
}

#[cfg(test)]
mod sys_call_label_tests {
    use super::*;
    use rust2genshin::asset::generated::identifier;
    use rust2genshin::node::NativeNodeId;

    /// 造一个 `NativeNodeId`。
    ///
    /// core 把 `selectors_*` / `imps_out` / `references` / `using_struct`
    /// 这些**外壳字段**从 `NodeKind` 搬进了 `NativeNodeId`(`refactor(core):
    /// NativeNodeId`),所以手写结构体字面量得把它们都填上。这几个字段
    /// 与标签无关,统一填"空"。
    ///
    /// **不要**改成 `..Default::default()` —— `NativeNodeId` 没有 `Default`
    /// impl,加了只是把编译错误推迟到字段变动时;显式列全更好。
    fn native_id(kind: identifier::AssetKind, id: i64, kernel: i64) -> NativeNodeId {
        NativeNodeId {
            kind,
            id,
            kernel,
            selectors_in: vec![],
            selectors_out: vec![],
            imps_out: vec![],
            references: vec![],
            using_struct: None,
        }
    }

    fn sys(id: i64) -> NativeNodeId {
        native_id(identifier::AssetKind::SysCallStub, id, 0)
    }

    /// 命中的 id → 短名,不再带 `#<id>`。带类型标注的也照样挂 `<T>`。
    #[test]
    fn recognized_sys_calls_get_short_names() {
        assert_eq!(label(&IrNodeId::Native(sys(1)), ""), "log");
        assert_eq!(label(&IrNodeId::Native(sys(2)), ""), "if");
        assert_eq!(label(&IrNodeId::Native(sys(5)), ""), "for");
        assert_eq!(label(&IrNodeId::Native(sys(77)), ""), "settle");
        // 类型标注照样挂上去
        assert_eq!(label(&IrNodeId::Native(sys(2)), "Bool"), "if<Bool>");
    }

    /// 没命中的 id → `syscall#<id>`,**不要**带 `SysCallStub#<id>` 前缀。
    #[test]
    fn unrecognized_sys_calls_fall_back_to_syscall_prefix() {
        assert_eq!(label(&IrNodeId::Native(sys(411)), ""), "syscall#411");
        assert_eq!(label(&IrNodeId::Native(sys(1928)), ""), "syscall#1928");
        assert_eq!(label(&IrNodeId::Native(sys(411)), "Int"), "syscall#411<Int>");
    }

    /// 非 `SysCallStub` 的 Native(`GeneratedStub` —— 用户声明的复合节点)
    /// 走原来的 `{kind:?}#{id}`,**不**被覆盖成 `syscall#<id>`。那是它们的
    /// 真实身份,viewer 不去给它起名。
    #[test]
    fn non_syscall_native_keeps_kind_prefix() {
        let composite = native_id(identifier::AssetKind::GeneratedStub, 7, 0);
        assert_eq!(
            label(&IrNodeId::Native(composite), ""),
            "GeneratedStub#7"
        );
    }

    /// `kernel` 是同 id 的**变体**编号(多分支 `node_switch` 就是 `kernel`
    /// 3=Int / 4=Str),标签只按 `id` 查 —— 同一个 `id` 的不同 kernel
    /// 共用同一个短名。这里锁住"kernel 不进短名表",不然将来有人想按
    /// (id, kernel) 建表时会被这条挡住。
    #[test]
    fn kernel_variant_does_not_change_the_label() {
        let int_switch = native_id(identifier::AssetKind::SysCallStub, 3, 3);
        let str_switch = native_id(identifier::AssetKind::SysCallStub, 3, 4);
        // 两个 kernel 都查 id=3,表里没有 → 都回退到同一个 `syscall#3`。
        assert_eq!(label(&IrNodeId::Native(int_switch), ""), "syscall#3");
        assert_eq!(label(&IrNodeId::Native(str_switch), ""), "syscall#3");
    }

    /// `label` 同时是 [`node_width`] 的依据 —— 短名应当比兜底短
    /// (回退路径会带 `#<id>`,撑宽节点)。
    #[test]
    fn short_name_does_not_force_widening() {
        let short = label(&IrNodeId::Native(sys(2)), "");
        let long = label(&IrNodeId::Native(sys(411)), "");
        assert!(short.len() < long.len(), "{short:?} 应当比 {long:?} 短");
        assert!(node_width(&short) <= NODE_W, "短名不应撑宽,实测 {}px", node_width(&short));
    }
}

/// 一个连到图外的 pin。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportPin {
    /// 节点侧第几个 pin(和渲染的 `pin_y` 用同一个序号)。
    pub pin: usize,
    /// 图外的边界号 —— `Link { target: LinkTarget::Export, index }` 的 `index`,
    /// 对应该图 `externals` 里第几组(控制入/控制出/参数/返回值各数各的)。
    pub export: usize,
}

pub struct NodeEntry {
    /// 这张图里的稳定身份(`Slab` 的 key)。**不是**下标 —— 优化/撤销会增删
    /// 节点,下标会变,`NodeRef` 不变:坐标就是靠它跨重展平对上的。
    pub node_ref: NodeRef,
    pub kind: IrNodeId,
    /// 类型标注,如 `"Int"`。取自 `NodeKind`:
    ///   `Local`    → `values_out_types[1]`  (`[0]` 是 `LocalRef(T)`)
    ///   `SetLocal` → `values_in_types[1]`   (`values_out_types` 为空)
    ///
    /// 其余类别为空串。
    pub type_label: String,
    /// 控制入/出 pin 的**总数**(不是已连接的边数)。
    /// 渲染要把 N 条入边在节点侧边纵向均开,得先知道分母。
    pub controls_in_num: usize,
    pub controls_out_num: usize,
    /// 值入/值出 pin 的总数(`values_in_types.len()` / `values_out_types.len()`)。
    /// 值流的连接点排在控制流下方,侧边的分母是这两类 pin 的总和。
    pub values_in_num: usize,
    pub values_out_num: usize,
    /// 每个**值入** pin 的类型标签,与 `values_in_num` 同长;**空串** = 这个
    /// pin 没类型(`NodeKind::values_in_types` 里是 `None`)。统一用空串而
    /// 不是 `Option`,和 `NodeEntry::type_label`、`NodeEntry::value_in_defaults`
    /// 的既有约定一致,悬停层不用到处判两种"没有"。
    ///
    /// 悬停气泡显示的就是它(`view::hit_test`)。节点级的 `type_label` 只覆盖
    /// `Local` / `SetLocal` 那几种,盖不住每个 pin。
    pub values_in_types: Vec<String>,
    /// 每个**值出** pin 的类型标签,与 `values_out_num` 同长。值出的
    /// `NodeKind::values_out_types` 是 `Vec<IrKind>`(没有"无类型"),所以
    /// 这里每一项都非空。
    pub values_out_types: Vec<String>,
    /// 连到**图外**的 pin(图边界)。渲染时从这些 pin 往图外引一小段桩线、
    /// 末端标上图外边界号(`Export N`)—— 取代了以前节点里的 `[in]` / `[out]`
    /// 文字标记,而且能精确到是**哪一个引脚、哪一号边界**。
    ///
    /// core 里 `LinkTarget::Export` 那侧的 `Links` 就是 `externals`
    /// (`node/mod.rs:405`),但节点侧同样留了反向记录,所以这四列直接从
    /// 节点自己的 `links` 里读。
    pub ctrl_in_exports: Vec<ExportPin>,
    pub ctrl_out_exports: Vec<ExportPin>,
    pub value_in_exports: Vec<ExportPin>,
    pub value_out_exports: Vec<ExportPin>,
    /// 自动整理算出的坐标(左上角),直接来自 IR 节点的 `Node::position`。
    /// 布局发生在 `Doc::push_target`(加载)与 `Doc::relayout`(自动整理按钮),
    /// 都由 core 的 `node::layout` 写进节点 —— viewer 自己不发明坐标。
    pub position: Point<f32>,
    /// 值入 pin 的**默认值**(与值入 pin 逐位对应,`None` = 没默认值)。
    /// 常量输入在 IR 里就是 `ValueIn::default`(如 IF 的条件默认 `true`、
    /// 字面量 `5`),不显示出来这一块信息在画布上就丢了。
    /// 用 `Debug` 格式化(`core/src/value.rs` 的 `NativeValue: Debug`,
    /// 具体值是 `bool` / `i32` / `f32` / `String` / `Vec3` 这些基本类型)。
    pub value_in_defaults: Vec<Option<String>>,
    pub size: Size<f32>,
}

pub struct Edge {
    /// `(节点下标, pin 序号)`
    pub from: (usize, usize),
    pub to: (usize, usize),
    /// `true` = 控制流边
    pub ctrl: bool,
}

pub struct GraphEntry {
    pub name: String,
    /// `NodeRef` → `nodes` 下标。`Slab` 删过节点后 key 有空洞,
    /// 两者**不恒等**,必须显式映射。
    pub index_of: HashMap<NodeRef, usize>,
    /// 入口块:控制流从图外流入的节点(展平下标)。分层布局的 BFS 起点。
    pub entry: Vec<usize>,
    /// 返回块:控制流流回图外的节点(展平下标)。渲染时标出来。
    pub exit: Vec<usize>,
    pub nodes: Vec<NodeEntry>,
    pub edges: Vec<Edge>,
    // 注:`externals` 本身的**值边**(参数传入 / 返回值传出 / 入口 / 返回)不画
    // 成普通边 —— 图外没有对端节点。边界改由每个节点侧边的桩线 + 图边界号覆盖
    // (`NodeEntry::*_exports`),两侧都不缺表示。
}

/// 一张图在它所属 `Target` 里的位置。
#[derive(Clone, Debug, PartialEq, Eq)]
enum GraphSource {
    /// `Target::main`
    Main,
    /// `Target::functions` 里的一个函数
    Fn(String),
}

/// `graphs[i]` 归属于谁:哪个 `Target`、那张 `Target` 里的哪张图。
#[derive(Clone)]
struct GraphOwner {
    target: usize,
    source: GraphSource,
}

/// 展平结果 + 它背后的原始数据。
///
/// 右栏的优化按钮要**直接改** `Target` 里的 `NodeGraphIr`(`Optimizer` 的消除
/// 规则全在它上面),所以 `Target` 必须活着 —— 不能像以前那样 flatten 完就丢。
///
/// # `targets` 与 `linkers` 为什么要分开存
///
/// `Linker::touch_fn` / `Optimizer::link_node` 会从 `Linker.target` 里
/// `remove` 函数,`Linker::link` 还会 `take` 走 main —— 它会**搬空**自己
/// 那个 `target` 字段。viewer 真正要的图数据(展平、encode/decode、
/// take/put)必须**走 `targets`**,**绝不**从 `linkers[i].target` 读。
/// `linkers` 只在 `Optimizer::link_node` 需要它的时候摸一下,之后那一份
/// 缓存就搁着不动(`AssetBundle`、已展开函数的 `CompiledFn` 都住里面,
/// 重复 link 命中缓存)。`Linker::link()` / `save()` viewer 从不调,
/// 所以 `Linker::output` 用空路径占位。
pub struct Doc {
    /// main 第一,其余按函数名排序
    pub graphs: Vec<GraphEntry>,
    /// 与 `graphs` 同长。
    owners: Vec<GraphOwner>,
    /// **唯一的图真值** —— 展平、encode、take/put、decode_into、relayout
    /// 全部走这里。与 `linkers` 同长,下标对齐。
    targets: Vec<Target>,
    /// 每个 Target 配套的 Linker(创建时从 `targets` clone 一份过去,
    /// 之后与 `targets` 状态分叉)。只供 `Optimizer::link_node` 摸 —
    /// **不要从这里读图数据**。
    linkers: Vec<Linker>,
}

impl Doc {
    pub fn new() -> Self {
        Self {
            graphs: Vec::new(),
            owners: Vec::new(),
            targets: Vec::new(),
            linkers: Vec::new(),
        }
    }

    /// 把一个 `Target` 的全部图展平追进来,返回第一张新图的下标。
    /// 一个图都没有(没有 main、也没有函数)时返回 `None`。
    ///
    /// 布局只在这里(加载时)算一次,之后切图不重算 —— 拖过的位置能留住,
    /// 想重排得走 [`Doc::relayout`](「自动整理」按钮)。
    pub fn push_target(&mut self, mut t: Target) -> Option<usize> {
        let target = self.targets.len();
        let first = self.graphs.len();
        if let Some(main) = &mut t.main {
            layout_graph(main);
        }
        for f in t.functions.values_mut() {
            layout_graph(&mut f.graph);
        }
        // 喂给 Linker 的是 **clone** —— `Linker::new` 会把传进去的 `Target`
        // 抢走,`targets[k]` 必须留着,后面 `take` / `put` / `reflatten` 全
        // 走它。Linker 那一份之后会随 `link_node` 状态分叉,与我们无关。
        let linker_t = t.clone();
        self.targets.push(t);
        self.linkers
            .push(Linker::new(GameMode::Beyond, linker_t, PathBuf::new()));
        // 展平读 `targets[k]`,**不读** `linkers[k].target`(后者的状态不可信)。
        if let Some(main) = &self.targets[target].main {
            self.graphs.push(flatten_one("main".to_string(), main));
            self.owners.push(GraphOwner {
                target,
                source: GraphSource::Main,
            });
        }
        let mut names: Vec<&String> = self.targets[target].functions.keys().collect();
        names.sort();
        for name in names {
            self.graphs
                .push(flatten_one(name.clone(), &self.targets[target].functions[name].graph));
            self.owners.push(GraphOwner {
                target,
                source: GraphSource::Fn(name.clone()),
            });
        }
        (self.graphs.len() > first).then_some(first)
    }

    /// 对第 `i` 张图跑一次自动整理(core 的布局把坐标写回 IR 节点),然后
    /// 重新展平(渲染读的是展平快照,不重展平看不见新坐标)。
    /// 返回 `false` 表示图或它背后的数据没了。
    ///
    /// 布局的**唯一实现**在 core(`rust2genshin::node::layout`):`.gia` 编码前
    /// 走的是同一套,viewer 与产物不会各排各的。
    pub fn relayout(&mut self, i: usize) -> bool {
        let Some(owner) = self.owners.get(i).cloned() else {
            return false;
        };
        let Some(t) = self.targets.get_mut(owner.target) else {
            return false;
        };
        let g = match &owner.source {
            GraphSource::Main => match t.main.as_mut() {
                Some(g) => g,
                None => return false,
            },
            GraphSource::Fn(name) => match t.functions.get_mut(name) {
                Some(f) => &mut f.graph,
                None => return false,
            },
        };
        layout_graph(g);
        self.reflatten(i)
    }

    /// 重新展平第 `i` 张图。优化 / 撤销改完 `Target` 里的图之后**必须**调用,
    /// 否则渲染的还是旧快照。返回 `false` 表示图或它背后的数据没了。
    pub fn reflatten(&mut self, i: usize) -> bool {
        let Some(owner) = self.owners.get(i).cloned() else {
            return false;
        };
        let Some(t) = self.targets.get(owner.target) else {
            return false;
        };
        let g = match &owner.source {
            GraphSource::Main => match &t.main {
                Some(g) => g,
                None => return false,
            },
            GraphSource::Fn(name) => match t.functions.get(name) {
                Some(f) => &f.graph,
                None => return false,
            },
        };
        self.graphs[i] = flatten_one(self.graphs[i].name.clone(), g);
        true
    }

    /// 第 `i` 张图的当前状态编码成字节(撤销 / panic 回滚共用)。
    ///
    /// 函数编**整个 `FnInfo`**(graph / decl / description / exported 一并保住),
    /// 主图编 `NodeGraphIr`(主图在 core 里本来就没有配套 decl,`link.rs:79`
    /// 用的也是 `Default::default()`)。配置必须与 `load_ogia` 完全一致。
    pub fn encode(&self, i: usize) -> Result<Vec<u8>, String> {
        let (t, source) = self.target_and_source(i).ok_or("图下标越界")?;
        match source {
            GraphSource::Main => encode_bytes(t.main.as_ref().ok_or("主图不存在")?),
            GraphSource::Fn(name) => encode_bytes(t.functions.get(name).ok_or("函数不存在")?),
        }
    }

    /// 用 [`Doc::encode`] 产出的字节整体换掉第 `i` 张图。
    ///
    /// 解码成功之前不改任何东西(原子):解码失败时原来的图原样留着。
    /// `Slab` 的 serde 保留 key(空洞保留),所以换回来 `NodeRef` 完全一致。
    pub fn decode_into(&mut self, i: usize, bytes: &[u8]) -> Result<(), String> {
        let owner = self.owners.get(i).cloned().ok_or("图下标越界")?;
        let t = self
            .targets
            .get_mut(owner.target)
            .ok_or("Target 不存在")?;
        match owner.source {
            GraphSource::Main => t.main = Some(decode_bytes(bytes)?),
            GraphSource::Fn(name) => {
                let info: FnInfo = decode_bytes(bytes)?;
                t.functions.insert(name, info);
            }
        }
        Ok(())
    }

    /// 把第 `i` 张图从 `Target` 里**搬出来**,包成 `Optimizer`(消除规则全在它上面)。
    ///
    /// 跑完必须用 [`Doc::put`] 或 [`Doc::decode_into`] 放回去 ——
    /// 搬出来期间 `Target` 里那张图是空的。
    pub fn take(&mut self, i: usize) -> Option<Optimizer> {
        let owner = self.owners.get(i).cloned()?;
        let t = self.targets.get_mut(owner.target)?;
        match owner.source {
            GraphSource::Main => {
                let graph = t.main.take()?;
                // 主图的 decl 在 core 里也是 `Default::default()`(`link.rs:79`)。
                Some(Optimizer {
                    graph,
                    decl: FnDecl::default(),
                })
            }
            GraphSource::Fn(name) => {
                let f = t.functions.get_mut(&name)?;
                // `NodeGraphIr` 没有 `Default`,占位图只为把真图换出来。
                let placeholder = NodeGraphIr::new(f.graph.class, f.graph.name.clone());
                let graph = std::mem::replace(&mut f.graph, placeholder);
                let decl = std::mem::take(&mut f.decl);
                Some(Optimizer { graph, decl })
            }
        }
    }

    /// 把 [`Doc::take`] 搬出来的图放回去(不重展平 —— 那是 [`Doc::reflatten`])。
    pub fn put(&mut self, i: usize, opt: Optimizer) {
        let Some(owner) = self.owners.get(i).cloned() else {
            return;
        };
        let Some(t) = self.targets.get_mut(owner.target) else {
            return;
        };
        match owner.source {
            GraphSource::Main => t.main = Some(opt.graph),
            GraphSource::Fn(name) => {
                if let Some(f) = t.functions.get_mut(&name) {
                    f.graph = opt.graph;
                    f.decl = opt.decl;
                }
            }
        }
    }

    fn target_and_source(&self, i: usize) -> Option<(&Target, &GraphSource)> {
        let owner = self.owners.get(i)?;
        let t = self.targets.get(owner.target)?;
        Some((t, &owner.source))
    }

    /// 第 `i` 张图所属的 `Linker` —— `Optimizer::link_node` 展开函数节点时要它
    /// (已展开函数的缓存、结构体资产都在里面)。**别**用它来读图数据,
    /// 那份与 `targets[i]` 已经分叉了。
    pub fn linker_mut(&mut self, i: usize) -> Option<&mut Linker> {
        let owner = self.owners.get(i)?;
        self.linkers.get_mut(owner.target)
    }
}

/// 编码配置与解析端(`load_ogia`)、core linker 完全一致,否则解不开。
fn encode_bytes<T: serde::Serialize>(v: &T) -> Result<Vec<u8>, String> {
    bincode::serde::encode_to_vec(v, bincode::config::standard())
        .map_err(|e| format!("编码图失败: {e}"))
}

fn decode_bytes<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard())
        .map(|(v, _)| v)
        .map_err(|e| format!("解码图失败: {e}"))
}

/// 类型标签。
///
/// 一律用 `Debug` 而**不是** `Display`:`impl Display for NativeKind`
/// (`core/src/value.rs:40`)在 `Struct` 上直接 `panic!("{r:?}")`,
/// 而 viewer 的输入是用户拖进来的任意 `.ogia`,不能假设碰不到这一支。
fn type_label(kind: Option<&IrKind>) -> String {
    match kind {
        // 剥掉 `IrKind::Native` 这层壳再打,简单类型才是 `Int` 而不是
        // `Native(Int)` —— 前者才是能直接贴到节点上的短标签。
        Some(IrKind::Native(native)) => format!("{native:?}"),
        // `compile_ty` 对每个非原生类型都返回 `Adt(key)`,是最常见的非 native
        // 路径。key 本身就是类型名,加外壳既没信息量又会在 140px 宽的节点上溢出。
        Some(IrKind::Adt(key)) => key.clone(),
        Some(kind) => format!("{kind:?}"),
        None => String::new(),
    }
}

/// 一个 IR 节点的类型标注(哪些节点有类型见 `flatten_one`)。
///
/// 哪些节点需要带 `<类型>`:
/// - `Local`    — 值的**出口**类型(`values_out_types[1]`,0 号是 `LocalRef`)
/// - `SetLocal` — 值的**入口**类型(`values_in_types[1]`,0 号是 `LocalRef`)
/// - `Assemble`     — 组装出的结构体类型(`values_out_types[0]`,入参是字段)
/// - `Destructure`  — 被解构的结构体类型(`values_in_types[0]`,出参是字段)
/// - `Modify`       — 被修改的结构体类型(`values_in_types[0]`,后面跟字段 setter)
///
/// `type_label` 对 `IrKind::Adt(key)` 直接返回 `key` 本身,组装的图节点
/// 不会显示成 `<Adt("MyStruct")>` 这种带壳的串 —— 节点宽 140,装不下。
fn node_type_label(kind: &NodeKind<IrNodeId, IrKind>) -> String {
    match &kind.id {
        IrNodeId::Local => type_label(kind.values_out_types.get(1)),
        IrNodeId::SetLocal => type_label(kind.values_in_types.get(1).and_then(|k| k.as_ref())),
        IrNodeId::Assemble => type_label(kind.values_out_types.get(0)),
        IrNodeId::Destructure => type_label(kind.values_in_types.get(0).and_then(|k| k.as_ref())),
        IrNodeId::Modify => type_label(kind.values_in_types.get(0).and_then(|k| k.as_ref())),
        _ => String::new(),
    }
}

/// 对 IR 图跑一遍自动整理(core 的布局),坐标写进每个节点的 `Node::position`。
///
/// 标签用**渲染同一个** `label`(带 `<类型>` 后缀)—— core 的宽度估算就是按
/// 这个字符串来的,换别的字符串算出来的宽度和画出来的字对不上。
fn layout_graph(g: &mut NodeGraphIr) {
    layout_with(g, |kind| label(&kind.id, &node_type_label(kind)));
}

/// 从一侧的 pin 列表里挑出「连到图外」的 pin(保持升序)。
/// `find_export` 返回该 pin 连到图外时的边界号 —— 一个 pin 连了多个边界时
/// 只记第一个(实际产物里不出现,留个确定性)。
fn export_pins<T>(pins: &[T], find_export: impl Fn(&T) -> Option<usize>) -> Vec<ExportPin> {
    pins.iter()
        .enumerate()
        .filter_map(|(pin, x)| find_export(x).map(|export| ExportPin { pin, export }))
        .collect()
}

fn flatten_one(name: String, g: &NodeGraphIr) -> GraphEntry {
    let mut index_of = HashMap::new();
    let mut nodes = Vec::new();
    for (key, node) in g.nodes.iter() {
        index_of.insert(NodeRef::from(key), nodes.len());
        let node_type = node_type_label(&node.kind);
        // 宽度按**显示出来的**标签估算 —— 和渲染用的是同一个 `label` 函数。
        let width = node_width(&label(&node.kind.id, &node_type));
        // 连到图外的 pin。节点侧的 `links` 里存着 `LinkTarget::Export` 的反向
        // 记录(`node/mod.rs:405`:Export 那侧才是 `externals`),所以按 pin
        // 逐个查即可 —— 边界号就在 `Link::index` 里,不用去翻 `g.externals`。
        let export_index = |l: &Link| (l.target == LinkTarget::Export).then_some(l.index);
        let ctrl_in_exports = export_pins(&node.links.controls_in, |ls: &Vec<Link>| {
            ls.iter().find_map(export_index)
        });
        let ctrl_out_exports = export_pins(&node.links.controls_out, |ls: &Vec<Link>| {
            ls.iter().find_map(export_index)
        });
        let value_in_exports = export_pins(&node.links.values_in, |v: &ValueIn| {
            v.link.and_then(|l| export_index(&l))
        });
        let value_out_exports = export_pins(&node.links.values_out, |ls: &Vec<Link>| {
            ls.iter().find_map(export_index)
        });
        let pins = (node.kind.controls_in_num + node.kind.values_in_types.len())
            .max(node.kind.controls_out_num + node.kind.values_out_types.len());
        nodes.push(NodeEntry {
            node_ref: NodeRef::from(key),
            position: point(node.position.0, node.position.1),
            kind: node.kind.id.clone(),
            type_label: node_type,
            controls_in_num: node.kind.controls_in_num,
            controls_out_num: node.kind.controls_out_num,
            values_in_num: node.kind.values_in_types.len(),
            values_out_num: node.kind.values_out_types.len(),
            // 逐 pin 的类型,悬停气泡用(`view::hit_test`)。`values_in_types`
            // 是 `Vec<Option<IrKind>>`,`None` 标签化成空串;`values_out_types`
            // 是 `Vec<IrKind>`,没有"无类型"这一说,每一项都非空。
            values_in_types: node
                .kind
                .values_in_types
                .iter()
                .map(|k| type_label(k.as_ref()))
                .collect(),
            values_out_types: node
                .kind
                .values_out_types
                .iter()
                .map(|k| type_label(Some(k)))
                .collect(),
            ctrl_in_exports,
            ctrl_out_exports,
            value_in_exports,
            value_out_exports,
            value_in_defaults: node
                .links
                .values_in
                .iter()
                .map(|v| v.default.as_ref().map(|d| format!("{d:?}")))
                .collect(),
            size: Size {
                width,
                height: node_height(pins),
            },
        });
    }
    let mut edges = Vec::new();
    // 边一律**从来源节点的 out 侧**发出:`links` 存的是双向的,
    // 走 out 侧才只画一次,不会一条边出两遍。
    for (key, node) in g.nodes.iter() {
        let from = match index_of.get(&NodeRef::from(key)) {
            Some(&i) => i,
            None => continue,
        };
        for (pin, outs) in node.links.controls_out.iter().enumerate() {
            for out in outs {
                // 指向已删除节点的边直接跳过,不能 panic
                if let Some(&to) = out.target.node().and_then(|n| index_of.get(&n)) {
                    edges.push(Edge {
                        from: (from, pin),
                        to: (to, out.index),
                        ctrl: true,
                    });
                }
            }
        }
        for (pin, outs) in node.links.values_out.iter().enumerate() {
            for out in outs {
                if out.target == LinkTarget::Export {
                    continue; // 指向 export 的值边不画
                }
                if let Some(&to) = out.target.node().and_then(|n| index_of.get(&n)) {
                    edges.push(Edge {
                        from: (from, pin),
                        to: (to, out.index),
                        ctrl: false,
                    });
                }
            }
        }
    }
    // 控制流边界。`externals` 的方向容易看反:
    //   `controls_out` = 从图外**流出**的边,收件人是图的入口块
    //     (`compile/mod.rs:324` 的 `link_control(Link::export(0), block.begin)`)
    //   `controls_in`  = 流向图外的边,来源是返回块
    //     (`compile/func.rs:393` 的 `link_control(block.end, Link::export(0))`)
    // Optimizer 可能把这些组整个清空(`compile/ir.rs:386-392`),所以用
    // `iter().flatten()` 而不是 `[0]`,空组自然得到空 Vec。
    // 指向已删除节点的记录同样过滤掉,不能 unwrap。
    let mut entry = Vec::new();
    for out in g.externals.controls_out.iter().flatten() {
        if let Some(&to) = out.target.node().and_then(|n| index_of.get(&n)) {
            entry.push(to);
        }
    }
    let mut exit = Vec::new();
    for out in g.externals.controls_in.iter().flatten() {
        if let Some(&to) = out.target.node().and_then(|n| index_of.get(&n)) {
            exit.push(to);
        }
    }
    GraphEntry {
        name,
        index_of,
        entry,
        exit,
        nodes,
        edges,
    }
}

/// 展平一个 `Target`(main 第一,其余按函数名排序)。
///
/// **按值收**:`Doc` 要把 `Target` 留下来给右栏的优化按钮改(`Doc::take`),
/// 所以调用方别再指望展平完自己还能用这个 `Target`。
pub fn flatten(t: Target) -> Doc {
    let mut d = Doc::new();
    d.push_target(t);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust2genshin::compile::func::{FnDecl, NodeGraphIr};
    use rust2genshin::compile::ir::FnInfo;
    use rust2genshin::node::{ExportDecl, NodeGraphKind};
    use std::io::Write;

    /// `NodeGraphIr` 没有 `Default` impl(core 侧只有 `NodeGraph::new`),所以只能显式构造。
    fn graph_named(name: &str) -> NodeGraphIr {
        NodeGraphIr::new(NodeGraphKind::ServerEntity, name)
    }

    /// 带名字的临时文件路径,避免并行测试互相覆盖。
    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rust2genshin-viewer-test-{}-{name}",
            std::process::id()
        ))
    }

    /// 测试跑完删掉临时文件;设 `R2G_KEEP_FIXTURE=1` 时保留。
    ///
    /// 本仓库**没有 `.ogia` 的产出端**(全仓只有 `core/src/linker.rs:50` 在读),
    /// 所以人工验证没有现成文件可拖。这个闸门让 `R2G_KEEP_FIXTURE=1 cargo test`
    /// 留下合成夹具,供 Task 6/7 拖进窗口打通"拖入 → 解析 → 渲染 → 拖动"这条链。
    ///
    /// 合成图**不能**验证 `entry` / `exit` 的方向(那是从 core 源码反推的),
    /// 详见 `.superpowers/sdd/.../progress.md` 的 R9。
    fn cleanup(path: &Path) {
        if std::env::var_os("R2G_KEEP_FIXTURE").is_none() {
            let _ = std::fs::remove_file(path);
        }
    }

    /// 诊断:把 `R2G_CHECK=<路径>` 指向的文件解析一遍并打印结果。
    ///
    /// 排查"拖进去显示 X"这类问题时,先用它确认到底是文件里没有图、还是图里
    /// 没有节点、还是解析直接失败。跑法:
    /// `R2G_CHECK=xxx.rlib cargo test -p rust2genshin-viewer --lib -- --nocapture diagnose`
    #[test]
    fn diagnose_a_file_from_env() {
        let Ok(path) = std::env::var("R2G_CHECK") else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        println!("=== 诊断 {} ===", path.display());
        match super::load(&path) {
            Err(e) => println!("解析失败: {e}"),
            Ok(t) => {
                println!(
                    "main: {}, functions: {} 个, adts: {} 个",
                    if t.main.is_some() { "有" } else { "无" },
                    t.functions.len(),
                    t.adts.len()
                );
                let d = super::flatten(t);
                println!("展平出 {} 张图", d.graphs.len());
                for g in &d.graphs {
                    println!(
                        "  {:<24} 节点 {:>4} 边 {:>4}(控制 {:>3} 值 {:>3}) 入口 {:?} 返回 {:?}",
                        g.name,
                        g.nodes.len(),
                        g.edges.len(),
                        g.edges.iter().filter(|e| e.ctrl).count(),
                        g.edges.iter().filter(|e| !e.ctrl).count(),
                        g.entry,
                        g.exit
                    );
                }
            }
        }
    }

    /// 按 core / linker 用的同一套 bincode 配置编码。
    fn encode(t: &Target) -> Vec<u8> {
        bincode::serde::encode_to_vec(t, bincode::config::standard()).expect("序列化 Target")
    }

    /// 用 `ar::Builder` 现场写一个合成 ar 归档(零新增依赖,`ar` 已是直接依赖)。
    fn write_ar(path: &Path, entries: &[(&str, Vec<u8>)]) {
        let mut b = ar::Builder::new(File::create(path).expect("创建临时 ar"));
        for (name, data) in entries {
            let header = ar::Header::new(name.as_bytes().to_vec(), data.len() as u64);
            b.append(&header, &data[..]).expect("append ar entry");
        }
        b.into_inner().expect("收尾 ar");
    }

    /// 只有一个全局头、零个 entry 的合法 ar。
    /// (不能用 `ar::Builder` 干这事:它只在第一次 `append` 时才写 `!<arch>\n`,
    ///  不 append 就会产出 0 字节文件,那样失败点会提前到读全局头,而不是我们要测的分支。)
    fn write_empty_ar(path: &Path) {
        File::create(path)
            .expect("创建临时 ar")
            .write_all(b"!<arch>\n")
            .expect("写 ar 全局头");
    }

    #[test]
    fn merge_keeps_first_main_instead_of_panicking() {
        let mut a = Target::default();
        a.main = Some(graph_named("a"));
        let mut b = Target::default();
        b.main = Some(graph_named("b"));
        // AddAssign 在这种输入下会 panic,merge 不能
        merge(&mut a, b);
        let main = a.main.as_ref().expect("merge 后 main 仍在");
        assert_eq!(main.name, "a", "merge 应保留先到的 main");
    }

    #[test]
    fn load_rejects_unknown_extension() {
        // 不能用 `unwrap_err()`:`Target` 没有 `Debug`(core 侧未 derive)
        let Err(err) = load(Path::new("foo.txt")) else {
            panic!("未知扩展名应当返回 Err");
        };
        assert!(err.contains("不支持的扩展名"), "实际错误: {err}");
    }

    #[test]
    fn load_missing_file_is_an_err_not_a_panic() {
        let r = load(Path::new("definitely_not_here.ogia"));
        assert!(r.is_err());
    }

    /// 上面三个测试都没走到 `load_ogia` 的**成功**路径,这里补一个 bincode 往返,
    /// 锁住「`load_ogia` 能原样解回自己编出来的字节」这个契约。
    /// 注意:两端都用 `standard()`,所以这是 `load_ogia` 的**自洽性**检查,
    /// 并不证明与 core 实际产出的 `.ogia` 兼容(那需要真实产物才能验证)。
    #[test]
    fn load_ogia_round_trips_a_target() {
        let mut t = Target::default();
        t.main = Some(graph_named("round-trip"));
        // functions 是 String 键 -> FnInfo 的 map,别让它一直是空的
        t.functions.insert(
            "fn-a".to_string(),
            FnInfo {
                description: "一个函数".to_string(),
                graph: graph_named("fn-a-graph"),
                decl: FnDecl::default(),
                exported: true,
            },
        );

        let path = temp_path("round-trip.ogia");
        std::fs::write(&path, encode(&t)).expect("写临时 .ogia");

        let loaded = load_ogia(&path).expect("load_ogia 应当成功");
        cleanup(&path);

        let main = loaded.main.as_ref().expect("main 应当被解出来");
        assert_eq!(main.name, "round-trip");
        // `NodeGraphKind` 没有 `PartialEq`/`Debug`,只能 `matches!`
        assert!(matches!(main.class, NodeGraphKind::ServerEntity));
        assert!(main.is_empty(), "构造的空图应当没有节点");

        let f = loaded.functions.get("fn-a").expect("functions 条目应当被解出来");
        assert_eq!(f.description, "一个函数");
        assert_eq!(f.graph.name, "fn-a-graph");
        assert!(f.exported, "exported 标志应当保持");
    }

    /// Review Focus #1:归档里没有 `.ogia` entry 时,必须报「未找到」,
    /// 而不是返回空图或 panic。
    #[test]
    fn load_rlib_reports_error_when_archive_has_no_ogia() {
        let path = temp_path("no-ogia.rlib");
        // 真实 rlib 里总有 `lib.rmeta` 之类,顺便覆盖 `.ogia` 过滤那步
        write_ar(&path, &[("lib.rmeta", b"not an ogia".to_vec())]);

        let Err(err) = load_rlib(&path) else {
            panic!("没有 .ogia entry 时应当返回 Err");
        };
        cleanup(&path);
        assert!(err.contains("未找到 .ogia"), "实际错误: {err}");
    }

    /// 同上,但归档是**零 entry**(只有全局头)。
    #[test]
    fn load_rlib_reports_error_on_empty_archive() {
        let path = temp_path("empty.rlib");
        write_empty_ar(&path);

        let Err(err) = load_rlib(&path) else {
            panic!("空归档时应当返回 Err");
        };
        cleanup(&path);
        assert!(err.contains("未找到 .ogia"), "实际错误: {err}");
    }

    /// Review Focus #2:多个 `.ogia` entry 都带 `main` 时,防御式合并取第一个,
    /// 既不 panic 也不把后面的丢掉。同时覆盖 ar 遍历 / `.ogia` 过滤 / 逐 entry decode 三段。
    #[test]
    fn load_merges_multiple_ogia_entries_keeping_first_main() {
        let mut a = Target::default();
        a.main = Some(graph_named("a"));
        let mut b = Target::default();
        b.main = Some(graph_named("b"));

        let path = temp_path("two-ogia.rlib");
        write_ar(&path, &[("a.ogia", encode(&a)), ("b.ogia", encode(&b))]);

        // load_rlib 是**未合并**的原始状态:两个 Target,各自保留自己的 main
        let targets = load_rlib(&path).expect("load_rlib 应当成功");
        assert_eq!(targets.len(), 2, "两个 .ogia entry 都应当被解出来");
        assert_eq!(targets[0].main.as_ref().unwrap().name, "a");
        assert_eq!(targets[1].main.as_ref().unwrap().name, "b");

        // load 走 merge:只保留第一个 main,且不 panic
        // (`Target: AddAssign` 在这里会 panic —— 这正是不能用的原因)
        let loaded = load(&path).expect("load .rlib 应当成功");
        cleanup(&path);
        let main = loaded.main.as_ref().expect("main 应当保留");
        assert_eq!(main.name, "a", "应当保留第一个 main");
    }

    // ---------------------------------------------------------------- 展平层

    /// `FnInfo` 没有 `Default`(core 侧只 derive 了 Clone + Serialize),
    /// 只能显式补全四个字段。
    fn fn_info(graph: NodeGraphIr) -> FnInfo {
        FnInfo {
            description: String::new(),
            graph,
            decl: FnDecl::default(),
            exported: false,
        }
    }

    #[test]
    fn flatten_produces_main_first_then_sorted_functions() {
        let mut t = Target::default();
        t.main = Some(graph_named("the-main"));
        t.functions.insert("zeta".into(), fn_info(graph_named("zeta")));
        t.functions.insert("alpha".into(), fn_info(graph_named("alpha")));
        let doc = flatten(t);
        // main 第一,其余按名排序
        assert_eq!(doc.graphs.len(), 3);
        assert_eq!(doc.graphs[0].name, "main");
        assert_eq!(doc.graphs[1].name, "alpha");
        assert_eq!(doc.graphs[2].name, "zeta");
    }

    #[test]
    fn flatten_of_empty_target_is_empty() {
        let doc = flatten(Target::default());
        assert!(doc.graphs.is_empty());
    }

    /// `NodeGraph::remove` 会先把边摘干净,所以**用**它删节点根本造不出悬空边,
    /// 那样这条测试只会走到「图是空的」那一步,根本碰不到 `index_of` 查不到时
    /// 跳过边的分支。这里直接操作 `g.nodes`(slab)绕过 unlink,人为留下悬空 link。
    #[test]
    fn flatten_skips_edges_pointing_at_removed_nodes() {
        use rust2genshin::compile::ir::{IrKind, node_ir_set_local};
        use rust2genshin::node::{Link, NodeGraphKind};
        use rust2genshin::value::NativeKind;

        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        // `node_ir_local` 是 0 进 0 出的纯值节点,连不了控制边;
        // `node_ir_set_local` 才是 1 进 1 出。
        let a = g.insert(node_ir_set_local(&IrKind::Native(NativeKind::Int)));
        let b = g.insert(node_ir_set_local(&IrKind::Native(NativeKind::Int)));
        g.link_control(Link::node(a, 0), Link::node(b, 0));
        // 绕过 `NodeGraph::remove`,直接捅 slab —— a 的 controls_out[0] 里
        // 留下一条指向已删除节点的边。
        g.nodes.remove(usize::from(b));

        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        let ge = &doc.graphs[0];
        // a 还在,b 没了 => NodeRef(0) 映射到下标 0,NodeRef(1) 查不到
        assert_eq!(ge.nodes.len(), 1, "被删的节点不该出现在展平结果里");
        assert_eq!(ge.index_of.get(&a), Some(&0));
        assert_eq!(ge.index_of.get(&b), None);
        assert_eq!(ge.nodes[0].type_label, "Int", "SetLocal 的类型标注取 values_in_types[1]");
        // 悬空边必须被跳过,不能 panic
        assert!(ge.edges.is_empty(), "指向已删除节点的边应当被跳过");
    }

    /// 边从**来源节点的 out 侧**发出,所以一条真实连接的边只应出现一次。
    /// 同时锁住 `values_out` 那条分支。
    #[test]
    fn flatten_emits_each_value_edge_once_with_target_pin() {
        use rust2genshin::compile::ir::{IrKind, node_ir_local, node_ir_set_local};
        use rust2genshin::node::{Link, NodeGraphKind};
        use rust2genshin::value::NativeKind;

        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        // 下标 0:值的来源(纯值节点,values_out[0] 是 Int)
        let src = g.insert(node_ir_local(&IrKind::Native(NativeKind::Int)));
        // 下标 1:消费方(SetLocal 的 values_in[1])
        let dst = g.insert(node_ir_set_local(&IrKind::Native(NativeKind::Int)));
        g.link_value(Link::node(src, 0), Link::node(dst, 1));

        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        let ge = &doc.graphs[0];
        assert_eq!(ge.nodes.len(), 2);
        assert_eq!(ge.nodes[0].type_label, "Int", "Local 的类型标注取 values_out_types[1]");
        assert_eq!(ge.edges.len(), 1, "一条连接只应产出一条边,不能正反各来一次");
        assert_eq!(ge.edges[0].from, (0, 0));
        assert_eq!(ge.edges[0].to, (1, 1), "目标 pin 序号应取自 Link.index");
        assert!(!ge.edges[0].ctrl, "values_out 出来的是值边");
    }

    // ------------------------------------------------------- F2:控制 pin 计数

    /// 渲染要在节点侧边把 N 条入边纵向均开,必须有 pin 总数。
    /// 展平时逐节点直接从 `NodeKind` 取,不再回原始 IR。
    ///
    /// 第三个节点是关键:它**声明** 2 个控制入但**只连了 1 条**边。
    /// 按「已连接边数」反推的实现会得到 1 而失败 —— 那正是分边逻辑最自然的错实现。
    #[test]
    fn flatten_copies_control_pin_counts_from_node_kind() {
        use rust2genshin::compile::ir::{IrKind, node_ir_local, node_ir_set_local};
        use rust2genshin::node::{Link, NodeGraphKind};
        use rust2genshin::value::NativeKind;

        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        g.insert(node_ir_local(&IrKind::Native(NativeKind::Int))); // 下标 0
        let src = g.insert(node_ir_set_local(&IrKind::Native(NativeKind::Int))); // 下标 1
        // `insert` 内部才按 `controls_in_num` 建 `Links`,所以改完再插才生效。
        let mut two_in = node_ir_set_local(&IrKind::Native(NativeKind::Int));
        two_in.controls_in_num = 2;
        let two = g.insert(two_in); // 下标 2
        // 只连 1 条边,另一个控制入空着
        g.link_control(Link::node(src, 0), Link::node(two, 0));

        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        let ge = &doc.graphs[0];
        assert_eq!((ge.nodes[0].controls_in_num, ge.nodes[0].controls_out_num), (0, 0), "Local 是 0 进 0 出");
        assert_eq!((ge.nodes[1].controls_in_num, ge.nodes[1].controls_out_num), (1, 1), "SetLocal 是 1 进 1 出");
        assert_eq!(ge.nodes[2].controls_in_num, 2, "读的是声明的 pin 数,不是已连接边数(那是 1)");
        assert_eq!(ge.nodes[2].controls_out_num, 1);
        assert_eq!(ge.edges.len(), 1, "确实只连了 1 条边");
    }

    // ------------------------------------------------------ F1:externals 边界

    /// 造一个带控制流边界的图:`externals.controls_out[0]` 是入口块,
    /// `externals.controls_in[0]` 是返回块(方向见 `compile/mod.rs:321-324`
    /// 与 `compile/func.rs:393`)。
    fn graph_with_externals() -> (NodeGraphIr, NodeRef, NodeRef) {
        use rust2genshin::compile::ir::{IrKind, node_ir_set_local};
        use rust2genshin::node::{Link, NodeGraphKind};
        use rust2genshin::value::NativeKind;

        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        g.push_export_control_in(ExportDecl::new("".into(), None));
        g.push_export_control_out(ExportDecl::new("".into(), None));
        let entry = g.insert(node_ir_set_local(&IrKind::Native(NativeKind::Int)));
        let exit = g.insert(node_ir_set_local(&IrKind::Native(NativeKind::Int)));
        // 外部 -> 入口块
        g.link_control(Link::export(0), Link::node(entry, 0));
        // 返回块 -> 外部
        g.link_control(Link::node(exit, 0), Link::export(0));
        (g, entry, exit)
    }

    /// **走 main 分支。** 入口/返回块必须被记成展平下标,
    /// 否则每个函数的边界都会画成孤立块 —— 而那正是本工具要帮用户排除的假警报。
    #[test]
    fn flatten_records_entry_and_exit_for_main() {
        let (g, entry, exit) = graph_with_externals();
        let mut t = Target::default();
        t.main = Some(g);
        let doc = flatten(t);
        let ge = &doc.graphs[0];
        assert_eq!(ge.name, "main");
        assert_eq!(ge.entry, vec![0], "entry 应指向展平后的入口块");
        assert_eq!(ge.exit, vec![1], "exit 应指向展平后的返回块");
        assert_eq!(ge.entry[0], ge.index_of[&entry]);
        assert_eq!(ge.exit[0], ge.index_of[&exit]);
    }

    /// 同上,但**走 functions 分支** —— 两条 push 路径都要有覆盖。
    #[test]
    fn flatten_records_entry_and_exit_for_functions() {
        let (g, entry, exit) = graph_with_externals();
        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        let ge = &doc.graphs[0];
        assert_eq!(ge.name, "f");
        assert_eq!(ge.entry, vec![0]);
        assert_eq!(ge.exit, vec![1]);
        assert_eq!(ge.entry[0], ge.index_of[&entry]);
        assert_eq!(ge.exit[0], ge.index_of[&exit]);
    }

    /// 没调过 `push_export_*` 的图,`externals` 四组边全是空的
    /// (连 `controls_out[0]` 这个下标都不存在),必须返回空 Vec 而不是 panic。
    #[test]
    fn flatten_of_graph_without_externals_has_empty_entry_and_exit() {
        use rust2genshin::compile::ir::{IrKind, node_ir_set_local};
        use rust2genshin::node::NodeGraphKind;
        use rust2genshin::value::NativeKind;

        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        g.insert(node_ir_set_local(&IrKind::Native(NativeKind::Int)));
        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        let ge = &doc.graphs[0];
        assert!(ge.entry.is_empty());
        assert!(ge.exit.is_empty());
    }

    /// 入口块自己被 Optimizer 删掉时,`externals` 里会留下悬空 link,
    /// 同样要过 `index_of` 过滤,不能 unwrap。
    #[test]
    fn flatten_skips_entry_and_exit_pointing_at_removed_nodes() {
        use rust2genshin::node::NodeGraphKind;
        let (mut g, entry, _exit) = graph_with_externals();
        assert!(matches!(g.class, NodeGraphKind::ServerEntity));
        g.nodes.remove(usize::from(entry));
        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        let ge = &doc.graphs[0];
        assert!(ge.entry.is_empty(), "指向已删除入口块的记录应当被跳过");
        assert_eq!(ge.exit.len(), 1, "返回块还在,应当保留");
    }

    // ---------------------------------------------------- F3:type_label 不 panic

    /// `NativeKind::Struct` 的 `Display` 在 `core/src/value.rs:47` 直接
    /// `panic!("{r:?}")`。viewer 的输入是用户拖进来的任意 `.ogia`,
    /// 走到这一支就必须炸,所以标签一律走 `Debug`。
    #[test]
    fn flatten_labels_a_struct_typed_local_without_panicking() {
        use rust2genshin::asset::Identifier;
        use rust2genshin::compile::ir::{IrKind, node_ir_local};
        use rust2genshin::node::NodeGraphKind;
        use rust2genshin::structure::StructRef;
        use rust2genshin::value::NativeKind;

        let sref = StructRef {
            id: Identifier::default(),
            // 字段带名字(core 的 `StructRef.fields` 是 `(String, NativeKind)`)。
            fields: vec![("value".to_string(), NativeKind::Int)],
        };
        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        g.insert(node_ir_local(&IrKind::Native(NativeKind::Struct(sref))));

        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        let label = &doc.graphs[0].nodes[0].type_label;
        assert!(label.contains("Struct"), "标签应体现这是结构体: {label}");
    }

    /// 简单类型的标签不受 F3 的改法影响,仍是 `Int` 而不是 Debug 的 `Native(Int)`。
    #[test]
    fn flatten_keeps_plain_labels_for_simple_types() {
        use rust2genshin::compile::ir::{IrKind, node_ir_local};
        use rust2genshin::node::NodeGraphKind;
        use rust2genshin::value::NativeKind;

        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        g.insert(node_ir_local(&IrKind::Native(NativeKind::Int)));
        g.insert(node_ir_local(&IrKind::Native(NativeKind::Bool)));
        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        assert_eq!(doc.graphs[0].nodes[0].type_label, "Int");
        assert_eq!(doc.graphs[0].nodes[1].type_label, "Bool");
    }

    /// `core/src/compile/ty.rs:132` 的 `compile_ty` 对**每一个**非原生类型
    /// (结构体、元组、闭包捕获……)都返回 `IrKind::Adt(key)` ——
    /// 结构体是本仓库最常见的非 native 类型,不是冷门路径。
    /// 140px 宽的节点上 Debug 的 `Adt("MyStruct")` 会溢出,标签只取 key。
    #[test]
    fn flatten_labels_an_adt_with_just_the_key() {
        use rust2genshin::compile::ir::{IrKind, node_ir_local};
        use rust2genshin::node::NodeGraphKind;

        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        g.insert(node_ir_local(&IrKind::Adt("MyStruct".into())));

        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        assert_eq!(doc.graphs[0].nodes[0].type_label, "MyStruct");
    }

    /// Assemble / Destructure / Modify 三个结构体节点的类型标注:
    /// - **Assemble**     取 `values_out_types[0]`(组装出的结构体)
    /// - **Destructure**  取 `values_in_types[0]`(被拆的结构体)
    /// - **Modify**       取 `values_in_types[0]`(被改的结构体)
    ///
    /// 整图节点宽 140px,只显示 `Adt` 的 key(`MyStruct`)就够,不能塞 `Adt("MyStruct")`。
    /// 「读错 pin 取错类型」是这处最自然的错实现 —— `values_in_types[0]` 拿到
    /// `MyStruct`,`values_out_types[0]` 在 Assemble 上**也是**`MyStruct`(输出),
    /// 看起来都对;但 Destructure 那边 `values_in_types[0]` 是 struct、
    /// `values_out_types[0]` 是**字段**(Int),跑反就拿到 `Int` 而不是 `MyStruct`。
    #[test]
    fn flatten_labels_the_three_struct_nodes_with_the_adt_key() {
        use rust2genshin::compile::ir::{IrKind, IrNodeId};
        use rust2genshin::node::{NodeGraphKind, NodeKind};
        use rust2genshin::value::NativeKind;

        let adt = IrKind::Adt("MyStruct".into());
        // 字段随便给一个 —— 标的是 struct 类型,不是字段类型。
        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        // Assemble: 0 进 0 出,fields 进(struct out)
        g.insert(NodeKind::new(
            IrNodeId::Assemble,
            0,
            0,
            vec![Some(IrKind::Native(NativeKind::Int))],
            vec![adt.clone()],
        ));
        // Destructure: 0 进 0 出,struct 进(fields out)
        g.insert(NodeKind::new(
            IrNodeId::Destructure,
            0,
            0,
            vec![Some(adt.clone())],
            vec![IrKind::Native(NativeKind::Int)],
        ));
        // Modify: 1 进 1 出,struct + key select + 字段 pairs
        g.insert(NodeKind::new(
            IrNodeId::Modify,
            1,
            1,
            vec![
                Some(adt.clone()),
                None,
                Some(IrKind::Native(NativeKind::Int)),
                Some(IrKind::Native(NativeKind::Bool)),
            ],
            vec![],
        ));

        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        let labels: Vec<&str> = doc.graphs[0]
            .nodes
            .iter()
            .map(|n| n.type_label.as_str())
            .collect();
        // 三个都该是 `MyStruct`,不是 `Int`、不是 `Adt("MyStruct")`。
        // (字段 Int 只能出现在 Destructure 的 `values_out_types[0]`,
        //  跑反就拿到 `Int` —— 那是最自然的错实现。)
        assert_eq!(labels, vec!["MyStruct", "MyStruct", "MyStruct"]);
    }

    // ------------------------------------------------- 优化/撤销的存取通道

    /// 主图走的是 `Target::main` 那组分支(`take` / `put` / `encode` /
    /// `decode_into` 都是 `match` 的另一臂),函数图测不到它,得单独走一遍。
    /// 同时锁住撤销的基石:**解码后 `NodeRef` 逐一相同**。
    #[test]
    fn main_graph_take_put_encode_decode_round_trip() {
        use rust2genshin::compile::ir::{IrKind, node_ir_set_local};
        use rust2genshin::node::NodeGraphKind;
        use rust2genshin::value::NativeKind;

        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "m");
        let n = g.insert(node_ir_set_local(&IrKind::Native(NativeKind::Int)));
        let mut t = Target::default();
        t.main = Some(g);
        let mut doc = flatten(t);
        assert_eq!(doc.graphs[0].name, "main");

        let bytes = doc.encode(0).expect("主图编码");
        // 搬出去、改掉、放回来,重展平要看到改动
        let mut opt = doc.take(0).expect("取出主图");
        opt.graph.remove(n);
        doc.put(0, opt);
        assert!(doc.reflatten(0));
        assert!(doc.graphs[0].nodes.is_empty(), "删掉的节点不该还在");

        // 存档换回去:`NodeRef` 必须原样回来(撤销就靠这个)
        doc.decode_into(0, &bytes).expect("主图解码");
        assert!(doc.reflatten(0));
        let refs_after: Vec<NodeRef> = doc.graphs[0].nodes.iter().map(|x| x.node_ref).collect();
        assert_eq!(refs_after, vec![n], "解码后 NodeRef 应逐一相同");
        assert!(doc.encode(0).is_ok(), "放回去之后还要能再编码");
    }

    /// 两张图各自独立:重新展平第 1 张不碰第 0 张。
    /// 这是撤销栈按图分开(`HashMap<图下标, ...>`)成立的前提。
    #[test]
    fn reflatten_only_touches_the_requested_graph() {
        use rust2genshin::compile::ir::{IrKind, node_ir_set_local};
        use rust2genshin::node::NodeGraphKind;
        use rust2genshin::value::NativeKind;

        let mk = |name: &str| {
            let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, name);
            g.insert(node_ir_set_local(&IrKind::Native(NativeKind::Int)));
            g
        };
        let mut doc = Doc::new();
        let mut t = Target::default();
        t.functions.insert("a".into(), fn_info(mk("a")));
        assert_eq!(doc.push_target(t), Some(0));
        let mut t = Target::default();
        t.functions.insert("b".into(), fn_info(mk("b")));
        assert_eq!(doc.push_target(t), Some(1));
        assert_eq!(doc.graphs.len(), 2);

        // 把第 1 张的节点删掉并重展平,第 0 张必须纹丝不动
        let n = doc.graphs[1].nodes[0].node_ref;
        let mut opt = doc.take(1).expect("取出第 1 张");
        opt.graph.remove(n);
        doc.put(1, opt);
        assert!(doc.reflatten(1));
        assert!(doc.graphs[1].nodes.is_empty());
        assert_eq!(doc.graphs[0].nodes.len(), 1, "第 0 张不该被碰到");
    }

    /// `Linker::touch_fn` 会在 link 时把函数从 `linker.target.functions` 里
    /// `remove` 走 —— 这是**它自己**的副本,我们的 `Doc::targets` 必须不被牵连。
    /// 跑过 `link_fn_node` 之后,`take` / `encode` / `reflatten` 仍要能从
    /// `Doc::targets` 里拿回被 link 那个函数的图(可写、可读、可重展平)。
    /// **`take` 在那一份 `&linker.target` 上跑的实现**会在这里失败 —— `remove`
    /// 之后 `get_mut("callee")` 拿不到,`take` 返回 `None`。
    ///
    /// 夹具用 `crate::doc::fixture::linkable_target()` —— `optimize.rs` 的
    /// `link_expands_a_fn_node_in_place` 也用同一份,这样新约束("link 之后
    /// callee 还能从 `Doc::targets` 拿回")和老约束("link 成功、Fn 节点变
    /// 低层复合节点")共用一份图。
    #[test]
    fn doc_targets_survive_linker_touch_fn_remove() {
        use crate::optimize::link_fn_node;

        let t = crate::doc::fixture::linkable_target();
        let mut doc = Doc::new();
        doc.push_target(t);
        let caller_i = doc
            .graphs
            .iter()
            .position(|g| g.name == "caller")
            .expect("caller 图存在");
        let callee_i = doc
            .graphs
            .iter()
            .position(|g| g.name == "callee")
            .expect("callee 图存在");
        let fnn_ref = doc.graphs[caller_i]
            .nodes
            .iter()
            .find(|n| matches!(n.kind, IrNodeId::Fn(_)))
            .expect("caller 里有 Fn 节点")
            .node_ref;

        // 触发 link_node —— 内部走 `linker.touch_fn("callee")`,把 callee 从
        // **Linker 自己的** target.functions 移除。这一步**只**应消耗 Linker
        // 副本,Doc::targets 里的 callee 必须原封不动。
        let refs = doc.graphs[caller_i]
            .nodes
            .iter()
            .map(|n| n.node_ref)
            .collect::<Vec<_>>();
        let pos = doc.graphs[caller_i]
            .nodes
            .iter()
            .map(|n| n.position)
            .collect::<Vec<_>>();
        let r = link_fn_node(&mut doc, caller_i, &refs, &pos, fnn_ref);
        assert!(r.undo.is_some(), "link 应当成功:{}", r.status);

        // 现在该检验的核心不变量:Doc::targets 仍持有 callee,走 `take` / `encode`
        // 都能拿到它(读 `&linker.target` 的实现在这里拿不到 —— `remove` 过了)。
        let opt = doc.take(callee_i).expect("callee 仍要能从 Doc::targets 取出");
        assert!(!opt.graph.is_empty(), "callee 图不能被 link 副作用吃掉");
        doc.put(callee_i, opt);

        let bytes = doc.encode(callee_i).expect("callee 仍要能编码");
        assert!(!bytes.is_empty());

        assert!(doc.reflatten(callee_i), "callee 仍要能重展平");
    }

    /// 图边界要精确到**哪个引脚**,不是只有节点级的 `entry` / `exit` ——
    /// 桩线渲染(`view::build_stubs`)吃的就是这四个列表。
    #[test]
    fn flatten_marks_pins_that_reach_outside_the_graph() {
        use rust2genshin::compile::ir::{IrKind, node_ir_local, node_ir_set_local};
        use rust2genshin::node::{Link, NodeGraphKind};
        use rust2genshin::value::NativeKind;

        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));
        // 值参数 / 返回值的边界槽位(**先 push 再 link** —— 不然
        // `externals.values_out` / `values_in` 里没有下标,`link_value` 直接越界)。
        g.push_export_value_in(ExportDecl::new("param".into(), None));
        g.push_export_value_in(ExportDecl::new("param2".into(), None));
        g.push_export_value_out(ExportDecl::new("ret".into(), None));
        let s = g.insert(node_ir_set_local(&IrKind::Native(NativeKind::Int)));
        let l = g.insert(node_ir_local(&IrKind::Native(NativeKind::Int)));

        // 控制流:图外 → s → 图外
        g.link_control(Link::export(0), Link::node(s, 0));
        g.link_control(Link::node(s, 0), Link::export(0));
        // 值流:普通连接(l 的 LocalRef 出口 → s 的 LocalRef 入口)、
        // 参数 0 进 s 的值入 1、参数 1 进 l 的值入 0、l 的值出 1 是返回值。
        g.link_value(Link::node(l, 0), Link::node(s, 0));
        g.link_value(Link::export(0), Link::node(s, 1));
        g.link_value(Link::export(1), Link::node(l, 0));
        g.link_value(Link::node(l, 1), Link::export(0));

        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        let ge = &doc.graphs[0];
        let ns = &ge.nodes[ge.index_of[&s]];
        let nl = &ge.nodes[ge.index_of[&l]];
        let pin = |pin, export| ExportPin { pin, export };

        assert_eq!(ns.ctrl_in_exports, vec![pin(0, 0)], "s 的控制入 0 从边界 0 来");
        assert_eq!(ns.ctrl_out_exports, vec![pin(0, 0)], "s 的控制出 0 去边界 0");
        assert_eq!(ns.value_in_exports, vec![pin(1, 0)], "s 的值入 1 是参数 0");
        assert_eq!(ns.value_out_exports, vec![], "s 没有值出边界");

        assert_eq!(nl.value_out_exports, vec![pin(1, 0)], "l 的值出 1 是返回值 0");
        // 牙口一:边界号必须逐 pin 对上 —— 「清一色记 0」的实现会在 l 上失败。
        assert_eq!(nl.value_in_exports, vec![pin(0, 1)], "l 的值入 0 是参数 **1**");
        // 牙口二:l 的值出 0(LocalRef)接的是**普通**边,
        // 「节点沾了边界就把整排 pin 都标上」的实现会在这里失败。
        assert_eq!(nl.ctrl_in_exports, vec![], "l 没有控制入边");
        assert_eq!(nl.ctrl_out_exports, vec![], "l 没有控制出边");
    }

    /// 值入 pin 的默认值要展平出来(常量输入在 IR 里就是 `ValueIn::default`)。
    #[test]
    fn flatten_shows_value_in_defaults() {
        use rust2genshin::compile::ir::{IrKind, node_ir_set_local};
        use rust2genshin::node::{Link, NodeGraphKind};
        use rust2genshin::value::NativeKind;

        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        let s = g.insert(node_ir_set_local(&IrKind::Native(NativeKind::Int)));
        // 值入 1(值槽)给个字面量,值入 0(LocalRef 槽)不给 —— 两条路都要。
        g.set_default(Link::node(s, 1), Some(5.into()));

        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        let ns = &doc.graphs[0].nodes[0];
        assert_eq!(ns.value_in_defaults, vec![None, Some("5".to_string())]);
    }

    /// 标签长了节点要自动拉宽(短标签保持下限),而且**往宽了估** ——
    /// 估窄了文字会被 `overflow_hidden` 裁掉,那正是要修的毛病。
    #[test]
    fn node_width_grows_with_long_labels() {
        assert_eq!(node_width("Local<Int>"), NODE_W, "短标签用下限");
        let long = "rust2genshin_demo::MyStruct::new";
        assert!(node_width(long) > NODE_W, "长标签要拉宽");
        assert!(node_width(long) >= long.len() as f32 * 6.6, "只许估宽");
    }

    /// 展平时算宽度用的必须是**显示出来的**那个标签 —— `Fn` 节点的标签是
    /// 整个函数名,最容易撑破 140px。
    #[test]
    fn flatten_widens_nodes_with_long_fn_names() {
        use rust2genshin::node::NodeKind;

        let long = "rust2genshin_demo::MyStruct::new";
        let mut g: NodeGraphIr = NodeGraphIr::new(NodeGraphKind::ServerEntity, "t");
        g.insert(NodeKind::new(
            IrNodeId::Fn(long.to_string()),
            0,
            0,
            vec![],
            vec![],
        ));

        let mut t = Target::default();
        t.functions.insert("f".into(), fn_info(g));
        let doc = flatten(t);
        let n = &doc.graphs[0].nodes[0];
        assert_eq!(crate::doc::label(&n.kind, &n.type_label), long);
        assert!(
            n.size.width > NODE_W,
            "Fn 名字长,节点应被拉宽,实际 {}",
            n.size.width
        );
    }

    /// 节点高度按 pin 数长高(`spread` 的段距 = 高/(pin+1)),间距才有下限。
    #[test]
    fn node_height_grows_with_pin_count() {
        assert_eq!(node_height(0), NODE_H, "没 pin 的节点用下限");
        assert_eq!(node_height(1), NODE_H, "1 个 pin 下限就够");
        assert!(node_height(3) > node_height(2), "3 个 pin 的节点要更高");
        for pins in 0..8 {
            let gap = node_height(pins) / (pins + 1) as f32;
            assert!(gap >= PIN_GAP, "{pins} 个 pin 的段距 {gap} 小于 {PIN_GAP}");
        }
    }
}

// ------------------------------------------------------------------ 夹具产出

/// 合成夹具。
///
/// # 它是什么
///
/// 一个**手写**的合成 `Target`,只作测试用的确定性输入。
///
/// 真实数据是有的:`cargo run -p build-demo` 会在
/// `target/genshin-unknown-server/release/` 下产出带 `.ogia` entry 的 rlib
/// (`librust2genshin_demo.rlib` 等)。所以这个夹具**不是**"没有真数据时的替代品",
/// 它只是让测试不必跑一次完整后端。
///
/// # 它验证不了什么
///
/// - 真实图上 `IrKind` / `LinkTarget` 的实际取值分布、大图性能,没在这个夹具上测过。
/// - 真实 rlib 里存在**完全空的函数图**(例如 `___rustc17rust_begin_unwind`,
///   0 节点 0 边、入口返回都空)—— 夹具里没有这种图。
pub mod fixture {
    use std::path::Path;

    use rust2genshin::compile::func::{FnDecl, NodeGraphIr, node_ir_native};
    use rust2genshin::compile::ir::{FnInfo, IrKind, IrNodeId, node_ir_local, node_ir_set_local};
    use rust2genshin::compile::link::Target;
    use rust2genshin::node::control::NODE_IF;
    use rust2genshin::node::{ExportDecl, Link, NodeGraphKind, NodeKind};
    use rust2genshin::value::NativeKind;

    fn int() -> IrKind {
        IrKind::Native(NativeKind::Int)
    }

    /// `entry → if → (a, b) → join → tail → exit`,外加一组数据流边。
    ///
    /// 控制流:
    /// ```text
    ///  entry(0) ─→ if(1) ─┬─→ a(2) ───┐
    ///                      └─→ b(3) ───┴─→ join(4) → tail(5) → exit(6)
    /// ```
    fn graph() -> NodeGraphIr {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "diamond");

        // `externals` 的两个方向最容易看反,照 core 源码写:
        //   `link_control(Link::export(0), block.begin)` → 落在 `controls_out`(入口)
        //   `link_control(block.end, Link::export(0))`     → 落在 `controls_in`(返回)
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));

        // `node_ir_local` 是 0 进 0 出的纯值节点,连不了控制边;要接控制流得用
        // `node_ir_set_local`(1 进 1 出)。
        let entry = g.insert(node_ir_set_local(&int()));
        let branch = g.insert(node_ir_native(NODE_IF.clone()));
        let a = g.insert(node_ir_set_local(&int()));
        let b = g.insert(node_ir_set_local(&int()));
        let join = g.insert(node_ir_set_local(&int()));
        let tail = g.insert(node_ir_set_local(&int()));
        let exit = g.insert(node_ir_set_local(&int()));

        // 纯值节点:0 控制进出,只走数据流。
        // 注意方向:`node_ir_local` 的 `values_out_types = [LocalRef(T), T]`(**2 个值出**),
        // 而 `node_ir_set_local` 的 `values_out_types` 是**空的**(0 个值出)——
        // 拿 SetLocal 当值边源会索引越界 panic(`node/mod.rs:419`)。
        // 真实的 IR 就是 `GetLocal → SetLocal` 这个方向。
        let get_a = g.insert(node_ir_local(&int()));
        let get_b = g.insert(node_ir_local(&int()));
        let get_join = g.insert(node_ir_local(&int()));

        g.link_control(Link::export(0), Link::node(entry, 0));
        g.link_control(Link::node(entry, 0), Link::node(branch, 0));
        g.link_control(Link::node(branch, 0), Link::node(a, 0));
        g.link_control(Link::node(branch, 1), Link::node(b, 0));
        g.link_control(Link::node(a, 0), Link::node(join, 0));
        g.link_control(Link::node(b, 0), Link::node(join, 0));
        g.link_control(Link::node(join, 0), Link::node(tail, 0));
        g.link_control(Link::node(tail, 0), Link::node(exit, 0));
        g.link_control(Link::node(exit, 0), Link::export(0));

        // 数据流:每个 SetLocal 的值输入由一个 GetLocal 提供,`join` 另有一条值流
        // 汇入 `tail`,让 viewer 的值流边(虚线)有东西可画。
        //
        // `values_in_types = [LocalRef(T), T]`、`values_out_types = [LocalRef(T), T]`,
        // 所以 T 的下标两边都是 **1**(0 号是 `LocalRef`)。
        g.link_value(Link::node(get_a, 1), Link::node(a, 1));
        g.link_value(Link::node(get_b, 1), Link::node(b, 1));
        g.link_value(Link::node(get_join, 1), Link::node(join, 1));
        g.link_value(Link::node(get_join, 1), Link::node(tail, 1));
        g
    }

    /// 造一个含上述图 + 几个不同形状的函数的 `Target`。
    pub fn target() -> Target {
        let mut t = Target {
            main: Some(graph()),
            functions: std::collections::HashMap::new(),
            adts: std::collections::HashMap::new(),
        };
        // 第二个函数:一条直链,没有分支 —— 让左侧列表有多于一项可切。
        t.functions.insert(
            "straight_line".into(),
            FnInfo {
                description: "线性函数".into(),
                graph: straight(),
                decl: Default::default(),
                exported: true,
            },
        );
        // 第三个:带环 —— 用来确认 layout 的环防护真的生效(不会挂死)。
        t.functions.insert(
            "has_a_loop".into(),
            FnInfo {
                description: "含回边,验证布局不会挂死".into(),
                graph: cyclic(),
                decl: Default::default(),
                exported: true,
            },
        );
        t
    }

    fn straight() -> NodeGraphIr {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "straight_line");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));
        let mut prev = None;
        let mut last = None;
        for _ in 0..4 {
            let n = g.insert(node_ir_set_local(&int()));
            if let Some(p) = prev {
                g.link_control(Link::node(p, 0), Link::node(n, 0));
            }
            prev = Some(n);
            last = Some(n);
        }
        g.link_control(Link::export(0), Link::node(prev.unwrap(), 0));
        g.link_control(Link::node(last.unwrap(), 0), Link::export(0));
        g
    }

    /// `0 → 1 → 2 → 0` 的回边。`func.rs:397` 的 `Goto` 不检查方向,
    /// 所以真实 crate 里的 `loop` / `while` 会产生这种图。
    fn cyclic() -> NodeGraphIr {
        let mut g = NodeGraphIr::new(NodeGraphKind::ServerEntity, "has_a_loop");
        g.push_export_control_in(ExportDecl::new("in".into(), None));
        g.push_export_control_out(ExportDecl::new("out".into(), None));
        let n0 = g.insert(node_ir_set_local(&int()));
        let n1 = g.insert(node_ir_set_local(&int()));
        let n2 = g.insert(node_ir_set_local(&int()));
        g.link_control(Link::export(0), Link::node(n0, 0));
        g.link_control(Link::node(n0, 0), Link::node(n1, 0));
        g.link_control(Link::node(n1, 0), Link::node(n2, 0));
        g.link_control(Link::node(n2, 0), Link::node(n0, 0)); // 回边
        g
    }

    /// 夹具自检:生成 → 写出 → 读回 → 展平 → 分层,每一环都必须成立。
    ///
    /// "能生成文件"不等于"能显示" —— 编码配置错一点、pin 序号对不上、布局拿不到
    /// 入口,结果都是窗口里一片空白,而不是报错。所以这条测试把整条链走一遍。
    #[cfg(test)]
    mod tests {
        #[test]
        fn fixture_survives_the_whole_pipeline() {
            let path = std::env::temp_dir().join("r2g-fixture-selftest.ogia");
            super::write_ogia(&path).expect("写出夹具");
            let t = super::super::load_ogia(&path).expect("读回夹具");
            let doc = super::super::flatten(t);
            let _ = std::fs::remove_file(&path);

            assert_eq!(doc.graphs.len(), 3, "main + 两个函数");
            assert_eq!(doc.graphs[0].name, "main", "main 排第一");

            let g = &doc.graphs[0];
            assert!(!g.entry.is_empty(), "入口块没被展平出来 —— 布局会全排成一列");
            assert!(!g.exit.is_empty(), "返回块没被展平出来");
            assert!(g.nodes.len() >= 10, "夹具节点数 {}", g.nodes.len());
            assert!(g.edges.iter().any(|e| e.ctrl), "没有控制流边");
            assert!(g.edges.iter().any(|e| !e.ctrl), "没有值流边");

            // 布局(`push_target` 里由 core 的 `node::layout` 完成)必须终止,
            // 且每个节点都拿到有限坐标。`has_a_loop` 那张图专门用来确认环
            // 防护生效 —— 它曾是无界松弛循环,会 100% CPU 挂死。层级钳位的
            // 数值界在 core 的 `node::layout` 测试里单独锁。
            for g in &doc.graphs {
                assert!(
                    g.nodes
                        .iter()
                        .all(|n| n.position.x.is_finite() && n.position.y.is_finite()),
                    "{}: 布局必须给每个节点有限坐标",
                    g.name
                );
            }
        }
    }

    /// 一个「caller 调用 callee」的 `Target`:caller 图里有一个引用 callee 的
    /// `IrNodeId::Fn` 节点(link 的靶子)。optimize 和 view 的 link 测试共用,
    /// 免得两处各造一份形状不同、哪天悄悄漂掉。
    ///
    /// 两个不变量(不满足的话 callee 展开时跑的 `optimize()` 会炸):
    /// - 图必须 push 过 export(不然 `optimize()` 里 `controls_in[0]` 越界);
    /// - SetLocal 的 `values_in[0]`(LocalRef)必须连着 Local 节点
    ///   (不然 `eliminate_set_local` 的 `unwrap` 炸)。
    pub fn linkable_target() -> Target {
        // callee:entry → set → exit 的直链,0 参 1 返回。
        let mut callee = NodeGraphIr::new(NodeGraphKind::ServerEntity, "callee");
        callee.push_export_control_in(ExportDecl::new("in".into(), None));
        callee.push_export_control_out(ExportDecl::new("out".into(), None));
        // ⚠️ 必须声明这个**返回值槽**,否则 link 一定 panic。
        //
        // `Optimizer::lower`(`core/src/compile/ir.rs` 尾部)会**重算**
        // `decl.proxies_out = externals.values_in.iter()...` —— 函数存进
        // `Target` 时手写的 `proxies_out` 一律作废。而 `link_node` 拿
        // `proxies_out` 当迭代器去填 `decl.ret.assemble_all(...)`:`ret` 是
        // `Singleton(())`(要 1 个值),`proxies_out` 空的话迭代器立刻
        // `next().unwrap()` 炸在 `place.rs` 里。
        //
        // 方向别看反:`push_export_value_out` 往 `externals.**values_in**`
        // 推(值流向图外 = 返回值),`values_out` 那侧才是函数参数。
        callee.push_export_value_out(ExportDecl::new("ret".into(), None));
        let cl = callee.insert(node_ir_local(&int()));
        let c0 = callee.insert(node_ir_set_local(&int()));
        callee.link_value(Link::node(cl, 0), Link::node(c0, 0));
        // `node_ir_local` 的 `values_out = [LocalRef(T), T]`,T 的下标是 **1**。
        callee.link_value(Link::node(cl, 1), Link::export(0));
        callee.link_control(Link::export(0), Link::node(c0, 0));
        callee.link_control(Link::node(c0, 0), Link::export(0));

        // caller:entry → fn 节点 → exit。
        let mut caller = NodeGraphIr::new(NodeGraphKind::ServerEntity, "caller");
        caller.push_export_control_in(ExportDecl::new("in".into(), None));
        caller.push_export_control_out(ExportDecl::new("out".into(), None));
        // SetLocal 的 `values_in[0]` 必须连着 Local —— 真实 IR 的不变量
        // (`eliminate_set_local` 会 unwrap 它)。单 link 不跑规则、裸着也没事,
        // 但「一键优化」会把规则链跑到这张图上,少了这一步直接 panic。
        let le = caller.insert(node_ir_local(&int()));
        let entry = caller.insert(node_ir_set_local(&int()));
        caller.link_value(Link::node(le, 0), Link::node(entry, 0));
        let lx = caller.insert(node_ir_local(&int()));
        let exit = caller.insert(node_ir_set_local(&int()));
        caller.link_value(Link::node(lx, 0), Link::node(exit, 0));
        let fnn = caller.insert(NodeKind::new(
            IrNodeId::Fn("callee".to_string()),
            1,
            1,
            vec![],
            vec![int()],
        ));
        caller.link_control(Link::export(0), Link::node(entry, 0));
        caller.link_control(Link::node(entry, 0), Link::node(fnn, 0));
        caller.link_control(Link::node(fnn, 0), Link::node(exit, 0));
        caller.link_control(Link::node(exit, 0), Link::export(0));

        let mut t = Target::default();
        t.functions.insert(
            "callee".into(),
            FnInfo {
                description: String::new(),
                graph: callee,
                // 0 参 1 返回:`ret` 的默认就是 `Singleton(())`,`proxies_out`
                // 给一个 `None` 让返回值接到低层节点的 0 号值出口。
                decl: FnDecl {
                    control: true,
                    proxies_out: vec![None],
                    ..Default::default()
                },
                exported: false,
            },
        );
        t.functions.insert(
            "caller".into(),
            FnInfo {
                description: String::new(),
                graph: caller,
                decl: Default::default(),
                exported: false,
            },
        );
        t
    }

    /// 把夹具写成 `.ogia` 文件。编码配置与解析端(`load_ogia`)、core linker 完全一致。
    pub fn write_ogia(path: &Path) -> std::io::Result<()> {
        let t = target();
        // 编码配置必须与 `load_ogia`、core linker 完全一致,否则解不开。
        let bytes = bincode::serde::encode_to_vec(&t, bincode::config::standard())
            .map_err(std::io::Error::other)?;
        std::fs::write(path, bytes)
    }
}
