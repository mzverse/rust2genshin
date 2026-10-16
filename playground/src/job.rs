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
