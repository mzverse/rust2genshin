//! git 调用。只有四件事:当前 sha、工作区脏不脏、快进拉取、判断一个 ref 是不是 sha。
//!
//! `pull_ff_only` **不把失败当错误** —— 这个仓库是开发者自己的 checkout,
//! 分叉 / 本地领先都是常态,服务不该把需要人处理的 merge 冲突态丢进去。
//! 快进不了就返回 `Pull::Skipped(原因)`,烘焙照常继续(烘当前工作区)。

use std::path::{Path, PathBuf};
use std::process::Stdio;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("git 执行失败:{0}")]
    Spawn(#[from] std::io::Error),
    #[error("git {args} 退出码 {code}:{stderr}")]
    Exit {
        args: String,
        code: String,
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
                code: out
                    .status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".to_string()),
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
