#import "/lib/lib.typ": *;
#let title = [viewer];
#show: template;

#set raw(lang: "rust");

`rust2genshin_viewer` 是一个 GPUI 写的节点浏览器：把一个 `.ogia` / `.rlib` 拖进去，画布上画出里面的节点图，节点可以用鼠标拖，右栏能一条条跑优化规则、看它把图改成什么样。

它是*调试工具*，用来回答「编译器把这段 Rust 翻成了什么」—— 尤其是#link("/dev/debugging.html")[优化器]那一步到底动了什么。它读的是未 lower 的中间态 IR，为什么这么设计见#link("/arch/viewer.html")[viewer 的设计]。

= 输入从哪来

本仓库的 `.ogia` 产出端就是 `build-demo`：

```shell
cargo run -p build-demo --bin beyond-server
```

产物 `target/genshin-unknown-beyond-server/release/librust2genshin_demo.rlib` 里带一个 `rust2genshin_demo.ogia` entry（`ar t` 可以列出来）。把这个 rlib 拖进 viewer 即可。

正常构建*不会*为它生成独立文件 —— `.ogia` 只以 entry 形式存在于 rlib 里。

= 怎么跑

```shell
cargo run -p rust2genshin-viewer -- <路径>
```

*必须走 `cargo run`，不能直接双击 `target/debug/rust2genshin_viewer.exe`* —— 它依赖 `rustc_driver-*.dll`，那个 DLL 在 `$(rustc --print sysroot)/lib/rustlib/<host>/lib/` 下，cargo 会把这个目录加进 DLL 搜索路径，直接跑不会。

命令行有四种用法：

#list(
    [不带参数 —— 开一个空窗，等拖放],
    [`<路径>` —— 启动时直接加载这个 `.ogia` / `.rlib`],
    [`--check <路径>` —— *不开窗*，把解析结果打到 stdout（每张图的节点数、边数、入口/返回下标）],
    [`--emit-fixture <路径>` —— 写一个合成的 `.ogia` 夹具后退出（给测试用的确定性输入）],
)

*排查时先 `--check`。* 它能区分「文件里没有图」和「图里没有节点」这两种看起来一样的现象。后者是正常的 —— `librust2genshin_lib-*.rlib` 里就有一张节点数为 0 的图（`rust_begin_unwind`），viewer 不该把它当错误。

拖放之外还要有命令行入口，也因为*拖放能不能用是平台的问题* —— 先 `--check` 确认文件本身对不对，再看窗口。

= 界面

左中右三栏：

- *左栏* —— 图列表，每项是 `名字 (节点数)`。点一下切图。
- *中画布* —— 拖节点、拖空白平移、滚轮缩放。控制流引脚画成*向右的蓝色三角形*，值流画成*黄色圆点*，两类都按 pin 序号在节点侧边纵向均分。图边界之外引出桩线，末端挂一个写 `Export N` 的小框。
- *右栏* —— 「自动整理」、「一键优化」、每条消除规则一个按钮（标签省掉了 `eliminate_` 前缀）、「撤销 (Ctrl+Z)」、状态文案。

右键节点弹菜单，只有 `IrNodeId::Fn` 节点有「link」项 —— 把 Fn 复合节点展开成它的内部图，展开出来的节点落在被 link 的节点原位。

= 构建

它不在工作区 `default-members` 里（原因见#link("/arch/viewer.html")[设计]）：

```shell
cargo build -p rust2genshin-viewer
cargo test -p rust2genshin-viewer
```
