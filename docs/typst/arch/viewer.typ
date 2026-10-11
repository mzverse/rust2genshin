#import "/lib/lib.typ": *;
#let title = [viewer];
#show: template;

#set raw(lang: "rust");

#link("/dev/viewer.html")[怎么用] 在开发栏；这里讲它的设计。

= 看的是中间态，不是最终产物

这是理解 viewer 的前提：

- *`.gia`* —— 最终产物。`compile/link.rs:93` 起把 `Target` 里每张图 `lower` 一遍，再编码成 `AssetBundle` 落盘。坐标、结构都已定型。
- *`.ogia`* —— *未 lower* 的 IR，bincode（serde）序列化的 `Target`（`viewer/src/doc.rs:3`），打包进 rlib 的一个 archive entry。

所以 rlib 里躺的正是*优化器的起点*。这不是猜的：`eliminate_destructure` 规则能命中 `IrNodeId::Destructure`，说明结构体展开还没发生。

`core/src/linker.rs:59` 是这条链的另一半 —— linker 吃进 `.ogia` entry，反序列化成 `Target`，然后才 lower、编码、写 `.gia`。viewer 和 linker 读的是同一份东西。

= 几条设计取舍

这些是查代码看不出来的。

*布局在 core，不在 viewer。* `core/src/node/layout.rs` 是布局的唯一实现，`.gia` 和 viewer 共用。`Node` 因此带一个 `position: (f32, f32)` 字段（serde 带上，所以这是 `.ogia` 线格式的一部分 —— 旧 rlib 解不开，重建即可）。好处是几何常量和尺寸函数只有一份，不会两边漂开；viewer 的标签经 `layout_with` 钩子传进去，宽度估算和实际显示的字不会对不上。

*无控制流的节点「就近锚定」。* 入口走不到的值流节点（局部变量槽、Assemble、Destructure、Native）如果一律排到最右一列，会叠成一根竖柱、每条值边倒着指。做法是 ALAP：`level = min(消费者层级) - 1`，汇点取 `max(生产者层级) + 1`，孤立节点留最右列。共享的局部变量槽锚在最早的消费者上；链比消费者还深时开负层级列。

*列内按交叉消减排。* 列内如果按下标排，共享槽会长边斜穿全场。两步求解：barycenter 扫描当起点，再用相邻交换爬山，目标函数是 `交叉数 × 100 + 纵向总行程`（交叉的权重必须远大于行程，否则会把长边交叉留着）。`by_layer` 用 `BTreeMap` 而不是 `HashMap` —— 否则爬山按列序迭代会不确定，同一个测试时红时绿。

*坐标按图存，布局只算一次。* `Viewer::positions[图下标]`，加载时算一次；切图不重算，拖过的位置切走再切回来还在。「自动整理」是唯一会丢拖动的操作，而且可以 Ctrl+Z 撤回。

*优化按钮有护栏。* 右键 link 和「一键优化」都是：存档 → `catch_unwind` → 出错回滚 → 重展平。一次点击一个存档，一次 Ctrl+Z 全撤；没有变化时不压栈。

*规则按钮只跑一遍。* 每条消除规则一个按钮，点一次 = 该规则在*整张图上过一遍*，不做「循环到不动点」—— 这样能看出是哪一步把图改坏的。要一步到位就点「一键优化」。

= 为什么不在 default-members

`viewer` 依赖 gpui，而 gpui 在 Linux 上要 X11 / Wayland / 字体等系统库，CI（`build.yml`）只装了 protobuf-compiler。CI 当前只跑 `cargo run -p build-demo`，本来就绕开了它；`default-members` 是防将来 CI 改成 `cargo build` 或 `cargo test --workspace`。

= 几个真实的坑

- *占位界面的 early-return 收不到拖放* —— 不带参数启动、停在「拖入 .ogia 或 .rlib 文件」那屏时，`on_drop` 只挂在正常分支的根 div 上，而占位是 `render` 的 early-return。最该接住拖放的那一刻反而没有监听器。已修（`Viewer::handle_drop` 两条分支都绑）。加任何全局交互（拖放、快捷键）时都要检查 early-return 分支。
- *右键菜单的 click 收不到* —— gpui 的 `on_click` 要求按下和抬起落在同一个元素上，而「点空白关菜单」如果在按下的冒泡里就把菜单标脏，抬起前会重绘一帧，菜单项连同它的 mouse-up 监听器都没了。修法是菜单盒子 `on_mouse_down(Left, stop_propagation)` 截住冒泡。
- *切图时要重置的东西不止坐标* —— `Viewer::select` 里除了换 `current`，还要把平移 `origin`、缩放 `scale`、拖拽 `drag`、状态文案 `status`、右键菜单 `menu` 全部清掉。后两者尤其：状态行说的是上一张图的操作，菜单记的节点下标属于上一张图，留着都会误导。
