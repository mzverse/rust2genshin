# 烘焙镜像:一份装好了工具链、预热了依赖、并且**已经编过一次 demo** 的
# 仓库。任务容器就是从这个镜像起的,所以"编一个任务"实际只编用户那一份。
#
# 三层是算过的,不是随手切的:
#   1 层 debian + apt        —— 一年变一次
#   2 层 rustup + 工具链      —— 跟着 rust-toolchain.toml 变
#   3 层 COPY . . + 编译      —— 每次 push 都失效,这一层省不掉
#
# 第 3 层不要再往下切。试过"只 COPY 各成员的清单先 cargo fetch":workspace
# 成员没有 src/ 时 cargo 直接报 `no targets specified in the manifest`,
# 得给每个成员逐个摆占位文件才骗得过去,于是每加一个成员就多一处会漂的
# 清单。而 COPY . . 每次 push 都失效,那一层省下的只有下载、省不下编译。

FROM debian:bookworm-slim AS base
# protobuf-compiler 是 core/build.rs 里 prost-build 要的;
# pkg-config / libssl-dev 是 cargo 自己的依赖拖进来的,
# ca-certificates 是 rustup 下载工具链要的。
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates curl git protobuf-compiler build-essential pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

FROM base
# 可选的镜像源。默认空 = 官方源;部署方在国内时由服务端通过
# `--build-arg` 传进来(见 `image_builder::build_args` 和
# `PLAYGROUND_RUSTUP_DIST_SERVER` 等环境变量)。不写死在这里,是因为
# 克隆这个仓库的人不一定在国内。
ARG RUSTUP_DIST_SERVER=
ARG RUSTUP_UPDATE_ROOT=
ARG CARGO_MIRROR=
# uid 1001 是任务容器 `--user 1001` 指的那个。宿主上的 out/ 由服务端
# chmod 0777,容器才能把产物写出去。
# /opt/rust2genshin 也在这里一并建好并 chown:下面 `COPY --chown` 只改
# 拷进去的文件,不改目标目录本身的属主 —— 留成 root:root(755)的话,
# 最后 `cargo run -p build-demo` 以 builder 在里面建 target/ 会 EACCES。
# /opt/ra 同理,而且**必须由 root 建**:`/opt` 在 debian:bookworm-slim 里
# 是 root:root 755,`USER builder` 之后 `mkdir -p /opt/ra` 会
# Permission denied、镜像直接构建失败。
RUN useradd -u 1001 -m -s /bin/bash builder \
 && mkdir -p /opt/rust2genshin /opt/ra \
 && chown builder:builder /opt/rust2genshin /opt/ra
USER builder
# 工具链跟着 rust-toolchain.toml 走 —— 它装在 builder 的家目录里,
# 所以必须在 USER builder 之后跑。
# WORKDIR 也必须落在 rustup 那条 RUN 之前:`rustup show` 靠 CWD 找
# `rust-toolchain.toml`(CWD 是 `/` 时看不到文件,工具链会推迟到第 3 层
# 的 cargo fetch 才靠 shim 惰性装)。它的值要和 `docker.rs` 里
# `RunSpec.workdir` 的默认值保持一致。
WORKDIR /opt/rust2genshin
COPY --chown=builder:builder rust-toolchain.toml /opt/rust2genshin/rust-toolchain.toml
# RUSTUP_DIST_SERVER / RUSTUP_UPDATE_ROOT 是 rustup 自己认的环境变量,
# 空串等于不设(回落到 static.rust-lang.org)。
RUN export RUSTUP_DIST_SERVER="${RUSTUP_DIST_SERVER:-}" \
           RUSTUP_UPDATE_ROOT="${RUSTUP_UPDATE_ROOT:-}" \
 && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain none --profile minimal \
 && . "$HOME/.cargo/env" \
 && rustup show
# 任务容器的命令是裸 `sh -c "cargo build …"`(见 `docker.rs`),非登录
# shell 既 source 不了 `$HOME/.cargo/env` 也不读 rc 文件 —— cargo 能不能
# 被找到全靠这行,它对后面所有层和**运行时**都生效。
ENV PATH=/home/builder/.cargo/bin:$PATH

# crates.io 镜像(可选)。cargo 没有「索引镜像」的环境变量,只能写
# source replacement;空值时不写文件,行为和不加这段完全一样。
RUN if [ -n "$CARGO_MIRROR" ]; then \
      mkdir -p "$HOME/.cargo" \
      && printf '[source.crates-io]\nreplace-with = "mirror"\n\n[source.mirror]\nregistry = "sparse+%s"\n' "$CARGO_MIRROR" \
           > "$HOME/.cargo/config.toml"; \
    fi

COPY --chown=builder:builder . .
# fetch 单独留一条:它让依赖下载和编译在日志里分得开,出问题时一眼看出
# 是网络慢还是编译慢。
RUN . "$HOME/.cargo/env" && cargo fetch
# 烘一次 demo。这一遍把一百多个 crate 都编进 target/,任务容器自己不挂
# target/,用的就是这个缓存 —— 这是"15 秒能编完一份节点图"的全部原因。
RUN . "$HOME/.cargo/env" && cargo run -p build-demo --bin beyond-server
# 经典目标也要烘。两个 target spec 的 triple 不同,cargo 在 target/ 下按
# triple 分目录 —— 经典模式无法复用超限模式的依赖缓存,不烘的话第一次
# 选经典要冷编译整棵依赖树,必然撞上任务超时。
RUN . "$HOME/.cargo/env" && cargo run -p build-demo --bin classic-server

# rust-analyzer 组件。RA 容器与任务容器用同一个镜像 —— 见计划里 Task 8 的说明,
# 不做第二个 tag(会翻倍镜像数,撞上 gc 的 keep_images)。
RUN . "$HOME/.cargo/env" && rustup component add rust-analyzer

# RA 的分析工作区。**只放 demo / lib / lib-internal 三个成员**:core 是
# rustc_private 后端,RA 为解析它要把整个编译器 sysroot 的元数据拉进来,
# 冷启动从 11s 变成 84s,而 demo 的依赖链上根本没有 core。
#
# 手写一份精简 Cargo.toml 会漏掉 workspace.package.* 的继承项,cargo metadata
# 会静默失败 —— 所以是复制根清单再改 members,不是新写一份。
RUN mkdir -p /opt/ra/ws \
 && cp rust-toolchain.toml Cargo.toml Cargo.lock /opt/ra/ws/ \
 && cp -r demo lib lib-internal /opt/ra/ws/ \
 && sed -i -e '/^members/,/^]/c\members = ["demo", "lib", "lib-internal"]' \
          -e '/^default-members/,/^]/c\default-members = ["demo", "lib", "lib-internal"]' \
          /opt/ra/ws/Cargo.toml \
 && cd /opt/ra/ws \
 && . "$HOME/.cargo/env" \
 && cargo metadata --format-version 1 --offline > /dev/null

# 任务容器从这一层起。镜像自己的 CMD 不会被用到(服务端总是显式给
# `sh -c "..."`),留着只为了 `docker run <image>` 手工排查时能进去看看。
CMD ["/bin/bash"]
