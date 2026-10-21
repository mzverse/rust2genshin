//! 后台轮询：每 5 分钟 `git pull --ff-only`，拉到新提交就触发烘焙。
//!
//! webhook 那条路需要两样东西：公网可达的地址、GitHub 仓库里配好的
//! secret。内网部署不一定有，配置漏了也不会有人知道 —— 表现就是
//! 「推送了但页面还是老的」。轮询是兜底，不依赖任何外部配置。
//!
//! 判断有没有更新只看 `git pull` 的 stdout：`Already up to date.` 以外
//! 就是拉到了东西（分类逻辑在 `repo::classify_pull`，和 webhook 那条路
//! 共用）。

use std::sync::Arc;
use std::time::Duration;

use crate::bake::{BakeRequest, Baker};
use crate::config::Config;
use crate::repo::{Pull, Repo};

/// 默认间隔。轮询一次是一次 `git pull`，间隔太短只是白耗网络。
pub const DEFAULT_PULL_INTERVAL: Duration = Duration::from_secs(300);

/// 一次轮询的结果该怎么处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// 没更新，或者这次 pull 跳过了（ref 钉在 sha 上）。
    Idle,
    /// 拉到新提交，该烘了。
    Bake,
}

/// `git pull` 的分类 → 该不该烘。
pub fn decide(pull: &Pull) -> Tick {
    match pull {
        Pull::Updated => Tick::Bake,
        Pull::UpToDate | Pull::Skipped(_) => Tick::Idle,
    }
}

/// 拉一次。失败 / 跳过都记进 `/status` 的 `pull_failure`，不打断循环。
pub async fn tick(repo: &Repo, cfg: &Config, baker: &Baker) {
    match repo.pull_ff_only(&cfg.remote, &cfg.git_ref).await {
        Ok(pull) => match decide(&pull) {
            Tick::Bake => {
                tracing::info!("轮询拉到新提交，触发烘焙");
                baker.set_pull_failure(None);
                baker.request(BakeRequest {
                    sha: None,
                    note: Some("轮询".into()),
                });
            }
            Tick::Idle => {
                if let Pull::Skipped(why) = pull {
                    tracing::warn!("轮询跳过 pull：{why}");
                    baker.set_pull_failure(Some(why));
                } else {
                    baker.set_pull_failure(None);
                }
            }
        },
        Err(e) => {
            tracing::warn!("轮询 pull 失败：{e}");
            baker.set_pull_failure(Some(e.to_string()));
        }
    }
}

/// 循环。`interval` 为零时整条轮询关掉（只想靠 webhook 的部署用得上）。
pub async fn run(repo: Arc<Repo>, cfg: Arc<Config>, baker: Arc<Baker>, interval: Duration) {
    if interval.is_zero() {
        tracing::info!("轮询关闭（PLAYGROUND_PULL_INTERVAL=0）");
        return;
    }
    loop {
        tokio::time::sleep(interval).await;
        tick(&repo, &cfg, &baker).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{git, git_local_clone, git_remote_with_commit};

    #[test]
    fn updated_means_bake() {
        assert_eq!(decide(&Pull::Updated), Tick::Bake);
    }

    #[test]
    fn nothing_new_means_idle() {
        assert_eq!(decide(&Pull::UpToDate), Tick::Idle);
        assert_eq!(decide(&Pull::Skipped("钉在 sha 上".into())), Tick::Idle);
    }

    /// 真的跑一次 `git pull`：远端没动 → Idle；远端前进 → Bake。
    #[tokio::test]
    async fn poll_only_bakes_when_the_remote_moved() {
        let (remote, _first) = git_remote_with_commit("a");
        let local = git_local_clone(remote.path());
        let repo = Repo::new(local.path().to_path_buf());

        let pull = repo.pull_ff_only("origin", "main").await.unwrap();
        assert_eq!(pull, Pull::UpToDate);
        assert_eq!(decide(&pull), Tick::Idle);

        std::fs::write(remote.path().join("f.txt"), "b").unwrap();
        git(remote.path(), &["add", "."]);
        git(remote.path(), &["commit", "-m", "b"]);
        let pull = repo.pull_ff_only("origin", "main").await.unwrap();
        assert_eq!(pull, Pull::Updated);
        assert_eq!(decide(&pull), Tick::Bake);

        // 拉过之后又回到「没更新」。
        let pull = repo.pull_ff_only("origin", "main").await.unwrap();
        assert_eq!(pull, Pull::UpToDate);
    }
}
