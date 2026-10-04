# .ogia 节点浏览器 — Design Spec

**Date:** 2026-10-03
**Status:** Draft (awaiting review)
**Scope:** 新建 `viewer` crate：拖入 `.ogia` / `.rlib` 文件，浏览其中的节点图 IR。

## Context

`core/src/compile/ir.rs` 的 `optimize()` 会删除、splice 节点。排查"控制流被断开"这类问题时，目前只能读 `eprintln!` 打出来的文本 dump 或用 `tools/gia2txt.py` 看最终 `.gia`——中间态（IR 层面）没有可视化手段。

`.ogia` 是已经存在的格式：`core/src/linker.rs:50` 用 `bincode::serde`（`config::standard()`）把 `compile::link::Target` 序列化后嵌进 `.rlib`，linker 读回它生成 `.gia`。`Target` 结构：

```rust
pub struct Target {
    pub main: Option<NodeGraphIr>,           // NodeGraphIr = NodeGraph<IrNodeId, IrKind>
    pub functions: HashMap<String, FnInfo>,  // FnInfo { graph: NodeGraphIr, .. }
    pub adts: HashMap<String, AdtInfo>,
}
```

本 spec 新增一个 GUI 工具，把这个 IR 画出来，供人眼判断结构是否正常（孤立节点、悬空分支、断开路径）。

**本 spec 起初不改 `core` 的任何逻辑，只新增 `viewer` crate。**（后续增量按用户要求把自动整理**迁进了 core**：`core/src/node/layout.rs` + `Node::position`，`.gia` 编码前也会跑 —— 见「布局」；`core` 里已有的 `Optimizer::disconnected_entries` 仍未接入 viewer，见 Out of scope。）

## Scope

**In scope:**

- 拖入 `.ogia` 文件：bincode 解码成 `Target`
- 拖入 `.rlib` 文件：遍历 ar archive，对每个 `.ogia` entry 各自解码后合并
- 左侧图列表（`main` + 所有 `functions`），右侧单画布
- 渲染每个节点（`IrNodeId` 分类上色）与节点之间的所有边
- 控制流边与值流边视觉区分
- 节点可鼠标拖动；画布可平移、缩放
- 打开时按控制流层级自动排布初始坐标

**Out of scope:**

- **可达性分析 / 断链高亮** —— 用户明确选择"纯手工看"。不做任何 CF 分析，viewer 不引入 `disconnected_entries`。
- 优化前后对比、IR ↔ `.gia` 对照
- 节点选中、删除、连线编辑、保存回文件
- 节点显示名（`.ogia` 不含节点元数据，`tools/node_data/nodes.json` 不参与）
- 搜索、缩放级别 UI、撤销

## 技术选型

**框架**：GPUI（crates.io `gpui` v0.2.2，默认 feature 含 `windows-manifest`）。

**依赖 `core`**：viewer 直接依赖 `rust2genshin`（rlib）。这意味着 viewer 需要 `#![feature(rustc_private)]`、跑在 `nightly-2026-10-01` 工具链上、运行时依赖 `rustc_driver` dll —— 与 `linker.rs` 相同。

依赖 `core` 的理由：`ValueIn.default` 是 `Option<Box<dyn NativeValue>>`（`core/src/node/mod.rs:257`），该 trait 带 `#[typetag::serde]`（`core/src/value.rs:53`），反序列化要求所有具体 impl 已注册。在 viewer 里镜像一份 serde 类型必须重新实现整个 `NativeValue` trait 及其全部 primitive impl，代价远大于背 `rustc_private` 的负担。

**不使用 `Target: AddAssign`**：它在两边都有 `main` 时 `panic!()`（`core/src/compile/link.rs:24`）。viewer 用防御式合并：`main` 取第一个非空的，`functions` 全部收进同一个 `HashMap`（同名后者覆盖），`adts` 同理。

## 组件

```
viewer/
  Cargo.toml            [lib] + [[bin]]
  src/lib.rs            重导出下面各模块（供单测）
  src/main.rs           bin：gpui Application::new().run()
  src/doc.rs            Doc：解析 .ogia/.rlib → 展平成 GraphEntry 列表
  src/view.rs           GPUI 渲染：canvas 画边 + 绝对定位 div 画节点
```

自动整理（分层 + 就近锚定 + 垂直排序）**不在 viewer 里**：实现在
`core/src/node/layout.rs`，坐标写进 IR 节点的 `Node::position`。viewer 在
加载（`Doc::push_target`）与点「自动整理」（`Doc::relayout`）时调用它并
重新展平；`.gia` 编码前 core 的 linker 也会跑同一套（见下文「布局」）。
`doc.rs` / `view.rs` 不触碰任何 GPUI 窗口 API 的部分（展平、几何计算）
可直接单测。

### 数据模型

```rust
pub struct Doc {
    pub graphs: Vec<GraphEntry>,   // main 第一，其余按函数名排序
}

pub struct GraphEntry {
    pub name: String,
    pub index_of: HashMap<NodeRef, usize>,  // NodeRef → nodes 下标
    pub nodes: Vec<NodeEntry>,     // 展平成 Vec，渲染时不必反复查 slab
    pub edges: Vec<Edge>,
    pub entry: Vec<usize>,         // 喂给 externals.controls_out[0] 的节点下标
    pub exit: Vec<usize>,          // externals.controls_in[0] 列出的节点下标
}

pub struct NodeEntry {
    pub kind: IrNodeId,
    pub position: Point<f32>,      // 读自 IR 节点的 Node::position（core 布局写进去的）
    pub type_label: String,        // 类型标注，如 "Int"；无已知类型时为空串
    pub controls_in_num: usize,    // 把入边在侧边纵向均分时需要
    pub controls_out_num: usize,
    pub size: Size<f32>,
}

pub struct Edge {
    pub from: (usize, usize),      // (nodes 下标, pin 序号)
    pub to: (usize, usize),
    pub ctrl: bool,                // true = 控制流边
}
```

**`index_of` 不能省**：`NodeGraph.nodes` 是 `Slab`，而 `NodeRef` 是 slab 的 key。IR 在编译期被 `Optimizer::remove()` 删过节点，key 会有空洞，**`NodeRef` 与展平后的 Vec 下标不是恒等映射**。展平时按 slab 迭代顺序 push，同时记下 `NodeRef → 下标`。

**`entry` / `exit` 不能省**：`externals` 的类型是 `Links`，除节点间的边外还有一组指向图边界的边。展平只遍历 `g.nodes`，若不单独记下 `controls_out[0]`（入口）与 `controls_in[0]`（返回）指向哪些节点，**每个函数的入口块和返回块都会被画成孤立的** —— 而"看起来断开"恰恰是这个工具要帮用户排除的假警报。`entry` 同时是布局分层的 BFS 起点（见下）。

**坐标进 `NodeEntry`，但拖动不回写 IR**：布局结果存在 IR 里（`Node::position`），展平时抄进 `NodeEntry`，viewer 的拖动只改 app state 的 `Vec<Point<f32>>`（与 `nodes` 同序，初值取自 `NodeEntry::position`）。所以拖过的位置切走再切回来还在，而 IR 里的坐标始终是**最后一次自动整理的结果**。

`Point` / `Size` 取自 gpui 的几何类型，是纯数据结构，单测里无需初始化窗口。

**图边界：`entry` / `exit` 之上还要 pin 级标记。** `NodeEntry` 另外记四列
`ctrl_in_exports` / `ctrl_out_exports` / `value_in_exports` / `value_out_exports`：
每项是 `ExportPin { pin, export }` —— 节点侧第几个 pin、连到图外第几号边界。
core 里 `LinkTarget::Export` 那侧的 `Links` 就是 `externals`（`node/mod.rs:405`），
但节点自己的 `links` 里同样留了反向记录（边界号在 `Link::index`），
所以展平时逐 pin 查得到，不用拿 `externals.index` 反查节点。渲染时从这些 pin
往图外引一小段桩线、末端标「Export N」（见「渲染」），取代了早期只在节点里
写 `[in]` / `[out]` 的做法 —— 桩线能精确到**哪一个引脚、哪一号边界**，
值流的边界（参数 / 返回值）也一视同仁。

### 加载

按扩展名分派，复用 `core/src/linker.rs:55-67` 的 ar 遍历逻辑：

- `.ogia` → `bincode::serde::decode_from_reader(BufReader::new(File::open(path)?), bincode::config::standard())`
- `.rlib` → `ar::Archive` 遍历 entry，对名字以 `.ogia` 结尾的逐个解码，再防御式合并
- 其他 → 报"不支持的扩展名"

拖入第二个文件不清空已有内容，追加进来。`doc.rs` 需要额外依赖 `ar`（与 `core` 同版本）。

### 布局（实现：`core/src/node/layout.rs`）

坐标存在 `Node::position` 里（`Node::new` 默认 `(0,0)`，`.ogia` 反序列化出来也是它）。布局对**整张 `NodeGraph`** 跑：按 slab 迭代序展平成内部模型（下标就是顺序）→ 分层 → 垂直排序 → 写回每个节点的 `position`。两条消费路径共用它：

- **core 的 linker** 在 `.gia` 编码前跑（`compile/link.rs` 在 `apply` 之前），`x_pos` / `y_pos` 由此而来 —— 导入节点编辑器时节点自带排布，不再全部堆在原点；
- **viewer** 在加载（`Doc::push_target`）与点「自动整理」（`Doc::relayout`）时跑，展平时把 `position` 抄进 `NodeEntry`。viewer 自己不发明坐标。

宽度按**显示出来的标签**估算（`layout_with` 的标签钩子）：`.gia` 用 `NodeId` 的紧凑形式（`NodeLabel`），viewer 传渲染用的同一个 `label`（带 `<类型>` 后缀）。尺寸（`node_width` / `node_height` / `NODE_W` / `PIN_GAP` …）由 core 提供、viewer 复用 —— 两份尺寸漂开的那一刻，坐标就会和框/字互相压盖。

从 `entry`（即 `externals.controls_out` 指向的那些节点）出发 BFS 求可达集，再只认 `ctrl: true` 的边松弛：

- `level[n] = max(level[succ]) + 1`（取**最长**路径，保证每条控制流边都从左往右指，不会有回边）
- **值边不影响控制流可达节点的分层** —— 值流是层间的数据流，混进主干松弛会把表达式节点错误地推到下一层（值边只用来锚定下一条的节点）
- 同层节点按下标序纵向排开
- 入口走不到的值流节点与掉队的死块**就近锚定**（ALAP，「能多晚放多晚」）：放在**最早消费者**左边一列（`level = min(消费者层级) − 1`），值的链（Local → 表达式 → 消费者）自消费者左侧逐段排开；没有消费者的汇点贴生产者右侧；完全孤立的仍停在最后一列。链比消费者深度更深时在入口列**左边**新开**负层级**列。效果是**每条值边都从左往右指** —— demo 实测 test_mut 的 11 个值节点，从「叠在最右一列、边全部倒着往回指」散开到各自消费者的左侧。共享槽（一个 Local 被多处使用）锚在**最早**消费者上，到更远消费者的边拉长但方向仍朝右
- 列内**纵向顺序**用局部搜索排（重心扫描当起点 + 相邻交换爬山），目标 = `交叉数 × 100 + 纵向总行程` —— 按下标排会把值节点甩得离消费者很远、共享槽长边斜穿全场（实测 test_mut 18 处交叉）；只压行程消不掉长边交叉（移动共享槽行程 +128 但交叉 −3），交叉要有足够权重。交叉用「节点中心连线」近似数。分组容器必须是 `BTreeMap`（不是 `HashMap`）：迭代序参与搜索，`HashMap` 的随机序会让布局**跑一次一个样**。实测：26 → 7 处交叉（test_mut 18→5）
- 层间距、层内节点间距写死为常量；**节点尺寸按内容自适应**：
  - 高度按 pin 数（`core::node::layout::node_height`：一侧最多 N 个 pin 时
    高度 ≥ (N+1) × `PIN_GAP`，`PIN_GAP = 16px`）—— `spread` 的段距 = 高/(N+1)，
    间距要够大只能让节点长高。布局的层高与层内推进必须用节点的**真实高度**。
  - 宽度按标签（`core::node::layout::node_width`，下限 `NODE_W`）—— 长标签
    （`Fn` 节点的整个函数名、长类型名）不再被裁掉。是**估算**不是测量：
    精确测量要 `TextSystem`，而宽度参与布局、必须是纯数据；因此按 6.6px/ASCII
    字符**往宽了估**（估宽只多留边距，估窄文字会被裁）。viewer 布局时喂给
    `layout_with` 的是**渲染同一个** `doc::label`，两边不会漂。
  - 层的**横向**步进按层内最宽节点逐层累加 —— 宽节点会把后面所有层推右；
    全是 `NODE_W` 宽时与固定步进等价。

无环时松弛必然收敛；有环（rustc 对 `loop` / `while` 的 MIR 有回边，任意输入的 `.ogia` 都可能带环）由**轮数封顶 + 层级钳位 `[-(n+1), n]`** 兜住：不挂死、画布宽度有界，结果是尽力而为的分层 —— 环在画布上一眼可见，不需要程序替他标注。`entry` 为空（空图 / 无 main）时没有控制流主干，全部节点按值流相对彼此锚定 —— 这是正确结果，不是 bug。

"入口走不到"只是布局时顺带得到的集合（必须先算可达性才能分层），不产生任何标注或高亮。

### 渲染

三层结构：GPUI `canvas()` 画所有边，下面盖一层绝对定位的 `div()` 画节点。节点用 `div` 是为了白拿 GPUI 的 hover / 事件系统。

**边**：
- 控制流边：实线、较粗
- 值流边：虚线、较细
- 两类用不同颜色，不靠线型单独区分
- 三次贝塞尔，不用直线 —— 控制流有分叉时曲线更好读

**连接点（pin 位置）**：控制流和值流**排法一致** —— 都从来源右边缘的 pin 位出、
进目标左边缘的 pin 位，**不再从节点中心的上下缘进出**。每条侧边上
「**控制流在前、值流在后**」：`k` 号 pin 的纵向位置是把两类 pin 的总数当分母
均分，控制流占前面的档位、值流的连接点依次排在其下方。

> 侧边的分母是「控制 pin 数 + 值 pin 数」（`NodeEntry::controls_*_num` /
> `values_*_num`），只用控制 pin 数当分母会让值流的端点又叠回控制流档位上。
>
> 值流的源节点（GetLocal / 表达式）在分层里全部落在**最右一列**（它们没有控制
> 边、从入口走不到），所以值流边多数是从右往左指的 —— 端点各自锚在 pin 位上，
> 正是为了在回指/同层的曲线堆里还能读出哪根线接哪个脚。

**引脚标记**：每个连接点都画一个标记（`view::build_pins` 出档位，渲染按
`ctrl` 分流）——**无论有没有连线**都画，节点有几个脚、哪个脚空着一眼可见。
形状按类别区分方向：**控制流是向右的三角形**（形状即方向，输入侧的引脚也
朝右 —— 流向从左往右），值流是圆点（数据流不分方向）。中心在 `pin_y` 的
档位上（一半在内、一半在外），颜色按类别：控制流蓝、值流黄。三角形用
canvas 填色画（gpui 的 div 边框只有一个颜色，拼不出 CSS 那套 border 三角），
顶点几何是纯函数 `view::pin_triangle`、可单测。

**值入默认值**：`NodeEntry::value_in_defaults` 与值入 pin 逐位对应；常量输入在
IR 里就是 `ValueIn::default`（IF 的条件默认、字面量……），不显示就丢了信息。
默认值文本紧贴值入引脚圆点**左侧**、纵向对齐该 pin，用 `Debug` 格式化
（`core/src/value.rs` 的 `NativeValue: Debug`，具体值是 `bool` / `i32` / `f32`
等基本类型）。框宽固定、**右缘**贴引脚，文字右对齐 —— 文本长度不确定，
只有固定右缘才能和引脚对齐；**不裁剪**，长默认值宁可伸出去也不能截剩尾巴。

**图边界桩 + 标签**：连到图外的 pin（`NodeEntry::*_exports`）从 pin 位往图外引
一小段直线（`view::build_boundaries`）—— 入边一侧向左、出边一侧向右，纵向用
**同一套** `pin_y`，所以桩和线的端点严丝合缝，相当于把"连到图外"的那半条线
画出来。桩末端挂一个 `Export N` 小标签框（`view::tag_box`；框贴末端外侧、
纵向以桩线为中心，canvas 画不了文字，用和节点同一层的绝对定位 `div`），
框里写**具体的边界号**（`ExportPin::export`）。样式与对应边一致（控制流实线蓝、
值流虚线黄）。入口/返回块仍描粗边框，但节点里**不再**拼 `[in]` / `[out]` 后缀：
桩 + 标签更精确，还能覆盖值流的参数 / 返回值边界。

**节点右键菜单**：右键节点弹出，按 `IrNodeId` 决定菜单项 —— 目前只有
`IrNodeId::Fn` 节点有「link」一项：对它执行一次 `Optimizer::link_node`
（`core/src/compile/ir.rs` 里 lowering 走的那条展开路径），把复合节点展开成
内联的低层节点。展开出来的新节点落在**被 link 节点的原位**（`rebuild_pos`
的 `anchor`），和优化同一套存档 → Ctrl+Z 可撤；panic 同样由护栏拦下回滚
（唯 linker 的持久状态 —— 展开缓存 / 资产 —— 没法回滚，状态栏会说明）。
空白处左右键、节点左键、切图都会关掉菜单。

**`Doc` 持有的是 `Linker` 而不是裸 `Target`**：link 需要它（展开过的函数、
结构体资产都攒在里面，重复 link 命中缓存，不会重复展开）。链接器在**加载时**
建好、随 `Doc` 一直留着；viewer 从不调 `Linker::link()` / `save()`，
所以 `output` 用空路径占位。

**节点**：圆角矩形，标题是 `IrNodeId` 的紧凑渲染。按类别上色：

| 类别 | 显示 | 颜色 |
|---|---|---|
| `Local` | `Local<Int>` | 灰 |
| `SetLocal` | `SetLocal<Int>` | 蓝 |
| `Unreachable` | `Unreachable` | 暗红 |
| `Native(..)` | `SysCallStub#2` | 按 `id` 哈希取色，天然区分不同原生节点 |
| `Assemble` / `Destructure` / `Modify` | 名字本身 | 绿 |
| `Fn(String)` | `Fn(name)` | 橙 |

> `IrNodeId` **没有 `If` 变体**（`core/src/compile/ir.rs:29-38` 只有 `Native` / `Unreachable` /
> `Local` / `SetLocal` / `Fn` / `Assemble` / `Destructure` / `Modify`）。分支节点在 IR 层
> 就是 `Native(NODE_IF)`，会走到 `Native(..)` 那一行按 id 哈希取色。
> 匹配 `IrNodeId` 时必须穷尽这八个变体 —— 它没有 `#[non_exhaustive]`，漏一个就是编译错。

`.ogia` 不含节点元数据，所以只显示 `IrNodeId` 原始形态。类型标注取自 `NodeKind`，两类节点位置不同（见 `core/src/compile/ir.rs:133-140`）：

- `Local`：`values_out_types = [LocalRef(T), T]` —— 实际类型是 `[1]`
- `SetLocal`：`values_in_types = [LocalRef(T), T]`，且 `values_out_types` 为空 —— 实际类型是 `values_in_types[1]`

类型标签用 `{:?}`（Debug）而不是 `Display`：`impl Display for NativeKind` 在 `Struct` 分支是 `panic!`（`core/src/value.rs:47`），而 viewer 的输入是用户拖进来的任意 `.ogia`，不能假设它由本仓库的 core 产出。Debug 是全的，Display 是半的。`Int` / `Bool` 等简单类型两者输出一致。

### 交互

| 操作 | 效果 |
|---|---|
| 拖节点 | 移动该节点，连线实时跟随 |
| 拖空白处 | 平移画布 |
| 滚轮 | 以光标为中心缩放 |

无选中态。

### 错误处理

只做一件事：解析失败时在窗口中央显示错误文本，解析成功则清掉。不分情况、不分类，不 panic，不中断程序。拖多个文件时逐个处理，某个失败不影响其他。

**占位界面同样接收拖放**：启动时没有文档的那块「拖入 .ogia 或 .rlib 文件」提示屏，恰恰是最该接住拖放的一刻 —— 它和正常视图是两条渲染分支，`on_drop` 两处都要绑（漏了占位分支的话，启动后拖文件会毫无反应，用户实测踩过）。

## 数据流

```
拖入文件
  → gpui on_drop 回调拿到 PathBuf
  → doc::load(path) -> Result<Doc, String>
      ├─ .ogia  → bincode decode → Target
      ├─ .rlib  → ar 遍历 → 多个 Target → 防御式合并
      └─ 其他   → Err("不支持的扩展名: ...")
  → 追加到 app state 的 Doc
  → 当前图索引变化时，用 layout::compute 算出初始坐标
  → view 渲染
```

## 测试

布局单测在 `core/src/node/layout.rs`（迁移一次带走 —— viewer 侧不再有布局测试）—— 它锁住可读性的地基：

- 手工构造一个小 `NodeGraph`（入口 → if → 两支汇合到出口）
- 断言分层坐标符合预期：两支同层、汇合点在更右一层
- 断言没有边往回指（每条边满足 `level[from] < level[to]`）

rlib 解析和防御式合并不写测试。

## 已知取舍

- **不用 `core` 的 `disconnected_entries`**：用户选择纯手工看。CF 校验目前只挂在 `core` 内部（`optimize()` 与 `lower()` 末尾），跟 viewer 无关。
- **不带节点显示名**：`tools/node_data/nodes.json` 是 gitignore 的 Python 工具数据，viewer 不依赖它。`Native(SysCallStub#2)` 这类原始形态对判断结构是否正常够用。
- **坐标不持久化**：拖动后的布局不保存，关掉窗口即丢。
