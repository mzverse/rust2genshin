#import "/lib/lib.typ": *;
#let title = [参考项目];
#show: template;

#set raw(lang: "rust");

`rust2genshin` 不是凭空写出来的 —— 节点 ID、proto 字段、`.gia` 的字节格式都需要一个外部依据。这个依据是另一个*非官方*项目：

#link("https://github.com/Wu-Yijun/Genshin-Impact-Miliastra-Wonderland-Code-Node-Editor-Pack")[Genshin-Impact-Miliastra-Wonderland-Code-Node-Editor-Pack]

它和 `rust2genshin` 一样*非官方*，与米哈游 / HoYoverse 无任何关联。

读它、对照它，是为了知道格式，不是为了复制受版权保护的素材 —— 涉及游戏内部数据时保持现有做法：从真实导出文件逆推，用自家 schema。

= 遇到问题去哪查

#table(columns: 2, align: left + horizon)[要查][去
][节点 ID / 引脚 / 分类][`utils/node_data/data.json`（约 3.8 MB，含描述、枚举、类型系统）或 `game_nodes.ts`
][proto 字段含义][`utils/protobuf/gia.proto`（每字段有用途说明，标了保留区）—— 改 `core/proto/asset.proto` 前先查它
][`.gia` 头部格式][`utils/protobuf/decode.ts` 的 `unwrap_gia` / `wrap_gia` —— 本站#link("/dev/debugging.html")[调试]页那个 20 字节头 + 4 字节尾就是照它实现的
][类型引擎的设计参考][`utils/node_data/node_type.ts`
][节点导出/导入规则][`docs/utils/原神导入 GIA 文件的规则说明.md`
]

本项目侧的对齐点：`tools/node_data/nodes.json` 是从 `game_nodes.ts` 解析出来的（`tools/node_data/parse_nodes.py`），`core/src/node/*.rs` 里 `/// ...(ID N)` 的注释和 `NodeKind` 的 `id` 参数靠它对齐。注意 `nodes.json` *只有结构*，没有 `InGameName` / `Description` —— 要查节点*语义*、审命名、定类型边界，必须去 `data.json`。

= 它的三层栈

```
DSL (.ts 风格文本)  →  IR (src/types/IR*.ts)  →  GIA 二进制 (utils/protobuf/gia.proto)
     ↑ tokenizer/parser      ↑ gia_ir.ts              ↑ decode.ts
     ↓ decompiler.ts
```

= 和本项目的关键差异

理解这些差异，才不会拿参考项目的设计往本项目上套。

- *没有 DSL 层* —— `rust2genshin` 直接吃 rustc MIR，跳过文本解析
- *没有显式 IR 阶段* —— `NodeGraph` / `Node` / `NodeKind` 直接对应 `.gia` 的 wire 结构，不是 AST
- *类型系统不对等* —— 参考项目有完整的运行时类型引擎（`ConstraintType` / `StructType` / `reflects`）；本项目用轻量的 `NativeValue` trait + `NativeKind` 枚举
- *节点元数据的组织方式不同* —— 参考项目用 `data.json` 集中管理并自动生成 `game_nodes.ts`；本项目在 `core/src/node/<域>.rs` 里手写构造函数，ID 靠注释对齐
- *proto 字段一致但注释详略不同* —— 参考项目的 `gia.proto` 是权威 schema
- *复合节点的存法可能不同* —— 参考项目说明游戏端把复合节点内嵌在 `NodeGraphData.Embedded`（字段 7），本项目把 `CompositeNodeGraph` 拆成独立 asset
