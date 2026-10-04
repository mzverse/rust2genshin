# ogia-viewer 右栏「优化」面板 —— 设计

`Optimizer::optimize`(`core/src/compile/ir.rs`)把整张图一路优化到不动点。
出问题时只有最终产物可看:中间是哪条规则改坏的、改坏成什么样,都不好查。
在 viewer 里加一个右栏,把 `eliminate_solo`(`ir.rs:402`)调用链上的每条规则
做成按钮,点一次 = 该规则在**当前图**上**过一遍**,可以一步一步走。

## 语义

- 9 条规则,顺序与 `eliminate_solo` 一致:
  `eliminate_if` / `eliminate_set_local` / `eliminate_local` / `eliminate_calc` /
  `eliminate_unnecessary_local_setter` / `eliminate_assemble` /
  `eliminate_destructure` / `eliminate_bool_eq_int` / `eliminate_not_not`。
- 「过一遍」= 对图里**每个节点**各调用一次该规则,**不**循环到不动点
  (`eliminate_solos` 那种循环刻意不做:一次点击固定是一遍,便于逐步观察)。
- 右栏底部状态栏给出「命中 N 次,节点 a → b」;panic 时给出消息。
- 「一键优化」(`optimize_all`):一次点击跑完 **IR 级整条流程** —— 先对
  **每个** `IrNodeId::Fn` 节点执行 `link_node`(右键「link」的批量版),
  再 `Optimizer::optimize()`(9 条规则**到不动点** + 控制边界收尾 +
  值导出索引压缩)。执行方式与右键「link」一致:护栏 + 存档 + 回滚;
  **整次点击只留一个存档**,一次 Ctrl+Z 全撤。没有变化时不压栈、
  状态栏说明("无变化")。
- 右栏顶部另有「自动整理」(重算坐标,见下)与「撤销 (Ctrl+Z)」。

## 坐标

优化后已有的节点坐标不变。`Slab` 的 key(`NodeRef`)在增删中稳定(空洞保留),
所以坐标按 `NodeRef` 对照:重新展平后,每个 `NodeRef` 回到点击前的
`(refs, pos)` 里查,查到就用原坐标。只有 `eliminate_bool_eq_int` 会新插节点
(一个 NOT),它没有历史坐标,停到最右侧空列,不叠在别的节点上。

### 布局只在两处发生:加载、点「自动整理」

- 每张图的坐标存在 `Viewer::positions[图下标]`(与 `doc.graphs` 平行)。
  **加载时**(`add_target`)对每张新图 `layout::compute` 一次;此后切图
  **不重算** —— 拖过的位置切走再切回来还在(之前每次切图都重排,
  拖了等于白拖)。
- 右栏顶部「自动整理」:重算当前图坐标,是唯一会丢拖动结果的操作。
  它会往撤销栈里压一条 `SnapshotKind::Arrange` 存档 —— Ctrl+Z 能把
  整理前的坐标拿回来(确定性布局,再点一次整理也回不到你拖的样子)。

## 撤销(Ctrl+Z)

- 每次点击前把当前图 bincode 存档(函数编整个 `FnInfo`,主图编 `NodeGraphIr`;
  与 `.ogia` 同一套 `config::standard()`)。`Slab` 的 serde 保留 key,
  解回来 `NodeRef` 完全一致 —— 这是坐标能对上的前提。
- 撤销 = 解码换回 + 重新展平 + 坐标按存档的 `SnapshotKind` 定优先级:
  - 优化(`Optimize`):**当前状态优先**(保住优化之后用户拖过的位置),
    存档兜底(补回被优化删掉的节点);
  - 整理(`Arrange`):**存档优先**(把整理前的坐标还回来)。
- 撤销栈按图分开(`HashMap<图下标, Vec<Snapshot>>`);图下标只增不改,不会串台。
- 键位 `ctrl-z`:App 级 action(`bind_keys` + `AppContext::on_action`),
  不依赖焦点;右栏另有「撤销」按钮,不记键位也能点。

## panic

规则里有大量 `unwrap`(`eliminate_if` 撞上没有条件的 IF、`eliminate_set_local`
撞上没有控制入的 setter……)。对任意拖进来的 `.ogia` 点按钮不能把窗口带走:
一遍扫描跑在 `catch_unwind` 里,panic 时用点击前的存档整体换回,
状态栏给出 panic 消息,**不进撤销栈**(这一跳没有成功)。

## 改动面

| 文件 | 改动 |
| --- | --- |
| `core/src/compile/ir.rs` | `eliminate_bool_eq_int` / `eliminate_not_not` 私有 → `pub`(其余 7 条本来就 pub);`optimize` 私有 → `pub`(一键优化用;`controls_in` 判空改 `first()`,同一张图可反复点) |
| `viewer/src/doc.rs` | `NodeEntry` 加 `node_ref`;`Doc` 持有 `Target`;`take`/`put`/`encode`/`decode_into`/`reflatten` |
| `viewer/src/optimize.rs`(新) | 规则表、一遍扫描、panic 拦截、坐标重建、撤销 |
| `viewer/src/view.rs` | 右栏 UI、状态栏、按图存坐标、`apply_rule` / `arrange` / `undo_step` |
| `viewer/src/main.rs` | `ctrl-z` 键位 + action 注册;`add_doc` → `add_target` |

## 不做

- 不做「选中某节点 → 只对它执行」:那需要先有节点选中态;按钮语义按上文的
  「过一遍」。
- 不做 redo;不做 `lower()` 里 `optimize()` 之后的**类型 lower 段** ——
  那是编码前的事,与翻图无关。「一键优化」按用户要求恰好跑到 `optimize()`
  为止(含 externals 收尾),类型 lower 不碰。
- 不自动 `verify()`:图被改坏时,本工具的任务是**看见**坏图,不是替 core 报错
  (且 `Helper` 出不了 rustc 环境)。
