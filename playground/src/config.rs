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
