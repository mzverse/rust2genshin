//! 后台 gc:每 5 分钟一次,清过期任务目录 + 按策略清镜像。
//!
//! 清理有两条铁律:只碰带 `rust2genshin.play=1` 的镜像(宿主机上别人的
//! 镜像一个都不许动),以及正在被任务引用的镜像不动(引用计数在
//! `ImageRegistry` 里,`protected` 就是它当前非零的那些 tag)。

use std::time::{Duration, SystemTime};

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
    keep_tag: Option<&str>,
) -> Vec<String> {
    let mut protected: Vec<&str> = images
        .iter()
        .filter(|i| registry.count(&i.tag) > 0)
        .map(|i| i.tag.as_str())
        .collect();
    // 正在服役的镜像(`serving`)也必须永不入选 —— `select_images_to_remove`
    // 的契约第一条就是它,而它不在 registry 的引用计数里。
    if let Some(t) = keep_tag {
        protected.push(t);
    }
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
            if !m.is_dir() {
                return None;
            }
            let modified = m.modified().ok()?;
            Some((e.path(), modified))
        })
        .collect();
    expired_dirs(&entries, SystemTime::now(), cfg.artifact_ttl)
}

/// 列镜像 → 选victim → `docker rmi`。**列的时候一定带 label 过滤**,
/// 这是"只碰我们自己的镜像"那道锁;`docker image ls` 不带过滤会返回
/// 宿主上所有镜像,后面再筛就晚了。
pub async fn sweep_images(
    cfg: &Config,
    registry: &ImageRegistry,
    keep_tag: Option<&str>,
) -> Result<usize, String> {
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
            last_used: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        })
        .collect();

    let victims = pick_image_victims(&images, cfg.keep_images, registry, keep_tag);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc::ImageInfo;

    fn info(tag: &str, refcount: u32, last_used: u64) -> ImageInfo {
        ImageInfo {
            tag: tag.to_string(),
            refcount,
            last_used,
        }
    }

    #[test]
    fn in_flight_images_are_never_picked() {
        let reg = ImageRegistry::new();
        let _g = reg.acquire("play:a");
        let images = vec![info("play:a", 0, 1), info("play:b", 0, 2)];
        let victims = pick_image_victims(&images, 0, &reg, None);
        assert!(!victims.contains(&"play:a".to_string()), "有引用计数的不能删");
    }

    #[test]
    fn keep_is_honored() {
        let reg = ImageRegistry::new();
        let images = vec![info("play:a", 0, 1), info("play:b", 0, 2), info("play:c", 0, 3)];
        let victims = pick_image_victims(&images, 2, &reg, None);
        assert_eq!(victims, vec!["play:a".to_string()], "留两个,删最旧那个");
    }

    #[test]
    fn the_serving_image_is_never_picked() {
        let reg = ImageRegistry::new();
        let images = vec![
            info("play:serving", 0, 1),
            info("play:b", 0, 2),
            info("play:c", 0, 3),
        ];
        let victims = pick_image_victims(&images, 0, &reg, Some("play:serving"));
        assert!(
            !victims.contains(&"play:serving".to_string()),
            "正在服役的镜像一个都不许删:{victims:?}"
        );
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
