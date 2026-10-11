#import "/lib/lib.typ": *;
#let title = [调试];
#show: template;

#set raw(lang: "rust");

= 打印诊断

后端跑在编译进程里，所以诊断走 rustc 自己的机制：`tcx.dcx().warn(...)` / `.err(...)`。它们和 rustc 的其它诊断一起打印，`-Z macro-backtrace` 之类也照常能用。

`core/src/compile/mod.rs` 的 `compile()`（入口，第 70 行）里就有一条 `tcx.dcx().warn` —— 目标 crate 没写 `#![no_std]` 时打的那条 —— 是现成的例子。

= 被忽略的东西

后端对这两类 MIR 终结符*不做处理*，只打一条提示就当无条件跳转：

- `Assert` —— 打印 `Ignored assert`（`core/src/compile/func.rs`）
- `panic` —— 打印 `Ignored panic`

`Assert` 覆盖的不只是溢出检查，还有数组越界、除零之类的运行时断言。它们全都被丢掉。

所以想靠「编译报错」发现这类问题是不可能的 —— 编译会通过，断言静默消失。调这类问题时，看编译输出里有没有 `Ignored` 提示比看退出码有用。

= 溢出检查

`i32` 溢出是回绕，不是 panic。

workspace 的 `[profile.release]` 里写着 `overflow-checks = false` —— 但 release profile *默认*就是这个值，那行是冗余的，写在那儿只是表明意图。

把溢出检查打开也不会改变什么：对应的 `assert` 反正会被丢掉（见上）。旧文档里说的「必须传 `-Coverflow-checks=off`」在当前的构建脚本和 CI 里都不存在，也不需要。

= 增量构建

两层机制保证改了东西之后产物会跟着更新：

*demo 侧*（`demo/build.rs`）向 cargo 声明了三类 `rerun-if-changed`：

- `src/` 下的每个文件（含子模块）
- 后端 cdylib（`target/debug/rust2genshin.dll`）—— 少了这条，只改后端源码时 `demo` 的增量缓存不会失效，`.gia` 会一直是旧的
- `.gia` 自身

*后端侧*（`core/src/asset.rs` 的 `AssetBundle::save`）在写盘前会把新字节和已有文件比对，*相同就不写*。

第二条是为了打破第一条里的第三个 `rerun-if-changed` 造成的死循环：每次构建后端都会尝试写 `.gia`，如果无条件写就会刷新 mtime，进而再次触发构建。内容比对让 mtime 只在内容真的变了时才前进，循环最多多跑一轮就停。

= Python 工具

`tools/` 被 `.gitignore` 忽略（不在仓库里，只有本地有），依赖 protobuf 运行时：

```shell
pip install protobuf
```

主要几个：

- `gia2txt.py` —— `.gia` 转文本，最常用。`--no-unknown` 只显示 proto 里已声明的字段
- `diff_gia.py` —— 比对两个 `.gia`
- `check_unknown.py` —— 统计有多少字段是 proto 未声明的
- `dev/wire_tree.py` —— 输出里的数字键（如 `2: 123`）代表 proto 没声明这个字段，用它识别类型
- `node_data/nodes.json` —— 节点 ID / 引脚 / 分类的真相源（只有结构、没有描述，见#link("/arch/reference.html")[参考项目]）

```shell
python tools/gia2txt.py target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia
```
