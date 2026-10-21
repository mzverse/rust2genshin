#import "/lib/lib.typ": *;
#let title = [playground];
#show: template;

#set raw(lang: "rust");

`rust2genshin-playground` 是文档站#link("/play/index.html")[「摆弄」页]的后端：一个独立的 web 服务，接受一份 Rust 源码，排队、在*一次性 docker 容器*里跑 `cargo build`，把日志实时推回浏览器，成功后把 `.gia` 经 WebSocket 二进制帧送回浏览器（不落服务端磁盘、无下载路由）。

它是*服务*而不是*构建工具* —— 编译环境由它自己维护：从本地仓库烘一份镜像（工具链 + 预热好的依赖），每个任务起一个一次性容器；`main` 的 push 触发重烘焙。

= 怎么跑

```shell
cargo run -p rust2genshin-playground
```

本机要有 `docker` 和 `git`，两个都在 `PATH` 上 —— 缺一个服务会*在启动时直接退出*，不会变成「能连上但永远烘不出来」的状态（`playground/src/main.rs` 的 `check_tools`）。默认监听 `127.0.0.1:8787`，要换监听地址设 `PLAYGROUND_LISTEN`。

同一条命令另一个细节：同一台机器上不要同时跑两个实例 —— 服务启动时会抢数据目录的排他锁（`playground/src/main.rs` 的 `lock_data_dir`），第二个实例会直接被拒。

= 配置

全部配置项都从环境变量读，默认值只写在 `playground/src/config.rs:148-175` 一处。空串与未设置等价（部署时 `VAR=` 和 `unset VAR` 结果一样）。

#table(
    columns: (auto, auto, 1fr),
    inset: 6pt,
    fill: (_, y) => if calc.odd(y) { luma(235) },
    [*变量*], [*默认值*], [*干什么用的*],

    [`PLAYGROUND_LISTEN`], [`127.0.0.1:8787`], [HTTP / WebSocket 的监听地址。],
    [`PLAYGROUND_REPO_DIR`], [`playground/` 的上一级（即仓库根）], [烘镜像的 docker build context。服务假定仓库就在自己旁边，*不 clone 到临时目录*。],
    [`PLAYGROUND_DATA_DIR`], [`<repo_dir>/target/playground`], [任务目录、产物、锁都在这底下（见「磁盘」）。*跑在容器里时这个路径必须是 docker daemon 也看得见的那个* —— 任务的源码目录要作为 `-v` 传给 dockerd，容器内的路径它在宿主上不存在时，docker 会在宿主上凭空建一个同名*空目录*挂进去，表现为 `can't find library rust2genshin_demo`（见「容器里跑」）。],
    [`PLAYGROUND_REF`], [`main`], [webhook 到达时 `git pull --ff-only` 的分支。钉成非分支名（比如 sha）时 pull 跳过，只重烘焙。],
    [`PLAYGROUND_REMOTE`], [`origin`], [pull 用的远端名。],
    [`PLAYGROUND_MAX_JOBS`], [`8`], [同时跑的任务数上限。],
    [`PLAYGROUND_MAX_QUEUE`], [`64`], [排队等待的任务数上限；超出后新任务被拒。],
    [`PLAYGROUND_JOB_TIMEOUT`], [`15`], [单个任务的超时，*秒*。],
    [`PLAYGROUND_JOB_CPUS`], [`2`], [任务容器的 CPU 限额。设成 `0` 或 `none` 则*不加* `--cpus` —— 宿主的 cgroup 没挂 cpu 控制器时，docker 会拒掉整个 `docker run`（`NanoCPUs can not be set…`），退出码 125，一条任务日志都没有。],
    [`PLAYGROUND_KEEPALIVE`], [`25`], [空闲连接多久发一个 WebSocket ping，*秒*。必须*短于*前面那层 nginx stream 的 `proxy_timeout`（部署上是 60s）：排队等镜像时一条业务消息都没有，不主动出声连接会被 nginx 悄悄关掉，`finished` 连同紧随其后的 `.gia` 二进制帧一起丢在半路。],
    [`PLAYGROUND_JOB_MEMORY`], [`1536m`], [任务容器的内存限额，同时给 `--memory` 和 `--memory-swap`。设成 `none` 则两个都不加 —— cgroup v1 不支持 `kmem.limit_in_bytes` 时，runc 建容器就会炸（`failed to write 1 to memory.kmem.limit_in_bytes`）。别写 `1.5g`，docker 对小数容量不友好。],
    [`PLAYGROUND_KEEP_IMAGES`], [`2`], [镜像 GC 保留几个最近在用的 tag。],
    [`PLAYGROUND_BUILD_CACHE_MAX`], [`10GB`], [`docker builder prune --keep-storage` 的值 —— 注意是*留到*多少而不是*删到*多少（见「磁盘」）。支持 `10GB` / `512m` / `1024`，倍数一律 1024 进制。],
    [`PLAYGROUND_MIN_FREE_DISK`], [`5GB`], [磁盘水位：低于这个值就不再接新任务。量的是 docker 的 data root（镜像层和 builder cache 都在那），不是数据目录。],
    [`PLAYGROUND_ARTIFACT_TTL`], [`1800`], [任务目录（`src/` 与 `meta.json`）的存活时间，*秒*（默认半小时）。过期由后台 GC 清掉。产物不落盘，见下文。],
    [`PLAYGROUND_ALLOWED_ORIGINS`], [`https://mzverse.github.io,http://127.0.0.1:3000,http://localhost:3000`], [WebSocket 握手时校验的 Origin 白名单，逗号分隔。WebSocket 不受 CORS 保护，这句话就是全部防线（`playground/src/server/mod.rs` 的 `origin_allowed`）。],
    [`PLAYGROUND_TRUST_PROXY`], [`false`], [*只在后端前面确实挂着一个反代时才设 `1`。* 为真时才信 `X-Forwarded-For` 头来决定客户端 IP；否则任何客户端都能伪造这个头，绕过「每 IP 一个任务」的限制（`playground/src/server/mod.rs:19-34` 的 `client_ip`）。],
    [`PLAYGROUND_FORCE_REBAKE`], [`false`], [启动时忽略本地现成的镜像，强制重烘一遍。],
    [`PLAYGROUND_IMAGE`], [`rust2genshin-play`], [烘出来的镜像名（tag 里会再带 short sha 和 `-dirty`）。],
    [`PLAYGROUND_ADMIN_TOKEN`], [无], [`/admin/rebuild` 的运维手工触发凭证（`Authorization: Bearer <token>`）。与 `GITHUB_WEBHOOK_SECRET` 一起决定这条路由能不能被调用 —— 两个都没配就是 403，不做「没配就放行」。],
    [`GITHUB_WEBHOOK_SECRET`], [无], [校验 GitHub webhook 的 HMAC 签名（`X-Hub-Signature-256`）。和上一个一样，未设则不接受这条路。],

    [`PLAYGROUND_LSP_DISABLED`], [`false`], [*整个关掉代码分析。* 为真时服务端不建任何 rust-analyzer 容器，`hello.lsp_supported` 报 false，编辑框退化成纯文本 —— 页面其余部分照常能用。],
    [`PLAYGROUND_LSP_POOL_MIN`], [`1`], [后台维持几个*待命*的 RA 会话。设 `0` 表示不预热：每个请求现场起一个，冷启动 11 秒（裁剪过 workspace，见「编辑器与 rust-analyzer」）。],
    [`PLAYGROUND_LSP_IDLE_TTL`], [`90`], [空闲会话闲置多久后销毁，*秒*。销毁是 `docker rm -f`，会连 RA 进程一起收掉；下次要用重新冷启动。],
    [`PLAYGROUND_LSP_BOOT_TIMEOUT`], [`30`], [一个 RA 会话从建容器到就绪的上限，*秒*。超时算失败，池会换一个。],
    [`PLAYGROUND_LSP_MEMORY`], [`2g`], [RA 容器的内存限额。*每个待命会话就吃这么多*，宿主上按 `LSP_POOL_MIN` 线性涨。设成 `none` 则不加 `--memory`。],
    [`PLAYGROUND_LSP_CPUS`], [`2`], [RA 容器的 CPU 限额。与 `PLAYGROUND_JOB_CPUS` 是同一个限制：设成 `0` 或 `none` 则不加 `--cpus`。两个开关必须配成一样 —— 只关任务容器那一半，`docker run` 会在 RA 容器上失败（`NanoCPUs can not be set`），表现为「能编译、编辑框永远停在分析中」。],
    [`PLAYGROUND_LSP_MAX_SOURCE_BYTES`], [`10240`], [代码分析的源码体量上限，*字节*（默认 10 KiB）。超过就不把源码送给 RA，编辑框只留 CodeMirror 的词法高亮，状态行提示「代码分析已停用」。按字节算（中文一个字 3 字节），前端用 `TextEncoder` 量，跟服务端 `str::len()` 对得上。],
)

布尔变量的写法：`1` / `true` / `yes` 为真，`0` / `false` / `no` 为假，其余报错。

= webhook 怎么配

要让 `push` 到 `main` 自动重烘焙，在 GitHub 仓库的 Settings → Webhooks 里加一个：

- *Payload URL* —— `https://<后端地址>/admin/rebuild`
- *Content type* —— `application/json`
- *Secret* —— 和 `GITHUB_WEBHOOK_SECRET` 一致

本地调试不想把仓库暴露出去，用 `gh webhook forward` 把 GitHub 的事件转发到本机的 `http://127.0.0.1:8787/admin/rebuild`。

只认 `main` 的 push：不是 push、或者不是 `main`，服务回 202 说「跳过」而不是 4xx —— 回 4xx 会让 GitHub 反复重投，而它本来就不该触发。

= `wss://` 那条

文档站是 GitHub Pages 上的 *https* 页面，而 *https 页面不允许连 `ws://`*（混合内容会被浏览器掐掉）。所以线上必须让后端挂在一个*有证书的反代*后面，前端才连得上 `wss://`。

本地开发走 `http://127.0.0.1:3000` 没有这个问题 —— http 页面连 `ws://127.0.0.1:8787` 是允许的，`PLAYGROUND_ALLOWED_ORIGINS` 的默认值里也留了这两个本地 origin。

= 磁盘怎么算

两块盘、两套清理，别混起来：

- *任务目录* —— `target/playground/` 下是每个任务的工作目录（只有 `src/lib.rs` 与 `meta.json`），按 `PLAYGROUND_ARTIFACT_TTL` 过期清掉。*.gia 不在里面*:它留在容器里，编译完用 `docker cp <容器>:<路径> -` 当 tar 流取回内存。因此任务容器*不带 `--rm`*（否则取不到），清理由 `ContainerGuard` 的 `docker rm -f` 负责。
- *镜像和 builder cache* —— 走 docker 自己的 data root（不是项目里的 `target/`）。镜像按 label 清，只认本服务烘的那些，*绝不碰宿主上别人的镜像*。

一条要紧的副作用：`docker builder prune` 是*全局*的 —— builder cache 没有 label、在这台机器上被所有项目共享，所以那一下清的是*所有* builder 的缓存，不只是我们的（`playground/src/docker.rs:166-169`）。还有 `PLAYGROUND_BUILD_CACHE_MAX` 传的是 `--keep-storage`，语义是*留到*多少，超预算的丢最旧的。

= 编辑器与 rust-analyzer

「摆弄」页的编辑框是 CodeMirror 6。语法高亮、红色波浪线、补全列表，三样全部来自容器里的 `rust-analyzer`，经 LSP 过一道 WebSocket 回来 —— *没有第二套本地语法分析在抢颜色*。本地只借了 `@codemirror/lang-rust` 做*词法*（bracket matching 之类），缩进是自己写的服务，*不装* `syntaxHighlighting`，所以它不会画出任何自己的颜色。

*token 类型下标由服务端翻译。* 我们在 `initialize` 里上报自己的 `tokenTypes` 表，但 RA *不按它编码* —— 它回一份自己的 legend（实测是字母序），索引编码进那份表。前端和后端因此不能共用一张表：后端在 `initialize` 之后按*名字*把 RA 的下标翻到我们的下标再转发，对不上名字的（`brace` / `comma` / `lifetime` 之类）编码成越界哨兵，前端解码时直接跳过。不这么翻的后果很具体：RA 的 `keyword` 是它的 6 号，而我们的 6 号是 `typeParameter` —— 于是 `pub` / `fn` 全被染成斜体。

*高亮和诊断是两个独立的 decoration 层*，可以叠在同一个 range 上：一个字既是函数名（高亮）又是错误位置（波浪线）时，两个 class 一起生效，不是二选一。RA 的错误说明挂在 decoration 的 `title` 上，悬停波浪线就能看到原文。

*诊断是整份替换的，不是逐条追加。* LSP 的 `publishDiagnostics` 语义就是「这份取代上一份」，空数组表示清空；协议因此一条消息带完整 `items`。逐条发送的旧形状下，服务端清空时一条消息都不发，前端既分不清一批何时结束也收不到清空信号，旧坐标只能永久累积 —— 文档一改就指到别的文字上，波浪线擦不掉。

*诊断是服务端主动「拉」回来的，不是等 RA 推。* RA 1.10x 把诊断改成了拉取式（`textDocument/diagnostic`）：`publishDiagnostics` 只在工作区加载完时推一次，之后文档怎么改都不再推。所以 `didOpen` / `didChange` 之后服务端都要自己拉一份，`workspace/diagnostic/refresh` 通知到达时也要再拉 —— 只监听推的那条链路，波浪线会永远停在第一次分析的结果上。RA 对没变的内容只回 `kind: "unchanged"` 且不带 `items`，会话又是跨连接复用的，所以服务端缓存最近一份完整集合，遇到 unchanged 就回放它。

*补全只在真有前缀时弹。* `activateOnTyping` 是在键入的字符*之前*触发的，`context.pos` 因此落在已敲字符之前 —— 直接拿它当 `from`，选中后那个字符会留在替换范围外，`log` 补成 `llog`。所以 `from` 由自己往回走过词字符算出来。另外没有前缀就直接返回 `null`：RA 会把整个 crate 的符号都列出来，满屏 item 只会挡住代码。

*缩进是自己写的服务，走 `indentService`。* CodeMirror 只提供*机制*（`indentService` facet、`defaultKeymap` 里的 Enter 命令、语法树兜底），每种语言的规则由各自的 language 包给 —— `@codemirror/lang-rust` 一条都没给，lezer-rust 也没定义 `indentNodeProp`，兜底算出来是 0。所以规则得自己写，一共两条：光标前*还没有非空白内容*时沿用本行已有缩进（这条保证反复回车不会越缩越深），否则前一个非空白字符是开括号就多一层、其余沿用本行缩进。

看光标*之前*那段文本而不是整行 —— 光标常停在 `}` 之前，看整行会先撞上它。另外 `indentService` 这条链路只在 `defaultKeymap` 被装上后才通：Enter 走的是里面的 `insertNewlineAndIndent`，不绑它服务一次都不会被调用。被顶下去的 `}` 不经过本服务，CodeMirror 自己按 `{` 行的前导空白算。

*`indentOnInput` 在这个组合下是空转，别配。* 它的实现是 transactionFilter，只在 `tr.isUserEvent("input.type")` 时跑，还要 `languageDataAt("indentOnInput")` 提供一个函数 —— `lang-rust` 不提供，配了也不触发。

*Tab 自己写 handler，但挂默认优先级。* 不用官方的 `indentWithTab`：从 esm.sh 加载时它带着另一份 `@codemirror/view`，产出的扩展会被本页这份 `state` 判成「Unrecognized extension value」，整个编辑器建不起来。挂 `Prec.high` 同样错 —— 补全弹窗开着自己的 Tab 绑定，高优先级会抢在它前面，弹窗一开 Tab 就失灵。

*RA 的分析范围裁到三个成员。* 镜像里除了任务容器要用的完整源码，另烘了一份 `/opt/ra/ws` —— 只有 `demo` / `lib` / `lib-internal`，没有 `core`。`core` 是 `rustc_private` 后端，RA 为了解析它要把整个编译器 sysroot 的元数据拖进来，冷启动从 11 秒涨到 84 秒，而 `demo` 的依赖链上根本没有它。裁剪在烘焙时用一次 `cargo metadata --offline` 自检，裁错了在烘焙时就炸，而不是用户第一次编辑时才发现。

*启动时有镜像就直接用，不按 tag 重烘。* 启动计划只看本机有没有带 commit label 的镜像：有就拿来服务，*不烘新的*，哪怕它的 tag 对应的是更早的 commit。镜像 tag 跟烘焙时的 commit 绑定，但服务进程和镜像里的后端/lib 是两份东西 —— 重启服务不该因为源码前进了几个 commit 就等十几分钟。实际跑的是哪一版由 `hello` 的 serving commit 如实报出；要换新版走 webhook 的 `main` push、手工调 `/admin/rebuild`，或者设 `PLAYGROUND_FORCE_REBAKE=1` 在启动时无视本地镜像重烘一遍。只有本机一张镜像都没有时才走冷启动烘焙。

*会话池是有代价的。* 一个待命的 RA 会话在 `did_open` 之后吃 839 MB；`PLAYGROUND_LSP_POOL_MIN` 乘以这个数就是这部分常驻内存。空闲超过 `PLAYGROUND_LSP_IDLE_TTL` 会话被 `docker rm -f` 收掉，RA 进程随之消失 —— 不靠谁的 `Drop`，也不靠连接断开。

*待命会话里已经有文档了。* 补池建会话时会先 `did_open` 一行占位内容把 crate graph 拉热。会话交棒给浏览器之前（`LspService::acquire`）与归还时（连接收尾）服务端各补一条 `didClose`，所以池里那个 URI 在交给浏览器时一定是关闭状态，浏览器的 `didOpen` 永远合法；用户的源码走 `didChange` 全文替换。

*目标选择只影响编译。* 顶部的「目标」下拉框列的是 `hello.targets`，由服务端那张表驱动（`playground/src/protocol.rs` 的 `TARGETS`）；超限与经典模式的差别只在 `--target` 指向哪份 spec、`-gamemode` 传什么，LSP 链路完全不感知。切换目标后要重新编译才生效（它改的是编译产物，不是编辑器状态）。重连 WebSocket 会把下拉框重置回第一项。

*源码太大就不分析。* 超过 `PLAYGROUND_LSP_MAX_SOURCE_BYTES`（默认 10 KiB）时，服务端不写盘、也不把源码转发给 RA —— 前端有同款判断，但那只是快路径（省一次往返、一次 RA 冷启动），闸门在服务端。超限时编辑框只剩 CodeMirror 的词法高亮（括号配对那些），语义高亮和诊断都不动，状态行写明「代码分析已停用」；把代码删回上限以下会自动恢复。阈值按*字节*算，中文一个字 3 字节。

*同一个 IP 同时只允许一条会话。* 这条限制挂在*连接*层，不是编译层 —— 编译那条路的「每 IP 一个」早就有了，但它管不住 rust-analyzer：代码分析不经过队列，一个 IP 开 N 个标签页就能起 N 个 RA 容器（每个常驻 839 MB）。所以第二条连接握手成功后立刻收到一条 `session_busy`、然后被关掉，前端看到这条就不再自动重连（否则会变成「连上 → 被拒 → 退避 → 再连」的循环）。关掉另一个标签页之后，点一次「编译」可以再试。

这里的 IP 是*归一化过的*（IPv6 按 /64 归并、IPv4-mapped 归一），在 /64 里换地址绕不过去。反代后面要看 `PLAYGROUND_TRUST_PROXY`。

= `cargo clean` 与锁

`cargo clean` 会把 `target/playground/`（数据目录默认就在 `target/` 底下）*一起删掉*。正在跑的任务会因此失败，但服务扛得住 —— 下次会重建目录（`playground/src/main.rs` 的 `prepare_dirs`）。`target/playground/lock` 是那份排他锁，同样在底下。

= 构建

它和 viewer 一样不在工作区 `default-members` 里：

```shell
cargo build -p rust2genshin-playground
cargo test -p rust2genshin-playground
```

`cargo test` 在*没有 docker 的机器上也能全绿* —— 需要容器的几条标了 `#[ignore]`，只在有 docker 的机器上用 `-- --ignored` 跑。

= 容器里跑服务

服务自己跑在一个容器里、任务容器由它通过 docker socket 拉起 —— 这个套娃有三个坑，都不是代码能兜住的，得在部署时配对。

*数据目录必须是宿主也看得见的路径。* 任务源码目录是拿 `-v <src_dir>:/opt/rust2genshin/demo/src:ro` 传给 dockerd 的，而 dockerd 在*宿主*上解析这个路径。数据目录在容器内（比如 `/opt/rust2genshin/target/playground`）时，宿主上并没有这条路径，docker 不会报错 —— 它会*在宿主根上凭空建一个同名空目录*挂进去。症状是 cargo 报 `can't find library 'rust2genshin_demo', rename file to src/lib.rs`，而文件明明就躺在旁边。做法是把宿主目录挂到*同名*路径再用：

```shell
-v /mnt/server/r2g-data:/mnt/server/r2g-data
-e PLAYGROUND_DATA_DIR=/mnt/server/r2g-data
```

*挂 docker 的 data root 进去。* 烘焙前的水位闸量的是 docker 的 data root（`docker info` 里那个）。容器里看不到它就报「读不到磁盘 /mnt/server/docker 的可用空间」，烘焙在起 `docker build` 之前就短路了（`state: no_disk`）。`-v /mnt/server/docker:/mnt/server/docker:ro` 就够，只读。

*工作目录别丢。* `--file` 是按 docker *客户端*的 CWD 解析的，不是按 context 解析。服务得用 `-w <repo_dir>` 起 —— 代码现在会拼绝对路径，但第三方 Dockerfile 未必。

另外这台宿主上 cgroup 的 cpu 与 kmem 控制器都不可用，`PLAYGROUND_JOB_CPUS=0` 和 `PLAYGROUND_JOB_MEMORY=none` 是必须的，见上面的配置表。两个 `LSP_` 版本（`PLAYGROUND_LSP_CPUS=0`、`PLAYGROUND_LSP_MEMORY=none`）同理 —— 它们控制的是 RA 容器，漏掉的话编译照常、编辑框永远「分析中」。

*构建脚本要把 `RUSTUP_HOME` / `CARGO_HOME` 挂到宿主上。* 服务本体的编译跑在一个 `docker run --rm rust:…` 里（`build-playground.sh`），而 `rust-toolchain.toml` 钉的是 nightly —— 不挂缓存的话 `/usr/local/rustup` 与 `/usr/local/cargo` 随容器一起消失，*每次构建都要重下整个 nightly（含 1.5 GB 的 `rustc-dev`）和整个 registry 索引*。这不是慢，是脆：镜像源抖一下（rsproxy 返回 504）构建就整个失败。挂两个宿主目录出去即可：

```shell
-v "$CACHE/rustup:/cargo-home/rustup" -v "$CACHE/cargo:/cargo-home/cargo"
```

第一次跑要付一次全量下载的钱（本机约 13 分钟），之后同一份代码重建是*秒级*（本机 2.5 s，`cargo` 自己都在 0.3 s 内 `Finished`）。
