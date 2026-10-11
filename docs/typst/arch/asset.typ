#import "/lib/lib.typ": *;
#let title = [资产格式];
#show: template;

#set raw(lang: "rust");

`.gia` = *20 字节头* + protobuf 数据 + *4 字节尾*，共 24 字节开销。编解码在 `core/src/asset.rs`。

= 头部

5 个 u32 大端（`asset.rs:111-126`）：

#table(columns: 2, align: center + horizon)[偏移][含义
][`0x00`][文件总大小 − 4（= proto 长度 + 20）
][`0x04`][schema 版本 = 1
][`0x08`][头部标记 `0x0326`（加载器严格校验）
][`0x0C`][文件类型 = 3（GIA，固定值）
][`0x10`][proto 数据长度（= 文件总大小 − 24）
]

尾部固定 `0x0679`。

`AssetBundle::save` 写盘前先比对字节，内容没变就跳过 —— 让 `demo/build.rs` 的 `cargo:rerun-if-changed` 不会因为一次成功构建而自我触发。

= protobuf 层

schema 在 `core/proto/asset.proto`，`core/build.rs` 用 `prost-build` 生成绑定到 `OUT_DIR/rust2genshin.rs`，`core/src/asset.rs:7-8` 里 `include!` 进来。

*改了 `.proto` 要重新 `cargo build`* 才会重新生成。Python 侧不用管 —— `tools/gia.py` 运行时读 proto 文件。

proto 字段是*从真实导出文件逆推*的。权威 schema 在参考项目的 `utils/protobuf/gia.proto`：字段含义的注释更全，标了保留区。要改 `asset.proto` 的字段，先去那边查。见#link("/arch/reference.html")[参考项目]。

= 值怎么编码

`TypedValue` 是值的通用编码，按类型分 `Int` / `Flt` / `Str` / `Enum` / `Vec3f` / `StructStorage` / `ListStorage` / `MapStorage` / `PolymorphicValue`。

编译器侧对应的类型体系在 `core/src/value.rs`：

- `NativeKind`（`value.rs:12`）—— 枚举：`Bool` / `Int` / `Float` / `String` / `Vec3` / `Guid` / `Faction` / `Config` / `Prefab` / `Entity` / `Enum(i32)` / `List` / `Dict` / `Struct(StructRef)` / `LocalVarRef` / `VarSnapshotRef`
- `NativeValue`（`value.rs:54`）—— trait，每种类型各自实现，负责编解码

= 节点的引用

`NodeRef` 序列化成 `id + NODE_ID_BEGIN`，`NODE_ID_BEGIN = 1`（`node/mod.rs:45,58,61`）。所以线上的 node id 从 1 开始，0 保留。

结构体类型走 `StructureDefinition` / `StructField`（`core/src/structure.rs`），对应 assemble / destructure / modify 三个节点 —— 用法见#link("/arch/compile/locals.html")[局部变量]。
