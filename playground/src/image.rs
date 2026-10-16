//! 镜像 tag 与引用计数。
//!
//! tag 用 sha 不用 `latest`:提交时告诉用户 commit A、起跑时 `latest` 可能已经
//! 指到 B —— 用户拿着"来自 A"的结论,拿到 B 编的产物。sha 让"报告 == 实际"。

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
        let key = tag.to_string();
        *self
            .counts
            .lock()
            .expect("镜像引用计数锁中毒")
            .entry(key.clone())
            .or_insert(0) += 1;
        ImageGuard {
            reg: Arc::clone(self),
            tag: key,
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

    #[test]
    fn guard_count_converges_under_concurrent_drop() {
        use std::sync::Barrier;
        use std::thread;

        const N: usize = 16;
        let reg = ImageRegistry::new();
        let barrier = std::sync::Arc::new(Barrier::new(N));
        let mut handles = Vec::with_capacity(N);
        for _ in 0..N {
            let reg = Arc::clone(&reg);
            let barrier = std::sync::Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                let g = reg.acquire("a:1");
                barrier.wait();
                drop(g);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(
            reg.count("a:1"),
            0,
            "并发 acquire+drop 后必须收敛回 0,漏一次就让镜像永远删不掉"
        );
    }
}
