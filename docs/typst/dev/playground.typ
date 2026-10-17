#import "/lib/lib.typ": *;
#let title = [playground];
#show: template;

#set raw(lang: "rust");

`rust2genshin-playground` 是文档站#link("/play/index.html")[「摆弄」页]的后端：一个独立的 web 服务，接受一份 Rust 源码，排队、在*一次性 docker 容器*里跑 `cargo build`，把日志实时推回浏览器，成功后给出可下载的 `.gia`。

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
    [`PLAYGROUND_JOB_MEMORY`], [`1536m`], [任务容器的内存限额，同时给 `--memory` 和 `--memory-swap`。设成 `none` 则两个都不加 —— cgroup v1 不支持 `kmem.limit_in_bytes` 时，runc 建容器就会炸（`failed to write 1 to memory.kmem.limit_in_bytes`）。别写 `1.5g`，docker 对小数容量不友好。],
    [`PLAYGROUND_KEEP_IMAGES`], [`2`], [镜像 GC 保留几个最近在用的 tag。],
    [`PLAYGROUND_BUILD_CACHE_MAX`], [`10GB`], [`docker builder prune --keep-storage` 的值 —— 注意是*留到*多少而不是*删到*多少（见「磁盘」）。支持 `10GB` / `512m` / `1024`，倍数一律 1024 进制。],
    [`PLAYGROUND_MIN_FREE_DISK`], [`5GB`], [磁盘水位：低于这个值就不再接新任务。量的是 docker 的 data root（镜像层和 builder cache 都在那），不是数据目录。],
    [`PLAYGROUND_ARTIFACT_TTL`], [`1800`], [产物的存活时间，*秒*（默认半小时）。过期由后台 GC 清掉。],
    [`PLAYGROUND_ALLOWED_ORIGINS`], [`https://mzverse.github.io,http://127.0.0.1:3000,http://localhost:3000`], [WebSocket 握手时校验的 Origin 白名单，逗号分隔。WebSocket 不受 CORS 保护，这句话就是全部防线（`playground/src/server/mod.rs` 的 `origin_allowed`）。],
    [`PLAYGROUND_TRUST_PROXY`], [`false`], [*只在后端前面确实挂着一个反代时才设 `1`。* 为真时才信 `X-Forwarded-For` 头来决定客户端 IP；否则任何客户端都能伪造这个头，绕过「每 IP 一个任务」的限制（`playground/src/server/mod.rs:19-34` 的 `client_ip`）。],
    [`PLAYGROUND_FORCE_REBAKE`], [`false`], [启动时忽略本地现成的镜像，强制重烘一遍。],
    [`PLAYGROUND_IMAGE`], [`rust2genshin-play`], [烘出来的镜像名（tag 里会再带 short sha 和 `-dirty`）。],
    [`PLAYGROUND_ADMIN_TOKEN`], [无], [`/admin/rebuild` 的运维手工触发凭证（`Authorization: Bearer <token>`）。与 `GITHUB_WEBHOOK_SECRET` 一起决定这条路由能不能被调用 —— 两个都没配就是 403，不做「没配就放行」。],
    [`GITHUB_WEBHOOK_SECRET`], [无], [校验 GitHub webhook 的 HMAC 签名（`X-Hub-Signature-256`）。和上一个一样，未设则不接受这条路。],
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

- *任务目录* —— `target/playground/` 下是每个任务的工作目录，按 `PLAYGROUND_ARTIFACT_TTL` 过期清掉。
- *镜像和 builder cache* —— 走 docker 自己的 data root（不是项目里的 `target/`）。镜像按 label 清，只认本服务烘的那些，*绝不碰宿主上别人的镜像*。

一条要紧的副作用：`docker builder prune` 是*全局*的 —— builder cache 没有 label、在这台机器上被所有项目共享，所以那一下清的是*所有* builder 的缓存，不只是我们的（`playground/src/docker.rs:166-169`）。还有 `PLAYGROUND_BUILD_CACHE_MAX` 传的是 `--keep-storage`，语义是*留到*多少，超预算的丢最旧的。

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

另外这台宿主上 cgroup 的 cpu 与 kmem 控制器都不可用，`PLAYGROUND_JOB_CPUS=0` 和 `PLAYGROUND_JOB_MEMORY=none` 是必须的，见上面的配置表。
