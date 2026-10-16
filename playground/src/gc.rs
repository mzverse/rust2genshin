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
        // 删的顺序 d(最旧)→c(更新)。
        let images = vec![
            img("i:a", 0, 4),
            img("i:b", 0, 3),
            img("i:c", 0, 2),
            img("i:d", 0, 1),
        ];
        let got = select_images_to_remove(&images, 2, &["i:a"]);
        assert_eq!(got, vec!["i:d".to_string(), "i:c".to_string()]);
    }

    #[test]
    fn keep_one_removes_everything_else() {
        let images = vec![
            img("i:a", 0, 4),
            img("i:b", 0, 3),
            img("i:c", 0, 2),
        ];
        let got = select_images_to_remove(&images, 1, &["i:a"]);
        assert_eq!(got, vec!["i:c".to_string(), "i:b".to_string()]);
    }

    #[test]
    fn in_use_images_survive_even_when_old() {
        // 一个长跑任务正拿 d 起容器:它在最老的位置,但 refcount 非零。
        // 删的顺序 c(2,最旧)→b(3,更新);d 因 refcount > 0 占额度必保。
        let images = vec![
            img("i:a", 0, 4),
            img("i:b", 0, 3),
            img("i:c", 0, 2),
            img("i:d", 1, 1),
        ];
        let got = select_images_to_remove(&images, 1, &["i:a"]);
        assert_eq!(got, vec!["i:c".to_string(), "i:b".to_string()]);
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
        assert_eq!(got, vec!["i:d".to_string(), "i:c".to_string()]);
    }

    #[test]
    fn doomed_list_is_sorted_oldest_first() {
        // 五个镜像,last_used 依次 5/4/3/2/1。keep=1 只留 a(最新)。
        // 待删 e/d/c/b 四个必须按最旧→次旧→新 排,先删最久没用过的:
        // 调用方只在磁盘吃紧时删到水位够就停,顺序不是装饰。
        let images = vec![
            img("i:a", 0, 5),
            img("i:b", 0, 4),
            img("i:c", 0, 3),
            img("i:d", 0, 2),
            img("i:e", 0, 1),
        ];
        let got = select_images_to_remove(&images, 1, &[]);
        assert_eq!(
            got,
            vec![
                "i:e".to_string(),
                "i:d".to_string(),
                "i:c".to_string(),
                "i:b".to_string(),
            ]
        );
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
}

/// 选出该删的 tag。规则:
///
/// 1. `protected`(`serving` + 最近一次成功)永不入选;
/// 2. `refcount > 0`(有容器在跑)永不入选,并且**占掉 `keep` 的额度**
///    —— 它反正留着,不该让别的老镜像因此多活一份;
/// 3. 其余按 `last_used` 从新到旧,填满 `keep` 剩下的额度,更老的删。
///
/// 返回顺序有实质意义:调用方(Task 14 的 `sweep_images`)在磁盘吃紧时
/// 只删到水位够就停,所以这个顺序就是删除顺序,直接决定先牺牲哪个镜像。
/// 必须最旧的最先删 —— 否则会把次新的、其实更常用的镜像错杀。
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
            .ok_or_else(|| format!("读不到磁盘 {} 的可用空间", self.path.display()))?;
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