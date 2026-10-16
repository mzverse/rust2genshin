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

use std::{fs, path::PathBuf, process::ExitCode, sync::Arc};

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

    let cfg = match config::load() {
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
/// 真正会长的是 docker 的 data root:镜像层和 builder cache 都写在它底下。
/// 部署机上它正被指到 `/mnt/server/docker`(搬之前它落在 `/` 那块 32G 系统
/// 盘上 —— 镜像一多就把系统盘吃满)。量服务自己的数据目录没意义:那底下只有
/// 几十 MB 的任务目录,水位永远不响,等于根本没设水位。
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
    fs4::FileExt::try_lock_exclusive(&f).map_err(|_| {
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

/// 启动快路径:本地有没有现成的、还配得上的镜像?有就拿来用,不重复烘。
///
/// 按 **label** 找而不是按 tag:label 里存的是**全长** sha(`DockerImageBuilder`
/// 收到的就是 `repo.head()`,见 `image_builder::build_args`),而 `label_filter`
/// 是精确匹配 —— 所以这里的 commit 也必须原样用全长 sha,截短了永远命中不了。
/// tag 那边带 `-dirty` 后缀,`image::tag` 算出的和这里要一致才认。
pub async fn startup_image(cfg: &Config, repo: &Repo) -> Option<crate::bake::BuiltImage> {
    // `PLAYGROUND_FORCE_REBAKE=1`:无视本地已有镜像,直接走烘焙分支。
    // 在碰 git / docker 之前就短路 —— 连 head 都不读。
    if cfg.force_rebake {
        return None;
    }
    let head = repo.head().await.ok()?;
    let dirty = repo.is_dirty().await.unwrap_or(true);
    let want = crate::image::tag(&cfg.image, &head, dirty);
    let out = tokio::process::Command::new("docker")
        .args(crate::docker::image_ls_args(&crate::image::label_filter(
            &cfg.image,
            Some(&head),
        )))
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let hit = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .any(|l| l == want);
    hit.then_some(crate::bake::BuiltImage { sha: head, tag: want })
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
    // 水位量 docker 的 data root,不是数据目录 —— 会长的是镜像那块的盘。
    // 读不出 docker info 时退回数据目录(至少还有个数),见 `docker_root`。
    let gate_path = docker_root().unwrap_or_else(|| {
        eprintln!("注意:读不出 docker 的 data root,磁盘水位改量数据目录——那底下只有任务目录,基本不会响。");
        cfg.data_dir.clone()
    });
    let gate: Arc<dyn gc::DiskGate> = Arc::new(RealDiskGate::new(
        gate_path,
        cfg.min_free_disk,
        cfg.build_cache_max,
    ));
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
        let baker = Arc::clone(&baker);
        async move {
            loop {
                for p in gc_loop::expired_tasks(&jobs, &cfg) {
                    if let Err(e) = fs::remove_dir_all(&p) {
                        tracing::warn!(path = %p.display(), "删过期任务目录失败:{e}");
                    }
                }
                // 每轮现取 serving 的 tag:服务运行中新烘好的镜像下一轮就受保护。
                let keep_tag = baker.status().serving_tag;
                if let Err(e) =
                    gc_loop::sweep_images(&cfg, &registry, keep_tag.as_deref()).await
                {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "需要本机有 docker"]
    async fn startup_image_finds_a_locally_baked_image() {
        // 需要先烘过一遍:镜像 label 里得有当前 HEAD 的全长 sha。
        let cfg = config::load().expect("读配置");
        let repo = Repo::new(cfg.repo_dir.clone());
        let head = repo.head().await.expect("读 HEAD");
        match startup_image(&cfg, &repo).await {
            Some(b) => {
                assert_eq!(b.sha, head, "adopt 的 sha 应是当前 HEAD 的全长");
                assert!(b.tag.contains(&head[..7]), "tag 带短 sha:{}", b.tag);
            }
            None => panic!("本机没有当前 HEAD 的镜像,测试没起到守门作用"),
        }
    }

    #[test]
    #[ignore = "需要本机有 docker"]
    fn docker_root_reports_where_images_live() {
        let p = docker_root().expect("有 docker 就该读得出来");
        assert!(p.is_absolute(), "{p:?}");
        assert!(p.exists(), "docker 的 data root 应该存在:{p:?}");
    }

    #[tokio::test]
    async fn force_rebake_skips_adopting_a_local_image() {
        // force_rebake=1 必须让 startup_image 在碰 git/docker 之前就短路成
        // None(走烘焙分支)。用一个真 git 仓库(head 能读出来),断言结果
        // None:若短路被删掉,函数会继续去 `docker image ls`,本机没 docker
        // 时同样返回 None —— 所以这条主要钉住"force_rebake 被读到、不再是
        // 无操作字段",并在 docker 机上退化成强断言(有镜像也返回 None)。
        let d = tempfile::tempdir().unwrap();
        let git_ok = std::process::Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(d.path())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !git_ok {
            return; // 环境里没有 git 就跳过,别让这条变成硬依赖。
        }
        let mut cfg = config::load().unwrap();
        cfg.repo_dir = d.path().to_path_buf();
        cfg.force_rebake = true;
        let repo = Repo::new(cfg.repo_dir.clone());
        assert!(startup_image(&cfg, &repo).await.is_none());
    }
}
