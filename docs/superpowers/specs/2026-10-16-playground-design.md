# 摆弄场（playground）—— 设计

日期：2026-10-16

## 目标

在文档站的「摆弄」分区里放一个网页：写一个单独的 Rust 文件，提交，排队编译成 `.gia`，实时看编译日志，成功后下载产物。后端是一个单独的 crate，作为独立的 web 服务跑，和前端跨域通信（WebSocket）。

服务自己负责维护编译环境：从本地仓库出一份 docker 镜像（工具链 + 预热好的依赖），每个任务在一个一次性容器里跑，push 到 `main` 触发镜像重烘焙。

**MVP 不做**：节点图预览、rust-analyzer 代码提示、多文件与自定义 `Cargo.toml`、账号体系、任务持久化、classic-server 目标平台。其中 rust-analyzer 要留出接口（见「前端」一节），其余不预留。

## 现状

构建链（全部在 `build-demo/src/build.rs:build()`）：

1. `cargo build -p rust2genshin` —— 出 debug 的 cdylib，以及 `target/debug/rust2genshin-linker`（bin）。
2. `gen_target(name)` —— 把 `genshin-unknown-<name>.json.template` 里的 `{backend}` 换成 cdylib 的绝对路径，写出 `genshin-unknown-<name>.json`。
3. `cargo build --release -p rust2genshin-demo --target ./genshin-unknown-<name>.json -Zunstable-options -Zjson-target-spec -Zbuild-std=core` —— 产物落在 `target/genshin-unknown-<name>/release/rust2genshin_demo.gia`。

三处会直接影响容器里的再现：

- 目标 json 的 `"linker": "./target/debug/rust2genshin-linker"` 是**相对路径**，所以工作目录必须是仓库根，且 `target/debug/` 里那两个产物必须留在镜像里。
- 工具链钉在 `rust-toolchain.toml`：`nightly-2026-10-08` + `rustc-dev` / `rust-src` / `llvm-tools-preview`。`-Zbuild-std=core` 靠 `rust-src`，CI 另外只装 `protobuf-compiler`（`core/build.rs` 走 prost-build）。
- 仓库没有 `.cargo/`，编译不需要额外配置。

前端一侧现成的：`lib/lib.typ:31` 的 `custom_element` + `lib/js/` + `lib/css/` 那套管线（全貌见站点「自定义元素」页）。`play/index.typ` 现在只是个说明页，`template.with(toc: false, with_outline: false)`。

`Cargo.toml` 的 `default-members` 不含 `viewer`（理由：gpui 的系统依赖），playground 同理。

开发机现状：这台 Windows 机器**没有装 Docker**（`docker` 不在 PATH，`/c/Program Files/Docker` 不存在）。实现时容器那条路只能在装了 Docker 的机器上验证。

## 方案

一个进程，三个阶段：

```
启动 ─┬─→ 烘焙：本地仓库 ──docker build──→ rust2genshin-play:<sha>
      │
      └─→ 服务：axum（WS + HTTP），承接任务，起一次性容器
            
push ───→ git pull（快进）──→ 重烘焙（后台，单飞）──→ 新 tag，新任务用它
```

### 源：本地仓库，不 clone

`PLAYGROUND_REPO_DIR` 默认 `CARGO_MANIFEST_DIR/..` —— 服务假定仓库就在自己旁边，**不做「clone 到临时文件夹」**，docker build 的 context 就是这个目录。

理由是开发期：改完 `core/`，重烘焙一次就能在网页里试，不用先 commit 再 push。被否掉的两个：

- **每次 `git clone` 到临时文件夹**：要网络、重复下载、而且和开发者眼前那份工作区可能不是一个东西（原始需求里写的是这条，这里改掉）。
- **`git archive` / `git worktree` 取某个 commit 的树**：可复现，但看不到未提交的改动 —— 而"用本地仓库"这个选择本身就是冲着开发期迭代去的。

代价是**工作区脏时镜像内容无法从 tag 复现**。所以 tag 里带上这件事：`rust2genshin-play:<short-sha>`，树脏则 `rust2genshin-play:<short-sha>-dirty`，`/status` 和 `/healthz` 一并报出来。

镜像还打两个 label：`rust2genshin.play=1`、`rust2genshin.commit=<sha>` —— 后面清理只认这个 label，绝不碰宿主上别人的镜像。

push 到达时的动作就是 **`git pull --ff-only`**（先 `git fetch`，再快进）。两个细节：

- 用 `--ff-only` 而不是裸 `git pull`：这个仓库是**开发者自己的 checkout**，裸 pull 在分叉时会在工作区留下一个待解决的 merge 冲突态，服务不该把需要人处理的东西丢进别人的仓库。快进不了就只记一条 warning，**照常烘焙当前工作区**（`/status` 里报失败原因）—— 开发期工作区脏、或者本地领先远端都是常态，不该因此拒绝服务。
- `PLAYGROUND_REF` 不是分支名时（钉了 sha），pull 跳过，只重烘焙。

`.dockerignore` 排掉 `target/`、`.git/`、`tools/`（本来就在 `.gitignore` 里）、`*.gia*`（根目录那堆调试残留）。`docs/` 和 `viewer/` **不能排**：它们是 workspace 成员，缺 `Cargo.toml` 会让 `cargo build -p rust2genshin` 解析不了 workspace。

### 身份：tag 用 sha，不用 `latest`

任务记录、`hello`、`/status` 都报"这一份产物出自哪个 commit"。如果 tag 是 `latest`，就会有一个 TOCTOU：提交时告诉用户 commit A，容器起跑时 `latest` 可能已经指到 B —— 用户拿着"来自 A"的结论，拿到 B 编的产物。sha tag 让"报告 == 实际"。

### 烘焙

`playground/Dockerfile`，context = 仓库根。**故意分层**，让不常变的东西停在下面的层，push 重烘焙时只失效最上面那层：

| 层 | 内容 | 什么时候失效 |
|---|---|---|
| 1 | `debian:bookworm-slim` + apt：`git` / `curl` / `protobuf-compiler` / `build-essential` / `ca-certificates` | 基本不变 |
| 2 | `useradd -u 1001 -m builder` + rustup + `rustup show`（按 `rust-toolchain.toml` 自动装 nightly 和三个 component） | 改 `rust-toolchain.toml` |
| 3 | `COPY --chown=builder:builder . .` + `cargo fetch` + `cargo run -p build-demo --bin beyond-server` | 每次 push |

> 原先设想在第 3 层再切一层「只 COPY 各成员清单 → `cargo fetch`」，把 registry 下载单独缓存起来。**做不到**：workspace 成员没有 `src/` 时 cargo 直接报 `no targets specified in the manifest`（`core` 的两个 target 还是显式 `path`，得把 `src/linker.rs`、`build-demo/src/beyond-server.rs` 等逐个占位才骗得过去，每加一个成员就多一处会漂的清单）。而 `COPY . .` 每次 push 都会失效，这一层省下的只有下载、省不下编译 —— 不值这份脆弱。（`cargo fetch` 并入第 3 层后仍值得留：它让依赖下载发生在编译之前，日志里能分开看。）

第 3 层那一次完整 `build-demo` 是**故意的**，它一次干三件事：

- 把 `target/debug/librust2genshin.so` 和 `rust2genshin-linker` 留在镜像里（目标 json 的相对路径靠它们）；
- 生成 `genshin-unknown-beyond-server.json`（里面是**容器内**的绝对路径，所以 WORKDIR 必须固定）；
- 预热真正慢的那几块：`build-std` 从源码编出来的 `core`、各依赖的 rlib、registry。

烘完把 demo 那次产出的 `.gia` 删掉（残留物，没用了）。

容器内的用户：`useradd -u 1001 -m builder`，`WORKDIR /opt/rust2genshin`，`USER builder` —— 任务期 cargo 要往 `target/` 写，非 root 用户得是这些目录的属主，所以在烘焙时就 `COPY --chown=builder:builder`。

烘焙由服务编排，不是 `docker build` 之外还有别的步骤：**host 只需要 `docker` 和 `git`** 两个命令。启动时探测这两个，缺任何一个立刻退（`exit 1`）并打印缺什么。

状态机：

```
Idle ──bake(sha X)──→ Baking(X) ──ok──→ Ready(X, serving)
                          │
                          └──fail──→ Ready(旧 serving) + baking.failed
```

- **单飞**：同一时刻只允许一个烘焙在跑，防止并发 `docker build` 把机器打满。
- **折叠 pending**：烘焙中又来 push（sha Y），只记 `pending = Some(Y)`（覆盖更早的 pending），当前这次烘完立刻再烘 Y。中间的 sha 不烘 —— 没人来得及用，连续烘焙只会让机器一直满负荷。
- **失败保旧**：`main` 上推了个编不过的 commit，不该把 playground 打死。服务继续用上一个成功的镜像接任务，`/status` 报 `{"serving": "abc1234", "baking": {"sha": "def5678", "state": "failed", "tail": [...]}}`。
- **从未成功过**时（首次烘焙就失败）进程继续跑，`/healthz` 报 `not_ready` 带失败原因和日志尾巴。不退 —— 前端能把日志显示出来，比进程没了强。

烘焙输出逐行进一个 200 行的 ring buffer（给 `/status` 看），同时原样打到服务自己的 tty。

### 镜像与构建缓存的管理

镜像和构建缓存都躺在 docker daemon 的默认存储里（Linux `/var/lib/docker`，Windows 是 WSL 那个 `ext4.vhdx`，Docker Desktop 的设置里能换盘）。服务**不搬运它们**，只保证它们不无限长。要管三样：

| 对象 | 谁产生的 | 怎么定位 | 量级 |
|---|---|---|---|
| 镜像 layer | 烘焙 | `docker image ls --filter label=rust2genshin.play=1` | 首次约 2.5–3.5 GB（apt + 工具链 + registry + 预热的 `target/`）；之后每次烘焙只新增第 4 层 |
| build cache | `docker build` 的中间层 | **没有 label**，只能按容量/时间裁 | 每次烘焙留一份旧的第 4 层，1 GB 上下 |
| 任务目录 | 任务 | `<data>/tasks/` 的 mtime | 每任务几 MB（`.gia` 本身是 KB 级） |

前两样是大头，而且**第二样最容易忘**：`docker rmi` 删掉旧 tag 之后，那次 build 的中间层还在 builder cache 里 —— 不清它，每次 push 都在磁盘上留一份没人引用的旧 `target/`。

三条策略：

1. **镜像：引用计数 + 只认自己的 label。** 服务内维护 `HashMap<Tag, u32>`（任务起跑 +1、容器退出 −1）。`serving` 和"最近一次成功的"永不删（后者是回滚用的），`PLAYGROUND_KEEP_IMAGES`（默认 2）之外的 tag，引用计数归零就 `docker rmi`，**失败忽略**（容器还在跑、被别的 tag 引用），下一轮再试。`docker rmi` 删不掉正在被使用的镜像这个行为本身就是安全网，清理不需要多精确。
2. **build cache：按容量裁，不按次数。** `docker builder prune -f --keep-storage $PLAYGROUND_BUILD_CACHE_MAX`（默认 `10GB`）—— 保留最近用到的层，超预算的丢最旧的。**已知副作用**：builder cache 全局共享、没有 label，这台机器上若还有别的项目在跑 `docker build`，这个 prune 会连它们的缓存一起裁（下次变慢，不会出错）。要彻底隔离得给 playground 单开一个 buildx builder 实例（`--driver docker-container`，自己的缓存存储），代价是产物默认不进本地 image store、每次得 `--load`。MVP 选前者，副作用写进部署文档。
3. **任务目录：TTL。** 完成后 `PLAYGROUND_ARTIFACT_TTL`（默认 1800 秒）删掉整个任务目录。

触发时机：启动时一次、每次烘焙成功后一次、外加一个每 5 分钟的后台 tick（顺带清过期任务目录）。任务容器一律 `--rm`，不留僵尸容器。

**水位兜底**：烘焙之前先看磁盘，可用空间低于 `PLAYGROUND_MIN_FREE_DISK`（默认 `5GB`）就先跑上面两条清理，清完还是低就把这次烘焙判失败（`baking.state = "no_disk"`，日志里写明原因）。否则磁盘满了以后第一次 `docker build` 会失败成一堆看不懂的层写入错误，而不是一句"没空间了"。

`/status` 上报 `disk: {images_bytes, cache_bytes, free_bytes}`（`docker system df` + 取可用空间），让"该清了"在页面上就能看见，不用登机器。

### 脚手架放哪

镜像本体在 daemon 的存储里，放不进 `target/`（上一节）。能放 `target/` 的是可丢弃的那部分——脚手架：

`PLAYGROUND_DATA_DIR` 默认 `<CARGO_MANIFEST_DIR>/../target/playground`：

```
target/playground/
  tasks/<task-id>/{src/lib.rs, out/rust2genshin_demo.gia, build.log, meta.json}
```

`target/` 本来就在 `.gitignore` 里，不用改忽略规则。代价写进文档：**`cargo clean` 会连带删掉这些**，服务在跑时别 clean。服务启动时自建目录，对目录被删的情况容错（每次任务前重建）。

### 任务执行

一次性容器，命令固定：

```sh
cargo build --release --offline -p rust2genshin-demo \
    --target ./genshin-unknown-beyond-server.json \
    -Zunstable-options -Zjson-target-spec -Zbuild-std=core \
  && cp target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia /out/
```

两处挂载（**不挂 `target/`** —— 挂了会盖掉烘培进去的预热缓存）：

| 宿主 | 容器 | 模式 |
|---|---|---|
| `<task>/src/`（里面只有 `lib.rs`） | `/opt/rust2genshin/demo/src` | 只读 |
| `<task>/out/` | `/out` | 读写 |

挂目录而不是挂单个文件：单文件 bind mount 在 Docker Desktop 上（Windows/macOS 的 9p、virtiofs 共享）最容易出怪问题，而 `demo/src/` 里就只有 `lib.rs` 一个文件，挂目录没有副作用。

**每次任务都重新写 `<task>/src/lib.rs`，不要复制文件时保留原 mtime。** cargo 的失效判定是 mtime 比较：如果用户这份文件的 mtime 比镜像里烘焙时那份产物还旧，cargo 会认为 `rust2genshin-demo` 是 fresh、跳过编译，于是容器把**烘焙时那个样例编出的 `.gia`** 当作用户的结果交出去 —— 一个没有任何报错的错答案。新写文件天然让 mtime 是"现在"，比镜像里的产物新，这条就成立了。

用户代码就是 **`demo/src/lib.rs` 的整份内容**：可以 `use rust2genshin_lib::*`、自己写 `#![no_std]` 和 feature 行、加 `#[event_listener]` 入口。`demo/src/lib.rs` 那 220 行就是范例，前端预填它（样例从 `GET /sample` 取，见下）。

`--offline` 是关键：依赖在烘焙时就 fetch 齐了，`Cargo.lock` 也没变，编译不该碰网络 —— 任务容器本来就是 `--network none`，`--offline` 让"万一要联网"变成一个明确的错误而不是一个超时。

日志采集：不shell出去读原始 stdout 再手工 demux 8 字节头（`docker run` 不带 `-t` 时 stdout/stderr 是分帧的：8 字节头 + 载荷，`header[0]` 1=stdout 2=stderr），保留 `stdout`/`stderr` 的区分给前端。被否掉的两个：

- **`docker run -t`**：省了 demux，但 stderr 被并进 stdout（丢掉区分）而且行尾变成 `\r\n`。
- **`bollard`（docker API 的 Rust 客户端）**：类型安全、自带 demux，但是个大依赖；而且 shell 出去的 argv 就是人能照着敲的那条命令，调试时值这个价。

超时：`PLAYGROUND_JOB_TIMEOUT`（默认 **15 秒**）到点 `docker kill`。退出码非 0 就是失败；**编译错误就是普通的 cargo 输出**，原样推给前端，服务自己不去解析，超时是唯一的例外（要能区分"编不过"和"编太慢"，否则用户看到一个没有错误信息的失败）。

15 秒这个数不是拍的：本机原生、依赖全热、`cargo clean -p rust2genshin-demo` 之后完整重编一次是 **4.5 秒**（`target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia` 重新生成）。容器里会比这慢，慢在两处：

- 2 个逻辑 CPU 的额度下 rustc 的并行 codegen 受限；
- **第一次跑（每次烘焙后的第一个任务）** 要把镜像层读进 page cache —— rustc、后端 cdylib、`core.rlib` 加起来几百 MB，磁盘快的话几秒，慢的话十几秒。

所以超时**从容器启动起算**（`docker run` 返回之前不开始计时，启动本身的开销不计入），失败消息里带上已用时，让用户知道"差多少"。验收里量烘焙后的第一个任务和第二个任务的耗时差 —— 如果第一个稳定超时，就在烘焙成功之后加一次预热 `docker run`（把热路径文件读一遍）再宣布 ready，这是 15 秒这个额度唯一的补救措施。

### 队列与限流

- 并发上限 `PLAYGROUND_MAX_JOBS`，默认 **8**，FIFO。
- 排队总数上限 `PLAYGROUND_MAX_QUEUE`，默认 **64**，超出回 `queue_full`。
- **每 IP 同时只允许一个任务**（跑着或排着都算），第二次提交直接回错误 `请求过多，请稍后重试`（NAT 后面多人共用一个出口 IP 会被误伤，认了）。
- IP 来源：`PLAYGROUND_TRUST_PROXY=1` 时取 `X-Forwarded-For` 第一段，否则 socket 的 peer 地址。IPv6 按 **/64** 归并（否则一个 /64 里的地址随便换就绕过了），IPv4 按 /32。
- **每个任务在 `submit` 那一刻就把当时 `serving` 的 tag 写进自己的记录**，起跑时用那一个 tag 起容器。这样"提交时看到的 commit"和"编出产物的 commit"永远是同一个，排队期间跨过重烘焙也不会错位。

`queued` 消息里带 `{position, queue, max_jobs}` 给前端显示排队人数 / 并发数。

### 协议

WS 一条连接，一问一答式的 json 信封，`{"type": ...}`：

| 方向 | 消息 | 载荷 |
|---|---|---|
| 服务→ | `hello` | `{protocol, image_commit, file: "demo/src/lib.rs", max_jobs, max_queue, targets: ["beyond-server"]}` |
| →服务 | `submit` | `{source, target?}` |
| 服务→ | `queued` | `{position, queue, max_jobs}` |
| 服务→ | `started` | `{task, image_commit}` |
| 服务→ | `log` | `{task, stream: "stdout"\|"stderr", text}` |
| 服务→ | `finished` | `{task, ok: true, bytes, download: "/artifact/<id>"}` |
| 服务→ | `failed` | `{task, exit_code?, message}` |
| →服务 | `cancel` | `{}` |
| 服务→ | `error` | `{code, message}`：`not_ready` / `ip_busy` / `queue_full` / `bad_request` / `internal` |

`target` 字段收下了但 MVP 只认 `"beyond-server"`，别的值回 `bad_request`。原因：classic 不是换个 json 就完事 —— `gen_target` 一次只生成一个 json，烘焙阶段得跑两个 bin、留两套 target 目录。等真要用再开。

HTTP：

| 端点 | 用途 |
|---|---|
| `GET /healthz` | `{"image": {"serving": ..., "baking": {...}}, "queue": {...}, "ready": bool}` |
| `GET /status` | 同上 + 烘焙日志尾巴（前端在 `not_ready` 时每 5 秒轮询） |
| `GET /sample` | 从**当前 serving 那个 sha** 对应的源码里读 `demo/src/lib.rs` —— 样例和镜像同源，不会漂 |
| `GET /artifact/<id>` | 产物，带 CORS 和 `Content-Disposition: attachment; filename="rust2genshin_demo.gia"` |
| `POST /admin/rebuild` | 触发重烘焙。两套认证：GitHub webhook（`X-Hub-Signature-256` 的 HMAC-SHA256，还要查 `X-GitHub-Event: push` 和 `ref == refs/heads/main`），或管理用 `Authorization: Bearer $PLAYGROUND_ADMIN_TOKEN`。立即回 202，烘焙在后台 |

`/sample` 直接读工作区里的 `demo/src/lib.rs`（服务不保存每个 sha 的源码快照 —— 那是 clone 方案才有的东西），响应里带上 `head`（工作区 HEAD）和 `baked`（当前 serving 镜像的 sha）。两者不一致时前端提示一句：样例来自比镜像新的源码，可能编不过（典型场景是给 `lib/` 加了 API 但还没重烘焙）。宁可给开发者眼前的版本，也不要给一份编不出来的旧样例。

CORS 走 `tower-http` 的 `CorsLayer`，允许列表 `PLAYGROUND_ALLOWED_ORIGINS`（默认 `https://mzverse.github.io` 加本地开发用的 `http://127.0.0.1:3000`）。**WS 的握手要单独查 `Origin`** —— WebSocket 不受 CORS 保护。

### 前端

`docs/typst/play/index.typ` 换成 `#playground()`，新增 `lib/playground.typ`（按 `custom_element` 声明 `<play-editor>` 的 `<template>`）+ `lib/js/playground.js` + `lib/css/playground.css`（light DOM 侧样式，shadow 内的内联 `<style>` 照旧）。

页面元素：编辑器、后端地址输入框（存 `localStorage`，默认 `ws://127.0.0.1:8787/ws`）、"提交"按钮、"载入样例"（打 `GET /sample`）、日志区、状态区（排队人数/并发数、`not_ready` 时的烘焙日志尾巴）、成功后出现的下载按钮。

- 编辑器 MVP 就是 `<textarea>`：等宽字体、阻止 Tab 跳焦点、行号先不做。
- 部署成 GitHub Pages（https）时 `ws://` 会被混合内容拦，正式部署必须 `wss://` 反代 —— 写进文档，别让下一个人以为是代码坏了。
- `submit` 前先看 `hello.image_commit` 是否为空 / `/status.ready`，不 ready 就别发。

**rust-analyzer 的预留**（落到四处，不是口头承诺）：

1. 协议是带 `type` 的可扩展信封，且**连接长开、服务端主动推**。将来加 `{"type":"lsp", ...}` 双向转发不需要动现有分支 —— 前提是别把它写成"一提交一连接就关"。
2. 前端把「编辑器」和「连接」拆成两个对象：`<play-editor>` 只管拿文本和光标，WS 客户端单独一个对象。将来换 CodeMirror 不动客户端，给客户端接 LSP 不动编辑器。
3. 后端把 `runner`（一次性 job 容器）和 `session`（连接生命周期）分成两个模块。rust-analyzer 要的是**长驻容器 + 双向流**，和现在"跑完就退"的模型是两码事；分开了将来加一层就行，不用拆。
4. `hello` 里报 `file: "demo/src/lib.rs"` —— 将来 LSP 的 `file://` URI 才有对应物。

### 安全边界

烘焙容器（要网络）和任务容器（**没有网络**）是两回事，只有烘焙能碰网络。

任务容器的参数：

| 参数 | 值 | 挡什么 |
|---|---|---|
| `--network none` | | 用户代码联网 |
| `--memory 1536m --memory-swap 1536m` | 写 `1536m` 不写 `1.5g`（docker 的容量解析对小数不友好） | 内存炸弹（proc-macro 在编译期就能跑代码） |
| `--cpus 2` | | 挖矿 |
| `--pids-limit 256` | | fork 炸弹 |
| `--cap-drop ALL --security-opt no-new-privileges` | | 提权 |
| `--user 1001` | | 容器内写系统路径 |
| `--tmpfs /tmp` | | 写临时文件 |
| 超时 kill（15 秒） | | 挂住不退出 |
| 不挂 docker socket | | 逃逸 |

**不用 `--read-only`**：cargo 要往 `target/` 和 `~/.cargo` 写，而这两个正是预热缓存所在，不能改成 tmpfs。靠上面几条兜。

8 并发 × (2 CPU / 1.5 GB) = 峰值 16 逻辑 CPU / 12 GB，**部署时要按机器调 `PLAYGROUND_MAX_JOBS`**，别默认 8 就上小机器。

### 配置

| env | 默认 | 含义 |
|---|---|---|
| `PLAYGROUND_LISTEN` | `127.0.0.1:8787` | HTTP/WS 监听地址 |
| `PLAYGROUND_REPO_DIR` | `CARGO_MANIFEST_DIR/..` | 仓库根（docker context） |
| `PLAYGROUND_DATA_DIR` | `<repo>/target/playground` | 脚手架目录 |
| `PLAYGROUND_REF` | `main` | 烘焙来源 ref；给 sha 就钉死 |
| `PLAYGROUND_REMOTE` | `origin` | `git pull` 用的远端名 |
| `PLAYGROUND_MAX_JOBS` | `8` | 并发任务数 |
| `PLAYGROUND_MAX_QUEUE` | `64` | 排队上限 |
| `PLAYGROUND_JOB_TIMEOUT` | `15` | 单任务秒数（从容器启动起算） |
| `PLAYGROUND_KEEP_IMAGES` | `2` | 保留的镜像数（含 serving） |
| `PLAYGROUND_BUILD_CACHE_MAX` | `10GB` | builder cache 容量上限，超了裁最旧的 |
| `PLAYGROUND_MIN_FREE_DISK` | `5GB` | 烘焙前的水位；低于它先清理，清完还低就拒烘 |
| `PLAYGROUND_ARTIFACT_TTL` | `1800` | 产物保留秒数 |
| `PLAYGROUND_ALLOWED_ORIGINS` | `https://mzverse.github.io,http://127.0.0.1:3000,http://localhost:3000` | CORS + WS Origin |
| `PLAYGROUND_TRUST_PROXY` | `0` | 1 = 从 `X-Forwarded-For` 取 IP |
| `PLAYGROUND_FORCE_REBAKE` | `0` | 1 = 启动时无视已有镜像强制重新烘焙 |
| `PLAYGROUND_IMAGE` | `rust2genshin-play` | 镜像名 |
| `GITHUB_WEBHOOK_SECRET` | 空 | 空则 webhook 端点直接 403（只留 Bearer 那条路） |
| `PLAYGROUND_ADMIN_TOKEN` | 空 | 空则 Bearer 那路不可用 |
| `RUST_LOG` | `info` | tracing 过滤 |

启动时如果本地已有 `PLAYGROUND_REF` 当前 sha 的镜像，**直接 ready、不烘焙**（除非 `PLAYGROUND_FORCE_REBAKE=1`）。这一步只查本地 tag，不查远端 —— 离线也能起；想追新就等 push 或手动 `/admin/rebuild`。

### 进程结构

```
playground/
  Cargo.toml         axum / tokio / tower-http / serde / thiserror / tracing / hmac+sha2+hex / fs4
  Dockerfile         上面那张分层表
  src/
    main.rs          装配：配置 → 探测 → 起服务，后台开始烘焙
    config.rs        env 解析（每个字段一个默认值 + 单测）
    repo.rs          git 调用：rev-parse / status --porcelain / pull --ff-only
    bake.rs          烘焙状态机：单飞、pending 折叠、日志 ring buffer、失败保旧
    docker.rs        docker CLI 封装：build / run / kill / rmi / image ls / builder prune / system df（含 8 字节帧 demux）
    image.rs         镜像 tag 生成、引用计数
    gc.rs            清理：镜像 tag、builder cache、任务目录 + 磁盘水位判定
    job.rs           任务模型与生命周期、产物落盘、TTL 清理
    queue.rs         并发信号量 + 每 IP 一个 + 排队上限（守卫 Drop 时释放）
    runner.rs        trait JobRunner（真实现 DockerRunner，测试用 FakeRunner）
    server/
      mod.rs         router 装配
      http.rs        /healthz /status /sample /artifact /admin/rebuild + CORS
      webhook.rs     HMAC 校验
      ws.rs          WS 握手（含 Origin 检查）+ 协议泵
      protocol.rs    信封消息的 serde 类型
```

依赖方向单向：`server` → `queue` → `job` → `runner`/`image`/`docker`；`bake` 和 `gc` 独立，只被 `main` 和 `server` 触发，`gc` 只用 `docker` + `image`。

## 验收

**单测**（`cargo test -p rust2genshin-playground`，不需要 Docker）：

- `config.rs`：每个 env 的默认值与解析（含坏值的报错）。
- `queue.rs`：并发上限；同一 IP 第二次提交被拒；守卫 Drop 后能再提交；排队上限。
- IP 归一化：IPv4 按 /32、IPv6 按 /64 归并；`X-Forwarded-For` 开了 trust_proxy 才读。
- `webhook.rs`：正确的 HMAC 通过；错签名 / 缺头 / 非 push 事件 / 非 main 分支各自被拒。
- `image.rs`：tag 生成的 sha 缩写与 `-dirty` 后缀；引用计数。
- `gc.rs`：给定 tag 集合 + 引用计数 + 保留数，选出该删的（serving 与"最近成功"永不入选、refcount 非零不入选）；水位判定（够 → 直接烘；低于水位 → 清理后复测；仍低 → `no_disk`）。
- `docker.rs` 的三块纯逻辑：8 字节帧 demux（喂构造好的字节流，含跨 read 边界的分帧）；`docker run` 的 argv 拼装；`docker build` / `builder prune` / `system df` 的 argv 拼装（断言参数表，不启动进程）。
- `protocol.rs`：每种消息的序列化 / 反序列化往返。
- `repo.rs`：`git pull` 失败（脏 / 分叉）不冒泡成错误，而是返回 `Pull::Skipped(原因)`。

**集成**（不需要 Docker）：`FakeRunner` 实现 `JobRunner`，配一个真的 axum 测试客户端，跑通"握手 → 提交 → queued → started → log → finished → 下载"整条链路，以及 `not_ready` / `ip_busy` / `queue_full` 三条错误路径；再加一条超时路径（`FakeRunner` 卡住不返回 → 到点被判失败、消息里带已用时、并发额度被释放）。

**手动端到端**（需要 Docker，标 `#[ignore]` 的一个测试 + 手工步骤）：

1. 起服务，看 `/healthz` 从 `baking` 变 `ready`，`/status` 的日志尾巴是刚才那次 `docker build` 的输出。
2. `cargo run -p rust2genshin-docs --bin watch` 起文档站，打开 `/play/`，提交预填的样例。
3. 日志逐行出现，成功后下载 `.gia`。**再提交一份只改了一行的代码**（比如把 `log(i)` 里的数改成别的），确认两次下载到的 `.gia` 字节不同 —— 这条专门抓"cargo 认为 fresh、把烘焙时那份样例产物发回来了"那个静默错答案。
4. **产物是后端真的编的**：`python tools/gia2txt.py <下载的> --no-unknown` 能解出节点内容。
5. 故意写个语法错误，确认 `failed` 且 cargo 的错误原文出现在日志区。
6. `curl -X POST -H "Authorization: Bearer $TOKEN" /admin/rebuild`，确认重烘焙、新任务用新 sha；期间提交返回 `not_ready` 且能看到烘焙日志。
7. 用 `GITHUB_WEBHOOK_SECRET` 和 `openssl dgst` 造一个真 push 的 payload，确认签名校验通过、202、后台开始烘焙。
8. 同一 IP 连开两个提交，确认第二个回"请求过多，请稍后重试"。
9. **清理真的生效**：连续烘焙两次（改一行注释再触发），确认第一次的 tag 被 `docker rmi`、`docker image ls --filter label=rust2genshin.play=1` 只剩 2 个，`docker system df` 的总量不再单调上涨。
10. **磁盘水位**：临时把 `PLAYGROUND_MIN_FREE_DISK` 设成一个大于剩余空间的值，确认烘焙被拒且 `/status` 给出 `no_disk` 与原因，而不是抛一堆层写入错误。
11. **量耗时**：烘焙刚结束后的第一个任务和紧接着的第二个任务各记一次时间。本机原生热编是 4.5 秒，容器里两次的差就是 page cache 冷的代价；如果第一个稳定超 15 秒，按「任务执行」那节加预热容器。
12. **超时路径**：把 `PLAYGROUND_JOB_TIMEOUT` 临时设成 1，确认回的是"编译超时"（带已用时）而不是一个没有信息的 `failed`，且容器真的被 kill（`docker ps` 里不留）。

## 文档

- 新增站点页 `docs/typst/dev/playground.typ`：怎么跑（`docker` + `git` 是前置）、每个配置项、怎么配 webhook（含本地开发用 `gh webhook forward`）、`wss://` 反代那件事、`cargo clean` 会清掉 `target/playground/`、镜像与 build cache 的磁盘账（含"builder cache 是全局的、prune 会连带裁别的项目"这个副作用）。
- `CLAUDE.md` 的「常用命令」加 `cargo run -p rust2genshin-playground`，「工作区结构」加一行 `playground/`，「文档地图」加那一页。
- `play/index.typ` 页内说明改成"这页在做什么 + 后端地址怎么配"，现在的"这一页还空着"那段删掉。
