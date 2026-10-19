#import "/lib/lib.typ": *;
#let title = [play];
#show: template;

#set raw(lang: "text");

#link("/dev/playground.html")[怎么跑、怎么配、怎么部署] 在开发栏；这里只讲它由什么组成、数据怎么流。

= 全景

三层，每层只依赖下一层的契约：

```text
浏览器（静态页）
   │  HTTP: /sample /status
   │  WebSocket: submit / cancel / lsp_* / 二进制产物帧
   ▼
playground 服务（axum 进程，不含 rustc）
   │  docker build / run / exec
   ▼
docker
   ├─ 镜像：工具链 + 预热依赖 + /opt/ra/ws
   ├─ 任务容器：一次性，挂入源码，跑 cargo build
   └─ RA 容器：常驻，跑 rust-analyzer
```

浏览器*不做任何编译或分析*：语法高亮、诊断、补全全部来自容器里的 `rust-analyzer`；编译产物来自任务容器。

= 组件

== playground 进程

`playground/src/main.rs` 是装配点：探工具 → 读配置 → 抢数据目录锁 → 构造下面这些组件 → 起三个后台循环（烘焙、GC、RA 补池/回收）→ 起 HTTP。

#table(
    columns: (auto, 1fr),
    inset: 6pt,
    fill: (_, y) => if calc.odd(y) { luma(235) },
    [*模块*], [*职责*],

    [`config`], [全部环境变量与默认值；空串等价于未设置。],
    [`server::http`], [路由、CORS、`/healthz` `/status` `/sample` `/admin/rebuild`；`AppState` 在这里定义。],
    [`server::ws`], [WebSocket 连接的生命周期：握手校验、消息分发、把任务输出转成 `ServerMsg`。],
    [`protocol`], [线协议的全部枚举与常量；`TARGETS` 是目标的唯一真源。],
    [`queue`], [并发额度 + 每 IP 一个 + 排队上限；额度靠 `Ticket` 的 `Drop` 归还。],
    [`job`], [任务目录布局 `<data>/tasks/<id>/{src/lib.rs, meta.json}`；每次新写源码。],
    [`runner`], [把 `cargo build` 放进一次性容器；分帧读日志、超时、判 `verdict`。],
    [`bake`], [烘焙状态机；单飞、折叠 pending、失败保旧。],
    [`image_builder`], [`docker build` 的参数与日志；tag/label 与镜像对齐。],
    [`image`], [镜像引用计数（`ImageGuard`），GC 据此避开正在用的镜像。],
    [`gc` / `gc_loop`], [磁盘水位闸、镜像与 builder cache 清理、过期任务目录。],
    [`repo`], [`git rev-parse` / `git pull --ff-only`；webhook 路径用。],
    [`lsp::*`], [RA 会话池、LSP 帧、JSON-RPC 分流、语义 token 与诊断。],
)

== 浏览器

#table(
    columns: (auto, 1fr),
    inset: 6pt,
    fill: (_, y) => if calc.odd(y) { luma(235) },
    [*文件*], [*职责*],

    [`play/index.typ`], [页面骨架：工具条、编辑器挂载点、右栏选项卡与日志面板。],
    [`play/script.js`], [连后端、发 `submit`、收消息、UI 状态（目标选择、日志、拖动条、样例载入、日志跳转）。],
    [`play/lsp-client.js`], [CodeMirror 与 LSP 的薄桥：语义高亮、诊断波浪线、补全弹窗、去抖与在途替换。],
    [`play/style.css`], [布局与主题变量；控件尺寸统一走 `--sl-input-height-medium`。],
    [`lib/shoelace.typ`], [`<sl-select>` / `<sl-button>` / `<sl-divider>` 的封装与按需注册。],
)

= 协议

== WebSocket

一条连接*长开*，同时最多跑一个编译任务；断开即取消。

#table(
    columns: (auto, auto, 1fr),
    inset: 6pt,
    fill: (_, y) => if calc.odd(y) { luma(235) },
    [*方向*], [*消息*], [*含义*],

    [客户端 → 服务端], [`submit`], [`{source, target?}`；开始一次编译。],
    [客户端 → 服务端], [`cancel`], [取消这一单；服务端回 `cancelled`。],
    [客户端 → 服务端], [`lsp_open`], [取一个 RA 会话；幂等。],
    [客户端 → 服务端], [`lsp_change`], [`{source}`；写盘 + `didChange` 全文替换。],
    [客户端 → 服务端], [`lsp_completion`], [`{request_id, line, column}`；0 基、UTF-16 列。],
    [服务端 → 客户端], [`hello`], [连接后第一条：协议版本、镜像 commit、目标表、`lsp_supported`。],
    [服务端 → 客户端], [`queued`], [排队位次与当前队列长度。],
    [服务端 → 客户端], [`started`], [容器开始跑；带 `task` 与 `image_commit`。],
    [服务端 → 客户端], [`log`], [`{task, stream, text}`；stdout / stderr 分开。],
    [服务端 → 客户端], [`finished`], [成功；带产物字节数。产物*紧跟其后*以二进制帧发过来。],
    [服务端 → 客户端], [`failed`], [失败；`exit_code` 可空，`message` 是给用户看的。],
    [服务端 → 客户端], [`cancelled`], [`abort` 本身静默，必须显式回这一条。],
    [服务端 → 客户端], [`error`], [请求被拒：`not_ready` / `ip_busy` / `queue_full` / `bad_request` / `internal`。],
    [服务端 → 客户端], [`lsp_ready` / `lsp_tokens` / `lsp_diagnostics` / `lsp_completion`], [RA 就绪、语义 token、*整份*诊断、补全结果。],
)

两条不变量：

- *一条连接一个任务*：`cancel` 才明确取消谁。任务跑在 spawn 出去的任务里，排队期间读循环不被阻塞。
- *断开 = 取消*：任务 `abort`，`Ticket` / `ImageGuard` 随之 `Drop`，额度、IP 占用、镜像引用计数一步还清。

== HTTP

#table(
    columns: (auto, 1fr),
    inset: 6pt,
    fill: (_, y) => if calc.odd(y) { luma(235) },
    [*路由*], [*用途*],

    [`/healthz`], [烘焙就绪返回 200，否则 503 并带日志尾巴。],
    [`/status`], [serving 镜像、烘焙状态、队列、git HEAD、pull 失败原因。],
    [`/sample`], [仓库里的 `demo/src/lib.rs`；响应头带 HEAD 与烘焙 commit。],
    [`/admin/rebuild`], [触发烘焙：GitHub webhook（HMAC）或 Bearer token；都没配就 403。],
    [`/ws`], [升级为 WebSocket；Origin 白名单在这里查。],
)

= 一次编译

```text
submit
  → validate（空 / 超 1 MiB / 未知 target 都在占额度之前拒掉）
  → queue.try_acquire(ip)          # 每 IP 一个；满了回 ip_busy / queue_full
  → registry.acquire(serving_tag)  # 先握住镜像引用，防 GC 中途删掉
  → 排队则回 queued，spawn 的任务等 ticket.wait_started()
  → 写 tasks/<id>/src/lib.rs       # 新写文件，mtime 必须是“现在”
  → docker run 一次性容器           # 无 --rm，挂 src 只读
  → 读 stdout 8 字节帧 / stderr 裸行 → log
  → docker cp <容器>:<路径> -      # tar 走 stdout，产物**不落宿主盘**
  → verdict(exit_code, artifact)   # 退出码 0 且产物取到了才算成功
  → finished + 二进制产物帧 | failed | cancelled
```

两个关键判断：

- *退出码 0 但没产物也是失败*。cargo 判 fresh、跳过编译时，容器会交出上一次的 `.gia` —— 那是没有报错的错答案。
- *产物不落宿主盘*。容器*故意不带 `--rm`*（否则 `docker cp` 之前文件系统就被删了），跑完用 `docker cp <容器>:<路径> -` 把 tar 从 stdout 读回来解出字节，直接推给前端。清理由 `ContainerGuard` 的 `docker rm -f` 负责。
- *判定顺序是超时 → 解码 → verdict*。被 kill 的容器会在帧中间截断，先判解码会把正常超时误报成“读输出失败”。

容器离开正常路径时（用户取消、进程被杀）由 `ContainerGuard::Drop` 补一发 `docker rm -f` —— 任务容器不带 `--rm`，一个死循环容器否则会一直占住 CPU 和内存，还会留下一个容器条目。

= 镜像生命周期

```text
启动 → startup_plan
        ├─ 本机有带 commit label 的镜像 → adopt，不烘
        └─ 没有（或 FORCE_REBAKE）      → request 烘焙

Baker: Idle ─bake→ Baking ─ok→ Ready(serving)
                       └─fail→ Ready(旧 serving) + baking.state = failed
```

*镜像身份由两个值组成*：tag 是 `镜像:短sha[-dirty]`，label 是*全长 sha*。GC 按 label 找，启动复用按 label 读 commit，二者必须同时对上才能算同一份镜像。

几条设计约束：

- *单飞 + 折叠 pending*：同一时刻只烘一份；烘焙中来的请求只留最后一个。
- *失败保旧*：main 推了个编不过的 commit，服务继续用旧镜像，`/status` 里能看到失败状态与日志尾巴。
- *服务进程与镜像里的后端是两份东西*：重启不按 HEAD 重烘，实际服务的是哪一版由 `hello.image_commit` 如实报出。
- *GC 每 300 秒一轮*：任务目录按 TTL 删；镜像保留 `keep` 个，`serving`、最近一次成功、引用计数非零的永不删；builder cache 按容量裁。
- *烘焙前量水位*，量的是 docker 的 data root（镜像与 cache 长在那里），不是数据目录。

= RA 会话

```text
浏览器编辑器
   │ lsp_change / lsp_completion（经 playground WebSocket）
   ▼
server::ws ──▶ LspService ──▶ Pool ──▶ Session ──▶ docker exec ──▶ rust-analyzer
   ▲               │             │          │
   └── lsp_tokens / lsp_diagnostics / lsp_completion ── Event ◀──┘
```

*镜像里另烘一份分析工作区* `/opt/ra/ws`：只有 `demo` / `lib` / `lib-internal`，没有 `core`。`core` 依赖 `rustc_private`，RA 解析它要把整个编译器 sysroot 拖进来，冷启动从 11 秒涨到 84 秒。

*会话创建五步，顺序不能乱*：拿 serving tag → `docker run -d` 待命容器 → `docker exec -i rust-analyzer` → `initialize` → *先 `subscribe` 再 `didOpen(占位)`* → 收到第一条诊断或 token 才算就绪。先订阅是为了不漏掉 RA 立刻推来的消息；占位内容让用户源码可以用 `didChange` 全文替换（同一 URI 重复 `didOpen` 是协议违规）。

*池只做状态机*：空闲数不超过下限（默认 1）、取用时用会话自己的探针查活、坏的就销毁重造、空闲超 TTL 由回收循环 `docker rm -f`。

*诊断是拉取式的*：RA 1.10x 只在 `didOpen` / `didChange` 后由服务端主动拉 `textDocument/diagnostic`，`publishDiagnostics` 基本不再推送；`workspace/diagnostic/refresh` 到达时也要再拉。RA 对没变的内容只回 `unchanged`，所以会话缓存最近一份完整集合用于回放。

*语义 token 的索引由服务端翻译*：RA 不按客户端上报的 legend 编码，而是按自己那份的字母序发索引；`Session` 在 `initialize` 后按*名字*把 RA 的下标翻成 `TOKEN_TYPES` 的下标再转发，对不上的名字编码成越界哨兵，前端跳过。

= 前端

*后端地址不归页面管*：`BACKENDS` 写死在 `script.js`，每次加载打乱、逐个尝试（4 秒超时），连上就停。页面里不给看也不给改。

*连接不是常开的*：页面把「用户操作」放宽到整页（`keydown` / `pointerdown` / `wheel`，加上每次发消息），90 秒没有任何操作就主动关掉 WebSocket。服务端把断开当成取消并把 RA 会话还回池子，一个没人看的页面不该一直占着几百 MB 的分析进程；空闲关闭显示中性提示，不标红。排队或编译中的连接不在此列：任务收摊（`finished` / `failed` / `cancelled` / `error`）之前计时只会顺延，否则用户挂着页面等编译，会被自己这端取消掉。下次打字或点「编译」走既有的自动重连，`hello` 重建 LSP 会话并整份同步文档。

*编辑器只借 CodeMirror 的词法层*：`@codemirror/lang-rust` 提供括号匹配等结构信息，*不装* `syntaxHighlighting`；颜色全部来自服务端语义 token。缩进是自写的 `indentService`（`lang-rust` 没有提供任何缩进规则）。

Tab / Shift+Tab 在 `script.js` 里自己写，没用 `indentWithTab`：那个包会拖进第二份 `@codemirror/view`，facet 值认不出来。有选区时 Tab 给选区覆盖的每一行加一个 `indentUnit`（空行跳过），Shift+Tab 从行首削掉一个单位；选区终点落在下一行行首时那一行不算。缩进单位走 `languageDataAt("indentUnit", pos)`，退化的情形（页面里不会发生）才回落到常量 `INDENT`。派发事务时*不给* `selection`：不给的话 CodeMirror 默认把选区跟着 `changes` 映射，正是这里要的；写成 `{ ranges, map: true }` 反而过不了校验 —— esm.sh 拉回来的 `@codemirror/state` 与 view 内那份不是同一个实例。

*高亮与诊断是两个独立 decoration 层*，可以叠在同一 range 上：一个字既是函数名又是错误位置时，两套 class 同时生效。诊断整份替换，空数组就是清空。

*重连不擦高亮*：新会话在收到 `lsp_ready` 之前转发的 token 帧一律丢弃 —— 那是 RA 对占位源码（或池里上一份文档）的分析结果，套到当前文档上会把已有高亮整片替换掉。`lsp_ready` 之后前端立刻整份同步文档，此后的 token 才是针对用户代码的。

*产物只在两头的内存里*：后端用一条二进制帧（裸 `.gia` 字节，socket 上设了 `binaryType = "arraybuffer"`）把它送过来，前端存进 `artifactBytes`，「下载」按钮做的事是 `Blob` + 临时 `<a download>`。没有下载链接、也没有第二次 HTTP 往返。点「编译」会把上一份产物*立刻作废* —— 留着它，一次编译失败之后用户点下载拿到的还是上一次的图，而界面上看不出来。预览面板空着时用 `.play-preview:empty::before` 画一个居中的「待编译」，收到产物后换成居中的文件名与体积：`:empty` 既是状态也是内容，没有占位元素需要记得删。

*选区与光标由 CodeMirror 自绘*（`drawSelection()`）：浏览器原生的光标在选区非空时会被隐藏，拖动或 Shift 选中后看不到落点；自绘层则在选区头部始终画一根光标。

自绘选区量的却是*字形盒*而不是行盒（12px 字约 14px 高，行高为 18px）：单行选中时上下各缩进 2px，跨行时 CM 又会把接缝那一侧补到行边界，同一个选区两侧不对称。`script.js` 的 `snapSelectionToLine` 在 CM 每次画完后把矩形上下缘取整到等距的行网格上，行与行严丝合缝、也不越过行边界；选区跨多行时中间那块大矩形同样落在网格上，不会被压成一行。选区色因此用不透明混色，避免亚像素重叠处出现深色接缝。

*补全不排队*：300 ms 去抖、2 秒兜底、在途替换 —— 旧请求回来时直接丢弃，否则会和下一次按键撞上，弹窗列出错位的内容。

*补全行里只有名字是可插入文本*。RA 把展示信息塞在两个地方：`label` 末尾（常量的 `MAX = 2147483647`、旧服务端的 `(as Trait)`）和 `detail`（类型、泛型参数、宏名）。前者在展示和插入时都剥掉，其中常量值紧跟名字显示（`MAX = 2147483647`），后者按 kind 归一成类型靠右显示；与名字重复的 `detail`（枚举成员 `A` / `A`、原生类型 `i32` / `i32`）和只是复述名字的 `macro_rules! x` 直接丢掉。

*编译日志里的源码位置可点*：`script.js` 从日志文本里识别 `--> 路径:行:列`，只把路径本身包成按钮；点击后仅当路径以 `demo/src/lib.rs` 结尾才把编辑器光标移到对应行列并聚焦，其余给一行提示。
