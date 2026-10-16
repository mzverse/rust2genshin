# 摆弄场（playground）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在文档站「摆弄」分区里做一个网页：写一份 `demo/src/lib.rs`，提交，排队在一次性 docker 容器里编成 `.gia`，实时看日志，成功后下载产物；服务自己从本地仓库烘焙镜像，push 到 `main` 触发重烘焙。

**Architecture:** 新 crate `playground/`（bin，不在 `default-members`）。一个进程，两条腿：`bake` 把本地仓库打成 `rust2genshin-play:<sha>` 镜像（单飞、pending 折叠、失败保旧），`server`（axum）承接任务、按每 IP 一个的规则排队、每次 `docker run` 起一个 `--rm` 的一次性容器跑固定那条 `cargo build`，把 stdout/stderr 按 8 字节帧解复用后经 WebSocket 推给前端。前端是文档站里的一个 `custom_element`（`<play-editor>`），纯 `textarea` + WebSocket 客户端。

**Tech Stack:** Rust（edition 2024，workspace 内新成员）、axum 0.8（ws）、tokio、tower-http（CORS）、serde/serde_json、thiserror、tracing、hmac+sha2+hex；docker CLI（shell 出去）；前端是 Typst 导出的静态 HTML + 原生 JS/CSS。

**Spec:** `docs/superpowers/specs/2026-10-16-playground-design.md`

## Global Constraints

- **MVP 只做**：单文件 `demo/src/lib.rs`、只支持 `target = "beyond-server"`、预览不做（只给下载）、rust-analyzer 只留接口。不做多文件 / 自定义 `Cargo.toml` / 账号 / 持久化 / classic-server。
- **任务容器固定参数**（逐字照抄，别改写）：`--network none`、`--memory 1536m --memory-swap 1536m`（**写 `1536m`，不写 `1.5g`**）、`--cpus 2`、`--pids-limit 256`、`--cap-drop ALL`、`--security-opt no-new-privileges`、`--user 1001`、`--tmpfs /tmp`、`--rm`、不挂 docker socket、**不加 `--read-only`**。
- **不挂 `target/`**：任务容器只挂 `<task>/src`（只读）到 `/opt/rust2genshin/demo/src`、`<task>/out`（读写）到 `/out`。
- 上限默认值：`max_jobs = 8`、`max_queue = 64`、`job_timeout = 15s`（**从容器启动起算**）、`keep_images = 2`、`artifact_ttl = 1800s`、`build_cache_max = 10GB`、`min_free_disk = 5GB`。
- 同一 IP 同时只允许一个任务（跑着或排着都算），第二次提交的错误消息**逐字**是 `请求过多，请稍后重试`（`error.code = "ip_busy"`）。
- 镜像 tag 用 **sha 不用 `latest`**：`<image>:<short-sha>`，工作区脏则加 `-dirty` 后缀。标签固定为 `rust2genshin.play=1` 和 `rust2genshin.commit=<sha>`；清理**只认** `rust2genshin.play=1`。
- push 的目标分支是 `refs/heads/main`；`PLAYGROUND_REF` 是 sha（`is_sha_like`）时跳过 `git pull`。
- 前端在 https（GitHub Pages）下必须走 `wss://`，MVP 不实现它，只在文档里写清楚。
- `docs/` 和 `viewer/` **不能**进 `.dockerignore`（workspace 成员）。
- 注释、提交信息用中文，与仓库其余部分一致；对外错误消息也用中文。
- 所有文件操作使用完整 Windows 绝对路径（`C:\projects\rust2genshin\...`）。
- 新 crate 加进 `[workspace] members`，**不加进 `default-members`**（与 `viewer` 同理，让根目录的 `cargo build` / `cargo test` 绕开它）。
- 开发机（这台 Windows）**没有 Docker**。所有需要 docker 的测试标 `#[ignore]` 或留作手工端到端步骤；`cargo test -p rust2genshin-playground` 在无 docker 机器上必须全绿。

### 部署机的既定事实（2026-10-16 实地侦察，不是猜的）

实体机 `root@server.local`，仓库放 `/mnt/server/rust2genshin`。这台机的几处**和通用假设不一样**，实现必须照这里写：

- **容器没有出网**。`/etc/docker/daemon.json` 里 `"iptables": false`，dockerd 不给 docker0 加 NAT，容器里 DNS 直接解析不了（实测 `wget: bad address`）。镜像能拉是因为那是守护进程自己在拉。**所以烘焙的 `docker build` 必须加 `--network=host`**（走宿主网络命名空间，不需要 NAT）。任务容器照旧 `--network none`，不受影响 —— 这和 spec 里「只有烘焙能碰网络」是同一件事。**不要**去把 `iptables` 改回 true，那台机的网络是另有用意配的。
- **docker 19.03.8，没有 `buildx`**。构建固定走 legacy builder：spawn `docker` 时显式 `.env("DOCKER_BUILDKIT", "0")`。`--progress=plain` 是 BuildKit 才认识的旗标，legacy 下没有意义，**不要加**（legacy 本身不刷 tty 进度条，它打的是 `Step k/n`）。Dockerfile 也必须只用 legacy 认得的东西：没有 heredoc、没有 `# syntax=`、没有 `RUN --mount=`。
- **docker 的 data-root 不在 `/` 上**。`/` 只有 32G（清完日志后 11G 空），而 `/mnt/server` 有 9.5T。所以在这台机上 data-root 被挪到 `/mnt/server/docker`，是一个 loop 挂上来的 256G ext4 稀疏镜像（`/mnt/server/docker.img`，fstab 里有 `x-systemd.requires-mounts-for=/mnt/server`），保住 overlay2。**推论：磁盘水位必须量 docker 的 data root，而不是数据目录** —— 数据目录在 /mnt/server 上有 9.5T，量它等于没有水位（见 Task 5 / Task 14）。
- **`PLAYGROUND_MAX_JOBS` 在这台机上取 4**，不是 8。15G 内存里常驻已用 6.3G，可用只有 8.3G；8 × 1.5G = 12G 装不下，而且烘焙本身（编整个后端 cdylib）很吃内存。4 × 1.5G = 6G，留够余量。
- 这台机上 `uid 1001` 是`jihua`，与容器 `--user 1001` 无关（容器内是纯数字 uid）；产物靠 `<task>/out` 的 0777 写出去，btrfs 支持 POSIX 权限位，没问题。

## Review Focus

spec 是愿景文档，没写到的输入不等于可以坏。以下五类最可能咬到用的人，每一类都在下面标了它归哪个任务：

1. **源码为空 / 全是空白**：用户点提交时编辑器是空的。期望是一句明确的 `bad_request`，而不是起一个容器、10 秒后回一句 cargo 的 `no targets specified`。（Task 12）
2. **退出码 0 但 `/out/*.gia` 不存在**：cargo 认为 demo 是 fresh、或用户代码把产物写到别处时会出现。期望判失败，绝不能回 `finished` 指一个不存在的下载。（Task 9）
3. **连接在任务跑着时断开**：用户关标签页 / 刷新。期望任务被取消（容器 kill）、并发额度和 IP 名额都释放，而不是泄漏到服务重启。（Task 13）
4. **`/artifact/<id>` 的 id 被构造成路径**（`..%2f..%2fCargo.toml`）：期望 404，任何情况下都不读 `tasks/` 之外的文件。（Task 12）
5. **超大源码**：贴进来几 MB。期望超上限直接 `bad_request`，服务内存不跟着涨。（Task 12）

---

### Task 1: crate 骨架与配置解析

**Files:**
- Modify: `C:\projects\rust2genshin\Cargo.toml`（members 加 `"playground"`，`default-members` 不动）
- Create: `C:\projects\rust2genshin\playground\Cargo.toml`
- Create: `C:\projects\rust2genshin\playground\src\main.rs`
- Create: `C:\projects\rust2genshin\playground\src\config.rs`

**Interfaces:**
- Consumes: 无
- Produces:
  - `config::Config`（字段见下），`config::ConfigError`，`config::resolve(get: &dyn Fn(&str) -> Option<String>, manifest_dir: &Path) -> Result<Config, ConfigError>`，`config::load() -> Result<Config, ConfigError>`，`config::parse_size(&str) -> Result<u64, String>`
  - `main.rs` 里列出的模块清单（后续任务往里加）

- [ ] **Step 1: 加 workspace 成员**

`C:\projects\rust2genshin\Cargo.toml`，`members` 数组末尾加一行（`viewer` 之后）：

```toml
    "playground",
```

注释块保持原位不动，另在 `default-members` 之后补一句注释：

```toml
# playground 是独立 web 服务(依赖 axum/tokio),不进 default-members:
# 根目录的 `cargo build` / `cargo test` 不该去编它。
# 需要时显式:`cargo build -p rust2genshin-playground`。
```

- [ ] **Step 2: 建 crate 骨架**

`C:\projects\rust2genshin\playground\Cargo.toml`：

```toml
[package]
name = "rust2genshin-playground"
version.workspace = true
edition.workspace = true
authors.workspace = true

[[bin]]
name = "rust2genshin-playground"
path = "src/main.rs"

[lints]
workspace = true

[dependencies]
async-trait = "0"
axum = { version = "0", features = ["ws"] }
fs4 = "0"
hex = "0"
hmac = "0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0"
thiserror = "0"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0", features = ["cors"] }
tracing = "0"
tracing-subscriber = { version = "0", features = ["env-filter"] }

[dev-dependencies]
tempfile = "3"
```

`C:\projects\rust2genshin\playground\src\main.rs`（本步先只有声明，`main` 之后的任务再填）：

```rust
//! 摆弄场的后端服务。结构和取舍见
//! `docs/superpowers/specs/2026-10-16-playground-design.md`。

mod config;

fn main() {
    println!("{}", config::load().is_ok());
}
```

- [ ] **Step 3: 写 config.rs 的失败测试**

`C:\projects\rust2genshin\playground\src\config.rs`，先写测试与类型壳：

```rust
//! 环境变量解析。默认值只在这里写一份,别处引用 Config 的字段。

use std::{
    fmt::Display,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub listen: SocketAddr,
    pub repo_dir: PathBuf,
    pub data_dir: PathBuf,
    pub git_ref: String,
    pub remote: String,
    pub max_jobs: usize,
    pub max_queue: usize,
    pub job_timeout: Duration,
    pub keep_images: usize,
    pub build_cache_max: u64,
    pub min_free_disk: u64,
    pub artifact_ttl: Duration,
    pub allowed_origins: Vec<String>,
    pub trust_proxy: bool,
    pub force_rebake: bool,
    pub image: String,
    pub webhook_secret: Option<String>,
    pub admin_token: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("环境变量 {key} 的值 {value:?} 无效:{reason}")]
    Invalid { key: &'static str, value: String, reason: String },
    #[error("仓库根不是 CARGO_MANIFEST_DIR 的上一级,请显式设 PLAYGROUND_REPO_DIR")]
    NoRepoDir,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 拿一张表当环境用:测试不碰进程环境(并行跑会互相污染)。
    fn getter(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + '_ {
        move |k: &str| pairs.iter().find(|(a, _)| *a == k).map(|(_, v)| v.to_string())
    }

    #[test]
    fn defaults() {
        let g = getter(&[]);
        let c = resolve(&g, Path::new("/repo/playground")).unwrap();
        assert_eq!(c.listen, "127.0.0.1:8787".parse().unwrap());
        assert_eq!(c.repo_dir, PathBuf::from("/repo"));
        assert_eq!(c.data_dir, PathBuf::from("/repo/target/playground"));
        assert_eq!(c.git_ref, "main");
        assert_eq!(c.remote, "origin");
        assert_eq!(c.max_jobs, 8);
        assert_eq!(c.max_queue, 64);
        assert_eq!(c.job_timeout, Duration::from_secs(15));
        assert_eq!(c.keep_images, 2);
        assert_eq!(c.build_cache_max, 10 * 1024 * 1024 * 1024);
        assert_eq!(c.min_free_disk, 5 * 1024 * 1024 * 1024);
        assert_eq!(c.artifact_ttl, Duration::from_secs(1800));
        assert_eq!(
            c.allowed_origins,
            vec![
                "https://mzverse.github.io",
                "http://127.0.0.1:3000",
                "http://localhost:3000",
            ]
        );
        assert!(!c.trust_proxy);
        assert!(!c.force_rebake);
        assert_eq!(c.image, "rust2genshin-play");
        assert!(c.webhook_secret.is_none());
        assert!(c.admin_token.is_none());
    }

    #[test]
    fn empty_value_counts_as_unset() {
        let g = getter(&[("PLAYGROUND_MAX_JOBS", "  "), ("PLAYGROUND_ALLOWED_ORIGINS", "")]);
        let c = resolve(&g, Path::new("/repo/playground")).unwrap();
        assert_eq!(c.max_jobs, 8);
        assert_eq!(c.allowed_origins.len(), 3);
    }

    #[test]
    fn bad_number_reports_key_and_value() {
        let g = getter(&[("PLAYGROUND_MAX_JOBS", "abc")]);
        assert_eq!(
            resolve(&g, Path::new("/repo/playground")).unwrap_err(),
            ConfigError::Invalid {
                key: "PLAYGROUND_MAX_JOBS",
                value: "abc".into(),
                reason: "invalid digit found in string".into(),
            }
        );
    }

    #[test]
    fn bad_listen_reports_key() {
        let g = getter(&[("PLAYGROUND_LISTEN", "127.0.0.1")]);
        match resolve(&g, Path::new("/repo/playground")).unwrap_err() {
            ConfigError::Invalid { key, .. } => assert_eq!(key, "PLAYGROUND_LISTEN"),
            other => panic!("期望 Invalid,得到 {other:?}"),
        }
    }

    #[test]
    fn sizes_are_1024_based() {
        assert_eq!(parse_size("10GB").unwrap(), 10 * 1024 * 1024 * 1024);
        assert_eq!(parse_size("5GB").unwrap(), 5 * 1024 * 1024 * 1024);
        assert_eq!(parse_size("512m").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_size("1kb").unwrap(), 1024);
        assert_eq!(parse_size("2048").unwrap(), 2048);
        assert!(parse_size("10 GiB").is_err());
    }

    #[test]
    fn sizes_from_env() {
        let g = getter(&[("PLAYGROUND_BUILD_CACHE_MAX", "20GB")]);
        let c = resolve(&g, Path::new("/repo/playground")).unwrap();
        assert_eq!(c.build_cache_max, 20 * 1024 * 1024 * 1024);
    }

    #[test]
    fn bools_accept_1_0_true_false() {
        let g = getter(&[("PLAYGROUND_TRUST_PROXY", "1"), ("PLAYGROUND_FORCE_REBAKE", "0")]);
        let c = resolve(&g, Path::new("/repo/playground")).unwrap();
        assert!(c.trust_proxy);
        assert!(!c.force_rebake);

        let g = getter(&[("PLAYGROUND_TRUST_PROXY", "yes")]);
        assert!(resolve(&g, Path::new("/repo/playground")).unwrap().trust_proxy);

        let g = getter(&[("PLAYGROUND_TRUST_PROXY", "maybe")]);
        assert!(resolve(&g, Path::new("/repo/playground")).is_err());
    }

    #[test]
    fn secrets_none_when_unset_empty_when_set() {
        let g = getter(&[("GITHUB_WEBHOOK_SECRET", "s3cret")]);
        let c = resolve(&g, Path::new("/repo/playground")).unwrap();
        assert_eq!(c.webhook_secret.as_deref(), Some("s3cret"));
        assert!(c.admin_token.is_none());
    }

    #[test]
    fn all_origins_parsed_and_trimmed() {
        let g = getter(&[("PLAYGROUND_ALLOWED_ORIGINS", " a , b ,,")]);
        let c = resolve(&g, Path::new("/repo/playground")).unwrap();
        assert_eq!(c.allowed_origins, vec!["a", "b"]);
    }
}
```

- [ ] **Step 4: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground`
Expected: 编译失败 —— `cannot find function resolve in this scope`（以及 `parse_size`）。

- [ ] **Step 5: 实现**

在 `config.rs` 的 `ConfigError` 之后、`#[cfg(test)]` 之前插入：

```rust
/// 空串(或全空白)与未设置等价 —— 部署时 `VAR=` 和 `unset VAR` 结果一样。
fn raw(key: &str, get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    get(key).map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn inv(key: &'static str, value: &str, reason: impl Display) -> ConfigError {
    ConfigError::Invalid {
        key,
        value: value.to_string(),
        reason: reason.to_string(),
    }
}

fn str_var(key: &'static str, get: &dyn Fn(&str) -> Option<String>, default: &str) -> String {
    raw(key, get).unwrap_or_else(|| default.to_string())
}

fn num_var<T: std::str::FromStr>(
    key: &'static str,
    get: &dyn Fn(&str) -> Option<String>,
    default: T,
) -> Result<T, ConfigError>
where
    T::Err: Display,
{
    match raw(key, get) {
        None => Ok(default),
        Some(v) => v.parse().map_err(|e| inv(key, &v, e)),
    }
}

/// `1` / `true` / `yes` 为真,`0` / `false` / `no` 为假,其余报错。
fn bool_var(key: &'static str, get: &dyn Fn(&str) -> Option<String>) -> Result<bool, ConfigError> {
    match raw(key, get) {
        None => Ok(false),
        Some(v) => match v.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => Ok(true),
            "0" | "false" | "no" => Ok(false),
            _ => Err(inv(key, &v, "不是布尔值(用 1/0/true/false/yes/no)")),
        },
    }
}

fn size_var(
    key: &'static str,
    get: &dyn Fn(&str) -> Option<String>,
    default: u64,
) -> Result<u64, ConfigError> {
    match raw(key, get) {
        None => Ok(default),
        Some(v) => parse_size(&v).map_err(|e| inv(key, &v, e)),
    }
}

/// `10GB` / `512m` / `1024`。倍数一律 1024 进制 —— docker 的 `--keep-storage`
/// 也按这个理解,别在两处用不同的进制。
pub fn parse_size(s: &str) -> Result<u64, String> {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    let (num, mult) = if let Some(n) = lower.strip_suffix("gb") {
        (n, 1024u64.pow(3))
    } else if let Some(n) = lower.strip_suffix("mb") {
        (n, 1024u64.pow(2))
    } else if let Some(n) = lower.strip_suffix("kb") {
        (n, 1024)
    } else if let Some(n) = lower.strip_suffix('g') {
        (n, 1024u64.pow(3))
    } else if let Some(n) = lower.strip_suffix('m') {
        (n, 1024u64.pow(2))
    } else if let Some(n) = lower.strip_suffix('k') {
        (n, 1024)
    } else if let Some(n) = lower.strip_suffix('b') {
        (n, 1)
    } else {
        (lower.as_str(), 1)
    };
    let v: u64 = num
        .trim()
        .parse()
        .map_err(|_| format!("不是数字:{t:?}"))?;
    v.checked_mul(mult).ok_or_else(|| format!("过大:{t:?}"))
}

const DEFAULT_ORIGINS: &str =
    "https://mzverse.github.io,http://127.0.0.1:3000,http://localhost:3000";

/// `manifest_dir` 是 `CARGO_MANIFEST_DIR`(即 `playground/`),仓库根取它的上一级。
pub fn resolve(
    get: &dyn Fn(&str) -> Option<String>,
    manifest_dir: &Path,
) -> Result<Config, ConfigError> {
    let repo_dir = match raw("PLAYGROUND_REPO_DIR", get) {
        Some(v) => PathBuf::from(v),
        None => manifest_dir
            .parent()
            .ok_or(ConfigError::NoRepoDir)?
            .to_path_buf(),
    };
    let data_dir = match raw("PLAYGROUND_DATA_DIR", get) {
        Some(v) => PathBuf::from(v),
        None => repo_dir.join("target").join("playground"),
    };
    let listen_raw = str_var("PLAYGROUND_LISTEN", get, "127.0.0.1:8787");
    let listen = listen_raw
        .parse()
        .map_err(|e| inv("PLAYGROUND_LISTEN", &listen_raw, e))?;
    Ok(Config {
        listen,
        repo_dir,
        data_dir,
        git_ref: str_var("PLAYGROUND_REF", get, "main"),
        remote: str_var("PLAYGROUND_REMOTE", get, "origin"),
        max_jobs: num_var("PLAYGROUND_MAX_JOBS", get, 8usize)?,
        max_queue: num_var("PLAYGROUND_MAX_QUEUE", get, 64usize)?,
        job_timeout: Duration::from_secs(num_var("PLAYGROUND_JOB_TIMEOUT", get, 15u64)?),
        keep_images: num_var("PLAYGROUND_KEEP_IMAGES", get, 2usize)?,
        build_cache_max: size_var(
            "PLAYGROUND_BUILD_CACHE_MAX",
            get,
            10 * 1024 * 1024 * 1024,
        )?,
        min_free_disk: size_var("PLAYGROUND_MIN_FREE_DISK", get, 5 * 1024 * 1024 * 1024)?,
        artifact_ttl: Duration::from_secs(num_var("PLAYGROUND_ARTIFACT_TTL", get, 1800u64)?),
        allowed_origins: str_var("PLAYGROUND_ALLOWED_ORIGINS", get, DEFAULT_ORIGINS)
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        trust_proxy: bool_var("PLAYGROUND_TRUST_PROXY", get)?,
        force_rebake: bool_var("PLAYGROUND_FORCE_REBAKE", get)?,
        image: str_var("PLAYGROUND_IMAGE", get, "rust2genshin-play"),
        webhook_secret: raw("GITHUB_WEBHOOK_SECRET", get),
        admin_token: raw("PLAYGROUND_ADMIN_TOKEN", get),
    })
}

pub fn load() -> Result<Config, ConfigError> {
    resolve(&|k| std::env::var(k).ok(), Path::new(env!("CARGO_MANIFEST_DIR")))
}
```

- [ ] **Step 6: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground`
Expected: `9 passed`（`defaults` / `empty_value_counts_as_unset` / `bad_number_reports_key_and_value` / `bad_listen_reports_key` / `sizes_are_1024_based` / `sizes_from_env` / `bools_accept_1_0_true_false` / `secrets_none_when_unset_empty_when_set` / `all_origins_parsed_and_trimmed`）。

再跑一次根目录的 `cargo build`，确认 workspace 没被带坏（playground 不在 default-members 里，不应被编译）。

- [ ] **Step 7: 提交**

```bash
git add Cargo.toml playground/Cargo.toml playground/src/main.rs playground/src/config.rs
git commit -m "feat(playground): crate 骨架与配置解析"
```

---

### Task 2: 仓库访问（git）

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\repo.rs`
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（`mod repo;`）

**Interfaces:**
- Consumes: 无
- Produces:
  - `repo::Repo::new(dir: PathBuf) -> Repo`
  - `Repo::head(&self) -> Result<String, RepoError>` —— 完整 sha
  - `Repo::is_dirty(&self) -> Result<bool, RepoError>`
  - `Repo::pull_ff_only(&self, remote: &str) -> Result<Pull, RepoError>`
  - `repo::Pull { UpToDate, Updated, Skipped(String) }`
  - `repo::short_sha(&str) -> String`、`repo::is_sha_like(&str) -> bool`、`repo::classify_pull(ok: bool, stdout: &str, stderr: &str) -> Pull`

- [ ] **Step 1: 写失败测试**

`C:\projects\rust2genshin\playground\src\repo.rs`：

```rust
//! git 调用。只有四件事:当前 sha、工作区脏不脏、快进拉取、判断一个 ref 是不是 sha。
//!
//! `pull_ff_only` **不把失败当错误** —— 这个仓库是开发者自己的 checkout,
//! 分叉 / 本地领先都是常态,服务不该把需要人处理的 merge 冲突态丢进去。
//! 快进不了就返回 `Pull::Skipped(原因)`,烘焙照常继续(烘当前工作区)。

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pull {
    UpToDate,
    Updated,
    Skipped(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_sha_takes_seven() {
        assert_eq!(short_sha("0123456789abcdef0123456789abcdef01234567"), "0123456");
    }

    #[test]
    fn short_sha_passes_through_short_input() {
        assert_eq!(short_sha("abc"), "abc");
    }

    #[test]
    fn sha_like_accepts_hex_of_7_to_40() {
        assert!(is_sha_like("0123456"));
        assert!(is_sha_like("0123456789abcdef0123456789abcdef01234567"));
        assert!(!is_sha_like("main"));
        assert!(!is_sha_like("012345")); // 太短
        assert!(!is_sha_like("0123456789abcdef0123456789abcdef012345678")); // 太长
        assert!(!is_sha_like("zzzzzzz"));
    }

    #[test]
    fn already_up_to_date() {
        assert_eq!(
            classify_pull(true, "Already up to date.\n", ""),
            Pull::UpToDate
        );
    }

    #[test]
    fn fast_forwarded() {
        assert_eq!(
            classify_pull(true, "Updating abc..def\nFast-forward\n", ""),
            Pull::Updated
        );
    }

    #[test]
    fn failure_is_skipped_not_error() {
        let p = classify_pull(
            false,
            "",
            "error: Your local changes to the following files would be overwritten by merge:\n\tdemo/src/lib.rs",
        );
        match p {
            Pull::Skipped(why) => assert!(why.contains("local changes"), "{why}"),
            other => panic!("期望 Skipped,得到 {other:?}"),
        }
    }

    #[test]
    fn failure_with_empty_stderr_still_reports_something() {
        match classify_pull(false, "", "") {
            Pull::Skipped(why) => assert!(!why.is_empty()),
            other => panic!("期望 Skipped,得到 {other:?}"),
        }
    }
}
```

- [ ] **Step 2: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground repo`
Expected: 编译失败 —— `cannot find function short_sha`。

- [ ] **Step 3: 实现**

在 `repo.rs` 的 `enum Pull` 之后、`#[cfg(test)]` 之前插入：

```rust
use std::process::Stdio;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("git 执行失败:{0}")]
    Spawn(#[from] std::io::Error),
    #[error("git {args} 退出码 {code}:{stderr}")]
    Exit {
        args: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("{0} 不是合法的 sha")]
    NotSha(String),
}

/// 只取前 7 位:够区分,而且和 `git rev-parse --short` 的默认长度一致。
pub fn short_sha(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// 全十六进制的 7..=40 位。`PLAYGROUND_REF` 长这样时,`git pull` 跳过
/// —— 钉在某个 commit 上的部署不该被 pull 改掉 ref。
pub fn is_sha_like(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// `git pull` 的结果分类。**只有这一处**决定失败要不要冒泡 ——
/// 这里永远不返回 `Err`,失败一律是 `Skipped`。
pub fn classify_pull(ok: bool, stdout: &str, stderr: &str) -> Pull {
    if !ok {
        let why = first_line(stderr)
            .or_else(|| first_line(stdout))
            .unwrap_or_else(|| "git pull 失败(无输出)".to_string());
        return Pull::Skipped(why);
    }
    if stdout.contains("Already up to date") || stdout.contains("Already up-to-date") {
        Pull::UpToDate
    } else {
        Pull::Updated
    }
}

fn first_line(s: &str) -> Option<String> {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.to_string())
}

#[derive(Debug, Clone)]
pub struct Repo {
    dir: PathBuf,
}

impl Repo {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    async fn git(&self, args: &[&str]) -> Result<String, RepoError> {
        let out = tokio::process::Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .stdin(Stdio::null())
            .output()
            .await?;
        if !out.status.success() {
            return Err(RepoError::Exit {
                args: args.join(" "),
                code: out.status.code(),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    pub async fn head(&self) -> Result<String, RepoError> {
        self.git(&["rev-parse", "HEAD"]).await
    }

    /// `status --porcelain` 非空即脏 —— 包含未跟踪文件,因为未跟踪的
    /// 源码也会被 `docker build` 的 context 带进镜像。
    pub async fn is_dirty(&self) -> Result<bool, RepoError> {
        Ok(!self
            .git(&["status", "--porcelain", "--untracked-files=normal"])
            .await?
            .is_empty())
    }

    /// `git pull --ff-only`。失败返回 `Skipped`,不返回 `Err`。
    pub async fn pull_ff_only(&self, remote: &str) -> Result<Pull, RepoError> {
        let out = tokio::process::Command::new("git")
            .args(["pull", "--ff-only", remote])
            .current_dir(&self.dir)
            .stdin(Stdio::null())
            .output()
            .await?;
        Ok(classify_pull(
            out.status.success(),
            &String::from_utf8_lossy(&out.stdout),
            &String::from_utf8_lossy(&out.stderr),
        ))
    }
}
```

- [ ] **Step 4: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground repo`
Expected: `6 passed`。

再手动确认一次真实调用路径（不需要真远端）：

Run: `cargo run -p rust2genshin-playground`（此时 `main` 还只打印配置是否 OK）—— 只为确认能编译通过。

- [ ] **Step 5: 提交**

```bash
git add playground/src/repo.rs playground/src/main.rs
git commit -m "feat(playground): git 访问与快进拉取"
```

---

### Task 3: 镜像 tag 与引用计数

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\image.rs`
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（`mod image;`）

**Interfaces:**
- Consumes: `repo::short_sha`（不必须，本模块自带字符串处理）
- Produces:
  - `image::tag(image: &str, sha: &str, dirty: bool) -> String`
  - `image::LABEL_PLAY = "rust2genshin.play"`、`image::LABEL_COMMIT = "rust2genshin.commit"`
  - `image::label_filter(image: &str, commit: Option<&str>) -> String` —— 给 `docker image ls --filter`
  - `image::ImageRegistry`，`ImageRegistry::new() -> Arc<ImageRegistry>`、`acquire(self: &Arc<Self>, tag: &str) -> ImageGuard`、`count(&self, tag: &str) -> u32`
  - `image::ImageGuard`（Drop 时减一）

- [ ] **Step 1: 写失败测试**

`C:\projects\rust2genshin\playground\src\image.rs`：

```rust
//! 镜像 tag 与引用计数。
//!
//! tag 用 sha 不用 `latest`:提交时告诉用户 commit A、起跑时 `latest` 可能已经
//! 指到 B —— 用户拿着"来自 A"的结论,拿到 B 编的产物。sha 让"报告 == 实际"。

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_uses_short_sha() {
        assert_eq!(
            tag("rust2genshin-play", "0123456789abcdef", false),
            "rust2genshin-play:0123456"
        );
    }

    #[test]
    fn tag_marks_dirty_worktree() {
        assert_eq!(
            tag("rust2genshin-play", "0123456789abcdef", true),
            "rust2genshin-play:0123456-dirty"
        );
    }

    #[test]
    fn label_filter_for_all_and_for_one_commit() {
        assert_eq!(
            label_filter("rust2genshin-play", None),
            "label=rust2genshin.play=1"
        );
        assert_eq!(
            label_filter("rust2genshin-play", Some("0123456")),
            "label=rust2genshin.commit=0123456"
        );
    }

    #[test]
    fn guard_increments_and_release_decrements() {
        let reg = ImageRegistry::new();
        assert_eq!(reg.count("a:1"), 0);
        let g1 = reg.acquire("a:1");
        let g2 = reg.acquire("a:1");
        assert_eq!(reg.count("a:1"), 2);
        drop(g1);
        assert_eq!(reg.count("a:1"), 1);
        drop(g2);
        assert_eq!(reg.count("a:1"), 0);
    }

    #[test]
    fn guards_are_per_tag() {
        let reg = ImageRegistry::new();
        let _g = reg.acquire("a:1");
        assert_eq!(reg.count("a:2"), 0);
    }

    #[test]
    fn guard_clone_keeps_count() {
        let reg = ImageRegistry::new();
        let g = reg.acquire("a:1");
        let g2 = g.clone();
        drop(g);
        assert_eq!(reg.count("a:1"), 1, "克隆出去的守卫也算一份引用");
        drop(g2);
        assert_eq!(reg.count("a:1"), 0);
    }
}
```

- [ ] **Step 2: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground image`
Expected: 编译失败 —— `cannot find function tag`。

- [ ] **Step 3: 实现**

```rust
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::repo::short_sha;

/// 清理只认这个 label —— 绝不碰宿主上别人的镜像。
pub const LABEL_PLAY: &str = "rust2genshin.play";
pub const LABEL_COMMIT: &str = "rust2genshin.commit";

pub fn tag(image: &str, sha: &str, dirty: bool) -> String {
    let s = short_sha(sha);
    if dirty {
        format!("{image}:{s}-dirty")
    } else {
        format!("{image}:{s}")
    }
}

pub fn label_filter(_image: &str, commit: Option<&str>) -> String {
    match commit {
        None => format!("label={LABEL_PLAY}=1"),
        Some(c) => format!("label={LABEL_COMMIT}={c}"),
    }
}

#[derive(Default)]
pub struct ImageRegistry {
    counts: Mutex<HashMap<String, u32>>,
}

impl ImageRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 任务起跑前调一次,守卫活到容器退出。
    pub fn acquire(self: &Arc<Self>, tag: &str) -> ImageGuard {
        *self
            .counts
            .lock()
            .expect("镜像引用计数锁中毒")
            .entry(tag.to_string())
            .or_insert(0) += 1;
        ImageGuard {
            reg: Arc::clone(self),
            tag: tag.to_string(),
        }
    }

    pub fn count(&self, tag: &str) -> u32 {
        self.counts
            .lock()
            .expect("镜像引用计数锁中毒")
            .get(tag)
            .copied()
            .unwrap_or(0)
    }
}

pub struct ImageGuard {
    reg: Arc<ImageRegistry>,
    tag: String,
}

impl Drop for ImageGuard {
    fn drop(&mut self) {
        let mut m = self.reg.counts.lock().expect("镜像引用计数锁中毒");
        if let Some(n) = m.get_mut(&self.tag) {
            *n -= 1;
            if *n == 0 {
                m.remove(&self.tag);
            }
        }
    }
}

impl Clone for ImageGuard {
    fn clone(&self) -> Self {
        *self
            .reg
            .counts
            .lock()
            .expect("镜像引用计数锁中毒")
            .entry(self.tag.clone())
            .or_insert(0) += 1;
        Self {
            reg: Arc::clone(&self.reg),
            tag: self.tag.clone(),
        }
    }
}
```

- [ ] **Step 4: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground image`
Expected: `6 passed`。

- [ ] **Step 5: 提交**

```bash
git add playground/src/image.rs playground/src/main.rs
git commit -m "feat(playground): 镜像 tag 与引用计数"
```

---

### Task 4: docker CLI 封装（帧解复用 + argv 拼装）

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\docker.rs`
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（`mod docker;`）
- Modify: `C:\projects\rust2genshin\playground\src\protocol.rs` 尚不存在，`Stream` 先定义在 `docker.rs`

**Interfaces:**
- Consumes: 无
- Produces:
  - `docker::Stream { Stdout, Stderr }`（serde `lowercase`，`protocol::LogMsg` 复用它）
  - `docker::Frame { stream, data: Vec<u8> }`、`docker::FrameDecoder::new()` / `.push(&[u8]) -> Result<Vec<Frame>, FrameError>`
  - `docker::RunSpec`、`docker::run_args(&RunSpec) -> Vec<String>`
  - `docker::build_args(ctx: &Path, dockerfile: &Path, image_tag: &str) -> Vec<String>`
  - `docker::kill_args(name: &str) -> Vec<String>`、`docker::prune_args(keep_storage: u64) -> Vec<String>`、`docker::image_ls_args(filter: &str) -> Vec<String>`、`docker::system_df_args() -> Vec<String>`

- [ ] **Step 1: 写失败测试（帧解复用）**

`C:\projects\rust2genshin\playground\src\docker.rs`：

```rust
//! docker CLI 的薄封装。
//!
//! 不引 `bollard`(docker API 的 Rust 客户端):它会带来一大堆类型,而 shell 出去
//! 的 argv 就是人能照着敲的那条命令,调 docker 问题时这个值钱。
//!
//! 代价是 stdout/stderr 的分帧要自己解:`docker run` 不带 `-t` 时它俩是
//! 8 字节头 + 载荷的 multiplexed stream(`header[0]` 1=stdout 2=stderr,
//! `header[4..8]` 是载荷长度,大端)。不用 `-t` 是因为 `-t` 会把 stderr 并进
//! stdout、并把行尾变成 `\r\n` —— 前端就拿不到 stream 区分了。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub stream: Stream,
    pub data: Vec<u8>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("未知的流字节 {0}(只认 1=stdout / 2=stderr)")]
    BadStream(u8),
    #[error("帧载荷 {0} 字节超出上限")]
    TooLarge(u32),
    /// 读容器输出时 IO 出错。存成 String 而不是 `io::Error`,是为了让
    /// 这个枚举能继续派生 `PartialEq` / `Eq`(测试里要 `assert_eq!`)——
    /// `io::Error` 两者都没有。
    #[error("读容器输出失败:{0}")]
    Io(String),
}

/// 单帧上限 4 MiB:正常编译输出远小于它,设上限是为了一个坏掉的
/// 头(比如没带 `--rm` 时 docker 打了别的东西)不会让服务去分配几个 GB。
const MAX_FRAME: u32 = 4 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂一块读到的字节,返回这块里已经完整解出的帧。
    /// 跨 read 边界断开的帧留在内部缓冲里,等下一次 push。
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<Frame>, FrameError> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        loop {
            if self.buf.len() < 8 {
                break;
            }
            let stream = match self.buf[0] {
                1 => Stream::Stdout,
                2 => Stream::Stderr,
                other => return Err(FrameError::BadStream(other)),
            };
            let len = u32::from_be_bytes([self.buf[4], self.buf[5], self.buf[6], self.buf[7]]);
            if len > MAX_FRAME {
                return Err(FrameError::TooLarge(len));
            }
            if self.buf.len() < 8 + len as usize {
                break;
            }
            let data = self.buf[8..8 + len as usize].to_vec();
            self.buf.drain(..8 + len as usize);
            out.push(Frame { stream, data });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(stream: u8, payload: &[u8]) -> Vec<u8> {
        let mut v = vec![stream, 0, 0, 0];
        v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        v.extend_from_slice(payload);
        v
    }

    fn decode_all(mut d: FrameDecoder, chunks: &[Vec<u8>]) -> Vec<Frame> {
        let mut out = Vec::new();
        for c in chunks {
            out.extend(d.push(c).unwrap());
        }
        out
    }

    #[test]
    fn single_stdout_frame() {
        let d = FrameDecoder::new();
        let got = decode_all(d, &[frame(1, b"Compiling demo\n")]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].stream, Stream::Stdout);
        assert_eq!(got[0].data, b"Compiling demo\n");
    }

    #[test]
    fn two_frames_in_one_chunk() {
        let mut chunk = frame(1, b"out");
        chunk.extend(frame(2, b"err"));
        let got = decode_all(FrameDecoder::new(), &[chunk]);
        assert_eq!(
            got,
            vec![
                Frame { stream: Stream::Stdout, data: b"out".to_vec() },
                Frame { stream: Stream::Stderr, data: b"err".to_vec() },
            ]
        );
    }

    #[test]
    fn frame_split_inside_header() {
        let whole = frame(1, b"hello");
        let got = decode_all(
            FrameDecoder::new(),
            &[whole[..3].to_vec(), whole[3..].to_vec()],
        );
        assert_eq!(got, vec![Frame { stream: Stream::Stdout, data: b"hello".to_vec() }]);
    }

    #[test]
    fn frame_split_inside_payload() {
        let whole = frame(2, b"0123456789");
        let got = decode_all(
            FrameDecoder::new(),
            &[whole[..12].to_vec(), whole[12..].to_vec()],
        );
        assert_eq!(got, vec![Frame { stream: Stream::Stderr, data: b"0123456789".to_vec() }]);
    }

    #[test]
    fn empty_chunks_are_harmless() {
        let mut d = FrameDecoder::new();
        assert!(d.push(&[]).unwrap().is_empty());
        let got = d.push(&frame(1, b"x")).unwrap();
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn zero_length_frame() {
        let got = decode_all(FrameDecoder::new(), &[frame(1, b"")]);
        assert_eq!(got, vec![Frame { stream: Stream::Stdout, data: vec![] }]);
    }

    #[test]
    fn bad_stream_byte_is_an_error() {
        let mut d = FrameDecoder::new();
        assert_eq!(d.push(&frame(3, b"x")), Err(FrameError::BadStream(3)));
    }

    #[test]
    fn oversized_frame_is_rejected() {
        let mut v = vec![1u8, 0, 0, 0];
        v.extend_from_slice(&(MAX_FRAME + 1).to_be_bytes());
        let mut d = FrameDecoder::new();
        assert_eq!(d.push(&v), Err(FrameError::TooLarge(MAX_FRAME + 1)));
    }
}
```

- [ ] **Step 2: 跑测试，确认能跑且全绿**

（帧解复用这一步是先实现再测的——它太机械，不值得先红。）

Run: `cargo test -p rust2genshin-playground docker`
Expected: `8 passed`。

- [ ] **Step 3: 加 argv 拼装的失败测试**

追加到同一个 `mod tests`：

```rust
    fn spec() -> RunSpec {
        RunSpec {
            name: "r2g-task1".into(),
            image: "rust2genshin-play:0123456".into(),
            src_dir: PathBuf::from("/data/tasks/task1/src"),
            out_dir: PathBuf::from("/data/tasks/task1/out"),
            workdir: "/opt/rust2genshin".into(),
            cpus: 2,
            memory: "1536m".into(),
            pids_limit: 256,
            user: "1001".into(),
            source_file: "demo/src/lib.rs".into(),
        }
    }

    #[test]
    fn run_args_hardening_is_complete() {
        let a = run_args(&spec());
        for want in [
            "--network", "none",
            "--memory", "1536m",
            "--cpus", "2",
            "--pids-limit", "256",
            "--user", "1001",
            "--tmpfs", "/tmp",
            "--rm",
        ] {
            assert!(a.contains(&want.to_string()), "argv 缺 {want}:{a:?}");
        }
        assert!(a.windows(2).any(|w| w == ["--cap-drop", "ALL"]));
        assert!(a
            .windows(2)
            .any(|w| w == ["--security-opt", "no-new-privileges"]));
        assert!(a.windows(2).any(|w| w == ["--memory-swap", "1536m"]));
        assert!(!a.contains(&"--read-only".to_string()), "cargo 要写 target/,不能只读");
        assert!(!a.iter().any(|s| s.contains("/var/run/docker.sock")));
    }

    #[test]
    fn run_args_mounts_only_src_and_out() {
        let a = run_args(&spec());
        let vols: Vec<String> = a
            .windows(2)
            .filter(|w| w[0] == "-v")
            .map(|w| w[1].clone())
            .collect();
        assert_eq!(
            vols,
            vec![
                "/data/tasks/task1/src:/opt/rust2genshin/demo/src:ro".to_string(),
                "/data/tasks/task1/out:/out:rw".to_string(),
            ]
        );
        assert!(
            !vols.iter().any(|v| v.contains("target")),
            "挂 target/ 会盖掉烘进去的预热缓存"
        );
    }

    #[test]
    fn run_args_names_container_and_ends_with_shell_command() {
        let a = run_args(&spec());
        assert!(a.windows(2).any(|w| w == ["--name", "r2g-task1"]));
        let img = a
            .iter()
            .position(|s| s == "rust2genshin-play:0123456")
            .unwrap();
        assert_eq!(a[img - 2], "--workdir");
        assert_eq!(a[img - 1], "/opt/rust2genshin");
        assert_eq!(a[img + 1], "sh");
        assert_eq!(a[img + 2], "-c");
        let cmd = &a[img + 3];
        assert!(cmd.contains("--offline"));
        assert!(cmd.contains("-Zbuild-std=core"));
        assert!(cmd.contains("cp target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia /out/"));
        assert_eq!(a.len(), img + 4, "命令之后不该再有别的参数");
    }

    #[test]
    fn build_args_pass_labels_and_dockerfile() {
        let a = build_args(
            Path::new("/repo"),
            Path::new("/repo/playground/Dockerfile"),
            "rust2genshin-play:0123456",
        );
        assert_eq!(
            a,
            vec![
                "build",
                "-t", "rust2genshin-play:0123456",
                "-f", "/repo/playground/Dockerfile",
                "--label", "rust2genshin.play=1",
                "--label", "rust2genshin.commit=0123456",
                "/repo",
            ]
        );
    }

    #[test]
    fn prune_keep_storage() {
        assert_eq!(
            prune_args(10 * 1024 * 1024 * 1024),
            vec!["builder", "prune", "-f", "--keep-storage", "10737418240"]
        );
    }

    #[test]
    fn misc_args() {
        assert_eq!(kill_args("r2g-t1"), vec!["kill", "r2g-t1"]);
        assert_eq!(
            image_ls_args("label=rust2genshin.play=1"),
            vec![
                "image", "ls", "--filter", "label=rust2genshin.play=1",
                "--format", "{{.Repository}}:{{.Tag}}",
            ]
        );
        assert_eq!(system_df_args(), vec!["system", "df", "--format", "{{json .}}"]);
    }
```

- [ ] **Step 4: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground docker`
Expected: 编译失败 —— `cannot find struct RunSpec`。

- [ ] **Step 5: 实现 argv 拼装**

在 `docker.rs` 的 `FrameDecoder` 之后插入：

```rust
/// 跑一个任务容器要的全部参数。做成结构体而不是一堆函数参数,
/// 是因为任务容器那串加固参数一个都不能少 —— 显式列出比默认值安全。
#[derive(Debug, Clone)]
pub struct RunSpec {
    pub name: String,
    pub image: String,
    pub src_dir: PathBuf,
    pub out_dir: PathBuf,
    pub workdir: String,
    pub cpus: u32,
    pub memory: String,
    pub pids_limit: u32,
    pub user: String,
    /// 用户代码在仓库里的相对路径(`demo/src/lib.rs`),只写进日志。
    pub source_file: String,
}

/// 容器里跑的那条命令。`--offline` 是关键:依赖在烘焙时 fetch 齐了,
/// 容器又是 `--network none`,让"万一要联网"变成一个明确的错误而不是超时。
pub fn container_command(spec: &RunSpec) -> String {
    format!(
        "cargo build --release --offline -p rust2genshin-demo \
         --target ./genshin-unknown-beyond-server.json \
         -Zunstable-options -Zjson-target-spec -Zbuild-std=core \
         && cp target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia /out/"
    )
}

/// 不挂 `target/` —— 挂了会盖掉烘焙进去的预热缓存,任务就必然超时。
/// 挂目录而不是挂单个文件:单文件 bind mount 在 Docker Desktop 的
/// 9p / virtiofs 共享上最容易出怪问题,而 `demo/src/` 里只有一个文件。
pub fn run_args(spec: &RunSpec) -> Vec<String> {
    let mut a: Vec<String> = vec!["run".into()];
    let cpus = spec.cpus.to_string();
    let pids = spec.pids_limit.to_string();
    for s in [
        "--network", "none",
        "--memory", &spec.memory,
        "--memory-swap", &spec.memory,
        "--cpus", &cpus,
        "--pids-limit", &pids,
        "--user", &spec.user,
        "--tmpfs", "/tmp",
    ] {
        a.push(s.into());
    }
    a.push("--cap-drop".into());
    a.push("ALL".into());
    a.push("--security-opt".into());
    a.push("no-new-privileges".into());
    a.push("--rm".into());
    a.push("--name".into());
    a.push(spec.name.clone());
    a.push("-v".into());
    a.push(format!(
        "{}:{}/{}:ro",
        spec.src_dir.display(),
        spec.workdir,
        "demo/src"
    ));
    a.push("-v".into());
    a.push(format!("{}:/out:rw", spec.out_dir.display()));
    a.push("--workdir".into());
    a.push(spec.workdir.clone());
    a.push(spec.image.clone());
    a.push("sh".into());
    a.push("-c".into());
    a.push(container_command(spec));
    a
}

/// 清理只认 label:同一个 daemon 上别人的镜像一个都不碰。
pub fn build_args(ctx: &Path, dockerfile: &Path, image_tag: &str) -> Vec<String> {
    let commit = image_tag
        .rsplit_once(':')
        .map(|(_, t)| t.trim_end_matches("-dirty").to_string())
        .unwrap_or_default();
    vec![
        "build".into(),
        "-t".into(), image_tag.into(),
        "-f".into(), dockerfile.display().to_string(),
        "--label".into(), format!("{}=1", crate::image::LABEL_PLAY),
        "--label".into(), format!("{}={}", crate::image::LABEL_COMMIT, commit),
        ctx.display().to_string(),
    ]
}

/// `--keep-storage` 是容量,不是次数:保留最近用到的层,超预算的丢最旧的。
/// **有副作用**:builder cache 全局共享、没有 label,这台机器上别的项目的
/// 构建缓存会被一起裁(下次变慢,不会出错)。写进部署文档。
pub fn prune_args(keep_storage: u64) -> Vec<String> {
    vec![
        "builder".into(),
        "prune".into(),
        "-f".into(),
        "--keep-storage".into(),
        keep_storage.to_string(),
    ]
}

pub fn kill_args(name: &str) -> Vec<String> {
    vec!["kill".into(), name.into()]
}

pub fn image_ls_args(filter: &str) -> Vec<String> {
    vec![
        "image".into(),
        "ls".into(),
        "--filter".into(),
        filter.into(),
        "--format".into(),
        "{{.Repository}}:{{.Tag}}".into(),
    ]
}

pub fn system_df_args() -> Vec<String> {
    vec![
        "system".into(),
        "df".into(),
        "--format".into(),
        "{{json .}}".into(),
    ]
}
```

注意 `container_command` 的 `format!` 里那个续行：Rust 的字符串续行会吃掉下一行行首的空白，所以源码里的缩进不会进命令。

- [ ] **Step 6: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground docker`
Expected: `14 passed`。

- [ ] **Step 7: 提交**

```bash
git add playground/src/docker.rs playground/src/main.rs
git commit -m "feat(playground): docker CLI 封装与输出帧解复用"
```

---

### Task 5: 清理（镜像 / build cache / 任务目录 / 磁盘水位）

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\gc.rs`
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（`mod gc;`）

**Interfaces:**
- Consumes: `image::ImageRegistry::count`、`docker::prune_args`
- Produces:
  - `gc::ImageInfo { tag: String, refcount: u32, last_used: u64 }`
  - `gc::select_images_to_remove(images: &[ImageInfo], keep: usize, protected: &[&str]) -> Vec<String>`
  - `gc::BakeGate { Proceed, CleanedThenProceed, NoDisk { free: u64 } }`
  - `gc::judge_disk(free_before: u64, free_after_clean: u64, min: u64) -> BakeGate`
  - `gc::expired_dirs(entries: &[(PathBuf, SystemTime)], now: SystemTime, ttl: Duration) -> Vec<PathBuf>`
  - `gc::DiskGate` trait（`async fn ensure_free(&self) -> Result<(), String>`），`gc::RealDiskGate`
  - `gc::free_bytes(path: &Path) -> Option<u64>`（`fs4::available_space`）

- [ ] **Step 1: 写失败测试**

`C:\projects\rust2genshin\playground\src\gc.rs`：

```rust
//! 清理。三样东西要管,大头是前两样:
//!
//! - 镜像 tag:引用计数归零 + 超出 `keep` 才删,`serving` 与"最近一次成功的"
//!   永不删(后者是回滚用的)。
//! - builder cache:**最容易忘的一件**。`docker rmi` 删掉旧 tag 之后,那次
//!   build 的中间层还在 —— 不清它,每次 push 都在磁盘上留一份没人引用的旧
//!   `target/`(1 GB 上下)。按容量裁,不按次数。
//! - 任务目录:TTL 到了整个删掉。
//!
//! 另外还有一道水位兜底:磁盘快满时先清理,清完还低就把这次烘焙判失败 ——
//! 否则第一次 `docker build` 会失败成一堆看不懂的层写入错误。

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInfo {
    pub tag: String,
    pub refcount: u32,
    /// 单调递增的序号,越大越近 —— 用单调计数而不是墙上时间,好测。
    pub last_used: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BakeGate {
    Proceed,
    CleanedThenProceed,
    NoDisk { free: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(tag: &str, refcount: u32, last_used: u64) -> ImageInfo {
        ImageInfo { tag: tag.into(), refcount, last_used }
    }

    #[test]
    fn serving_and_recent_are_never_removed() {
        let images = vec![img("i:a", 0, 4), img("i:b", 0, 3)];
        let got = select_images_to_remove(&images, 1, &["i:a", "i:b"]);
        assert!(got.is_empty(), "受保护的两个不该被删:{got:?}");
    }

    #[test]
    fn keeps_newest_beyond_the_protected_ones() {
        // a 最新(serving),b 次新,c/d 更老。keep=2 → 除 a 外还留 b。
        let images = vec![
            img("i:a", 0, 4),
            img("i:b", 0, 3),
            img("i:c", 0, 2),
            img("i:d", 0, 1),
        ];
        let got = select_images_to_remove(&images, 2, &["i:a"]);
        assert_eq!(got, vec!["i:c".to_string(), "i:d".to_string()]);
    }

    #[test]
    fn keep_one_removes_everything_else() {
        let images = vec![
            img("i:a", 0, 4),
            img("i:b", 0, 3),
            img("i:c", 0, 2),
        ];
        let got = select_images_to_remove(&images, 1, &["i:a"]);
        assert_eq!(got, vec!["i:b".to_string(), "i:c".to_string()]);
    }

    #[test]
    fn in_use_images_survive_even_when_old() {
        // 一个长跑任务正拿 d 起容器:它在最老的位置,但 refcount 非零。
        let images = vec![
            img("i:a", 0, 4),
            img("i:b", 0, 3),
            img("i:c", 0, 2),
            img("i:d", 1, 1),
        ];
        let got = select_images_to_remove(&images, 1, &["i:a"]);
        assert_eq!(got, vec!["i:b".to_string(), "i:c".to_string()]);
        assert!(!got.contains(&"i:d".to_string()));
    }

    #[test]
    fn input_order_does_not_matter() {
        let images = vec![
            img("i:d", 0, 1),
            img("i:a", 0, 4),
            img("i:c", 0, 2),
            img("i:b", 0, 3),
        ];
        let got = select_images_to_remove(&images, 2, &["i:a"]);
        assert_eq!(got, vec!["i:c".to_string(), "i:d".to_string()]);
    }

    #[test]
    fn disk_above_watermark_proceeds() {
        assert_eq!(judge_disk(10, 10, 5), BakeGate::Proceed);
        assert_eq!(judge_disk(5, 5, 5), BakeGate::Proceed, "等于水位算够");
    }

    #[test]
    fn disk_recovered_after_clean() {
        assert_eq!(judge_disk(4, 6, 5), BakeGate::CleanedThenProceed);
    }

    #[test]
    fn disk_still_short_is_no_disk() {
        assert_eq!(judge_disk(4, 4, 5), BakeGate::NoDisk { free: 4 });
    }

    #[test]
    fn expired_task_dirs() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        let entries = vec![
            (PathBuf::from("/t/old"), now - Duration::from_secs(3600)),
            (PathBuf::from("/t/new"), now - Duration::from_secs(60)),
            (PathBuf::from("/t/edge"), now - Duration::from_secs(1800)),
        ];
        let got = expired_dirs(&entries, now, Duration::from_secs(1800));
        assert_eq!(got, vec![PathBuf::from("/t/old"), PathBuf::from("/t/edge")]);
    }
}
```

- [ ] **Step 2: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground gc`
Expected: 编译失败 —— `cannot find function select_images_to_remove`。

- [ ] **Step 3: 实现选择逻辑**

```rust
/// 选出该删的 tag。规则:
///
/// 1. `protected`(`serving` + 最近一次成功)永不入选;
/// 2. `refcount > 0`(有容器在跑)永不入选,并且**占掉 `keep` 的额度**
///    —— 它反正留着,不该让别的老镜像因此多活一份;
/// 3. 其余按 `last_used` 从新到旧,填满 `keep` 剩下的额度,更老的删。
///
/// 返回顺序:老的在先(删除顺序无所谓,但测试要确定)。
pub fn select_images_to_remove(
    images: &[ImageInfo],
    keep: usize,
    protected: &[&str],
) -> Vec<String> {
    let mut sorted: Vec<&ImageInfo> = images.iter().collect();
    sorted.sort_by(|a, b| b.last_used.cmp(&a.last_used));

    let mut survivors = 0usize;
    for i in &sorted {
        if protected.contains(&i.tag.as_str()) || i.refcount > 0 {
            survivors += 1;
        }
    }
    let budget = keep.saturating_sub(survivors);

    let mut doomed: Vec<&ImageInfo> = Vec::new();
    let mut kept_others = 0usize;
    for i in &sorted {
        if protected.contains(&i.tag.as_str()) || i.refcount > 0 {
            continue;
        }
        if kept_others < budget {
            kept_others += 1;
        } else {
            doomed.push(i);
        }
    }
    doomed.sort_by(|a, b| a.last_used.cmp(&b.last_used));
    doomed.into_iter().map(|i| i.tag.clone()).collect()
}

pub fn judge_disk(free_before: u64, free_after_clean: u64, min: u64) -> BakeGate {
    if free_before >= min {
        BakeGate::Proceed
    } else if free_after_clean >= min {
        BakeGate::CleanedThenProceed
    } else {
        BakeGate::NoDisk { free: free_after_clean }
    }
}

/// 目录 mtime 早于或等于 `now - ttl` 就算过期。
pub fn expired_dirs(
    entries: &[(PathBuf, SystemTime)],
    now: SystemTime,
    ttl: Duration,
) -> Vec<PathBuf> {
    let cutoff = now.checked_sub(ttl).unwrap_or(SystemTime::UNIX_EPOCH);
    entries
        .iter()
        .filter(|(_, m)| *m <= cutoff)
        .map(|(p, _)| p.clone())
        .collect()
}
```

- [ ] **Step 4: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground gc`
Expected: `9 passed`。

- [ ] **Step 5: 加 `DiskGate` 与真实实现的失败测试**

追加测试：

```rust
    #[test]
    fn real_disk_gate_reports_free() {
        let d = RealDiskGate::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")), 0);
        let free = free_bytes(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(free > 0);
        // 水位 0 → 永远够
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(rt.block_on(d.ensure_free()), Ok(()));
    }

    #[test]
    fn real_disk_gate_refuses_when_watermark_is_absurd() {
        let d = RealDiskGate::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")), u64::MAX);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let err = rt.block_on(d.ensure_free()).unwrap_err();
        assert!(err.contains("磁盘"), "{err}");
    }
```

- [ ] **Step 6: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground gc`
Expected: 编译失败 —— `cannot find struct RealDiskGate`。

- [ ] **Step 7: 实现 DiskGate**

```rust
/// 烘焙前的水位闸。做成 trait 是为了让 bake 的单测能换一个假的
/// —— 真实现要读磁盘,不该进单测。
#[async_trait::async_trait]
pub trait DiskGate: Send + Sync {
    /// `Ok(())` 表示可以烘;`Err(原因)` 表示这次烘焙判 `no_disk`。
    async fn ensure_free(&self) -> Result<(), String>;
}

pub struct RealDiskGate {
    path: PathBuf,
    min_free: u64,
}

impl RealDiskGate {
    pub fn new(path: PathBuf, min_free: u64) -> Self {
        Self { path, min_free }
    }
}

pub fn free_bytes(path: &Path) -> Option<u64> {
    fs4::available_space(path).ok()
}

#[async_trait::async_trait]
impl DiskGate for RealDiskGate {
    async fn ensure_free(&self) -> Result<(), String> {
        let free = free_bytes(&self.path)
            .ok_or_else(|| format!("读不到 {} 的可用空间", self.path.display()))?;
        if free >= self.min_free {
            Ok(())
        } else {
            Err(format!(
                "磁盘可用空间 {} 字节低于水位 {} 字节",
                free, self.min_free
            ))
        }
    }
}
```

> 注：真实实现里"先清理再复测"由 `main.rs` 的 gc 循环做（先 `run_gc()` 再 `ensure_free()`），`DiskGate` 只做复测那一半。这样单测只测判定，不测编排。

- [ ] **Step 8: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground gc`
Expected: `11 passed`。

- [ ] **Step 9: 提交**

```bash
git add playground/src/gc.rs playground/src/main.rs
git commit -m "feat(playground): 镜像/缓存/任务目录清理与磁盘水位"
```

---

### Task 6: 协议信封

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\protocol.rs`
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（`mod protocol;`）

**Interfaces:**
- Consumes: `docker::Stream`
- Produces:
  - `protocol::ClientMsg { Submit { source: String, target: Option<String> }, Cancel {} }`
  - `protocol::ServerMsg { Hello{..}, Queued{..}, Started{..}, Log{..}, Finished{..}, Failed{..}, Error{..} }`
  - `protocol::ErrorCode { NotReady, IpBusy, QueueFull, BadRequest, Internal }`
  - `protocol::PROTOCOL_VERSION: u32 = 1`、`protocol::SOURCE_FILE: &str = "demo/src/lib.rs"`、`protocol::MAX_SOURCE_BYTES: usize = 1 << 20`
  - `protocol::ErrorCode::ip_busy_message() -> &'static str`（逐字 `请求过多，请稍后重试`）

- [ ] **Step 1: 写失败测试**

`C:\projects\rust2genshin\playground\src\protocol.rs`：

```rust
//! WebSocket 的信封消息。
//!
//! **可扩展 + 连接长开**是两条硬约束,不是为了好看:将来要加
//! `{"type":"lsp", ...}` 的双向转发(rust-analyzer 代码提示),不需要动
//! 现有分支。写成"一提交一连接就关"就得推倒重来。

use serde::{Deserialize, Serialize};

use crate::docker::Stream;

pub const PROTOCOL_VERSION: u32 = 1;
/// `hello` 里报的路径,将来 LSP 的 `file://` URI 才有对应物。
pub const SOURCE_FILE: &str = "demo/src/lib.rs";
/// 源码上限 1 MiB。正常一份 `demo/src/lib.rs` 是几 KB;设上限是为了
/// 一次心跳式的大 payload 不会让服务去分配几百 MB。
pub const MAX_SOURCE_BYTES: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Submit { source: String, target: Option<String> },
    Cancel {},
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Hello {
        protocol: u32,
        image_commit: Option<String>,
        file: String,
        max_jobs: usize,
        max_queue: usize,
        targets: Vec<String>,
    },
    Queued {
        position: usize,
        queue: usize,
        max_jobs: usize,
    },
    Started {
        task: String,
        image_commit: String,
    },
    Log {
        task: String,
        stream: Stream,
        text: String,
    },
    Finished {
        task: String,
        ok: bool,
        bytes: u64,
        download: String,
    },
    Failed {
        task: String,
        exit_code: Option<i32>,
        message: String,
    },
    Error {
        code: ErrorCode,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotReady,
    IpBusy,
    QueueFull,
    BadRequest,
    Internal,
}

impl ErrorCode {
    /// NAT 后面多人共用一个出口 IP 会被误伤 —— 认了,但要把话说清楚。
    pub fn ip_busy_message() -> &'static str {
        "请求过多，请稍后重试"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(v: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let s = serde_json::to_string(v).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn submit_round_trip_keeps_target_optional() {
        let m = ClientMsg::Submit { source: "fn main() {}".into(), target: None };
        assert_eq!(round_trip(&m), m);

        let m = ClientMsg::Submit { source: "x".into(), target: Some("beyond-server".into()) };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn cancel_round_trip() {
        assert_eq!(round_trip(&ClientMsg::Cancel {}), ClientMsg::Cancel {});
    }

    #[test]
    fn hello_json_shape() {
        let m = ServerMsg::Hello {
            protocol: PROTOCOL_VERSION,
            image_commit: Some("abc1234".into()),
            file: SOURCE_FILE.into(),
            max_jobs: 8,
            max_queue: 64,
            targets: vec!["beyond-server".into()],
        };
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({
                "type": "hello",
                "protocol": 1,
                "image_commit": "abc1234",
                "file": "demo/src/lib.rs",
                "max_jobs": 8,
                "max_queue": 64,
                "targets": ["beyond-server"],
            })
        );
    }

    #[test]
    fn queued_json_shape() {
        let m = ServerMsg::Queued { position: 2, queue: 5, max_jobs: 8 };
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({ "type": "queued", "position": 2, "queue": 5, "max_jobs": 8 })
        );
    }

    #[test]
    fn log_carries_stream() {
        let m = ServerMsg::Log {
            task: "t1".into(),
            stream: Stream::Stderr,
            text: "error[E0308]\n".into(),
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["type"], "log");
        assert_eq!(v["stream"], "stderr");
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn finished_and_failed_round_trip() {
        let m = ServerMsg::Finished {
            task: "t1".into(),
            ok: true,
            bytes: 15860,
            download: "/artifact/t1".into(),
        };
        assert_eq!(round_trip(&m), m);

        let m = ServerMsg::Failed {
            task: "t1".into(),
            exit_code: Some(101),
            message: "编译失败".into(),
        };
        assert_eq!(round_trip(&m), m);
        assert_eq!(serde_json::to_value(&m).unwrap()["exit_code"], 101);
    }

    #[test]
    fn error_codes_serialize_snake_case() {
        for (code, s) in [
            (ErrorCode::NotReady, "not_ready"),
            (ErrorCode::IpBusy, "ip_busy"),
            (ErrorCode::QueueFull, "queue_full"),
            (ErrorCode::BadRequest, "bad_request"),
            (ErrorCode::Internal, "internal"),
        ] {
            assert_eq!(serde_json::to_value(code).unwrap(), serde_json::json!(s));
        }
    }

    #[test]
    fn ip_busy_message_is_exact() {
        assert_eq!(ErrorCode::ip_busy_message(), "请求过多，请稍后重试");
    }

    #[test]
    fn unknown_type_is_rejected_not_ignored() {
        assert!(serde_json::from_str::<ClientMsg>(r#"{"type":"nope"}"#).is_err());
    }
}
```

- [ ] **Step 2: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground protocol`
Expected: 编译失败 —— `cannot find enum ClientMsg`（文件里只有测试）。

- [ ] **Step 3: 把类型移到测试之上**

把 Step 1 里 `#[cfg(test)]` 之前的所有内容原样保留（它已经在文件上方），确认：
- `use serde::{Deserialize, Serialize};` 与 `use crate::docker::Stream;` 在文件顶；
- `ClientMsg` / `ServerMsg` / `ErrorCode` / 三个常量在 `mod tests` 之前。

- [ ] **Step 4: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground protocol`
Expected: `9 passed`。

- [ ] **Step 5: 提交**

```bash
git add playground/src/protocol.rs playground/src/main.rs
git commit -m "feat(playground): WebSocket 协议信封"
```

---

### Task 7: 任务目录脚手架

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\job.rs`
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（`mod job;`）

**Interfaces:**
- Consumes: `gc::expired_dirs`
- Produces:
  - `job::JobStore::new(tasks_root: PathBuf) -> JobStore`、`JobStore::prepare(&self, task: &str, source: &str) -> io::Result<JobPaths>`、`JobStore::sweep_expired(&self, ttl: Duration) -> io::Result<usize>`、`JobStore::artifact_of(&self, task: &str) -> Option<PathBuf>`、`JobStore::meta_of(&self, task: &str) -> PathBuf`
  - `job::JobPaths { dir, src_dir, out_dir, artifact }`
  - `job::JobMeta { task, target, image_tag, image_commit, created_unix, source_bytes }`
  - `job::new_task_id() -> String`、`job::is_valid_task_id(&str) -> bool`

- [ ] **Step 1: 写失败测试**

`C:\projects\rust2genshin\playground\src\job.rs`：

```rust
//! 任务目录:`<data>/tasks/<task-id>/{src/lib.rs, out/rust2genshin_demo.gia, meta.json}`
//!
//! 这里有一处**必须**做对的地方:每次任务都要**新写** `src/lib.rs`,
//! 不能复制文件时保留原 mtime。cargo 的失效判定是 mtime 比较 —— 用户这份
//! 文件如果比镜像里烘焙时那份产物还旧,cargo 会认为 `rust2genshin-demo`
//! 是 fresh、跳过编译,容器就把**烘焙时那个样例编出的 .gia** 当用户的结果
//! 交出去。一个没有任何报错的错答案。新写文件天然让 mtime 是"现在"。

use std::{
    fs,
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobPaths {
    pub dir: PathBuf,
    pub src_dir: PathBuf,
    pub out_dir: PathBuf,
    /// 容器里 `cp ... /out/` 落下来的位置,也就是挂载回宿主后的路径。
    pub artifact: PathBuf,
}

pub const ARTIFACT_NAME: &str = "rust2genshin_demo.gia";

/// 只允许 `[A-Za-z0-9-]`,长度 1..=64。id 是服务自己生成的,这个函数是
/// `/artifact/<id>` 的输入校验 —— 它挡的是 `../` 那类构造,不是用户输入错误。
pub fn is_valid_task_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

static SEQ: AtomicU64 = AtomicU64::new(0);

/// 16 位纳秒 + 4 位序号,全十六进制。同一纳秒内并发提交也不会撞。
pub fn new_task_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed) & 0xffff;
    format!("{nanos:016x}{seq:04x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, JobStore) {
        let d = tempfile::tempdir().unwrap();
        let s = JobStore::new(d.path().join("tasks"));
        (d, s)
    }

    #[test]
    fn prepare_creates_dirs_and_writes_source() {
        let (_d, s) = store();
        let p = s.prepare("t1", "fn main() {}").unwrap();
        assert!(p.src_dir.is_dir());
        assert!(p.out_dir.is_dir());
        assert_eq!(
            fs::read_to_string(p.src_dir.join("lib.rs")).unwrap(),
            "fn main() {}"
        );
        assert_eq!(p.artifact, p.out_dir.join(ARTIFACT_NAME));
    }

    #[test]
    fn prepare_overwrites_previous_source() {
        let (_d, s) = store();
        s.prepare("t1", "first").unwrap();
        let p = s.prepare("t1", "second").unwrap();
        assert_eq!(fs::read_to_string(p.src_dir.join("lib.rs")).unwrap(), "second");
    }

    #[test]
    fn rewritten_source_is_newer_than_the_baked_artifact() {
        // 造一个"烘焙时的产物",mtime 是一小时前。
        let (_d, s) = store();
        let p = s.prepare("t1", "old").unwrap();
        let baked = p.out_dir.join("baked.gia");
        fs::write(&baked, b"baked").unwrap();
        let long_ago = SystemTime::now() - Duration::from_secs(3600);
        let f = fs::File::options().write(true).open(&baked).unwrap();
        f.set_modified(long_ago).unwrap();
        drop(f);

        s.prepare("t1", "new source").unwrap();
        let lib = p.src_dir.join("lib.rs");
        assert!(
            fs::metadata(&lib).unwrap().modified().unwrap() > long_ago,
            "重写后的 lib.rs 必须比镜像里的产物新,否则 cargo 判 fresh、发回旧产物"
        );
    }

    #[test]
    fn prepare_recreates_a_deleted_task_dir() {
        let (_d, s) = store();
        let p = s.prepare("t1", "x").unwrap();
        fs::remove_dir_all(&p.dir).unwrap();
        assert!(s.prepare("t1", "y").is_ok(), "服务在跑时 cargo clean 删掉 target/ 也要能扛");
    }

    #[tokio::test]
    async fn artifact_of_is_none_before_the_task_ran() {
        let (_d, s) = store();
        assert_eq!(s.artifact_of("t1"), None);
        let p = s.prepare("t1", "x").unwrap();
        fs::write(&p.artifact, b"gia").unwrap();
        assert_eq!(s.artifact_of("t1"), Some(p.artifact));
    }

    #[test]
    fn sweep_removes_expired_dirs() {
        let (_d, s) = store();
        s.prepare("t1", "x").unwrap();
        let n = s.sweep_expired(Duration::ZERO).unwrap();
        assert_eq!(n, 1);
        assert!(!s.tasks_root().join("t1").exists());
    }

    #[test]
    fn sweep_keeps_fresh_dirs() {
        let (_d, s) = store();
        s.prepare("t1", "x").unwrap();
        assert_eq!(s.sweep_expired(Duration::from_secs(3600)).unwrap(), 0);
    }

    #[test]
    fn task_ids_are_valid_and_unique() {
        let a = new_task_id();
        let b = new_task_id();
        assert_ne!(a, b);
        assert!(is_valid_task_id(&a));
        assert_eq!(a.len(), 20);
    }

    #[test]
    fn task_id_validation_rejects_traversal() {
        assert!(is_valid_task_id("abc123"));
        for bad in ["", "../x", "a/b", "a\\b", "a b", "..", &"x".repeat(65)] {
            assert!(!is_valid_task_id(bad), "{bad:?} 不该通过");
        }
    }

    #[test]
    fn meta_round_trips() {
        let (_d, s) = store();
        let p = s.prepare("t1", "x").unwrap();
        let m = JobMeta {
            task: "t1".into(),
            target: "beyond-server".into(),
            image_tag: "rust2genshin-play:0123456".into(),
            image_commit: "0123456".into(),
            created_unix: 1_700_000_000,
            source_bytes: 2,
        };
        s.write_meta(&p, &m).unwrap();
        assert_eq!(s.read_meta("t1").unwrap(), m);
    }
}
```

- [ ] **Step 2: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground job`
Expected: 编译失败 —— `cannot find type JobPaths`（文件里只有测试与两个小函数）。

- [ ] **Step 3: 实现**

在 `job.rs` 的 `new_task_id` 之后、`#[cfg(test)]` 之前插入：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobMeta {
    pub task: String,
    pub target: String,
    pub image_tag: String,
    pub image_commit: String,
    pub created_unix: u64,
    pub source_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct JobStore {
    tasks_root: PathBuf,
}

impl JobStore {
    pub fn new(tasks_root: PathBuf) -> Self {
        Self { tasks_root }
    }

    pub fn tasks_root(&self) -> &Path {
        &self.tasks_root
    }

    /// 建目录 + **新写**源码。每次任务都调用它,不复用上一次的目录内容。
    pub fn prepare(&self, task: &str, source: &str) -> io::Result<JobPaths> {
        assert!(is_valid_task_id(task), "任务 id 由 new_task_id() 生成");
        let dir = self.tasks_root.join(task);
        let src_dir = dir.join("src");
        let out_dir = dir.join("out");
        fs::create_dir_all(&src_dir)?;
        fs::create_dir_all(&out_dir)?;

        // out/ 是宿主目录 bind mount 进容器,容器以 uid 1001 跑。
        // Linux 上服务进程多半不是 1001,不给 0777 的话容器写不进去。
        // Windows / macOS 的 Docker Desktop 忽略这个位,无害。
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&out_dir, fs::Permissions::from_mode(0o777))?;
            fs::set_permissions(&src_dir, fs::Permissions::from_mode(0o755))?;
        }

        // 不要拷贝:拷贝会保留原 mtime。见文件头那段。
        fs::write(src_dir.join("lib.rs"), source)?;
        Ok(JobPaths {
            artifact: out_dir.join(ARTIFACT_NAME),
            dir,
            src_dir,
            out_dir,
        })
    }

    pub fn artifact_of(&self, task: &str) -> Option<PathBuf> {
        if !is_valid_task_id(task) {
            return None;
        }
        let p = self.tasks_root.join(task).join("out").join(ARTIFACT_NAME);
        p.is_file().then_some(p)
    }

    pub fn meta_of(&self, task: &str) -> PathBuf {
        self.tasks_root.join(task).join("meta.json")
    }

    pub fn write_meta(&self, paths: &JobPaths, meta: &JobMeta) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(meta).expect("JobMeta 一定能序列化");
        fs::write(paths.dir.join("meta.json"), json)
    }

    pub fn read_meta(&self, task: &str) -> io::Result<JobMeta> {
        let raw = fs::read(self.meta_of(task))?;
        serde_json::from_slice(&raw)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// 删掉 mtime 早于 `now - ttl` 的整个任务目录,返回删了几个。
    pub fn sweep_expired(&self, ttl: Duration) -> io::Result<usize> {
        let Ok(rd) = fs::read_dir(&self.tasks_root) else {
            return Ok(0); // 目录还不存在(首次启动)不算错
        };
        let mut entries = Vec::new();
        for e in rd.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            let Ok(m) = meta.modified() else { continue };
            if meta.is_dir() {
                entries.push((e.path(), m));
            }
        }
        let doomed = crate::gc::expired_dirs(&entries, SystemTime::now(), ttl);
        let mut n = 0;
        for d in doomed {
            if fs::remove_dir_all(&d).is_ok() {
                n += 1;
            }
        }
        Ok(n)
    }
}
```

- [ ] **Step 4: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground job`
Expected: `9 passed`。

- [ ] **Step 5: 提交**

```bash
git add playground/src/job.rs playground/src/main.rs
git commit -m "feat(playground): 任务目录与产物落盘"
```

---

### Task 8: 队列与限流

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\queue.rs`
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（`mod queue;`）

**Interfaces:**
- Consumes: 无
- Produces:
  - `queue::Queue::new(max_jobs: usize, max_queue: usize) -> Arc<Queue>`
  - `Queue::try_acquire(self: &Arc<Self>, ip: &str) -> Result<Ticket, SubmitError>`
  - `Queue::snapshot(&self) -> Snapshot { running, waiting, max_jobs }`
  - `queue::Ticket`：`.is_running() -> bool`、`.position() -> usize`（1 起，在跑时为 0）、`.wait_started(&self).await`、`.ip() -> &str`；Drop 释放
  - `queue::SubmitError { IpBusy, QueueFull }`
  - `queue::client_key(ip: IpAddr) -> String`（IPv4 按 /32、IPv6 按 /64）

- [ ] **Step 1: 写失败测试**

`C:\projects\rust2genshin\playground\src\queue.rs`：

```rust
//! 并发额度 + 每 IP 一个 + 排队上限。
//!
//! 两条规则来自需求:同时最多 `max_jobs` 个任务;同一个 IP 同时只能有一个
//! (跑着或排着都算),第二次直接拒。NAT 后面多人共用一个出口 IP 会被误伤
//! —— 认了,回一句"请求过多，请稍后重试"。
//!
//! 额度的释放靠 `Ticket` 的 Drop:连接断开、任务失败、进程 panic 折叠栈
//! 都会走到它。**没有任何一条路径需要手工 `release()`**。

use std::{
    collections::{HashSet, VecDeque},
    net::IpAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use tokio::sync::Notify;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitError {
    IpBusy,
    QueueFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub running: usize,
    pub waiting: usize,
    pub max_jobs: usize,
}

/// IPv6 按 /64 归并:一个 /64 里的地址随便换就绕过了"每 IP 一个"。
/// IPv4 按 /32(即原样)。
pub fn client_key(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => {
            let mut seg = v6.segments();
            for s in &mut seg[4..] {
                *s = 0;
            }
            format!("{}/64", std::net::Ipv6Addr::from(seg))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn q(jobs: usize, queue: usize) -> Arc<Queue> {
        Queue::new(jobs, queue)
    }

    #[test]
    fn ipv4_keys_are_per_address() {
        let a: IpAddr = "1.2.3.4".parse().unwrap();
        let b: IpAddr = "1.2.3.5".parse().unwrap();
        assert_ne!(client_key(a), client_key(b));
        assert_eq!(client_key(a), client_key(a));
    }

    #[test]
    fn ipv6_keys_merge_within_a_64() {
        let a: IpAddr = "2001:db8:1:2::1".parse().unwrap();
        let b: IpAddr = "2001:db8:1:2:ffff:ffff:ffff:ffff".parse().unwrap();
        let c: IpAddr = "2001:db8:1:3::1".parse().unwrap();
        assert_eq!(client_key(a), client_key(b), "同一个 /64 要算同一个客户端");
        assert_ne!(client_key(a), client_key(c));
    }

    #[test]
    fn under_the_limit_runs_immediately() {
        let q = q(2, 4);
        let a = q.try_acquire("1.1.1.1").unwrap();
        assert!(a.is_running());
        assert_eq!(a.position(), 0);
        let b = q.try_acquire("2.2.2.2").unwrap();
        assert!(b.is_running());
        assert_eq!(q.snapshot().running, 2);
    }

    #[test]
    fn same_ip_twice_is_rejected() {
        let q = q(8, 8);
        let _a = q.try_acquire("1.1.1.1").unwrap();
        assert_eq!(q.try_acquire("1.1.1.1").unwrap_err(), SubmitError::IpBusy);
    }

    #[test]
    fn queued_ticket_still_counts_as_that_ip() {
        let q = q(1, 4);
        let _a = q.try_acquire("1.1.1.1").unwrap();
        let b = q.try_acquire("2.2.2.2").unwrap();
        assert!(!b.is_running());
        assert_eq!(q.try_acquire("2.2.2.2").unwrap_err(), SubmitError::IpBusy);
    }

    #[test]
    fn release_frees_both_the_ip_and_the_slot() {
        let q = q(1, 4);
        let a = q.try_acquire("1.1.1.1").unwrap();
        drop(a);
        assert_eq!(q.snapshot().running, 0);
        assert!(q.try_acquire("1.1.1.1").is_ok(), "同一 IP 释放后要能再提交");
    }

    #[test]
    fn position_is_one_based_and_snapshot_counts() {
        let q = q(1, 4);
        let _a = q.try_acquire("1.1.1.1").unwrap();
        let b = q.try_acquire("2.2.2.2").unwrap();
        let c = q.try_acquire("3.3.3.3").unwrap();
        assert_eq!(b.position(), 1);
        assert_eq!(c.position(), 2);
        let s = q.snapshot();
        assert_eq!((s.running, s.waiting, s.max_jobs), (1, 2, 1));
    }

    #[test]
    fn max_queue_is_enforced() {
        let q = q(1, 1);
        let _a = q.try_acquire("1.1.1.1").unwrap();
        let _b = q.try_acquire("2.2.2.2").unwrap();
        assert_eq!(q.try_acquire("3.3.3.3").unwrap_err(), SubmitError::QueueFull);
        // 被拒的 IP 不该留下痕迹
        assert_eq!(q.snapshot().waiting, 1);
    }

    #[test]
    fn dropping_a_queued_ticket_leaves_no_trace() {
        let q = q(1, 1);
        let _a = q.try_acquire("1.1.1.1").unwrap();
        let b = q.try_acquire("2.2.2.2").unwrap();
        drop(b);
        assert_eq!(q.snapshot().waiting, 0);
        assert!(q.try_acquire("2.2.2.2").is_ok());
        assert!(q.try_acquire("3.3.3.3").is_ok(), "额度空出来了,应该能排进来");
    }

    #[tokio::test]
    async fn waiting_ticket_starts_when_a_slot_frees() {
        let q = q(1, 1);
        let a = q.try_acquire("1.1.1.1").unwrap();
        let b = Arc::new(q.try_acquire("2.2.2.2").unwrap());
        let waiter = {
            let b = Arc::clone(&b);
            tokio::spawn(async move { b.wait_started().await })
        };
        drop(a);
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("额度交接必须唤醒排队者")
            .unwrap();
        assert!(b.is_running());
        assert_eq!(q.snapshot().running, 1);
    }

    #[tokio::test]
    async fn wait_started_returns_immediately_when_not_queued() {
        let q = q(4, 4);
        let a = q.try_acquire("1.1.1.1").unwrap();
        tokio::time::timeout(Duration::from_millis(50), a.wait_started())
            .await
            .expect("已在跑的票不该等");
    }
}
```

- [ ] **Step 2: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground queue`
Expected: 编译失败 —— `cannot find type Queue`。

- [ ] **Step 3: 实现**

在 `client_key` 之后、`#[cfg(test)]` 之前插入：

```rust
#[derive(Default)]
struct TicketState {
    promoted: AtomicBool,
    notify: Notify,
}

#[derive(Default)]
struct Inner {
    running: usize,
    /// (客户端 key, 票的状态)。FIFO,谁先排谁先拿额度。
    waiting: VecDeque<(String, Arc<TicketState>)>,
    /// 占着额度的客户端 key —— 跑着和排着都算。
    ips: HashSet<String>,
}

pub struct Queue {
    max_jobs: usize,
    max_queue: usize,
    inner: Mutex<Inner>,
}

impl Queue {
    pub fn new(max_jobs: usize, max_queue: usize) -> Arc<Self> {
        Arc::new(Self {
            max_jobs: max_jobs.max(1),
            max_queue,
            inner: Mutex::new(Inner::default()),
        })
    }

    pub fn try_acquire(self: &Arc<Self>, ip: &str) -> Result<Ticket, SubmitError> {
        let mut inner = self.inner.lock().expect("队列锁中毒");
        if inner.ips.contains(ip) {
            return Err(SubmitError::IpBusy);
        }
        let state = Arc::new(TicketState::default());
        if inner.running < self.max_jobs {
            inner.running += 1;
            state.promoted.store(true, Ordering::SeqCst);
        } else {
            if inner.waiting.len() >= self.max_queue {
                return Err(SubmitError::QueueFull);
            }
            inner.waiting.push_back((ip.to_string(), Arc::clone(&state)));
        }
        inner.ips.insert(ip.to_string());
        Ok(Ticket {
            queue: Arc::clone(self),
            ip: ip.to_string(),
            state,
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        let inner = self.inner.lock().expect("队列锁中毒");
        Snapshot {
            running: inner.running,
            waiting: inner.waiting.len(),
            max_jobs: self.max_jobs,
        }
    }

    fn promote(&self, inner: &mut Inner) {
        while inner.running < self.max_jobs {
            let Some((_, st)) = inner.waiting.pop_front() else {
                break;
            };
            inner.running += 1;
            // 先置位再唤醒:等待方在 await 之前先查这个位,不会丢唤醒。
            st.promoted.store(true, Ordering::SeqCst);
            st.notify.notify_one();
        }
    }
}

pub struct Ticket {
    queue: Arc<Queue>,
    ip: String,
    state: Arc<TicketState>,
}

impl Ticket {
    pub fn ip(&self) -> &str {
        &self.ip
    }

    pub fn is_running(&self) -> bool {
        self.state.promoted.load(Ordering::SeqCst)
    }

    /// 排队中的位次(从 1 起);已经在跑返回 0。
    pub fn position(&self) -> usize {
        let inner = self.queue.inner.lock().expect("队列锁中毒");
        inner
            .waiting
            .iter()
            .position(|(_, st)| Arc::ptr_eq(st, &self.state))
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    /// 等到拿到并发额度才返回。
    pub async fn wait_started(&self) {
        loop {
            if self.state.promoted.load(Ordering::SeqCst) {
                return;
            }
            self.state.notify.notified().await;
        }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let mut inner = self.queue.inner.lock().expect("队列锁中毒");
        inner.ips.remove(&self.ip);
        if self.state.promoted.load(Ordering::SeqCst) {
            inner.running -= 1;
            self.queue.promote(&mut inner);
        } else {
            inner
                .waiting
                .retain(|(_, st)| !Arc::ptr_eq(st, &self.state));
        }
    }
}
```

- [ ] **Step 4: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground queue`
Expected: `11 passed`。`waiting_ticket_starts_when_a_slot_frees` 必须**稳定**通过 —— 连跑 20 次确认没有丢唤醒：

Run: `for i in $(seq 20); do cargo test -q -p rust2genshin-playground queue::tests::waiting_ticket_starts_when_a_slot_frees || break; done`

- [ ] **Step 5: 提交**

```bash
git add playground/src/queue.rs playground/src/main.rs
git commit -m "feat(playground): 并发队列与每 IP 限流"
```

---

### Task 9: 任务执行器

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\runner.rs`
- Create: `C:\projects\rust2genshin\playground\src\testing.rs`
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（`mod runner;`、`#[cfg(test)] mod testing;`）

**Interfaces:**
- Consumes: `docker::{FrameDecoder, Stream, RunSpec, run_args, kill_args}`、`job::{JobPaths, JobMeta}`、`image::ImageGuard`
- Produces:
  - `runner::JobSpec { task, source, image_tag, image_commit, target, paths: JobPaths }`
  - `runner::RunOutcome { exit_code: Option<i32>, timed_out: bool, elapsed: Duration, artifact_bytes: u64 }`
  - `runner::RunnerError { Spawn, NoArtifact { code }, Decode }`
  - `runner::LogSink = Arc<dyn Fn(Stream, &str) + Send + Sync>`
  - `runner::verdict(code: Option<i32>, artifact_bytes: Option<u64>) -> Result<u64, RunnerError>`
  - `#[async_trait] pub trait JobRunner: Send + Sync { async fn run(&self, job: &JobSpec, log: LogSink) -> Result<RunOutcome, RunnerError>; }`
  - `runner::DockerRunner { new(opts: DockerOptions) }`，`DockerOptions { docker, workdir, cpus, memory, pids_limit, user, timeout, containers_prefix }`
  - `testing::FakeRunner`（`Behavior::{Ok, Fail, Hang}`，`.calls()` 记录调用）

- [ ] **Step 1: 写失败测试（判定逻辑）**

`C:\projects\rust2genshin\playground\src\runner.rs`：

```rust
//! 跑一个任务容器。
//!
//! 退出码非 0 → 编译失败;**退出码 0 但没有 `/out/*.gia` 也判失败** ——
//! 那会给出一个指向不存在文件的下载链接,或者更糟:把上一次的产物当成
//! 这一次的结果。超时是唯一需要服务自己解释的失败(要带上已用时),其余
//! 一律把 cargo 的原文推给前端,服务不解析。

use std::{path::PathBuf, sync::Arc, time::Duration};

use thiserror::Error;

use crate::{
    docker::{FrameError, Stream},
    job::JobPaths,
};

#[derive(Debug, Clone)]
pub struct JobSpec {
    pub task: String,
    pub source: String,
    pub target: String,
    pub image_tag: String,
    pub image_commit: String,
    pub paths: JobPaths,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub exit_code: Option<i32>,
    pub elapsed: Duration,
    pub artifact_bytes: u64,
}

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("起容器失败:{0}")]
    Spawn(#[from] std::io::Error),
    #[error("容器退出码 {code:?} 但 /out/ 里没有 .gia")]
    NoArtifact { code: Option<i32> },
    #[error("超过 {0:?} 还没跑完,已经强制结束")]
    Timeout(Duration),
    #[error("解析容器输出失败:{0}")]
    Decode(#[from] FrameError),
}

pub type LogSink = Arc<dyn Fn(Stream, &str) + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_exit_with_artifact_is_ok() {
        assert_eq!(verdict(Some(0), Some(15860)), Ok(15860));
    }

    #[test]
    fn nonzero_exit_is_failure_even_with_artifact() {
        // cargo 有时会先写产物再报错(后端自己 warn/err 也算非 0)。
        match verdict(Some(101), Some(100)) {
            Err(RunnerError::NoArtifact { code }) => assert_eq!(code, Some(101)),
            other => panic!("期望 NoArtifact,得到 {other:?}"),
        }
    }

    #[test]
    fn zero_exit_without_artifact_is_failure() {
        // 这条抓的就是"cargo 判 fresh、跳过编译"那种静默错答案。
        match verdict(Some(0), None) {
            Err(RunnerError::NoArtifact { code }) => assert_eq!(code, Some(0)),
            other => panic!("期望 NoArtifact,得到 {other:?}"),
        }
    }

    #[test]
    fn killed_container_has_no_exit_code() {
        assert!(matches!(
            verdict(None, None),
            Err(RunnerError::NoArtifact { code: None })
        ));
    }

    #[test]
    fn timeout_says_so_instead_of_blaming_missing_output() {
        // 超时会走到 verdict 之外的那条分支(见 DockerRunner)。这条单测钉的是
        // 变体自己的契约:报的是"超时了多少",不是"没有产物"。
        let e = RunnerError::Timeout(Duration::from_secs(15));
        assert!(e.to_string().contains("15"), "{e}");
    }
}
```

- [ ] **Step 2: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground runner`
Expected: 编译失败 —— `cannot find function verdict`。

- [ ] **Step 3: 实现 verdict**

```rust
/// 只有"退出码 0 且产物在"才算成功。产物大小由调用方给出(`None` = 文件不在)。
pub fn verdict(code: Option<i32>, artifact_bytes: Option<u64>) -> Result<u64, RunnerError> {
    match (code, artifact_bytes) {
        (Some(0), Some(n)) => Ok(n),
        (code, _) => Err(RunnerError::NoArtifact { code }),
    }
}
```

- [ ] **Step 4: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground runner`
Expected: `4 passed`。

- [ ] **Step 5: 实现 trait 与 DockerRunner**

追加到 `runner.rs`（`#[cfg(test)]` 之前）：

```rust
#[async_trait::async_trait]
pub trait JobRunner: Send + Sync {
    /// 跑完返回结果。`log` 会在每一行输出到达时被调用 —— 实现里要把
    /// stdout / stderr 分开(靠 8 字节帧),前端就能把它们染成不同颜色。
    async fn run(&self, job: &JobSpec, log: LogSink) -> Result<RunOutcome, RunnerError>;
}

/// 任务容器里**固定**的那几个参数。做成一个结构体传进去,免得
/// "哪个任务偷偷少挂了一个 `--network none`"这种事发生。
#[derive(Debug, Clone)]
pub struct DockerOptions {
    pub docker: String,
    pub workdir: String,
    pub cpus: u32,
    pub memory: String,
    pub pids_limit: u32,
    pub user: String,
    pub timeout: Duration,
    pub container_prefix: String,
}

impl Default for DockerOptions {
    fn default() -> Self {
        Self {
            docker: "docker".into(),
            workdir: "/opt/rust2genshin".into(),
            cpus: 2,
            memory: "1536m".into(), // 写 1536m 不写 1.5g:docker 对小数容量不友好
            pids_limit: 256,
            user: "1001".into(),
            timeout: Duration::from_secs(15),
            container_prefix: "r2g".into(),
        }
    }
}

pub struct DockerRunner {
    opts: DockerOptions,
}

impl DockerRunner {
    pub fn new(opts: DockerOptions) -> Self {
        Self { opts }
    }

    async fn docker(&self, args: &[String]) -> std::io::Result<std::process::Output> {
        tokio::process::Command::new(&self.opts.docker)
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()
            .await
    }
}

#[async_trait::async_trait]
impl JobRunner for DockerRunner {
    async fn run(&self, job: &JobSpec, log: LogSink) -> Result<RunOutcome, RunnerError> {
        let name = format!("{}-{}", self.opts.container_prefix, job.task);
        let spec = crate::docker::RunSpec {
            name: name.clone(),
            image: job.image_tag.clone(),
            src_dir: job.paths.src_dir.clone(),
            out_dir: job.paths.out_dir.clone(),
            workdir: self.opts.workdir.clone(),
            cpus: self.opts.cpus,
            memory: self.opts.memory.clone(),
            pids_limit: self.opts.pids_limit,
            user: self.opts.user.clone(),
            source_file: crate::protocol::SOURCE_FILE.to_string(),
        };
        let mut child = tokio::process::Command::new(&self.opts.docker)
            .args(crate::docker::run_args(&spec))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        // 容器自己的 stdout 是分帧的(8 字节头 + 载荷),docker CLI 自己的
        // stderr 是裸文本 —— 两条分开读。
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let loud = Arc::clone(&log);
        let stdout_task = tokio::spawn(async move {
            read_frames(stdout, loud).await
        });
        let lerr = Arc::clone(&log);
        let stderr_task = tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                lerr(Stream::Stderr, &format!("{l}\n"));
            }
        });

        // 超时**从容器启动起算** —— docker run 返回之前不计时,启动本身
        // 的开销不该算在用户的 15 秒里。
        let started = std::time::Instant::now();
        let mut timed_out = false;
        let status = match tokio::time::timeout(self.opts.timeout, child.wait()).await {
            Ok(s) => Some(s?),
            Err(_) => {
                timed_out = true;
                // 光 kill CLI 不够:容器会留在那儿跑。先 docker kill。
                let _ = self.docker(&crate::docker::kill_args(&name)).await;
                let _ = child.kill().await;
                let _ = child.wait().await;
                None
            }
        };
        let elapsed = started.elapsed();
        let _ = tokio::join!(stdout_task, stderr_task);

        // 超时要走自己那条路:被杀掉的容器既没有退出码也没有产物,
        // 交给 `verdict` 只会报成"没有产物" —— 用户看到的是"编译没产出",
        // 而不是"你写了个死循环"。
        if timed_out {
            return Err(RunnerError::Timeout(elapsed));
        }

        // `--rm` 保证不留僵尸容器;产物已经落在宿主挂进来的 out/ 上。
        let bytes = std::fs::metadata(&job.paths.artifact).ok().map(|m| m.len());
        let code = status.as_ref().and_then(|s| s.code());
        let n = verdict(code, bytes)?;
        Ok(RunOutcome {
            exit_code: code,
            elapsed,
            artifact_bytes: n,
        })
    }
}

async fn read_frames<R>(r: R, log: LogSink) -> Result<(), FrameError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;
    let mut r = r;
    let mut dec = crate::docker::FrameDecoder::new();
    let mut buf = vec![0u8; 8192];
    loop {
        let n = r.read(&mut buf).await.map_err(|e| FrameError::Io(e.to_string()))?;
        if n == 0 {
            return Ok(());
        }
        for f in dec.push(&buf[..n])? {
            log(f.stream, &String::from_utf8_lossy(&f.data));
        }
    }
}
```

- [ ] **Step 6: 写 FakeRunner 与它的测试**

`C:\projects\rust2genshin\playground\src\testing.rs`：

```rust
//! 只在测试里编译的替身。放一个文件而不是每个测试各写一份,
//! 是为了 HTTP / WS / 整条链路三处用的是同一个行为定义。

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use crate::{
    docker::Stream,
    job::JobPaths,
    runner::{JobRunner, JobSpec, LogSink, RunOutcome, RunnerError},
};

#[derive(Clone)]
pub enum Behavior {
    /// 吐这些行,退出码 0,写出产物。
    Ok { lines: Vec<(Stream, String)>, bytes: u64 },
    /// 吐这些行,退出码非 0,不写产物。
    Fail { lines: Vec<(Stream, String)>, code: i32 },
    /// 卡住不返回,让上层的超时路径被走到。
    Hang,
}

pub struct FakeRunner {
    behavior: Mutex<Behavior>,
    calls: Mutex<Vec<String>>,
    running: AtomicUsize,
}

impl FakeRunner {
    pub fn new(behavior: Behavior) -> Arc<Self> {
        Arc::new(Self {
            behavior: Mutex::new(behavior),
            calls: Mutex::new(Vec::new()),
            running: AtomicUsize::new(0),
        })
    }

    /// 中途换行为。测试用它模拟「容器卡住 → 上层超时把它掐了」这类时序
    /// 变化,所以必须在 `run` 已经开始之后还能改 —— 字段是 `Mutex` 而不是
    /// 裸 `Behavior` 就是为了这一步。
    pub fn set(&self, b: Behavior) {
        *self.behavior.lock().unwrap() = b;
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub fn running(&self) -> usize {
        self.running.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl JobRunner for FakeRunner {
    async fn run(&self, job: &JobSpec, log: LogSink) -> Result<RunOutcome, RunnerError> {
        self.calls.lock().unwrap().push(job.task.clone());
        self.running.fetch_add(1, Ordering::SeqCst);
        // 先克隆一份再 match:锁只在这一行里握着,后面 `run` 里要 await,
        // 拿着 `MutexGuard` 跨 await 会让整个 fake 变成 !Send。
        let behavior = self.behavior.lock().unwrap().clone();
        let out = match &behavior {
            Behavior::Ok { lines, bytes } => {
                for (s, t) in lines {
                    log(*s, t);
                }
                std::fs::write(&job.paths.artifact, vec![0u8; *bytes as usize]).unwrap();
                Ok(RunOutcome {
                    exit_code: Some(0),
                    elapsed: Duration::from_millis(1),
                    artifact_bytes: *bytes,
                })
            }
            Behavior::Fail { lines, code } => {
                for (s, t) in lines {
                    log(*s, t);
                }
                Err(RunnerError::NoArtifact { code: Some(*code) })
            }
            Behavior::Hang => {
                tokio::time::sleep(Duration::from_secs(3600)).await;
                unreachable!()
            }
        };
        self.running.fetch_sub(1, Ordering::SeqCst);
        out
    }
}

/// 给测试用的 `JobPaths`:在临时目录里建一份真的。
pub fn fake_paths(dir: &std::path::Path, task: &str) -> JobPaths {
    let src = dir.join(task).join("src");
    let out = dir.join(task).join("out");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    JobPaths {
        artifact: out.join(crate::job::ARTIFACT_NAME),
        dir: dir.join(task),
        src_dir: src,
        out_dir: out,
    }
}
```

测试：只测 `fake_paths` 与 `FakeRunner` 的记账（`calls` / `running` 回到 0），因为它是测试工具本身：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn spec(dir: &std::path::Path, task: &str) -> JobSpec {
        JobSpec {
            task: task.into(),
            source: "fn main() {}".into(),
            target: "beyond-server".into(),
            image_tag: "rust2genshin-play:0123456".into(),
            image_commit: "0123456".into(),
            paths: fake_paths(dir, task),
        }
    }

    #[tokio::test]
    async fn ok_writes_artifact_and_reports_bytes() {
        let d = tempfile::tempdir().unwrap();
        let r = FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 7 });
        let s = spec(d.path(), "t1");
        let out = r.run(&s, Arc::new(|_, _| {})).await.unwrap();
        assert_eq!(out.artifact_bytes, 7);
        assert!(s.paths.artifact.is_file());
        assert_eq!(r.calls(), vec!["t1"]);
        assert_eq!(r.running(), 0, "收尾要把计数还回去");
    }

    #[tokio::test]
    async fn fail_leaves_no_artifact() {
        let d = tempfile::tempdir().unwrap();
        let r = FakeRunner::new(Behavior::Fail { lines: vec![], code: 101 });
        let s = spec(d.path(), "t1");
        assert!(r.run(&s, Arc::new(|_, _| {})).await.is_err());
        assert!(!s.paths.artifact.exists());
    }
}
```

- [ ] **Step 7: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground`
Expected: 之前所有测试 + `runner` 4 条 + `testing` 2 条全绿。

- [ ] **Step 8: 提交**

```bash
git add playground/src/runner.rs playground/src/testing.rs playground/src/main.rs
git commit -m "feat(playground): 任务执行器与测试替身"
```

---

### Task 10: 烘焙状态机

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\bake.rs`
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（`mod bake;`）

**Interfaces:**
- Consumes: `gc::DiskGate`
- Produces:
  - `bake::BakeState { Running, Ok, Failed, NoDisk }`（serde snake_case）
  - `bake::BakeRequest { sha: Option<String>, note: Option<String> }`
  - `bake::BuiltImage { sha: String, tag: String }`
  - `#[async_trait] pub trait ImageBuilder: Send + Sync { async fn build(&self, sha: Option<&str>, log: &(dyn Fn(&str) + Send + Sync)) -> Result<BuiltImage, String>; }`
  - `bake::BakeStatus { serving: Option<String>, serving_tag: Option<String>, baking: Option<BakingStatus> }`、`bake::BakingStatus { sha, state, note, tail }`
  - `bake::Baker::new(builder: Arc<dyn ImageBuilder>, gate: Arc<dyn DiskGate>) -> Arc<Baker>`、`.request(&self, req: BakeRequest)`、`.status(&self) -> BakeStatus`、`.tail(&self) -> Vec<String>`、`.ready(&self) -> bool`、`.step(self: &Arc<Self>).await -> bool`、`.run_loop(self: Arc<Self>)`
  - `bake::RING_CAPACITY: usize = 200`

- [ ] **Step 1: 写失败测试**

`C:\projects\rust2genshin\playground\src\bake.rs`：

```rust
//! 烘焙状态机。
//!
//! ```
//! Idle ──bake(sha X)──→ Baking(X) ──ok──→ Ready(X, serving)
//!                           │
//!                           └──fail──→ Ready(旧 serving) + baking.state = failed
//! ```
//!
//! 三条规矩:**单飞**(同一时刻只有一个烘焙,免得把机器打满)、
//! **折叠 pending**(烘焙中又来的 push 只记最后一个,中间的没人来得及用)、
//! **失败保旧**(main 上推了个编不过的 commit 不该把 playground 打死)。
//! 从未成功过时进程继续跑,`/healthz` 报 not_ready 带日志尾巴 —— 前端能
//! 把日志显示出来,比进程没了强。

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use tokio::sync::Notify;

use crate::gc::DiskGate;

/// `/status` 给的日志尾巴行数。
pub const RING_CAPACITY: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BakeState {
    Running,
    Ok,
    Failed,
    NoDisk,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BakeRequest {
    /// `None` = 烘当前工作区 HEAD,由 `ImageBuilder` 自己解析。
    pub sha: Option<String>,
    /// 给 `/status` 看的一句话(pull 跳过的原因之类)。
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltImage {
    pub sha: String,
    pub tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BakingStatus {
    pub sha: Option<String>,
    pub state: BakeState,
    pub note: Option<String>,
    pub tail: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BakeStatus {
    pub serving: Option<String>,
    pub serving_tag: Option<String>,
    pub baking: Option<BakingStatus>,
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Mutex,
        },
        time::Duration,
    };

    use super::*;

    struct FakeBuilder {
        results: Mutex<VecDeque<Result<BuiltImage, String>>>,
        lines: Vec<String>,
        calls: Mutex<Vec<Option<String>>>,
    }

    impl FakeBuilder {
        fn new(results: Vec<Result<BuiltImage, String>>) -> Arc<Self> {
            Arc::new(Self {
                results: Mutex::new(results.into()),
                lines: vec!["step 1/4".into(), "step 4/4".into()],
                calls: Mutex::new(Vec::new()),
            })
        }
        fn calls(&self) -> Vec<Option<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl ImageBuilder for FakeBuilder {
        async fn build(
            &self,
            sha: Option<&str>,
            log: &(dyn Fn(&str) + Send + Sync),
        ) -> Result<BuiltImage, String> {
            self.calls.lock().unwrap().push(sha.map(str::to_string));
            for l in &self.lines {
                log(l);
            }
            self.results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Err("没有更多预设结果".into()))
        }
    }

    struct OkGate;
    #[async_trait::async_trait]
    impl DiskGate for OkGate {
        async fn ensure_free(&self) -> Result<(), String> {
            Ok(())
        }
    }

    struct LowGate;
    #[async_trait::async_trait]
    impl DiskGate for LowGate {
        async fn ensure_free(&self) -> Result<(), String> {
            Err("磁盘可用空间 1 字节低于水位".into())
        }
    }

    fn image(sha: &str) -> BuiltImage {
        BuiltImage { sha: sha.into(), tag: format!("rust2genshin-play:{sha}") }
    }

    fn baker(b: Arc<FakeBuilder>, g: Arc<dyn DiskGate>) -> Arc<Baker> {
        Baker::new(b, g)
    }

    #[tokio::test]
    async fn first_bake_sets_serving() {
        let b = FakeBuilder::new(vec![Ok(image("aaa1111"))]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        assert!(!k.ready());
        k.request(BakeRequest::default());
        assert!(k.step().await);
        let s = k.status();
        assert!(k.ready());
        assert_eq!(s.serving.as_deref(), Some("aaa1111"));
        assert_eq!(s.serving_tag.as_deref(), Some("rust2genshin-play:aaa1111"));
        assert_eq!(s.baking, None, "成功后不再报 baking");
    }

    #[tokio::test]
    async fn failure_keeps_the_old_serving_image() {
        let b = FakeBuilder::new(vec![Ok(image("aaa1111")), Err("编译不过".into())]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        k.request(BakeRequest::default());
        k.step().await;
        k.request(BakeRequest { sha: Some("bbb2222".into()), note: None });
        k.step().await;
        let s = k.status();
        assert_eq!(s.serving.as_deref(), Some("aaa1111"), "失败的烘焙不该换掉 serving");
        let baking = s.baking.expect("失败要留在状态里");
        assert_eq!(baking.state, BakeState::Failed);
        assert_eq!(baking.sha.as_deref(), Some("bbb2222"));
        assert_eq!(baking.note.as_deref(), Some("编译不过"));
    }

    #[tokio::test]
    async fn no_disk_skips_the_builder() {
        let b = FakeBuilder::new(vec![Ok(image("aaa1111"))]);
        let k = baker(Arc::clone(&b), Arc::new(LowGate));
        k.request(BakeRequest::default());
        assert!(k.step().await);
        assert!(b.calls().is_empty(), "水位不够时不该起 docker build");
        let s = k.status();
        assert!(!k.ready());
        let baking = s.baking.unwrap();
        assert_eq!(baking.state, BakeState::NoDisk);
        assert!(baking.note.unwrap().contains("磁盘"));
    }

    #[tokio::test]
    async fn pending_is_folded_to_the_latest() {
        let b = FakeBuilder::new(vec![Ok(image("aaa1111")), Ok(image("ccc3333"))]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        k.request(BakeRequest { sha: Some("aaa1111".into()), note: None });
        k.request(BakeRequest { sha: Some("bbb2222".into()), note: None });
        k.request(BakeRequest { sha: Some("ccc3333".into()), note: None });
        assert!(k.step().await);
        assert!(!k.step().await, "折叠之后只剩一次烘焙");
        assert_eq!(b.calls(), vec![Some("ccc3333".into())]);
        assert_eq!(k.status().serving.as_deref(), Some("ccc3333"));
    }

    #[tokio::test]
    async fn log_lines_land_in_the_tail() {
        let b = FakeBuilder::new(vec![Ok(image("aaa1111"))]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        k.request(BakeRequest::default());
        k.step().await;
        assert_eq!(k.tail(), vec!["step 1/4".to_string(), "step 4/4".to_string()]);
    }

    #[tokio::test]
    async fn tail_is_capped() {
        let mut r = Ring::new(3);
        for i in 0..10 {
            r.push(&format!("line {i}"));
        }
        assert_eq!(r.snapshot(), vec!["line 7", "line 8", "line 9"]);
    }

    #[tokio::test]
    async fn run_loop_bakes_after_a_request() {
        let b = FakeBuilder::new(vec![Ok(image("aaa1111"))]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        let loop_handle = {
            let k = Arc::clone(&k);
            tokio::spawn(async move { k.run_loop().await })
        };
        // 循环已经在等 kick 了再发请求,免得测试靠 sleep 猜时序。
        tokio::time::sleep(Duration::from_millis(20)).await;
        k.request(BakeRequest::default());
        for _ in 0..50 {
            if k.ready() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(k.ready(), "run_loop 必须把请求烘出来");
        loop_handle.abort();
    }

    #[tokio::test]
    async fn step_without_a_request_does_nothing() {
        let b = FakeBuilder::new(vec![]);
        let k = baker(Arc::clone(&b), Arc::new(OkGate));
        assert!(!k.step().await);
        assert!(b.calls().is_empty());
    }
}
```

- [ ] **Step 2: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground bake`
Expected: 编译失败 —— `cannot find type Baker`。

- [ ] **Step 3: 实现**

```rust
#[async_trait::async_trait]
pub trait ImageBuilder: Send + Sync {
    /// 生成镜像。`sha = None` 表示烘当前工作区 HEAD(实现自己 `git rev-parse`)。
    /// 日志逐行回调,由 `Baker` 收进 ring buffer。
    async fn build(
        &self,
        sha: Option<&str>,
        log: &(dyn Fn(&str) + Send + Sync),
    ) -> Result<BuiltImage, String>;
}

/// 定长环形缓冲。烘焙输出逐行进这里(给 `/status` 看),同时原样打到 tty。
#[derive(Debug)]
pub struct Ring {
    cap: usize,
    lines: VecDeque<String>,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        Self { cap, lines: VecDeque::with_capacity(cap) }
    }

    pub fn push(&mut self, line: &str) {
        if self.lines.len() == self.cap {
            self.lines.pop_front();
        }
        self.lines.push_back(line.to_string());
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.lines.iter().cloned().collect()
    }
}

#[derive(Default)]
struct Inner {
    serving: Option<String>,
    serving_tag: Option<String>,
    /// 只能有一个:**覆盖**就是折叠。
    pending: Option<BakeRequest>,
    baking: Option<BakingStatus>,
}

pub struct Baker {
    builder: Arc<dyn ImageBuilder>,
    gate: Arc<dyn DiskGate>,
    inner: Mutex<Inner>,
    ring: Mutex<Ring>,
    kick: Notify,
}

impl Baker {
    pub fn new(builder: Arc<dyn ImageBuilder>, gate: Arc<dyn DiskGate>) -> Arc<Self> {
        Arc::new(Self {
            builder,
            gate,
            inner: Mutex::new(Inner::default()),
            ring: Mutex::new(Ring::new(RING_CAPACITY)),
            kick: Notify::new(),
        })
    }

    pub fn request(&self, req: BakeRequest) {
        self.inner.lock().expect("烘焙状态锁中毒").pending = Some(req);
        self.kick.notify_one();
    }

    pub fn status(&self) -> BakeStatus {
        let inner = self.inner.lock().expect("烘焙状态锁中毒");
        BakeStatus {
            serving: inner.serving.clone(),
            serving_tag: inner.serving_tag.clone(),
            baking: inner.baking.clone().map(|mut b| {
                b.tail = self.ring.lock().expect("烘焙日志锁中毒").snapshot();
                b
            }),
        }
    }

    pub fn tail(&self) -> Vec<String> {
        self.ring.lock().expect("烘焙日志锁中毒").snapshot()
    }

    pub fn ready(&self) -> bool {
        self.inner.lock().expect("烘焙状态锁中毒").serving.is_some()
    }

    /// 循环体的一步:有 pending 就烘一次,返回是否干了活。测试直接调它,
    /// 时序完全确定。
    pub async fn step(self: &Arc<Self>) -> bool {
        let Some(req) = self
            .inner
            .lock()
            .expect("烘焙状态锁中毒")
            .pending
            .take()
        else {
            return false;
        };

        {
            let mut inner = self.inner.lock().expect("烘焙状态锁中毒");
            inner.baking = Some(BakingStatus {
                sha: req.sha.clone(),
                state: BakeState::Running,
                note: req.note.clone(),
                tail: Vec::new(),
            });
        }
        self.ring.lock().expect("烘焙日志锁中毒").clear();

        let me = Arc::clone(self);
        let log = move |line: &str| {
            tracing::info!(target: "bake", "{line}");
            me.ring.lock().expect("烘焙日志锁中毒").push(line);
        };

        let outcome = match self.gate.ensure_free().await {
            Err(why) => Err((BakeState::NoDisk, why)),
            Ok(()) => match self.builder.build(req.sha.as_deref(), &log).await {
                Ok(built) => {
                    let mut inner = self.inner.lock().expect("烘焙状态锁中毒");
                    inner.serving = Some(built.sha);
                    inner.serving_tag = Some(built.tag);
                    Ok(())
                }
                Err(why) => Err((BakeState::Failed, why)),
            },
        };

        if let Err((state, note)) = outcome {
            // 失败保旧:serving 一个字都不动。
            let mut inner = self.inner.lock().expect("烘焙状态锁中毒");
            inner.baking = Some(BakingStatus {
                sha: req.sha,
                state,
                note: Some(note),
                tail: Vec::new(),
            });
        } else {
            self.inner.lock().expect("烘焙状态锁中毒").baking = None;
        }
        true
    }

    /// 后台循环:没活干就在 `kick` 上等,不轮询。
    pub async fn run_loop(self: Arc<Self>) {
        loop {
            if !self.step().await {
                self.kick.notified().await;
            }
        }
    }
}
```

- [ ] **Step 4: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground bake`
Expected: `8 passed`。

- [ ] **Step 5: 提交**

```bash
git add playground/src/bake.rs playground/src/main.rs
git commit -m "feat(playground): 烘焙状态机(单飞/折叠/失败保旧)"
```

---

### Task 11: 客户端识别与 webhook 校验

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\server\mod.rs`
- Create: `C:\projects\rust2genshin\playground\src\server\webhook.rs`
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（`mod server;`）

**Interfaces:**
- Consumes: `queue::client_key`
- Produces:
  - `server::client_ip(headers: &HeaderMap, peer: SocketAddr, trust_proxy: bool) -> IpAddr`
  - `server::origin_allowed(origin: Option<&str>, allowed: &[String]) -> bool`
  - `server::AppState`（字段见下，Task 12/13 用它装配 router）
  - `webhook::verify_signature(secret: &str, body: &[u8], header: Option<&str>) -> bool`
  - `webhook::is_push_to_main(event: Option<&str>, body: &[u8]) -> bool`

- [ ] **Step 1: 写失败测试（webhook）**

`C:\projects\rust2genshin\playground\src\server\webhook.rs`：

```rust
//! GitHub webhook 的签名校验。
//!
//! 签名是 `HMAC-SHA256(secret, 原始 body)`,放进 `X-Hub-Signature-256`。
//! 必须拿**未解析的原始字节**去算 —— 解析再重新序列化会得到不同的字节,
//! 签名必然对不上。

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// 常量时间比较(hmac 的 `verify_slice` 自带),不给时序攻击留缝。
pub fn verify_signature(secret: &str, body: &[u8], header: Option<&str>) -> bool {
    let Some(header) = header else { return false };
    let Some(hex_sig) = header.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(raw) = hex::decode(hex_sig.trim()) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&raw).is_ok()
}

/// 只认 `push` 事件且 ref 是 `refs/heads/main`。
pub fn is_push_to_main(event: Option<&str>, body: &[u8]) -> bool {
    if event != Some("push") {
        return false;
    }
    #[derive(serde::Deserialize)]
    struct Payload {
        #[serde(default)]
        r#ref: String,
    }
    serde_json::from_slice::<Payload>(body)
        .map(|p| p.r#ref == "refs/heads/main")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 这两条向量是用 python 的 hmac 独立算出来的,不是本函数算的。
    const SECRET: &str = "It's a Secret to Everybody";
    const BODY: &[u8] = b"Hello, World!";
    const SIG: &str = "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";

    #[test]
    fn correct_signature_passes() {
        assert!(verify_signature(SECRET, BODY, Some(SIG)));
    }

    #[test]
    fn wrong_secret_fails() {
        assert!(!verify_signature("wrong", BODY, Some(SIG)));
    }

    #[test]
    fn modified_body_fails() {
        assert!(!verify_signature(SECRET, b"Hello, World?", Some(SIG)));
    }

    #[test]
    fn missing_or_malformed_header_fails() {
        assert!(!verify_signature(SECRET, BODY, None));
        assert!(!verify_signature(SECRET, BODY, Some("deadbeef")));
        assert!(!verify_signature(SECRET, BODY, Some("sha1=abcd")));
        assert!(!verify_signature(SECRET, BODY, Some("sha256=zzzz")));
    }

    #[test]
    fn empty_secret_never_matches() {
        // GITHUB_WEBHOOK_SECRET 为空时端点必须整条 403 —— 这一层只保证
        // 空 secret 不会意外放行(空 key 的 HMAC 是算得出来的)。
        assert!(!verify_signature("", BODY, Some(SIG)));
    }

    #[test]
    fn push_to_main_is_accepted() {
        let body = br#"{"ref":"refs/heads/main","after":"abc"}"#;
        assert!(is_push_to_main(Some("push"), body));
    }

    #[test]
    fn other_branches_and_events_are_rejected() {
        let body = br#"{"ref":"refs/heads/main"}"#;
        assert!(!is_push_to_main(Some("pull_request"), body));
        assert!(!is_push_to_main(None, body));
        assert!(!is_push_to_main(Some("push"), br#"{"ref":"refs/heads/dev"}"#));
        assert!(!is_push_to_main(Some("push"), br#"{"ref":"refs/tags/v1"}"#));
        assert!(!is_push_to_main(Some("push"), b"not json"));
    }
}
```

- [ ] **Step 2: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground webhook`
Expected: 编译失败 —— `cannot find function verify_signature`。

- [ ] **Step 3: 实现**

在 `webhook.rs` 的 `#[cfg(test)]` 之前插入 `verify_signature` / `is_push_to_main`，并在文件顶加 `use hmac::{Hmac, Mac};` 与 `use sha2::Sha256;`（Step 1 里已经写了）。

- [ ] **Step 4: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground webhook`
Expected: `7 passed`。

- [ ] **Step 5: 写 client_ip / origin 的失败测试**

`C:\projects\rust2genshin\playground\src\server\mod.rs`：

```rust
//! HTTP / WS 服务的装配点。`server` 只依赖 `queue` / `job` / `runner`,
//! 反向不成立 —— 将来加 rust-analyzer 的长驻会话,是往这一层加 `session`
//! 模块,不动下面。

pub mod webhook;

use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

pub use crate::server::webhook::{is_push_to_main, verify_signature};

/// 客户端 IP。**只有** `PLAYGROUND_TRUST_PROXY=1` 时才看
/// `X-Forwarded-For` —— 否则谁都能伪造这个头绕过"每 IP 一个"。
pub fn client_ip(headers: &HeaderMap, peer: SocketAddr, trust_proxy: bool) -> IpAddr {
    if trust_proxy
        && let Some(first) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
        && let Ok(ip) = first.trim().parse::<IpAddr>()
    {
        return ip;
    }
    peer.ip()
}

/// WebSocket **不受 CORS 保护**,握手必须自己查 Origin。
/// 没有 Origin 头(非浏览器客户端:curl、测试)放行 —— 它没有"被第三方
/// 页面借用身份"的问题,而浏览器一定会带。
pub fn origin_allowed(origin: Option<&str>, allowed: &[String]) -> bool {
    match origin {
        None => true,
        Some(o) => allowed.iter().any(|a| a == o),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn peer(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    fn headers_with(name: &str, value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            name.parse::<axum::http::HeaderName>().unwrap(),
            HeaderValue::from_str(value).unwrap(),
        );
        h
    }

    #[test]
    fn without_trust_proxy_the_header_is_ignored() {
        let h = headers_with("x-forwarded-for", "9.9.9.9");
        assert_eq!(
            client_ip(&h, peer("1.2.3.4:5000"), false),
            "1.2.3.4".parse().unwrap()
        );
    }

    #[test]
    fn with_trust_proxy_the_first_hop_wins() {
        let h = headers_with("x-forwarded-for", "9.9.9.9, 8.8.8.8");
        assert_eq!(
            client_ip(&h, peer("1.2.3.4:5000"), true),
            "9.9.9.9".parse().unwrap()
        );
    }

    #[test]
    fn with_trust_proxy_but_no_header_falls_back_to_peer() {
        let h = HeaderMap::new();
        assert_eq!(
            client_ip(&h, peer("1.2.3.4:5000"), true),
            "1.2.3.4".parse().unwrap()
        );
    }

    #[test]
    fn garbage_forwarded_for_falls_back_to_peer() {
        let h = headers_with("x-forwarded-for", "not-an-ip");
        assert_eq!(
            client_ip(&h, peer("1.2.3.4:5000"), true),
            "1.2.3.4".parse().unwrap()
        );
    }

    #[test]
    fn origins() {
        let allowed = vec!["https://mzverse.github.io".to_string()];
        assert!(origin_allowed(Some("https://mzverse.github.io"), &allowed));
        assert!(!origin_allowed(Some("https://evil.example"), &allowed));
        assert!(origin_allowed(None, &allowed), "非浏览器客户端没有 Origin");
        assert!(!origin_allowed(Some("https://mzverse.github.io.evil.com"), &allowed));
    }
}
```

- [ ] **Step 6: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground server`
Expected: `12 passed`（webhook 7 + server 5）。

- [ ] **Step 7: 提交**

```bash
git add playground/src/server playground/src/main.rs
git commit -m "feat(playground): 客户端识别与 webhook 签名校验"
```

---

### Task 12: HTTP 面

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\server\http.rs`
- Modify: `C:\projects\rust2genshin\playground\src\server\mod.rs`（`pub mod http;` + `AppState`）
- Modify: `C:\projects\rust2genshin\playground\src\testing.rs`（把 `FakeBuilder` / `OkGate` / `LowGate` 从 `bake.rs` 的测试模块搬过来）
- Modify: `C:\projects\rust2genshin\playground\src\bake.rs`（测试模块改成 `use crate::testing::{FakeBuilder, OkGate, LowGate};`，删掉本地那三份）
- Modify: `C:\projects\rust2genshin\playground\Cargo.toml`（dev-dependencies 加 `tower`、`http-body-util`）
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（无，`server` 已经在 Task 11 挂上）

**Interfaces:**
- Consumes: `Config`、`Queue`、`Baker`、`JobStore`、`Repo`、`ImageRegistry`、`protocol::MAX_SOURCE_BYTES`
- Produces:
  - `server::AppState { cfg: Arc<Config>, queue: Arc<Queue>, baker: Arc<Baker>, jobs: Arc<JobStore>, repo: Arc<Repo>, registry: Arc<ImageRegistry>, started: Instant }`
  - `http::router(state: Arc<AppState>) -> axum::Router`
  - `http::StatusBody`（`Serialize`）
  - `http::sample_path(cfg: &Config) -> PathBuf`
  - `http::resolve_artifact(tasks_root: &Path, id: &str) -> Option<PathBuf>`

- [ ] **Step 1: 把测试替身挪到 `testing.rs`**

先把 `bake.rs` 测试模块里的 `FakeBuilder` / `OkGate` / `LowGate` 三个类型**搬**到 `testing.rs`（`pub`，去掉 `#[cfg(test)]` 之外的东西 —— `testing.rs` 整个模块已经在 `#[cfg(test)]` 之下），`bake.rs` 的测试模块顶部加：

```rust
    use crate::testing::{FakeBuilder, LowGate, OkGate};
```

这不是洁癖：HTTP 和 WS 的测试也要造一个「立刻烘好」的 `Baker`，不搬就得抄第二遍。

- [ ] **Step 2: 写失败测试**

`C:\projects\rust2genshin\playground\src\server\http.rs`：

```rust
//! HTTP 面：健康检查、状态、样例源码、产物下载、重建。
//!
//! CORS 只对浏览器生效;WebSocket 那侧不吃 CORS,得自己查 Origin(见 ws.rs)。
//! 这一层不做业务判断 —— 所有"能不能跑"都在 `queue` 和 `bake` 里。

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use axum::{
    Json, Router,
    body::Body,
    extract::{Path as AxPath, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    bake::BakeRequest,
    config::Config,
    image::ImageRegistry,
    job::JobStore,
    protocol::MAX_SOURCE_BYTES,
    queue::Queue,
    repo::Repo,
    server::webhook::{is_push_to_main, verify_signature},
};

pub struct AppState {
    pub cfg: Arc<Config>,
    pub queue: Arc<Queue>,
    pub baker: Arc<crate::bake::Baker>,
    pub jobs: Arc<JobStore>,
    pub repo: Arc<Repo>,
    pub registry: Arc<ImageRegistry>,
    /// 做成 trait object 是这一层全部可测性的来源:HTTP 和 WS 的测试
    /// 换成 `testing::FakeRunner`,不需要 docker。
    pub runner: Arc<dyn crate::runner::JobRunner>,
    pub started: Instant,
}

/// `/status` 的响应体。字段是给前端和运维看的,名字直接抄过去的
/// 就是这些 —— 改字段名等于改前端。
#[derive(Debug, Clone, Serialize)]
pub struct StatusBody {
    pub ready: bool,
    pub serving: Option<String>,
    pub serving_tag: Option<String>,
    pub baking: Option<crate::bake::BakingStatus>,
    pub running: usize,
    pub waiting: usize,
    pub max_jobs: usize,
    pub head: Option<String>,
    pub dirty: Option<bool>,
    pub uptime_secs: u64,
}

/// 用户源码在仓库里的路径。样例和任务用的是同一个文件 —— 用户点
/// "载入样例"拿到的就是仓库当前那份 `demo/src/lib.rs`。
pub fn sample_path(cfg: &Config) -> PathBuf {
    cfg.repo_dir.join(crate::protocol::SOURCE_FILE)
}

/// `/artifact/<id>` 的解析。**这是唯一的路径拼接点**,所以两道锁都放在这里:
/// 先按 id 的字符合法性挡掉 `../../etc/passwd` 这类,再 canonicalize 确认
/// 结果真的在 `tasks/` 底下(软链接、盘符大小写这类 canonicalize 才能识破)。
pub fn resolve_artifact(tasks_root: &Path, id: &str) -> Option<PathBuf> {
    if !crate::job::is_valid_task_id(id) {
        return None;
    }
    let p = tasks_root.join(id).join("out").join(crate::job::ARTIFACT_NAME);
    let real = p.canonicalize().ok()?;
    let root = tasks_root.canonicalize().ok()?;
    real.starts_with(&root).then_some(real)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        bake::Baker,
        config::resolve,
        gc::DiskGate,
        image::ImageRegistry,
        job::JobStore,
        queue::Queue,
        repo::Repo,
        testing::{Behavior, FakeBuilder, FakeRunner, OkGate},
    };

    fn cfg_of(repo: &Path) -> Config {
        let mut c = resolve(&|_| None, &repo.join("playground")).unwrap();
        c.repo_dir = repo.to_path_buf();
        c.data_dir = repo.join("data");
        c
    }

    /// 一个"已经烘好 `sha`"的 baker。`sha` 传空 vec 的 builder 就是一个
    /// 从没成功过的 baker —— 这条路径 `/healthz` 要报 503。
    async fn baked(results: Vec<Result<crate::bake::BuiltImage, String>>) -> Arc<Baker> {
        let baker = Baker::new(
            FakeBuilder::new(results),
            Arc::new(OkGate) as Arc<dyn DiskGate>,
        );
        baker.request(BakeRequest::default());
        baker.step().await;
        baker
    }

    fn baked_one(sha: &str) -> crate::bake::BuiltImage {
        crate::bake::BuiltImage {
            sha: sha.into(),
            tag: format!("rust2genshin-play:{sha}"),
        }
    }

    /// 测试里的 AppState 装配点只此一处 —— 三个测试模块(这里、ws、将来的)
    /// 都挂在这上面,加一个字段就只用改这里。
    fn state(repo: &Path, baker: Arc<Baker>) -> Arc<AppState> {
        let cfg = Arc::new(cfg_of(repo));
        Arc::new(AppState {
            queue: Queue::new(cfg.max_jobs, cfg.max_queue),
            baker,
            jobs: Arc::new(JobStore::new(cfg.data_dir.join("tasks"))),
            repo: Arc::new(Repo::new(cfg.repo_dir.clone())),
            registry: ImageRegistry::new(),
            runner: crate::testing::FakeRunner::new(crate::testing::Behavior::Ok {
                lines: vec![],
                bytes: 1,
            }),
            started: Instant::now(),
            cfg,
        })
    }

    /// "已经烘好 aaa1111"的状态,HTTP 面的测试几乎都用它。
    async fn ready_state(repo: &Path) -> Arc<AppState> {
        state(repo, baked(vec![Ok(baked_one("aaa1111"))]).await)
    }

    async fn body_json(res: Response) -> serde_json::Value {
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn get(state: Arc<AppState>, uri: &str) -> Response {
        router(state)
            .oneshot(axum::http::Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    /// 带 `Origin` 的 GET。CORS 中间件只在有 Origin 时才加回响应头,
    /// 查 `access-control-expose-headers` 必须走这个。
    async fn get_with_origin(state: Arc<AppState>, uri: &str, origin: &str) -> Response {
        router(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri(uri)
                    .header(header::ORIGIN, origin)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn healthz_is_ok_when_ready() {
        let d = tempfile::tempdir().unwrap();
        let res = get(ready_state(d.path()).await, "/healthz").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_json(res).await["ready"], true);
    }

    #[tokio::test]
    async fn healthz_is_503_before_the_first_bake() {
        // 从未烘成功过时进程还活着,但**不算健康** —— 带着日志尾巴报 503,
        // 前端能把原因显示出来,比进程直接退出强。
        let d = tempfile::tempdir().unwrap();
        let res = get(state(d.path(), baked(vec![]).await), "/healthz").await;
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body_json(res).await["ready"], false);
    }

    #[tokio::test]
    async fn status_reports_the_serving_image_and_queue() {
        let d = tempfile::tempdir().unwrap();
        let res = get(ready_state(d.path()).await, "/status").await;
        assert_eq!(res.status(), StatusCode::OK);
        let j = body_json(res).await;
        assert_eq!(j["serving"], "aaa1111");
        assert_eq!(j["serving_tag"], "rust2genshin-play:aaa1111");
        assert_eq!(j["max_jobs"], 8);
        assert_eq!(j["running"], 0);
        assert_eq!(j["waiting"], 0);
    }

    #[tokio::test]
    async fn status_survives_a_directory_that_is_not_a_git_repo() {
        // /status 不该因为 `git rev-parse` 失败而 500:运维面板在仓库被搬走、
        // git 没装好的时候也得打得开。
        let d = tempfile::tempdir().unwrap();
        let res = get(ready_state(d.path()).await, "/status").await;
        let j = body_json(res).await;
        assert_eq!(j["head"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn sample_returns_the_repo_file() {
        let d = tempfile::tempdir().unwrap();
        let lib = d.path().join(crate::protocol::SOURCE_FILE);
        std::fs::create_dir_all(lib.parent().unwrap()).unwrap();
        std::fs::write(&lib, "fn main() {}").unwrap();
        let res = get(ready_state(d.path()).await, "/sample").await;
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"fn main() {}");
    }

    #[tokio::test]
    async fn sample_is_404_when_the_file_is_gone() {
        let d = tempfile::tempdir().unwrap();
        let res = get(ready_state(d.path()).await, "/sample").await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn sample_carries_the_baked_sha_and_exposes_it_to_the_browser() {
        // 样例来自工作区、镜像来自某个 sha,两者可能不是同一个 commit
        // (典型:给 lib/ 加了 API 还没重烘焙)。前端要能看出这一点并提示,
        // 所以 `baked` 得随 /sample 回去 —— 光靠 /status 不够,那是两次
        // 请求,中间会变。
        let d = tempfile::tempdir().unwrap();
        let lib = d.path().join(crate::protocol::SOURCE_FILE);
        std::fs::create_dir_all(lib.parent().unwrap()).unwrap();
        std::fs::write(&lib, "fn main() {}").unwrap();
        let st = ready_state(d.path()).await;
        let res = get_with_origin(st, "/sample", "https://mzverse.github.io").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get("x-playground-baked").unwrap(),
            "aaa1111",
            "serving 的 sha 要跟着样例一起回去"
        );
        // 跨域下浏览器默认读不到自定义响应头,除非服务端显式 expose。
        // 忘了这一条,前端那句"样例比镜像新"的提示永远不会触发。
        let expose = res.headers()[header::ACCESS_CONTROL_EXPOSE_HEADERS]
            .to_str()
            .unwrap()
            .to_string();
        assert!(expose.contains("x-playground-baked"), "expose 里有:{expose}");
        assert!(expose.contains("x-playground-head"), "expose 里有:{expose}");
    }

    #[test]
    fn artifact_ids_never_escape_the_tasks_dir() {
        // 用真目录:root 不存在的话 canonicalize 一律失败,所有断言都会
        // "因为错误的原因通过"。
        let d = tempfile::tempdir().unwrap();
        let root = d.path().to_path_buf();
        let jobs = JobStore::new(root.clone());
        let p = jobs.prepare("deadbeef", "x").unwrap();
        std::fs::write(&p.artifact, b"gia").unwrap();

        assert!(resolve_artifact(&root, "deadbeef").is_some(), "正常 id 要能过");
        for bad in ["../secrets", "..%2fsecrets", "a/b", "", "..", "deadbeef/../x"] {
            assert_eq!(resolve_artifact(&root, bad), None, "{bad:?} 不该能过");
        }
        assert_eq!(resolve_artifact(&root, "0123456789abcdef"), None, "目录不存在就是 None");
    }

    #[tokio::test]
    async fn artifact_404s_on_traversal() {
        // Review Focus #4:id 里塞 `..` 只能得到 404,绝不能读到 tasks/ 外面。
        let d = tempfile::tempdir().unwrap();
        // 在 tasks/ 外面放一份"机密",确认拿不到。
        std::fs::write(d.path().join("data/secret.gia"), b"secret").unwrap();
        let st = ready_state(d.path()).await;
        let p = st.jobs.prepare("deadbeef", "x").unwrap();
        std::fs::write(&p.artifact, b"gia").unwrap();

        assert_eq!(
            get(Arc::clone(&st), "/artifact/deadbeef").await.status(),
            StatusCode::OK
        );
        for bad in ["%2e%2e%2fsecret.gia", "..", "a%2Fb", "%2e%2e"] {
            let res = get(Arc::clone(&st), &format!("/artifact/{bad}")).await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND, "{bad} 不该能读到东西");
        }
    }

    #[tokio::test]
    async fn artifact_download_has_a_filename() {
        let d = tempfile::tempdir().unwrap();
        let st = ready_state(d.path()).await;
        let p = st.jobs.prepare("deadbeef", "x").unwrap();
        std::fs::write(&p.artifact, b"gia").unwrap();
        let res = get(st, "/artifact/deadbeef").await;
        assert!(res.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("rust2genshin_demo.gia"));
    }

    #[tokio::test]
    async fn rebuild_needs_a_token() {
        let d = tempfile::tempdir().unwrap();
        let res = router(ready_state(d.path()).await)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/admin/rebuild")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "没配 token 就该整条关掉");
    }
}
```

- [ ] **Step 3: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground server::http`
Expected: 编译失败 —— `cannot find struct AppState`（`server/mod.rs` 里还没有）。

- [ ] **Step 4: 实现**

在 `server/mod.rs` 顶部加：

```rust
pub mod http;
```

并把 `AppState` 留在 `http.rs`（就是 Step 2 里那个），`server/mod.rs` 只 `pub use http::AppState;`。

在 `http.rs` 的 `#[cfg(test)]` 之前插入：

```rust
/// `/sample` 上用两个响应头捎回「样例出自哪个 commit / 镜像出自哪个
/// commit」。不用 JSON 包一层是因为正文要原样进编辑器 —— 包了的话前端
/// 每次都得剥一层。名字定成常量,`router` 里的 expose 列表和这里必须
/// 是同一份,写死字符串迟早对不上。
pub const HEADER_HEAD: &str = "x-playground-head";
pub const HEADER_BAKED: &str = "x-playground-baked";

pub fn router(state: Arc<AppState>) -> Router {
    let origins: Vec<HeaderValue> = state
        .cfg
        .allowed_origins
        .iter()
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect();
    let cors = if origins.is_empty() {
        // 显式配成空 = 谁都不许。比"不小心开了 Any"安全。
        CorsLayer::new()
    } else {
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any)
            // 跨域下自定义响应头默认**读不到**,/sample 的 head / baked
            // 必须在这里列出来,否则前端那句"样例比镜像新"的提示是死的。
            .expose_headers([
                HeaderName::from_static(HEADER_HEAD),
                HeaderName::from_static(HEADER_BAKED),
            ])
    };

    Router::new()
        .route("/healthz", get(healthz))
        .route("/status", get(status))
        .route("/sample", get(sample))
        .route("/artifact/{id}", get(artifact))
        .route("/admin/rebuild", post(admin_rebuild))
        .route("/ws", get(crate::server::ws::ws_upgrade))
        .layer(cors)
        .with_state(state)
}

async fn healthz(State(st): State<Arc<AppState>>) -> Response {
    let body = serde_json::json!({
        "ready": st.baker.ready(),
        "serving": st.baker.status().serving,
        "tail": st.baker.tail(),
    });
    let code = if st.baker.ready() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(body)).into_response()
}

async fn status(State(st): State<Arc<AppState>>) -> Json<StatusBody> {
    let bake = st.baker.status();
    let snap = st.queue.snapshot();
    // git 探测失败不是错误:head/dirty 是给运维看的锦上添花。
    let head = st.repo.head().await.ok();
    let dirty = st.repo.is_dirty().await.ok();
    Json(StatusBody {
        ready: bake.serving.is_some(),
        serving: bake.serving,
        serving_tag: bake.serving_tag,
        baking: bake.baking,
        running: snap.running,
        waiting: snap.waiting,
        max_jobs: snap.max_jobs,
        head,
        dirty,
        uptime_secs: st.started.elapsed().as_secs(),
    })
}

async fn sample(State(st): State<Arc<AppState>>) -> Response {
    let Ok(bytes) = tokio::fs::read(sample_path(&st.cfg)).await else {
        return (StatusCode::NOT_FOUND, "样例源码不在仓库里").into_response();
    };
    let mut res = ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], bytes).into_response();
    let h = res.headers_mut();
    // 两个头都是**有才发**。工作区不是 git 仓库时 head 探测会失败,那时
    // 发一个空串只会让前端以为"HEAD 是空的",不如不发。
    if let Ok(head) = st.repo.head().await {
        if let Ok(v) = HeaderValue::from_str(&head) {
            h.insert(HeaderName::from_static(HEADER_HEAD), v);
        }
    }
    if let Some(sha) = st.baker.status().serving {
        if let Ok(v) = HeaderValue::from_str(&sha) {
            h.insert(HeaderName::from_static(HEADER_BAKED), v);
        }
    }
    res
}

async fn artifact(State(st): State<Arc<AppState>>, AxPath(id): AxPath<String>) -> Response {
    let Some(path) = resolve_artifact(st.jobs.tasks_root(), &id) else {
        return (StatusCode::NOT_FOUND, "没有这个产物").into_response();
    };
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!(
                        "attachment; filename=\"{}\"",
                        crate::job::ARTIFACT_NAME
                    ),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "没有这个产物").into_response(),
    }
}

/// 两条进得来:GitHub webhook(HMAC)和运维手工触发(Bearer token)。
/// 两个都没配 = 403,不做"没配就放行"这种事。
async fn admin_rebuild(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let sig = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok());
    let event = headers.get("x-github-event").and_then(|v| v.to_str().ok());

    let authed_by_webhook = match (&st.cfg.webhook_secret, sig) {
        (Some(secret), Some(s)) => {
            if !verify_signature(secret, &body, Some(s)) {
                return (StatusCode::FORBIDDEN, "签名不对").into_response();
            }
            true
        }
        _ => false,
    };

    if !authed_by_webhook {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        let ok = match (&st.cfg.admin_token, token) {
            (Some(want), Some(got)) => want == got,
            _ => false,
        };
        if !ok {
            return (StatusCode::FORBIDDEN, "没带凭证或者凭证不对").into_response();
        }
    }

    // webhook 这条路只认 main 的 push。不是 push、不是 main,一律 202
    // 说"跳过" —— 回 4xx 会让 GitHub 反复重投,而它本来就不该触发。
    if authed_by_webhook && !is_push_to_main(event, &body) {
        return (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({ "accepted": false, "reason": "忽略:不是 main 的 push" })),
        )
            .into_response();
    }

    st.baker.request(BakeRequest {
        sha: None,
        note: Some(if authed_by_webhook { "webhook".into() } else { "手工触发".into() }),
    });
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "accepted": true })),
    )
        .into_response()
}
```

- [ ] **Step 5: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground server::http`
Expected: `11 passed`。

- [ ] **Step 6: 提交**

```bash
git add playground/src/server playground/src/testing.rs playground/src/bake.rs playground/Cargo.toml
git commit -m "feat(playground): HTTP 面(健康检查/状态/样例/产物/重建)"
```

---

### Task 13: WebSocket 与整条链路

**Files:**
- Create: `C:\projects\rust2genshin\playground\src\server\ws.rs`
- Modify: `C:\projects\rust2genshin\playground\src\server\mod.rs`（`pub mod ws;`）
- Modify: `C:\projects\rust2genshin\playground\Cargo.toml`（dependencies 加 `futures-util`；dev-dependencies 加 `tokio-tungstenite`）

**Interfaces:**
- Consumes: `AppState`、`Queue`、`Ticket`、`Baker`、`JobStore`、`JobRunner`、`ImageRegistry`、`protocol::*`、`server::origin_allowed`、`server::client_ip`
- Produces:
  - `ws::ws_upgrade(State<Arc<AppState>>, WebSocketUpgrade, HeaderMap, ConnectInfo<SocketAddr>) -> Response`
  - `ws::run_socket(socket: WebSocket, state: Arc<AppState>, ip: String)`
  - `ws::validate_submit(source: &str, target: Option<&str>) -> Result<(), ServerMsg>`
  - `ws::describe_error(e: &RunnerError, task: &str) -> ServerMsg`

- [ ] **Step 1: 写失败测试**

`C:\projects\rust2genshin\playground\src\server\ws.rs`：

```rust
//! WebSocket:一条连接、一问一答式地把整条链路走完。
//!
//! 一条连接同时只跑一个任务 —— 不然 `cancel` 取消的是哪一个?排队期间
//! **不阻塞**读循环(额度交接在 spawn 出去的任务里等),所以排队时断开、
//! 点取消都还能被看见。
//!
//! 断开 = 取消。任务被 `abort` 掉,它手里的 `Ticket` 随之 drop,该 IP 的
//! 占用和并发额度在同一步还回去 —— 泄漏的路径只有"进程死掉"这一条,
//! 那条路上操作系统会把容器一起收走。

use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    extract::{
        ConnectInfo, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    docker::Stream,
    job::{ARTIFACT_NAME, JobMeta},
    protocol::{ClientMsg, ErrorCode, MAX_SOURCE_BYTES, PROTOCOL_VERSION, SOURCE_FILE, ServerMsg},
    runner::{JobSpec, RunnerError},
    server::{AppState, client_ip, origin_allowed},
};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        bake::{BakeRequest, Baker, BuiltImage},
        config::resolve,
        gc::DiskGate,
        image::ImageRegistry,
        job::JobStore,
        queue::Queue,
        repo::Repo,
        server::http::{router, AppState},
        testing::{Behavior, FakeBuilder, FakeRunner, OkGate},
    };

    type Ws = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    /// 一个"已经烘好 aaa1111"的后端,`runner` 顶掉真容器。
    fn ready_state(root: &std::path::Path, runner: Arc<FakeRunner>) -> Arc<AppState> {
        let mut cfg = resolve(&|_| None, &root.join("playground")).unwrap();
        cfg.repo_dir = root.to_path_buf();
        cfg.data_dir = root.join("data");
        let cfg = Arc::new(cfg);
        Arc::new(AppState {
            queue: Queue::new(cfg.max_jobs, cfg.max_queue),
            baker: Baker::new(
                FakeBuilder::new(vec![Ok(BuiltImage {
                    sha: "aaa1111".into(),
                    tag: "rust2genshin-play:aaa1111".into(),
                })]),
                Arc::new(OkGate) as Arc<dyn DiskGate>,
            ),
            jobs: Arc::new(JobStore::new(cfg.data_dir.join("tasks"))),
            repo: Arc::new(Repo::new(cfg.repo_dir.clone())),
            registry: ImageRegistry::new(),
            runner,
            started: std::time::Instant::now(),
            cfg,
        })
    }

    /// 起一个真服务(127.0.0.1:0),拿一个真 ws 客户端连上去。
    /// 不需要 docker:`FakeRunner` 顶掉容器那一段。
    async fn serve(state: Arc<AppState>) -> (String, Arc<AppState>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = router(Arc::clone(&state));
        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        (format!("ws://{addr}/ws"), state)
    }

    /// 建一个临时工作区 + 后端的组合。返回的 `TempDir` 必须活到测试结束,
    /// 别 `mem::forget`(那样每次测试都往 %TEMP% 里漏目录)。
    async fn spawn(runner: Arc<FakeRunner>) -> (tempfile::TempDir, String, Arc<AppState>) {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("demo/src")).unwrap();
        std::fs::write(d.path().join(SOURCE_FILE), "fn sample() {}").unwrap();
        let state = ready_state(d.path(), runner);
        // 先把 baker 烘好,不然每个用例都得先处理一条 not_ready。
        state.baker.request(BakeRequest::default());
        state.baker.step().await;
        let (url, state) = serve(state).await;
        (d, url, state)
    }

    /// 连上去,收 `n` 条消息(或直到超时)。收到的必须都是 ServerMsg。
    async fn read_n(ws: &mut Ws, n: usize) -> Vec<ServerMsg> {
        let mut out = Vec::new();
        for _ in 0..n {
            let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
                .await
                .expect("消息来晚了")
                .expect("连接断了")
                .unwrap();
            if let tokio_tungstenite::tungstenite::Message::Text(t) = msg {
                out.push(serde_json::from_str(&t).expect("服务发的东西必须是 ServerMsg"));
            }
        }
        out
    }

    async fn connect(url: &str) -> Ws {
        let (ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        ws
    }

    async fn send(ws: &mut Ws, m: ClientMsg) {
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&m).unwrap().into(),
        ))
        .await
        .unwrap();
    }

    fn submit(src: &str) -> ClientMsg {
        ClientMsg::Submit { source: src.into(), target: None }
    }

    // ── 纯校验:不需要连接,不需要 docker ─────────────────────────────

    #[test]
    fn validate_rejects_empty_and_whitespace_only_sources() {
        // Review Focus #1:空白源码必须在**占额度之前**被挡住,不能让它
        // 起一个注定编不过的容器、占住这个 IP 的排队位。
        for bad in ["", " ", "\n", "\t \r\n"] {
            let m = validate_submit(bad, None).unwrap_err();
            match m {
                ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::BadRequest, "{bad:?}"),
                other => panic!("期望 error,得到 {other:?}"),
            }
        }
    }

    #[test]
    fn validate_rejects_an_oversized_source() {
        // Review Focus #5:上限是"先量再分配"的依据,不能只靠消息上限兜底。
        let big = "x".repeat(MAX_SOURCE_BYTES + 1);
        assert!(validate_submit(&big, None).is_err());
        // 刚好卡在上限上是合法的 —— 边界不能off-by-one。
        let exact = "x".repeat(MAX_SOURCE_BYTES);
        assert!(validate_submit(&exact, None).is_ok());
    }

    #[test]
    fn validate_rejects_an_unknown_target() {
        assert!(validate_submit("fn main() {}", Some("beyond-server")).is_ok());
        assert!(validate_submit("fn main() {}", None).is_ok());
        let m = validate_submit("fn main() {}", Some("classic-server")).unwrap_err();
        match m {
            ServerMsg::Error { message, .. } => assert!(message.contains("classic-server"), "{message}"),
            other => panic!("期望 error,得到 {other:?}"),
        }
    }

    #[test]
    fn validate_lets_a_normal_source_through() {
        assert!(validate_submit("fn main() {}", None).is_ok());
    }

    #[test]
    fn describe_error_says_which_kind_of_failure_it_was() {
        // 三种失败给用户看的话必须不一样 —— 尤其是超时:它的容器既没有
        // 退出码也没有产物,落到"NoArtifact"那条上就成了"编译没产出",
        // 用户会去改代码,而问题是他写了个死循环。
        let t = describe_error(
            &crate::runner::RunnerError::Timeout(Duration::from_secs(15)),
            "t1",
        );
        match t {
            ServerMsg::Failed { message, exit_code, .. } => {
                assert!(message.contains("15"), "要说清超了多少秒:{message}");
                assert_eq!(exit_code, None);
            }
            other => panic!("期望 failed,得到 {other:?}"),
        }

        let n = describe_error(
            &crate::runner::RunnerError::NoArtifact { code: Some(101) },
            "t1",
        );
        match n {
            ServerMsg::Failed { exit_code, .. } => assert_eq!(exit_code, Some(101)),
            other => panic!("期望 failed,得到 {other:?}"),
        }
    }

    // ── 整条链路 ────────────────────────────────────────────────────

    #[tokio::test]
    async fn hello_carries_the_serving_commit() {
        let (_d, url, _st) = spawn(FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 })).await;
        let mut ws = connect(&url).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Hello { protocol, image_commit, file, max_jobs, targets, .. } => {
                assert_eq!(protocol, PROTOCOL_VERSION);
                assert_eq!(image_commit.as_deref(), Some("aaa1111"));
                assert_eq!(file, SOURCE_FILE);
                assert_eq!(max_jobs, 8);
                assert_eq!(targets, vec!["beyond-server".to_string()]);
            }
            other => panic!("第一条必须是 hello,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_submit_runs_all_the_way_to_finished() {
        let runner = FakeRunner::new(Behavior::Ok {
            lines: vec![(Stream::Stdout, "Compiling\n".into())],
            bytes: 15860,
        });
        let (_d, url, st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await; // hello
        send(&mut ws, submit("fn main() {}")).await;
        // FakeRunner 只吐一行日志,所以正好三条:started / log / finished。
        let msgs = read_n(&mut ws, 3).await;

        assert!(matches!(msgs[0], ServerMsg::Started { .. }), "{:?}", msgs[0]);
        match &msgs[1] {
            ServerMsg::Log { text, stream, .. } => {
                assert_eq!(text, "Compiling\n");
                assert_eq!(*stream, Stream::Stdout);
            }
            other => panic!("期望 log,得到 {other:?}"),
        }
        let task = match &msgs[2] {
            ServerMsg::Finished { ok, bytes, download, task } => {
                assert!(*ok);
                assert_eq!(*bytes, 15860);
                assert_eq!(download, &format!("/artifact/{task}"));
                task.clone()
            }
            other => panic!("期望 finished,得到 {other:?}"),
        };
        assert_eq!(runner.calls(), vec![task.clone()], "任务 id 要和 runner 收到的一致");
        assert!(st.jobs.artifact_of(&task).is_some(), "产物要真落在盘上");
        assert_eq!(st.queue.snapshot().running, 0, "跑完要把额度还回去");
    }

    #[tokio::test]
    async fn a_second_submit_on_the_same_connection_is_rejected() {
        let runner = FakeRunner::new(Behavior::Hang);
        let (_d, url, _st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        read_n(&mut ws, 1).await; // started
        send(&mut ws, submit("fn main() {}")).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::BadRequest),
            other => panic!("期望 bad_request,得到 {other:?}"),
        }
        assert_eq!(runner.calls().len(), 1);
    }

    #[tokio::test]
    async fn same_ip_on_a_second_connection_is_rejected() {
        let runner = FakeRunner::new(Behavior::Hang);
        let (_d, url, _st) = spawn(Arc::clone(&runner)).await;
        let mut a = connect(&url).await;
        read_n(&mut a, 1).await;
        send(&mut a, submit("fn main() {}")).await;
        read_n(&mut a, 1).await; // started

        let mut b = connect(&url).await;
        read_n(&mut b, 1).await; // hello
        send(&mut b, submit("fn main() {}")).await;
        match read_n(&mut b, 1).await[0].clone() {
            ServerMsg::Error { code, message } => {
                assert_eq!(code, ErrorCode::IpBusy);
                assert_eq!(message, "请求过多，请稍后重试");
            }
            other => panic!("期望 ip_busy,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_empty_source_never_reaches_the_runner() {
        let runner = FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 });
        let (_d, url, st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("   \n\t ")).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::BadRequest),
            other => panic!("期望 bad_request,得到 {other:?}"),
        }
        assert!(runner.calls().is_empty(), "空源码不该起容器");
        assert_eq!(st.queue.snapshot().running, 0);
        assert_eq!(st.queue.snapshot().waiting, 0);
    }

    #[tokio::test]
    async fn a_failed_run_reports_failed_with_the_exit_code() {
        let runner = FakeRunner::new(Behavior::Fail {
            lines: vec![(Stream::Stderr, "error[E0308]\n".into())],
            code: 101,
        });
        let (_d, url, _st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        let msgs = read_n(&mut ws, 3).await; // started / log / failed
        match &msgs[2] {
            ServerMsg::Failed { exit_code, .. } => assert_eq!(*exit_code, Some(101)),
            other => panic!("期望 failed,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn not_ready_is_reported_before_the_first_bake() {
        // 新的 baker 一次都没烘成功过 —— 提交必须回 not_ready,
        // 而不是排进队列然后等一个不存在的镜像。
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("demo/src")).unwrap();
        let st = ready_state(
            d.path(),
            FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 }),
        );
        let (url, _st) = serve(st).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        match read_n(&mut ws, 1).await[0].clone() {
            ServerMsg::Error { code, .. } => assert_eq!(code, ErrorCode::NotReady),
            other => panic!("期望 not_ready,得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn disconnecting_mid_job_frees_the_quota_and_the_ip() {
        // Review Focus #3:连接断在半路必须把额度还回去,否则一个客户端
        // 跑到一半挂断,能把这个 IP 和并发位永远占住。
        let runner = FakeRunner::new(Behavior::Hang);
        let (_d, url, st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        read_n(&mut ws, 1).await; // started
        assert_eq!(st.queue.snapshot().running, 1);

        drop(ws);
        for _ in 0..100 {
            if st.queue.snapshot().running == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(st.queue.snapshot().running, 0, "断开后额度必须还回去");

        // 而且同一个 IP 立刻能再提交。
        let mut again = connect(&url).await;
        read_n(&mut again, 1).await;
        send(&mut again, submit("fn main() {}")).await;
        assert!(
            matches!(read_n(&mut again, 1).await[0], ServerMsg::Started { .. }),
            "释放之后同一个 IP 应该能重新提交"
        );
    }

    #[tokio::test]
    async fn cancel_releases_the_slot() {
        let runner = FakeRunner::new(Behavior::Hang);
        let (_d, url, st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        read_n(&mut ws, 1).await; // started
        send(&mut ws, ClientMsg::Cancel {}).await;
        for _ in 0..100 {
            if st.queue.snapshot().running == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(st.queue.snapshot().running, 0);
    }

    #[tokio::test]
    async fn a_browser_origin_that_is_not_allowed_gets_a_403() {
        // WebSocket 不吃 CORS —— 这一条要是没有,任何网页都能拿访客的
        // 浏览器当跳板打这个后端。
        let (_d, url, _st) = spawn(FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 1 })).await;
        let req = tokio_tungstenite::tungstenite::handshake::client::Request::builder()
            .uri(&url)
            .header("Origin", "https://evil.example")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tokio_tungstenite::tungstenite::handshake::client::generate_key(),
            )
            .header("Host", "127.0.0.1")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .body(())
            .unwrap();
        let err = tokio_tungstenite::connect_async(req).await.unwrap_err();
        assert!(
            format!("{err}").contains("403"),
            "没在白名单里的 Origin 必须被握手挡掉:{err}"
        );
    }

    // 「不带 Origin 的客户端放行」不单写一条:上面每条 `connect` 走的都是
    // 这条路(tokio-tungstenite 默认不发 Origin),它们能连通本身就是这个
    // 保底的另一半。

    #[tokio::test]
    async fn the_task_meta_records_which_image_ran_it() {
        let runner = FakeRunner::new(Behavior::Ok { lines: vec![], bytes: 42 });
        let (_d, url, st) = spawn(Arc::clone(&runner)).await;
        let mut ws = connect(&url).await;
        read_n(&mut ws, 1).await;
        send(&mut ws, submit("fn main() {}")).await;
        let msgs = read_n(&mut ws, 3).await;
        let task = match &msgs[2] {
            ServerMsg::Finished { task, .. } => task.clone(),
            other => panic!("期望 finished,得到 {other:?}"),
        };
        let meta: JobMeta = st.jobs.read_meta(&task).unwrap();
        assert_eq!(meta.task, task);
        assert_eq!(meta.image_tag, "rust2genshin-play:aaa1111");
        assert_eq!(meta.image_commit, "aaa1111");
        assert_eq!(meta.target, "beyond-server");
        assert_eq!(meta.source_bytes, "fn main() {}".len());
        assert_eq!(
            st.jobs.artifact_of(&task).unwrap().file_name().unwrap(),
            ARTIFACT_NAME
        );
    }
}
```

- [ ] **Step 2: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground server::ws`
Expected: 编译失败 —— `cannot find function ws_upgrade`。

- [ ] **Step 3: 实现**

在 `ws.rs` 的 `#[cfg(test)]` 之前插入：

```rust
pub async fn ws_upgrade(
    State(st): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
    headers: HeaderMap,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> Response {
    let origin = headers.get(axum::http::header::ORIGIN).and_then(|v| v.to_str().ok());
    // CORS 管不着 WebSocket:浏览器对 ws:// 不预检,也不看 ACAO。
    // 不查这一句,任何页面都能拿用户的浏览器当跳板。
    if !origin_allowed(origin, &st.cfg.allowed_origins) {
        return (StatusCode::FORBIDDEN, "来源不在白名单里").into_response();
    }
    let ip = client_ip(&headers, peer, st.cfg.trust_proxy).to_string();
    // 排队时最长要等几十秒,握手完就得能持续收发。
    upgrade
        .max_message_size(MAX_SOURCE_BYTES + 4096)
        .on_upgrade(move |socket| run_socket(socket, st, ip))
}

/// 失败转成给用户看的消息。**没有任何一条路径吞掉失败** ——
/// 退出码 0 但没产物也走这里(见 runner::verdict)。
pub fn describe_error(e: &RunnerError, task: &str) -> ServerMsg {
    let (exit_code, message) = match e {
        RunnerError::NoArtifact { code } => (
            *code,
            "编译没有产出 .gia。容器输出见上面的日志。".to_string(),
        ),
        RunnerError::Timeout(t) => (
            None,
            format!("超过 {} 秒还没编完,已经结束。把图写小一点,或者拆成几个复合节点。", t.as_secs()),
        ),
        RunnerError::Spawn(why) => (None, format!("起不了容器:{why}")),
        RunnerError::Decode(why) => (None, format!("读容器输出失败:{why}")),
    };
    ServerMsg::Failed {
        task: task.to_string(),
        exit_code,
        message,
    }
}

pub async fn run_socket(socket: WebSocket, state: Arc<AppState>, ip: String) {
    let bake = state.baker.status();
    let hello = ServerMsg::Hello {
        protocol: PROTOCOL_VERSION,
        image_commit: bake.serving.clone(),
        file: SOURCE_FILE.to_string(),
        max_jobs: state.cfg.max_jobs,
        max_queue: state.cfg.max_queue,
        // MVP 只有服务器节点图,所以这张表只有一个 —— 但它是个表,
        // 加客户端节点图时是往这里 push,不是改协议。
        targets: vec!["beyond-server".to_string()],
    };
    let (mut sink, mut stream) = socket.split();
    if send(&mut sink, &hello).await.is_err() {
        return;
    }

    // 任务往这条通道里塞消息,不直接碰 socket —— 于是"任务在跑"和
    // "读循环还活着"是两件事,排队期间照样能看见断开和 cancel。
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();
    let mut job: Option<JoinHandle<()>> = None;

    loop {
        tokio::select! {
            incoming = stream.next() => {
                match incoming {
                    None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMsg>(&text) {
                            Err(e) => {
                                if send(&mut sink, &bad_request(format!("看不懂的消息:{e}"))).await.is_err() {
                                    break;
                                }
                            }
                            Ok(ClientMsg::Cancel {}) => {
                                if let Some(h) = job.take() {
                                    // abort → 任务手里的 Ticket 与 ImageGuard 一起 drop
                                    // → 额度、IP 占用、镜像引用计数一步还清。
                                    h.abort();
                                    let _ = h.await;
                                }
                            }
                            Ok(ClientMsg::Submit { source, target }) => {
                                if job.is_some() {
                                    if send(&mut sink, &bad_request("已有任务在跑,先发 cancel")).await.is_err() {
                                        break;
                                    }
                                    continue;
                                }
                                match start(&state, &ip, source, target, &tx) {
                                    Err(m) => {
                                        if send(&mut sink, &m).await.is_err() {
                                            break;
                                        }
                                    }
                                    Ok(handle) => job = Some(handle),
                                }
                            }
                        }
                    }
                    Some(Ok(_)) => {} // 二进制 / ping / pong:不收,也不回
                }
            }
            out = rx.recv() => {
                match out {
                    None => break,
                    Some(m) => {
                        let terminal = matches!(m, ServerMsg::Finished { .. } | ServerMsg::Failed { .. });
                        if send(&mut sink, &m).await.is_err() {
                            break;
                        }
                        if terminal {
                            job = None;
                        }
                    }
                }
            }
        }
    }

    // 每一条离开路径都走这里:断开、协议错、任务收尾。
    if let Some(h) = job.take() {
        h.abort();
        let _ = h.await;
    }
}

async fn send<S>(sink: &mut S, m: &ServerMsg) -> Result<(), axum::Error>
where
    S: futures_util::Sink<Message, Error = axum::Error> + Unpin,
{
    let json = serde_json::to_string(m).expect("ServerMsg 一定能序列化");
    sink.send(Message::Text(json.into())).await
}

fn bad_request(why: impl Into<String>) -> ServerMsg {
    ServerMsg::Error {
        code: ErrorCode::BadRequest,
        message: why.into(),
    }
}

fn err(code: ErrorCode, why: impl Into<String>) -> ServerMsg {
    ServerMsg::Error {
        code,
        message: why.into(),
    }
}

/// 提交内容的校验。**纯函数**,和 `start` 分开是为了让它能被单测直接调 ——
/// 这几条判断决定了"什么该起容器",不该只能通过开一条 WebSocket 去验。
///
/// 位置也很要紧:它在抢额度**之前**,所以一个空字符串既不会占住提交者的
/// IP 排队位,也不会浪费一个并发位。
pub fn validate_submit(source: &str, target: Option<&str>) -> Result<(), ServerMsg> {
    if source.trim().is_empty() {
        return Err(bad_request("源码是空的"));
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err(bad_request(format!(
            "源码 {} 字节,超过 {} 字节上限",
            source.len(),
            MAX_SOURCE_BYTES
        )));
    }
    if let Some(t) = target
        && t != "beyond-server"
    {
        return Err(bad_request(format!(
            "不认识的目标 {t},目前只有 beyond-server"
        )));
    }
    Ok(())
}

/// 校验 → 抢额度 → 起任务。返回 `Err(ServerMsg::Error)` 时**什么都没占**。
fn start(
    state: &Arc<AppState>,
    ip: &str,
    source: String,
    target: Option<String>,
    tx: &mpsc::UnboundedSender<ServerMsg>,
) -> Result<JoinHandle<()>, ServerMsg> {
    validate_submit(&source, target.as_deref())?;

    let bake = state.baker.status();
    let (Some(commit), Some(tag)) = (bake.serving.clone(), bake.serving_tag.clone()) else {
        return Err(err(ErrorCode::NotReady, "镜像还在烘焙,过一会儿再试"));
    };

    let ticket = match state.queue.try_acquire(ip) {
        Ok(t) => t,
        Err(crate::queue::SubmitError::IpBusy) => {
            return Err(err(ErrorCode::IpBusy, ErrorCode::ip_busy_message()));
        }
        Err(crate::queue::SubmitError::QueueFull) => {
            return Err(err(ErrorCode::QueueFull, "排队的人满了,过一会儿再试"));
        }
    };

    // 排队里就先把镜像引用握住:等在前面的时候 gc 可能正要清理旧镜像,
    // 计数是它唯一的依据。
    let guard = state.registry.acquire(&tag);

    if !ticket.is_running() {
        let snap = state.queue.snapshot();
        let _ = tx.send(ServerMsg::Queued {
            position: ticket.position(),
            queue: snap.waiting,
            max_jobs: snap.max_jobs,
        });
    }

    let state = Arc::clone(state);
    let tx = tx.clone();
    let task = crate::job::new_task_id();
    Ok(tokio::spawn(async move {
        // 额度交接在这里等 —— 不阻塞上面的读循环。
        ticket.wait_started().await;
        let _guard = guard;
        let _ = tx.send(ServerMsg::Started {
            task: task.clone(),
            image_commit: commit.clone(),
        });

        let outcome = run_one(&state, &task, &source, &tag, &commit, &tx).await;
        let _ = tx.send(outcome);
        // _guard 与 ticket 在这里 drop:额度、IP、镜像计数一起还。
    }))
}

async fn run_one(
    state: &Arc<AppState>,
    task: &str,
    source: &str,
    tag: &str,
    commit: &str,
    tx: &mpsc::UnboundedSender<ServerMsg>,
) -> ServerMsg {
    let paths = match state.jobs.prepare(task, source) {
        Ok(p) => p,
        Err(e) => {
            return err(ErrorCode::Internal, format!("建任务目录失败:{e}"));
        }
    };
    let meta = JobMeta {
        task: task.to_string(),
        target: "beyond-server".to_string(),
        image_tag: tag.to_string(),
        image_commit: commit.to_string(),
        created_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        source_bytes: source.len(),
    };
    if let Err(e) = state.jobs.write_meta(&paths, &meta) {
        return err(ErrorCode::Internal, format!("写 meta 失败:{e}"));
    }

    let spec = JobSpec {
        task: task.to_string(),
        source: source.to_string(),
        target: "beyond-server".to_string(),
        image_tag: tag.to_string(),
        image_commit: commit.to_string(),
        paths,
    };
    let sink_tx = tx.clone();
    let sink_task = task.to_string();
    let log: crate::runner::LogSink = Arc::new(move |stream: Stream, text: &str| {
        let _ = sink_tx.send(ServerMsg::Log {
            task: sink_task.clone(),
            stream,
            text: text.to_string(),
        });
    });

    match state.runner.run(&spec, log).await {
        Ok(out) => ServerMsg::Finished {
            task: task.to_string(),
            ok: true,
            bytes: out.artifact_bytes,
            download: format!("/artifact/{task}"),
        },
        Err(e) => describe_error(&e, task),
    }
}
```

- [ ] **Step 4: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground server::ws`
Expected: `16 passed`（5 条纯校验 + 11 条整链路）。其中
`disconnecting_mid_job_frees_the_quota_and_the_ip` 和 `cancel_releases_the_slot`
**必须真过** —— 它们是多跑两遍才看得到的那类:连跑 5 次确认不飘。

Run: `for i in $(seq 5); do cargo test -q -p rust2genshin-playground server::ws || break; done`

- [ ] **Step 5: 提交**

```bash
git add playground/src/server playground/Cargo.toml
git commit -m "feat(playground): WebSocket 链路(排队/取消/断开即释放)"
```

---

### Task 14: 进程装配与后台循环

**Files:**
- Modify: `C:\projects\rust2genshin\playground\src\main.rs`（整份重写）
- Create: `C:\projects\rust2genshin\playground\src\gc_loop.rs`

**Interfaces:**
- Consumes: 前面所有模块
- Produces:
  - `main.rs::main() -> ExitCode`
  - `main.rs::prepare_dirs(cfg: &Config) -> io::Result<()>`
  - `main.rs::check_tools() -> Result<(), String>`（`docker` / `git` 在不在 PATH）
  - `gc_loop::GC_INTERVAL: Duration`（300 秒）
  - `gc_loop::pick_image_victims(images: &[ImageInfo], keep: usize, registry: &ImageRegistry) -> Vec<String>`
  - `gc_loop::expired_tasks(jobs: &JobStore, cfg: &Config) -> Vec<PathBuf>`
  - `gc_loop::sweep_images(cfg: &Config, registry: &ImageRegistry) -> Result<usize, String>`（真跑 docker,无单测）

- [ ] **Step 1: 写失败测试**

`C:\projects\rust2genshin\playground\src\gc_loop.rs`：

```rust
//! 后台 gc:每 5 分钟一次,清过期任务目录 + 按策略清镜像。
//!
//! 清理有两条铁律:只碰带 `rust2genshin.play=1` 的镜像(宿主机上别人的
//! 镜像一个都不许动),以及正在被任务引用的镜像不动(引用计数在
//! `ImageRegistry` 里,`protected` 就是它当前非零的那些 tag)。

use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, SystemTime},
};

use crate::{
    config::Config,
    gc::{expired_dirs, select_images_to_remove, ImageInfo},
    image::ImageRegistry,
    job::JobStore,
};

pub const GC_INTERVAL: Duration = Duration::from_secs(300);

/// 选哪些镜像该删。纯函数:`registry` 只看快照,`keep` 是"每个 commit
/// 至少留几个"。
pub fn pick_image_victims(
    images: &[ImageInfo],
    keep: usize,
    registry: &ImageRegistry,
) -> Vec<String> {
    let protected: HashSet<String> = images
        .iter()
        .filter(|i| registry.count(&i.tag) > 0)
        .map(|i| i.tag.clone())
        .collect();
    select_images_to_remove(images, keep, &protected)
}

/// 过期任务目录的 mtime 基准:`artifact_ttl`。
pub fn expired_tasks(jobs: &JobStore, cfg: &Config) -> Vec<std::path::PathBuf> {
    let Ok(rd) = std::fs::read_dir(jobs.tasks_root()) else {
        return Vec::new();
    };
    let entries: Vec<_> = rd
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_dir().then(|| (e.path(), m.modified().ok()?))
        })
        .collect();
    expired_dirs(&entries, SystemTime::now(), cfg.artifact_ttl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc::ImageInfo;

    fn info(tag: &str, refcount: u32, age_secs: u64) -> ImageInfo {
        ImageInfo {
            tag: tag.to_string(),
            refcount,
            last_used: SystemTime::UNIX_EPOCH + Duration::from_secs(age_secs),
        }
    }

    #[test]
    fn in_flight_images_are_never_picked() {
        let reg = ImageRegistry::new();
        let _g = reg.acquire("play:a");
        let images = vec![info("play:a", 0, 1), info("play:b", 0, 2)];
        let victims = pick_image_victims(&images, 0, &reg);
        assert!(!victims.contains(&"play:a".to_string()), "有引用计数的不能删");
    }

    #[test]
    fn keep_is_honored() {
        let reg = ImageRegistry::new();
        let images = vec![info("play:a", 0, 1), info("play:b", 0, 2), info("play:c", 0, 3)];
        let victims = pick_image_victims(&images, 2, &reg);
        assert_eq!(victims, vec!["play:a".to_string()], "留两个,删最旧那个");
    }

    #[test]
    fn expired_tasks_uses_the_configured_ttl() {
        let d = tempfile::tempdir().unwrap();
        let jobs = JobStore::new(d.path().join("tasks"));
        jobs.prepare("t1", "x").unwrap();
        let mut cfg = crate::config::resolve(&|_| None, d.path()).unwrap();
        cfg.data_dir = d.path().to_path_buf();
        cfg.artifact_ttl = Duration::ZERO;
        assert_eq!(expired_tasks(&jobs, &cfg).len(), 1);
        cfg.artifact_ttl = Duration::from_secs(3600);
        assert!(expired_tasks(&jobs, &cfg).is_empty());
    }
}
```

- [ ] **Step 2: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground gc_loop`
Expected: 编译失败 —— `cannot find function pick_image_victims`（文件里只有测试）。

- [ ] **Step 3: 实现纯函数部分**

把 Step 1 里 `pick_image_victims` / `expired_tasks` 提到 `#[cfg(test)]` 之前。`ImageInfo { tag, refcount, last_used }` 来自 Task 5 的 `gc.rs`。

- [ ] **Step 4: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground gc_loop`
Expected: `3 passed`。

- [ ] **Step 5: 重写 main.rs**

```rust
//! playground 服务进程。
//!
//! 启动顺序是刻意的:先探工具(docker / git 不在就**直接退出**,不要
//! 变成一个"能连上但永远烘不出来"的服务),再抢数据目录的锁(同一台
//! 机器上两个实例会互相清对方的镜像),最后才起 HTTP。
//!
//! 烘焙是后台跑的,HTTP 立刻可用:`/healthz` 在烘好之前报 503。

mod bake;
mod config;
mod docker;
mod gc;
mod gc_loop;
mod image;
mod image_builder;
mod job;
mod protocol;
mod queue;
mod repo;
mod runner;
mod server;
#[cfg(test)]
mod testing;

use std::{fs, process::ExitCode, sync::Arc};

use tokio::net::TcpListener;

use crate::{
    bake::Baker,
    config::Config,
    gc::RealDiskGate,
    image::ImageRegistry,
    job::JobStore,
    queue::Queue,
    repo::Repo,
    runner::{DockerOptions, DockerRunner, JobRunner},
    server::{AppState, http},
};

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,rust2genshin_playground=debug".into()),
        )
        .init();

    if let Err(why) = check_tools() {
        eprintln!("{why}");
        return ExitCode::FAILURE;
    }

    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("配置有问题:{e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = prepare_dirs(&cfg) {
        eprintln!("准备目录失败:{e}");
        return ExitCode::FAILURE;
    }

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("起 tokio 运行时失败:{e}");
            return ExitCode::FAILURE;
        }
    };
    rt.block_on(async move {
        match run(cfg).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        }
    })
}

/// 两个外部程序缺一个都没法干活。**早失败**:服务起来之后再发现
/// `docker` 不在,症状是每个任务超时,排起来很费劲。
pub fn check_tools() -> Result<(), String> {
    for tool in ["docker", "git"] {
        let ok = std::process::Command::new(tool)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return Err(format!(
                "找不到可用的 `{tool}`(或者它跑不起来)。playground 需要 docker 与 git 都在 PATH 上。"
            ));
        }
    }
    Ok(())
}

/// docker 把镜像层和构建缓存放在哪。**磁盘水位必须量这一块。**
///
/// 这不是洁癖:部署机上 docker 的 data root 和数据目录在**两块不同的盘**上
/// (data root 在 /mnt/server 那块 loop ext4 上,数据目录在同盘的 /mnt/server
/// 下,而真正的风险是镜像层把 docker 那块盘吃空)。原来量的是数据目录 ——
/// 那块盘有 9.5T 空余,水位永远够,等于根本没设水位。
///
/// 读不出来就返回 `None`,由调用方退回数据目录:水位宁可量得糙一点,也比
/// 没有强。`check_tools` 已经在启动时保证 docker 至少存在。
pub fn docker_root() -> Option<PathBuf> {
    let out = std::process::Command::new("docker")
        .args(["info", "--format", "{{.DockerRootDir}}"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then(|| PathBuf::from(s))
}

pub fn prepare_dirs(cfg: &Config) -> std::io::Result<()> {
    fs::create_dir_all(cfg.data_dir.join("tasks"))?;
    Ok(())
}

/// 数据目录的独占锁。用 `fs4` 的**排他锁**而不是一个 pid 文件:进程被
/// `kill -9` 之后 pid 文件会留下,内核的锁不会。
fn lock_data_dir(cfg: &Config) -> std::io::Result<fs::File> {
    let path = cfg.data_dir.join("lock");
    let f = fs::File::create(&path)?;
    fs4::fs_std::FileExt::try_lock_exclusive(&f).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::AddrInUse,
            format!(
                "{} 已经被另一个 playground 实例锁住了 —— 同一台机器不要同时跑两个",
                path.display()
            ),
        )
    })?;
    Ok(f)
}

async fn run(cfg: Config) -> Result<(), String> {
    let cfg = Arc::new(cfg);
    let _lock = lock_data_dir(&cfg).map_err(|e| e.to_string())?;

    let repo = Arc::new(Repo::new(cfg.repo_dir.clone()));
    let registry = ImageRegistry::new();
    let jobs = Arc::new(JobStore::new(cfg.data_dir.join("tasks")));
    let queue = Queue::new(cfg.max_jobs, cfg.max_queue);

    let builder: Arc<dyn bake::ImageBuilder> = Arc::new(
        crate::image_builder::DockerImageBuilder::new(cfg.repo_dir.clone(), cfg.image.clone()),
    );
    // 水位量 docker 的 data root,不是数据目录 —— 会被吃满的是镜像那块的盘。
    // 读不出 docker info 时退回数据目录(至少还有个数),见 `docker_root`。
    let gate_path = docker_root().unwrap_or_else(|| {
        eprintln!("注意:读不出 docker 的 data root,磁盘水位改量数据目录——两个目录不在同一块盘时这个水位不算数。");
        cfg.data_dir.clone()
    });
    let gate: Arc<dyn gc::DiskGate> =
        Arc::new(RealDiskGate::new(gate_path, cfg.min_free_disk));
    let baker = Baker::new(builder, gate);

    let runner: Arc<dyn JobRunner> = Arc::new(DockerRunner::new(DockerOptions {
        timeout: cfg.job_timeout,
        ..DockerOptions::default()
    }));

    let state = Arc::new(AppState {
        queue: Arc::clone(&queue),
        baker: Arc::clone(&baker),
        jobs: Arc::clone(&jobs),
        repo: Arc::clone(&repo),
        registry: Arc::clone(&registry),
        runner,
        started: std::time::Instant::now(),
        cfg: Arc::clone(&cfg),
    });

    // 1) 启动时先看本地有没有现成的镜像:有就拿来用,不重复烘。
    //    这是"重启一下服务不该重编几分钟"的全部实现。
    tokio::spawn({
        let baker = Arc::clone(&baker);
        let cfg = Arc::clone(&cfg);
        let repo = Arc::clone(&repo);
        async move {
            match startup_image(&cfg, &repo).await {
                Some(built) => {
                    tracing::info!(tag = %built.tag, "本地已有可用镜像,跳过烘焙");
                    baker.adopt(built);
                }
                None => {
                    tracing::info!("本地没有可用镜像,开始烘焙");
                    baker.request(bake::BakeRequest {
                        sha: None,
                        note: Some("启动".into()),
                    });
                }
            }
            baker.clone().run_loop().await;
        }
    });

    // 2) gc 循环。
    tokio::spawn({
        let cfg = Arc::clone(&cfg);
        let jobs = Arc::clone(&jobs);
        let registry = Arc::clone(&registry);
        async move {
            loop {
                for p in gc_loop::expired_tasks(&jobs, &cfg) {
                    if let Err(e) = fs::remove_dir_all(&p) {
                        tracing::warn!(path = %p.display(), "删过期任务目录失败:{e}");
                    }
                }
                if let Err(e) = gc_loop::sweep_images(&cfg, &registry).await {
                    tracing::warn!("清镜像失败:{e}");
                }
                tokio::time::sleep(gc_loop::GC_INTERVAL).await;
            }
        }
    });

    let listener = TcpListener::bind(cfg.listen)
        .await
        .map_err(|e| format!("绑不住 {}:{e}", cfg.listen))?;
    tracing::info!(addr = %listener.local_addr().unwrap(), "playground 起来了");
    axum::serve(
        listener,
        http::router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .map_err(|e| e.to_string())
}
```

- [ ] **Step 6: 补 startup_image / Baker::adopt / sweep_images**

`main.rs` 用到但还没写的三样，各自按 TDD 加：

1. `Baker::adopt(&self, built: BuiltImage)` —— 把"本地已有的镜像"登记成 serving，`#[cfg(test)]` 里加一条:adopt 之后 `ready()` 为真、`status().serving` 是那个 sha、`baking` 是 `None`。

2. `startup_image(cfg: &Config, repo: &Repo) -> Option<BuiltImage>` —— 读 `repo.head()`(失败就 `None`)、`repo.is_dirty()`，算出 tag，然后 `docker image ls --filter label=rust2genshin.commit=<sha> --format {{.Repository}}:{{.Tag}}`，命中就返回。这次的 docker 调用是**真**的，所以这条测试标 `#[ignore]`:

```rust
#[tokio::test]
#[ignore = "需要本机有 docker"]
async fn startup_image_finds_a_locally_baked_image() { /* … */ }
```

`docker_root` 也要一条,同样标 `#[ignore]`(它要真的问 docker)。**在部署机上跑这条**,
它是"水位量对了盘"的唯一直接证据:

```rust
#[test]
#[ignore = "需要本机有 docker"]
fn docker_root_reports_where_images_live() {
    let p = docker_root().expect("有 docker 就该读得出来");
    assert!(p.is_absolute(), "{p:?}");
    assert!(p.exists(), "docker 的 data root 应该存在:{p:?}");
}
```


把 argv 依赖的纯逻辑（`tag` 的计算、`image::label_filter`）留给已有的单测。

3. `gc_loop::sweep_images(&cfg, &registry) -> Result<usize, String>`：

```rust
/// 列镜像 → 选victim → `docker rmi`。**列的时候一定带 label 过滤**,
/// 这是"只碰我们自己的镜像"那道锁;`docker image ls` 不带过滤会返回
/// 宿主上所有镜像,后面再筛就晚了。
pub async fn sweep_images(cfg: &Config, registry: &ImageRegistry) -> Result<usize, String> {
    let out = tokio::process::Command::new("docker")
        .args(crate::docker::image_ls_args(&crate::image::label_filter(
            &cfg.image,
            None,
        )))
        .output()
        .await
        .map_err(|e| format!("列镜像失败:{e}"))?;
    if !out.status.success() {
        return Err(format!(
            "列镜像退出码 {:?}:{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let images: Vec<ImageInfo> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|tag| ImageInfo {
            tag: tag.to_string(),
            refcount: registry.count(tag),
            last_used: std::time::SystemTime::now(),
        })
        .collect();

    let victims = pick_image_victims(&images, cfg.keep_images, registry);
    let mut removed = 0;
    for tag in victims {
        let st = tokio::process::Command::new("docker")
            .args(["rmi", &tag])
            .output()
            .await;
        match st {
            Ok(o) if o.status.success() => removed += 1,
            Ok(o) => tracing::warn!(
                tag = %tag,
                "删镜像失败:{}",
                String::from_utf8_lossy(&o.stderr).trim()
            ),
            Err(e) => tracing::warn!(tag = %tag, "删镜像失败:{e}"),
        }
    }
    if removed > 0 {
        // 顺手把 builder cache 收进预算里。`--keep-storage` 是"留到
        // 这个量",不是"删到这个量",所以它是个上限而不是精确值。
        let _ = tokio::process::Command::new("docker")
            .args(crate::docker::prune_args(cfg.build_cache_max))
            .output()
            .await;
    }
    Ok(removed)
}
```

> `sweep_images` 只做真命令,不写单测(它一半是 `docker` 的输出解析,一半是已有纯函数的编排);`pick_image_victims` 那三条才是它的判定。**别**为了"测一下"去 mock `Command`。

- [ ] **Step 7: 跑全量测试**

Run: `cargo test -p rust2genshin-playground`
Expected: 全绿(标了 `#[ignore]` 的 docker 项不算)。**这台 Windows 机器没有 docker**,所以这一条必须过 —— 过不了说明有测试偷偷依赖了 docker。

Run: `cargo test -p rust2genshin-playground -- --ignored`（在有 docker 的机器上跑）

- [ ] **Step 8: 手动冒烟**

Run: `cargo run -p rust2genshin-playground`
Expected: 本机没有 docker，因此应该在**探工具那一步就退出**并打印 `找不到可用的 docker`。这正好验证了 Step 5 里那条"早失败"。

- [ ] **Step 9: 提交**

```bash
git add playground/src/main.rs playground/src/gc_loop.rs playground/src/bake.rs playground/src/image_builder.rs
git commit -m "feat(playground): 进程装配、启动快路径与后台 gc"
```

---

### Task 15: 烘焙镜像的 Dockerfile

**Files:**
- Create: `C:\projects\rust2genshin\playground\Dockerfile`
- Create: `C:\projects\rust2genshin\.dockerignore`
- Create: `C:\projects\rust2genshin\playground\src\image_builder.rs`

**Interfaces:**
- Consumes: `bake::ImageBuilder`、`repo::{Repo, short_sha, is_sha_like}`、`image::{tag, LABEL_PLAY, LABEL_COMMIT}`
- Produces: `image_builder::DockerImageBuilder::{new(repo_dir, image, build_cache_max), }`，`impl ImageBuilder`

- [ ] **Step 1: 写 Dockerfile**

`C:\projects\rust2genshin\playground\Dockerfile`（**三层**，理由写在 spec 的表格下面那段引用里）：

```dockerfile
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
# uid 1001 是任务容器 `--user 1001` 指的那个。宿主上的 out/ 由服务端
# chmod 0777,容器才能把产物写出去。
RUN useradd -u 1001 -m -s /bin/bash builder
USER builder
# 工具链跟着 rust-toolchain.toml 走 —— 它装在 builder 的家目录里,
# 所以必须在 USER builder 之后跑。
COPY --chown=builder:builder rust-toolchain.toml /opt/rust2genshin/rust-toolchain.toml
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain none --profile minimal \
 && . "$HOME/.cargo/env" \
 && rustup show

WORKDIR /opt/rust2genshin
COPY --chown=builder:builder . .
# fetch 单独留一条:它让依赖下载和编译在日志里分得开,出问题时一眼看出
# 是网络慢还是编译慢。
RUN . "$HOME/.cargo/env" && cargo fetch
# 烘一次 demo。这一遍把一百多个 crate 都编进 target/,任务容器自己不挂
# target/,用的就是这个缓存 —— 这是"15 秒能编完一份节点图"的全部原因。
RUN . "$HOME/.cargo/env" && cargo run -p build-demo --bin beyond-server

# 任务容器从这一层起。镜像自己的 CMD 不会被用到(服务端总是显式给
# `sh -c "..."`),留着只为了 `docker run <image>` 手工排查时能进去看看。
CMD ["/bin/bash"]
```

- [ ] **Step 2: 写 .dockerignore**

`C:\projects\rust2genshin\.dockerignore`：

```gitignore
# context 越小，push 之后的 docker build 越快。
target/
.git/
.gitignore
tools/
export.gia*
*.ogia
docs/superpowers/
.superpowers/

# 注意:**不要**把 docs/ 和 viewer/ 排除掉 —— 它们是 workspace 成员,
# 少了任何一个 `cargo fetch` 都会因为 "no targets specified in the
# manifest" 直接失败。这一条是踩过的坑,别"顺手"清理。
```

- [ ] **Step 3: 写失败测试（DockerImageBuilder 的纯逻辑）**

`C:\projects\rust2genshin\playground\src\image_builder.rs`：

```rust
//! 真正去 `docker build` 的 `ImageBuilder`。
//!
//! 唯一需要小心的是 tag 的算法要和镜像里的 label 对齐:tag 带 `-dirty`
//! 后缀、label 只带 sha,清理和"本地有没有现成的"两处都靠这两个值互相找。

use std::path::PathBuf;

use crate::{
    bake::{BuiltImage, ImageBuilder},
    image::{tag, LABEL_COMMIT, LABEL_PLAY},
    repo::Repo,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_pin_the_dockerfile_and_labels() {
        let a = build_args(
            "linux/amd64",
            "rust2genshin-play:abc1234",
            "abc1234",
            PathBuf::from("/repo").as_path(),
        );
        assert_eq!(a[0], "build");
        assert!(a.contains(&"--file".to_string()));
        assert!(a.contains(&"playground/Dockerfile".to_string()));
        assert!(a.contains(&"--tag=rust2genshin-play:abc1234".to_string()));
        assert!(a.contains(&format!("--label={LABEL_PLAY}=1")));
        assert!(a.contains(&format!("--label={LABEL_COMMIT}=abc1234")));
        // 部署机的 dockerd 是 `iptables: false`,容器默认没网 —— 烘焙那几层
        // RUN 全靠这条才下得动 rustup / apt / cargo 依赖。
        assert!(a.contains(&"--network=host".to_string()));
        // `--progress=plain` 是 BuildKit 的旗标:加了它,legacy builder
        // (docker 19.03,部署机上没有 buildx)会直接报未知旗标。
        assert!(
            !a.iter().any(|x| x.starts_with("--progress")),
            "legacy builder 不认 --progress:{a:?}"
        );
        assert_eq!(a.last().unwrap(), "/repo", "最后一个参数是 context");
    }

    #[test]
    fn labels_and_tags_agree_on_the_commit() {
        // 全限定写 `label_filter`:它只在测试里用,放文件头的 `use` 会让
        // 非测试构建吃一个 unused_imports。
        let t = tag("play", "abcdef1234567890", false);
        let f = crate::image::label_filter("play", Some("abcdef1234567890"));
        assert!(f.contains("abcdef1234567890"), "按 commit 找镜像要用全长 sha:{f}");
        assert!(t.starts_with("play:abcdef1"), "{t}");
    }
}
```

- [ ] **Step 4: 跑测试，确认全红**

Run: `cargo test -p rust2genshin-playground image_builder`
Expected: 编译失败 —— `cannot find function build_args`。

- [ ] **Step 5: 实现**

```rust
/// `docker build` 的全部参数。`--platform` 单列出来:目标机是 amd64 而开发机
/// 是 arm 时,不钉住会烘出一个跑不起来的镜像,而且症状很难认。
///
/// `--network=host` 是被部署机逼出来的:那台机的 dockerd 配了
/// `"iptables": false`,容器默认没有出网,layered RUN 里的 `apt-get` /
/// `curl rustup` / `cargo fetch` 全会挂在 DNS 解析上。走宿主网络才下得动。
/// 这是**烘焙独有**的放宽 —— 任务容器仍然是 `--network none`。
///
/// **不要**加 `--progress=plain`:它是 BuildKit 的旗标,而这台机只有
/// docker 19.03 的 legacy builder(见 `build()` 里钉的 `DOCKER_BUILDKIT=0`)。
pub fn build_args(
    platform: &str,
    image_tag: &str,
    commit: &str,
    context: &std::path::Path,
) -> Vec<String> {
    vec![
        "build".into(),
        format!("--platform={platform}"),
        "--network=host".into(),
        "--file".into(),
        "playground/Dockerfile".into(),
        format!("--tag={image_tag}"),
        format!("--label={LABEL_PLAY}=1"),
        format!("--label={LABEL_COMMIT}={commit}"),
        context.display().to_string(),
    ]
}

pub struct DockerImageBuilder {
    repo_dir: PathBuf,
    image: String,
    platform: String,
    repo: Repo,
}

impl DockerImageBuilder {
    /// `build_cache_max` 不在这里 —— 它是 gc 那边 `docker builder prune`
    /// 的参数(见 `gc_loop::sweep_images`),builder 自己不管预算。
    pub fn new(repo_dir: PathBuf, image: String) -> Self {
        Self {
            repo: Repo::new(repo_dir.clone()),
            repo_dir,
            image,
            platform: "linux/amd64".into(),
        }
    }
}

#[async_trait::async_trait]
impl ImageBuilder for DockerImageBuilder {
    async fn build(
        &self,
        sha: Option<&str>,
        log: &(dyn Fn(&str) + Send + Sync),
    ) -> Result<BuiltImage, String> {
        // 镜像内容来自工作区目录,所以"烘哪个 commit"= 当前 HEAD。
        // 传进来的 sha 只用来对账,不切分支 —— 服务只服务 main。
        let head = self
            .repo
            .head()
            .await
            .map_err(|e| format!("读 HEAD 失败:{e}"))?;
        if let Some(want) = sha
            && !head.starts_with(want)
        {
            log(&format!("注意:工作区 HEAD 是 {head},不是 {want},按工作区烘"));
        }
        let dirty = self.repo.is_dirty().await.unwrap_or(true);
        let built_tag = tag(&self.image, &head, dirty);

        let mut child = tokio::process::Command::new("docker")
            // 钉死 legacy builder。部署机是 docker 19.03 且没装 buildx,
            // 而 BuildKit 一旦被打开,输出格式就换了(没有 `Step k/n`),
            // 日志解析和"进度看哪一行"全得跟着变。显式关掉,让两台机器
            // (开发机没有 docker,部署机是老版本)的行为一致。
            .env("DOCKER_BUILDKIT", "0")
            .args(build_args(
                &self.platform,
                &built_tag,
                &head,
                self.repo_dir.as_path(),
            ))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("起 docker build 失败:{e}"))?;

        // legacy builder 的进度(`Step k/n : …`)走 stderr;stdout 基本是
        // 空的,但两边都读,免得管道写满把 docker 卡住。
        let err_lines = read_lines(child.stderr.take().expect("piped"), log);
        let out_lines = read_lines(child.stdout.take().expect("piped"), log);
        let status = child
            .wait()
            .await
            .map_err(|e| format!("等 docker build 失败:{e}"))?;
        let _ = tokio::join!(err_lines, out_lines);

        if !status.success() {
            return Err(format!(
                "docker build 失败,退出码 {:?}。上面最后几行是原因。",
                status.code()
            ));
        }
        log(&format!("烘好了:{built_tag}"));
        Ok(BuiltImage {
            sha: head,
            tag: built_tag,
        })
    }
}

async fn read_lines<R>(r: R, log: &(dyn Fn(&str) + Send + Sync))
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut lines = BufReader::new(r).lines();
    while let Ok(Some(l)) = lines.next_line().await {
        log(&l);
    }
}
```

- [ ] **Step 6: 跑测试，确认全绿**

Run: `cargo test -p rust2genshin-playground image_builder`
Expected: `2 passed`。另外 `cargo clippy -p rust2genshin-playground --all-targets`
要干净 —— 这个 crate 有 `unused_imports` / `dead_code` 的地方就是没接上线的代码。

- [ ] **Step 7: 提交**

```bash
git add playground/Dockerfile playground/src/image_builder.rs .dockerignore
git commit -m "feat(playground): 烘焙镜像的 Dockerfile 与 builder"
```

---

### Task 16: 前端

**Files:**
- Create: `C:\projects\rust2genshin\docs\typst\lib\playground.typ`
- Create: `C:\projects\rust2genshin\docs\typst\lib\js\playground.js`
- Create: `C:\projects\rust2genshin\docs\typst\lib\css\playground.css`
- Modify: `C:\projects\rust2genshin\docs\typst\play\index.typ`（正文换成 `#playground()`）

**Interfaces:**
- Consumes: `ServerMsg` / `ClientMsg` 的 JSON 形状（Task 6）、`/sample`、`/artifact/<id>`
- Produces: `#playground()`，输出 `<playground-app>`

- [ ] **Step 1: 写 Typst 侧的壳**

`C:\projects\rust2genshin\docs\typst\lib\playground.typ`。**照 `lib/card.typ` 抄** —— 那里定下了本站自定义元素的三件套:`custom_element` 里声明 `<template>` 的原型、`import_script` 挂脚本、`import_style` 挂样式。样式**不进 shadow root**,而是像 `card.css` 那样用 `::part()` 从外面穿进去（`::part` 和 `::slotted` 是穿透 shadow 边界的两个官方口子）。

```typ
#import "/lib/lib.typ": *;

// 摆弄页的主体。app 的状态全在 JS 里,Typst 只出一个宿主标签加一份
// shadow root 的模板原型 —— 和 card / expand-panel 同一个路子。
#let playground() = custom_element(
  () => {
    html_elem("template", id: "playground")[
      #html_elem("div", part: "toolbar")[
        #html_elem("button", part: "sample", type: "button")[载入样例]
        #html_elem("button", part: "run", type: "button")[编译]
        #html_elem("button", part: "cancel", type: "button", hidden: "")[取消]
        #html_elem("button", part: "download", type: "button", hidden: "")[下载 .gia]
      ]
      #html_elem("div", part: "layout")[
        #html_elem("textarea", part: "editor", spellcheck: "false", autocomplete: "off")
        #html_elem("div", part: "right")[
          #html_elem("div", part: "status")[]
          #html_elem("pre", part: "log")
        ]
      ]
      #html_elem("div", part: "backend")[
        #html_elem("label")[
          后端地址
          #html_elem("input", part: "backend-input", type: "text")
        ]
        #html_elem("span", part: "hint")[默认连本机 8787;线上要填 wss:// 的地址]
      ]
    ]
    import_script("/lib/js/playground.js");
    import_style("/lib/css/playground.css");
  },
  html_elem("playground-app"),
)
```

- [ ] **Step 2: 写 JS**

`C:\projects\rust2genshin\docs\typst\lib\js\playground.js`。**两个对象分开** —— 编辑器只管取文本 / 显示文本,WS 客户端只管连接和消息。将来换 CodeMirror、或者加 rust-analyzer 的补全,换掉其中一个不影响另一个。

```js
/* 摆弄页。
 *
 * 分成两个对象是刻意的:
 *   - PlayEditor  只认「取文本 / 显示文本」几个方法,将来换 CodeMirror
 *                 或加 rust-analyzer 的补全,只动它。
 *   - PlayClient  只认「连上 / 发 submit / 收消息」,不知道编辑器存在。
 * 两者在 PlaygroundApp 里通过那几个回调接起来。
 *
 * 默认后端是 ws://127.0.0.1:8787/ws —— 本机跑 `cargo run -p
 * rust2genshin-playground` 就能直接用。地址存在 localStorage 里。
 */

const DEFAULT_BACKEND = "ws://127.0.0.1:8787/ws";
const LS_KEY = "playground.backend";

/* 把 ws:// 的地址换成同一个服务的 http:// 地址。`/artifact/<id>` 和
 * `/sample` 都挂在同一个 origin 上。 */
function httpOrigin(wsUrl) {
    return wsUrl.replace(/^ws/, "http").replace(/\/ws\/?$/, "");
}

class PlayEditor {
    constructor(textarea) {
        this.textarea = textarea;
        this.textarea.addEventListener("keydown", (e) => {
            // Tab 在 textarea 里默认是切焦点,写 Rust 的人会疯。
            if (e.key === "Tab") {
                e.preventDefault();
                const s = this.textarea.selectionStart;
                const e2 = this.textarea.selectionEnd;
                const v = this.textarea.value;
                this.textarea.value = v.slice(0, s) + "    " + v.slice(e2);
                this.textarea.selectionStart = this.textarea.selectionEnd = s + 4;
            }
        });
    }
    getText() { return this.textarea.value; }
    setText(t) { this.textarea.value = t; }
    setEnabled(on) { this.textarea.readOnly = !on; }
}

class PlayClient {
    constructor(url, handlers) {
        this.url = url;
        this.handlers = handlers;
        this.ws = null;
    }

    connect() {
        if (this.ws && this.ws.readyState <= 1) return;
        this.handlers.onStatus("连接中…", "pending");
        const ws = new WebSocket(this.url);
        this.ws = ws;
        ws.onopen = () => this.handlers.onStatus("已连接", "ok");
        ws.onclose = () => {
            this.handlers.onStatus("连接已断开", "err");
            this.handlers.onIdle();
        };
        ws.onerror = () => this.handlers.onStatus("连不上后端", "err");
        ws.onmessage = (ev) => {
            let m;
            try { m = JSON.parse(ev.data); } catch { return; }
            this.#dispatch(m);
        };
    }

    submit(source) {
        this.connect();
        // 回包之前先按"排队中"显示,免得点了没反应。
        this.handlers.onStatus("排队中…", "pending");
        this.#send({ type: "submit", source, target: "beyond-server" });
    }

    cancel() { this.#send({ type: "cancel" }); }

    #send(o) {
        if (this.ws && this.ws.readyState === 1) this.ws.send(JSON.stringify(o));
    }

    #dispatch(m) {
        switch (m.type) {
            case "hello":
                this.handlers.onHello(m);
                break;
            case "queued":
                // 用户要的就是「队列人数/并发数」。
                this.handlers.onStatus(
                    `排队中,前面 ${m.position} 位（队列 ${m.queue}/${m.max_jobs}）`,
                    "pending");
                break;
            case "started":
                this.handlers.onStarted(m);
                break;
            case "log":
                this.handlers.onLog(m);
                break;
            case "finished":
                this.handlers.onFinished(m);
                break;
            case "failed":
                this.handlers.onFailed(m);
                break;
            case "error":
                this.handlers.onError(m);
                break;
            default:
                // 认不出来的消息不忽略 —— 记一行。将来加 lsp 消息时,
                // 老前端就是靠这一行走过去的。
                this.handlers.onLog({ stream: "stderr", text: `未知消息 ${m.type}\n` });
        }
    }
}

class PlaygroundApp extends HTMLElement {
    constructor() {
        super();
        this.attachShadow({ mode: "open" });
        const tpl = document.getElementById("playground");
        this.shadowRoot.appendChild(tpl.content.cloneNode(true));
    }

    connectedCallback() {
        const $ = (p) => this.shadowRoot.querySelector(`[part="${p}"]`);
        const editor = new PlayEditor($("editor"));
        const status = $("status");
        const log = $("log");
        const runBtn = $("run");
        const cancelBtn = $("cancel");
        const dlBtn = $("download");
        const backendInput = $("backend-input");
        backendInput.value = localStorage.getItem(LS_KEY) || DEFAULT_BACKEND;

        const setStatus = (text, kind) => {
            status.textContent = text;
            status.dataset.kind = kind || "";
        };
        const appendLog = (stream, text) => {
            log.textContent += text;
            log.scrollTop = log.scrollHeight;
            log.dataset.lastStream = stream;
        };
        const setBusy = (busy) => {
            // 两个理由都能让「编译」不可点:正在跑,或者镜像还没烘好。
            // 写成 `|| !ready` 是为了让忙碌结束之后按钮不会在 not_ready
            // 的情况下被错误地恢复成可点。
            runBtn.disabled = busy || !ready;
            cancelBtn.hidden = !busy;
            editor.setEnabled(!busy);
        };

        // 下载地址在收到 finished 时才算得出来:后端给的是**服务端相对**
        // 路径(`/artifact/xxx`),而这一页在 GitHub Pages 上,直接用相对
        // 路径会去文档站找那个文件、必然 404。
        let downloadUrl = null;
        // 第一次连上时镜像往往还在烘。这时要每 5 秒看一眼 /status 把烘焙
        // 日志尾巴显示出来 —— 否则用户看到的就是一句"还在烘焙",不知道
        // 是卡住了还是在动。
        let ready = false;
        let bakeTimer = null;
        let tailSeen = 0;

        const stopPolling = () => {
            if (bakeTimer !== null) {
                clearInterval(bakeTimer);
                bakeTimer = null;
            }
        };
        const pollBake = () => {
            if (bakeTimer !== null || ready) return;
            bakeTimer = setInterval(async () => {
                let j;
                try {
                    const r = await fetch(`${httpOrigin(client.url)}/status`);
                    if (!r.ok) return;
                    j = await r.json();
                } catch {
                    return;   // 后端还没起来,下一轮再说
                }
                const tail = (j.baking && j.baking.tail) || [];
                // 只贴新增的那几行,不然每 5 秒把整个尾巴重打一遍。
                if (tail.length > tailSeen) {
                    for (const line of tail.slice(tailSeen)) appendLog("stdout", line + "\n");
                    tailSeen = tail.length;
                }
                if (j.ready) {
                    ready = true;
                    stopPolling();
                    runBtn.disabled = false;
                    setStatus(`就绪 · 镜像 ${(j.serving || "").slice(0, 7)}`, "ok");
                    appendLog("stdout", "── 镜像好了,可以编译了 ──\n");
                }
            }, 5000);
        };

        const client = new PlayClient(backendInput.value, {
            onStatus: setStatus,
            onHello: (m) => {
                ready = !!m.image_commit;
                // 没烘好之前按钮是灰的 —— 后端也会拒,但让用户先点再被拒
                // 是白费一次往返。
                runBtn.disabled = !ready;
                const c = ready ? m.image_commit.slice(0, 7) : "尚无";
                setStatus(`就绪 · 镜像 ${c}`, ready ? "ok" : "pending");
                if (!ready) {
                    appendLog("stderr", "后端还在烘焙镜像，第一次可能要几分钟。\n");
                    pollBake();
                }
            },
            onStarted: (m) => {
                appendLog("stdout", `── 开始编译（镜像 ${m.image_commit.slice(0, 7)}）──\n`);
                setBusy(true);
                setStatus("编译中…", "pending");
            },
            onLog: (m) => appendLog(m.stream, m.text),
            onFinished: (m) => {
                setBusy(false);
                setStatus(`编译成功 · ${m.bytes} 字节`, "ok");
                downloadUrl = new URL(m.download, httpOrigin(client.url)).href;
                dlBtn.hidden = false;
            },
            onFailed: (m) => {
                setBusy(false);
                setStatus(m.message || "编译失败", "err");
                appendLog("stderr", `\n${m.message}\n`);
            },
            onError: (m) => {
                setBusy(false);
                // ip_busy 那条后端给的就是中文原文,直接显示。
                setStatus(m.message, m.code === "ip_busy" ? "warn" : "err");
            },
            onIdle: () => {
                // 连接断了就别再轮询 /status —— 那是在冲一个没人应答的
                // 后端每 5 秒发一次请求。
                stopPolling();
                setBusy(false);
            },
        });

        runBtn.addEventListener("click", () => {
            log.textContent = "";
            dlBtn.hidden = true;
            downloadUrl = null;
            client.submit(editor.getText());
        });
        cancelBtn.addEventListener("click", () => client.cancel());
        dlBtn.addEventListener("click", () => {
            if (downloadUrl) window.location.href = downloadUrl;
        });
        $("sample").addEventListener("click", async () => {
            try {
                const r = await fetch(`${httpOrigin(client.url)}/sample`);
                editor.setText(r.ok ? await r.text() : "// 取不到样例源码\n");
                if (!r.ok) return;
                // 样例来自工作区,镜像是某个 sha 烘的,两者可能不是同一个
                // commit(典型:lib/ 加了 API 但还没重烘焙)。这时样例可能
                // 编不过,提前说一句,别让人以为是自己的代码写坏了。
                const head = r.headers.get("x-playground-head");
                const baked = r.headers.get("x-playground-baked");
                if (head && baked && !baked.startsWith(head.slice(0, 7))) {
                    appendLog("stderr",
                        `注意:样例来自 ${head.slice(0, 7)},而镜像是 ${baked.slice(0, 7)} 烘的,可能编不过。\n`);
                }
            } catch {
                editor.setText("// 取不到样例源码,检查上面的后端地址\n");
            }
        });
        backendInput.addEventListener("change", () => {
            localStorage.setItem(LS_KEY, backendInput.value);
            window.location.reload();
        });

        client.connect();
    }
}

if (!customElements.get("playground-app")) {
    customElements.define("playground-app", PlaygroundApp);
}
```

- [ ] **Step 3: 写 CSS**

`C:\projects\rust2genshin\docs\typst\lib\css\playground.css` —— **light DOM 侧的样式表**,靠 `&::part(...)` 穿进 shadow root（`card.css` 从头到尾都是这么写的）。所以选择器写在宿主标签 `playground-app` 上,不写 `:host`。

```css
/* 摆弄页。样式从外面穿进 shadow root —— `::part()` 是官方口子,
 * card.css / expand-panel.css 都是这个写法,别改成内联 <style>。 */

playground-app {
    display: block;
    margin: 1rem 0;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

/* 顶上一排按钮。布局和右半边在同一个 media query 里换档。 */
playground-app::part(toolbar) {
    display: flex;
    gap: 8px;
    margin-bottom: 8px;
}
playground-app::part(toolbar) button {
    font: inherit;
    font-size: 13px;
    padding: 4px 12px;
    cursor: pointer;
}
playground-app::part(toolbar) button:disabled {
    cursor: default;
    opacity: 0.5;
}

playground-app::part(layout) {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 12px;
}

playground-app::part(editor) {
    width: 100%;
    min-height: 24rem;
    box-sizing: border-box;
    padding: 8px;
    font: inherit;
    font-size: 13px;
    line-height: 1.5;
    tab-size: 4;
    border: 1px solid var(--border);
    border-radius: 4px;
    resize: vertical;
}

playground-app::part(status) {
    min-height: 1.6em;
    font-size: 13px;
}
playground-app::part(status)[data-kind="ok"] { color: var(--ok); }
playground-app::part(status)[data-kind="err"] { color: var(--err); }
playground-app::part(status)[data-kind="warn"] { color: var(--warn); }
playground-app::part(status)[data-kind="pending"] { opacity: 0.75; }

playground-app::part(log) {
    min-height: 22rem;
    max-height: 22rem;
    margin: 0;
    padding: 8px;
    overflow: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-size: 12px;
    line-height: 1.4;
    background-color: var(--code-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
}

playground-app::part(backend) {
    margin-top: 12px;
    font-size: 12px;
}
playground-app::part(backend-input) {
    margin-left: 6px;
    font: inherit;
}
playground-app::part(hint) {
    margin-left: 8px;
    opacity: 0.7;
}

/* 窄屏上下摞起来。两个面板各 24rem 高,横着塞不下。 */
@media (max-width: 900px) {
    playground-app::part(layout) {
        grid-template-columns: minmax(0, 1fr);
    }
    playground-app::part(editor),
    playground-app::part(log) {
        min-height: 16rem;
    }
}
```

> `--border` 和 `--code-bg` 是 `theme.css` 里已有的 token,直接用。`--ok` / `--err` / `--warn` 没有 —— 下一步补。**不要**在这里写 `#ccc` 这类硬编码颜色,深色主题下会瞎。

- [ ] **Step 4: 给 `theme.css` 补三个状态色 token**

`C:\projects\rust2genshin\docs\typst\lib\css\theme.css` 里,`:root` 和 `:root[data-theme="dark"]` 两个块各加一组（三个色值都从已有的、已经过对比度核算的值里取,不新编颜色）。

浅色块,插在 `--header-h: 48px;` 之后、`}` 之前：

```css
    /* 摆弄页的状态行。三个值分别沿用 tip 的绿、attention 的黄、
     * 行内代码的红 —— 那三处已经按 WCAG 算过对比度,不再编新颜色。 */
    --ok: #155724;
    --warn: #856404;
    --err: #d73948;
```

深色块,插在 `--graph-fallback-bg: #17171c;` 之后、`}` 之前：

```css
    --ok: #a5e0b0;
    --warn: #f0cf85;
    --err: #ff8080;
```

- [ ] **Step 5: 改 `play/index.typ`**

正文换成（保留文件头那两行注释和 `title` / `order`）：

```typ
#import "/lib/lib.typ": *;
// 顶栏左侧那两个选项卡的另一半。toc / with_outline 都关掉:这一页没有
// 目录可列,`#let title` 已经由 template 渲染成 <h1> 了。
#show: template.with(toc: false, with_outline: false);

#let title = [摆弄];
#let order = 2;

#set raw(lang: "rust");

左半边写 Rust,右半边是编译日志。点「编译」把源码发到后端,后端在一个
一次性容器里跑 `cargo build`,回来的 `.gia` 可以直接下载进游戏。

第一次编译可能要多等几分钟 —— 后端要先烘一个镜像出来,`/healthz`
在那之前会报 `not_ready`。

#playground()
```

- [ ] **Step 6: 起 watch 看效果**

Run: `cargo run -p rust2genshin-docs --bin watch`
然后开 `http://127.0.0.1:3000/play/index.html`。

Expected:
- 页面出现左右两个面板、四个按钮、一个日志框。
- 浏览器控制台**没有报错**（元素注册失败、`fetch` 404 都会在这里现形）。
- 后端没起时点「编译」→ 状态变红「连不上后端」,不是白屏、不是静默。
- 后端起着但镜像还没烘好（`hello.image_commit` 为空）→「编译」是灰的,
  状态行是 pending，日志区每 5 秒滚出 `/status` 里的烘焙日志尾巴。
- 试一下深色主题切换：状态行、日志框、边框都要跟着变（这就是 Step 4 那三个
  token 的用处；如果切到深色后日志框底还是白的,说明 `theme.css` 那份没插进
  `[data-theme="dark"]` 块里）。

查错看 watch 的输出，别另起 `http.server`。

- [ ] **Step 7: 提交**

```bash
git add docs/typst/lib/playground.typ docs/typst/lib/js/playground.js docs/typst/lib/css/playground.css docs/typst/lib/css/theme.css docs/typst/play/index.typ
git commit -m "feat(docs): 摆弄页前端(编辑器 + WebSocket 客户端)"
```

---

### Task 17: 文档

**Files:**
- Create: `C:\projects\rust2genshin\docs\typst\dev\playground.typ`
- Modify: `C:\projects\rust2genshin\CLAUDE.md`（常用命令、工作区结构、文档地图）

**Interfaces:**
- Consumes: 无
- Produces: 无（纯文档）

- [ ] **Step 1: 写 `docs/typst/dev/playground.typ`**

页面骨架照 `docs/typst/dev/` 下其它页抄（`#import "/lib/lib.typ": *`、`#let title`、`#show: template`），内容覆盖：

- **怎么跑**：`cargo run -p rust2genshin-playground`；需要本机有 `docker` 和 `git`；`PLAYGROUND_LISTEN` 改监听地址。
- **每个配置项**：一张表，18 个 `PLAYGROUND_*` 环境变量，列「默认值 / 干什么用的」。默认值直接抄 `config.rs` 的单测，别凭记忆写。
- **webhook 怎么配**：GitHub 仓库设置里填 `https://<后端>/admin/rebuild`、content type `application/json`、secret 和 `GITHUB_WEBHOOK_SECRET` 一致；本地调试用 `gh webhook forward`。
- **`wss://` 那条**：文档站是 GitHub Pages 的 https 页面，**https 页面不许连 `ws://`**。线上必须让后端挂在一个有证书的反代后面；本地开发 `http://127.0.0.1:3000` 没这个问题。
- **磁盘怎么算**：`target/playground/` 是任务目录，会被 TTL 清；镜像和 builder cache 走 docker 自己的路径。一条要紧的副作用：`docker builder prune` 是**全局**的 —— 它清的是所有 builder 的缓存，不只是我们的。`PLAYGROUND_BUILD_CACHE_MAX` 是"留到多少"而不是"删到多少"。
- **`cargo clean` 会把 `target/playground/` 一起删掉**，正在跑的任务会失败（服务能扛，`prepare` 会重建目录）；`data/lock` 也是锁。

- [ ] **Step 2: 改 `CLAUDE.md` 三处**

1. 「常用命令」加一段：

```shell
# 起 playground 服务（需要本机有 docker 与 git；见站点「playground」页）
cargo run -p rust2genshin-playground
```

2. 「工作区结构」在 `viewer/` 那行下面加：

```
playground/          rust2genshin-playground：文档站「摆弄」页的后端（WebSocket +
                     一次性 docker 容器跑 cargo build；不在 default-members）
```

3. 「文档地图」表加一行：

```
| **playground**：怎么跑、配置项、webhook、磁盘怎么算 | <https://mzverse.github.io/rust2genshin/dev/playground.html> |
```

- [ ] **Step 3: 起 watch 确认**

Run: `cargo run -p rust2genshin-docs --bin watch`
Expected: 输出里没有新页面的报错；`/dev/playground.html` 打得开，表格排版正常。

**别**跑 `--bin compile`，也别另起 python 预览。

- [ ] **Step 4: 提交**

```bash
git add docs/typst/dev/playground.typ CLAUDE.md
git commit -m "docs(playground): 加一站页面与 CLAUDE.md 指路"
```

---

## 收尾

全部 17 个任务做完之后，**在部署机上**跑一遍 spec 里那 12 步手动 e2e（`docs/superpowers/specs/2026-10-16-playground-design.md` 的「验收」一节）。这台 Windows 开发机没有 docker，跑不了其中的容器部分 —— 到部署机之前，`#[ignore]` 那几条和 12 步 e2e 都算未验证，别当成"已经验收"。

### 部署机：`root@server.local`

仓库放 `/mnt/server/rust2genshin`。相关事实见开头「部署机的既定事实」一节，这里只说**上手步骤**。

```bash
# 已经开始做的（改动前都留了 .bak）：
#   - 清 /var/log（rsyslog 的文本日志 5.6G → 256M；journal 只有 155M，没用）
#   - / 空余 5.7G → 11G
#   - /mnt/server/docker.img（256G 稀疏 ext4）+ 挂到 /mnt/server/docker + fstab 一行
```

**还剩一步要人工做**（改全局 docker 配置 + 重启 dockerd，agent 的权限拦了，这是对的）：

```shell
# 在 server.local 上，把 data-root 指到那块 256G 的 loop ext4。
# 只加 data-root 一行，iptables 和镜像源原样保留。
cp -a /etc/docker/daemon.json /etc/docker/daemon.json.bak
# 手工把 "data-root": "/mnt/server/docker", 加进去
cat /etc/docker/daemon.json
systemctl daemon-reload && systemctl restart docker
docker info --format 'root={{.DockerRootDir}} driver={{.Driver}}'
# 期望：root=/mnt/server/docker  driver=overlay2
```

没做这一步之前，`docker build` 会往 `/` 上写 4~5G 的镜像层，把那块 32G 的系统盘顶满。

**跑测试的机器分工**（别搞混）：`cargo test -p rust2genshin-playground` 在**这台 Windows** 上跑（它是唯一有 rust 工具链的机器，也是"没有 docker 也全绿"那条约束的来源）。`-- --ignored` 那几条和 12 步 e2e 在 `server.local` 上跑 —— 但那台机器**没装 rust/cargo**（只有 docker 和 git），所以得先把代码送过去，再在容器里跑，或者临时装一套工具链。

`PLAYGROUND_MAX_JOBS` 在这台机上取 **4**（理由见开头那节）：15G 内存已用 6.3G、可用 8.3G，8 并发要 12G 装不下。

