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
    /// 任务容器的 CPU 限额;`None` = 不下 `--cpus`。
    pub job_cpus: Option<u32>,
    /// 任务容器的内存限额;`None` = 不下 `--memory`。
    pub job_memory: Option<String>,
    pub max_jobs: usize,
    pub max_queue: usize,
    pub job_timeout: Duration,
    /// 多久给连接发一个 WebSocket ping。**别关掉**:前面那层 nginx 的 stream
    /// 模块按「最后一次有字节」掐空闲连接(`proxy_timeout`),排队等镜像的
    /// 那几十秒里一条业务消息都没有,不给它续命连接就被悄悄关掉,`finished`
    /// 连同 .gia 下载链接一起丢在半路。
    pub keepalive: Duration,
    pub keep_images: usize,
    pub build_cache_max: u64,
    pub min_free_disk: u64,
    pub artifact_ttl: Duration,
    pub allowed_origins: Vec<String>,
    pub trust_proxy: bool,
    pub force_rebake: bool,
    pub image: String,
    /// 烘焙时给 Dockerfile 的 rustup / crates.io 镜像,留空 = 官方源。
    ///
    /// 部署机在国内,裸连官方源要几十分钟;但公开仓库不该把第三方镜像
    /// 写死给所有人 —— 由部署方在环境里显式打开。
    pub rustup_dist_server: Option<String>,
    pub rustup_update_root: Option<String>,
    pub cargo_mirror: Option<String>,
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

/// 任务容器的 CPU 限额。没设就是 `Some(2)`(保守默认);设成 `0` 或 `none`
/// 表示**不加** `--cpus` —— 宿主的 cgroup 没挂 cpu 控制器时,加了这个旗标
/// docker 会拒掉整个 `run`,一个任务都跑不起来。
fn cpus_var(key: &'static str, get: &dyn Fn(&str) -> Option<String>) -> Result<Option<u32>, ConfigError> {
    match raw(key, get) {
        None => Ok(Some(2)),
        Some(v) => {
            let lower = v.to_ascii_lowercase();
            if lower == "none" {
                return Ok(None);
            }
            let n: u32 = lower.parse().map_err(|e| inv(key, &v, e))?;
            Ok((n > 0).then_some(n))
        }
    }
}

/// 任务容器的内存限额。没设是 `1536m`;设成 `none` 表示**不加**
/// `--memory`(宿主的 cgroup v1 不支持 kmem 时,加了 docker 会拒掉整个 run)。
/// `PLAYGROUND_JOB_MEMORY=none` 是不限额,别的都按 docker 的写法原样传给 `--memory`
/// (`1536m` / `2g` —— 一律 1024 进制,别写 `1.5g`,docker 对小数容量不友好)。
fn memory_var(key: &'static str, get: &dyn Fn(&str) -> Option<String>) -> Result<Option<String>, ConfigError> {
    match raw(key, get) {
        None => Ok(Some("1536m".into())),
        Some(v) if v.eq_ignore_ascii_case("none") => Ok(None),
        Some(v) => {
            parse_size(&v).map_err(|reason| inv(key, &v, reason))?;
            Ok(Some(v))
        }
    }
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
        job_cpus: cpus_var("PLAYGROUND_JOB_CPUS", get)?,
        job_memory: memory_var("PLAYGROUND_JOB_MEMORY", get)?,
        max_queue: num_var("PLAYGROUND_MAX_QUEUE", get, 64usize)?,
        job_timeout: Duration::from_secs(num_var("PLAYGROUND_JOB_TIMEOUT", get, 15u64)?),
        keepalive: Duration::from_secs(num_var("PLAYGROUND_KEEPALIVE", get, 25u64)?),
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
        rustup_dist_server: raw("PLAYGROUND_RUSTUP_DIST_SERVER", get),
        rustup_update_root: raw("PLAYGROUND_RUSTUP_UPDATE_ROOT", get),
        cargo_mirror: raw("PLAYGROUND_CARGO_MIRROR", get),
        webhook_secret: raw("GITHUB_WEBHOOK_SECRET", get),
        admin_token: raw("PLAYGROUND_ADMIN_TOKEN", get),
    })
}

pub fn load() -> Result<Config, ConfigError> {
    resolve(&|k| std::env::var(k).ok(), Path::new(env!("CARGO_MANIFEST_DIR")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 拿一张表当环境用:测试不碰进程环境(并行跑会互相污染)。
    fn getter<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
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
        assert_eq!(c.job_cpus, Some(2));
        assert_eq!(c.job_memory.as_deref(), Some("1536m"));
        assert_eq!(c.max_queue, 64);
        assert_eq!(c.job_timeout, Duration::from_secs(15));
        // 必须短于 nginx stream 层的 proxy_timeout(部署上是 60s),否则
        // 保活还没发出去连接先被关了。
        assert_eq!(c.keepalive, Duration::from_secs(25));
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
    fn zero_or_none_drops_the_cpu_limit() {
        // 宿主 cgroup 不支持 cpu 配额时,`--cpus` 会让 docker 直接拒整个 run。
        let g = getter(&[("PLAYGROUND_JOB_CPUS", "0")]);
        assert_eq!(resolve(&g, Path::new("/repo/playground")).unwrap().job_cpus, None);
        let g = getter(&[("PLAYGROUND_JOB_CPUS", "none")]);
        assert_eq!(resolve(&g, Path::new("/repo/playground")).unwrap().job_cpus, None);
        let g = getter(&[("PLAYGROUND_JOB_CPUS", "4")]);
        assert_eq!(resolve(&g, Path::new("/repo/playground")).unwrap().job_cpus, Some(4));
        let g = getter(&[("PLAYGROUND_JOB_CPUS", "abc")]);
        assert!(resolve(&g, Path::new("/repo/playground")).is_err());
    }

    #[test]
    fn none_drops_the_memory_limit_and_junk_is_rejected() {
        // kmem 不被支持时,`--memory` 会让 docker 整个 run 失败,得能关。
        let g = getter(&[("PLAYGROUND_JOB_MEMORY", "none")]);
        assert_eq!(resolve(&g, Path::new("/repo/playground")).unwrap().job_memory, None);
        let g = getter(&[("PLAYGROUND_JOB_MEMORY", "2g")]);
        assert_eq!(
            resolve(&g, Path::new("/repo/playground")).unwrap().job_memory.as_deref(),
            Some("2g")
        );
        // 大小按 1024 进制解析,和 docker 的 --keep-storage 一个理解。
        let g = getter(&[("PLAYGROUND_JOB_MEMORY", "1.5g")]);
        assert!(resolve(&g, Path::new("/repo/playground")).is_err());
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
