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
mod lsp;
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
/// 解析 `docker image ls` 的输出(每行 `tag\tcreatedAt`),按创建时间**从新到旧**
/// 排好序返回候选 tag。
///
/// 排序在客户端做 —— 见 [`crate::docker::newest_labeled_image_args`] 的注释:
/// 部署用的宿主上的 docker CLI 不认 `--sort`。`CreatedAt` 是定长的
/// `2026-10-18 01:36:52 +0800 CST`,字典序就是时间序。
///
/// 缺列的行(理论上不该有,`-` 表示镜像被删过)直接丢。
fn parse_image_candidates(stdout: &str) -> Vec<String> {
    let mut rows: Vec<(&str, &str)> = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut cols = line.split('\t');
        let tag = cols.next().unwrap_or("").trim();
        let created = cols.next().unwrap_or("").trim();
        if tag.is_empty()
            || created.is_empty()
            || created == "-"
            || tag == "<none>"
            || created == "<none>"
        {
            continue;
        }
        rows.push((created, tag));
    }
    rows.sort_by(|a, b| b.0.cmp(a.0));
    rows.into_iter().map(|(_, tag)| tag.to_string()).collect()
}

/// 启动时对「用哪份镜像」的决定。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct StartupPlan {
    pub serving: Option<crate::bake::BuiltImage>,
    pub needs_bake: bool,
}

/// 本机有烘好的镜像:拿来用,**不烘**。
pub fn plan_reuse(built: crate::bake::BuiltImage) -> StartupPlan {
    StartupPlan {
        serving: Some(built),
        needs_bake: false,
    }
}

/// 什么都没有:从零烘。
pub fn plan_cold() -> StartupPlan {
    StartupPlan {
        serving: None,
        needs_bake: true,
    }
}

pub async fn startup_plan(cfg: &Config, repo: &Repo) -> StartupPlan {
    // `PLAYGROUND_FORCE_REBAKE=1`:无视本地已有镜像,直接走烘焙分支。
    // 在碰 git / docker 之前就短路 —— 连 head 都不读。
    if cfg.force_rebake {
        return plan_cold();
    }
    // **只认「本机有没有镜像」,不认 tag。** tag 绑的是烘焙时的 commit,
    // 而服务进程和镜像里的后端/lib 是两份东西:只要本地有烘好的镜像就
    // 直接顶上,重启一次服务不该因为源码前进了几个 commit 就重烘十几分钟。
    // 实际跑的是哪一版由 `hello.image_commit` 如实报出,觉得旧了再手工
    // 调 `/admin/rebuild` 或设 `PLAYGROUND_FORCE_REBAKE=1`。
    let head = repo.head().await.ok().unwrap_or_default();
    let out = tokio::process::Command::new("docker")
        .args(crate::docker::newest_labeled_image_args(&cfg.image))
        .output()
        .await
        .ok();
    let candidates = match out {
        Some(o) if o.status.success() => parse_image_candidates(&String::from_utf8_lossy(&o.stdout)),
        _ => Vec::new(),
    };
    for tag in candidates {
        // commit label 只在 `image inspect` 的模板里拿得到(`image ls` 的模板
        // 没有 `.Label` 字段)。拿不到就换下一份候选 —— 手工 import 进来的
        // 镜像没有这个 label,而没有 sha 就没法在 `hello` 里如实报当前服务的是
        // 哪个 commit,拿它顶替等于把这件事重新变成不可信。
        let out = tokio::process::Command::new("docker")
            .args(crate::docker::image_inspect_commit_args(&tag))
            .output()
            .await
            .ok();
        let sha = match out {
            Some(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            }
            _ => String::new(),
        };
        if sha.is_empty() {
            continue;
        }
        if sha != head {
            tracing::warn!(
                serving = %tag, head = %head,
                "本地有镜像但不是当前 HEAD,直接顶上,不重烘"
            );
        }
        return plan_reuse(crate::bake::BuiltImage { sha, tag });
    }
    plan_cold()
}

async fn run(cfg: Config) -> Result<(), String> {
    let cfg = Arc::new(cfg);
    let _lock = lock_data_dir(&cfg).map_err(|e| e.to_string())?;

    let repo = Arc::new(Repo::new(cfg.repo_dir.clone()));
    let registry = ImageRegistry::new();
    let jobs = Arc::new(JobStore::new(cfg.data_dir.join("tasks")));
    let queue = Queue::new(cfg.max_jobs, cfg.max_queue);

    let builder: Arc<dyn bake::ImageBuilder> = Arc::new(
        crate::image_builder::DockerImageBuilder::new(
            cfg.repo_dir.clone(),
            cfg.image.clone(),
            crate::image_builder::Mirrors {
                rustup_dist_server: cfg.rustup_dist_server.clone(),
                rustup_update_root: cfg.rustup_update_root.clone(),
                cargo_mirror: cfg.cargo_mirror.clone(),
            },
        ),
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
        cpus: cfg.job_cpus,
        // `cfg` 是 Arc<Config>,`job_memory` 不是 Copy,得 clone 一份出来。
        memory: cfg.job_memory.clone(),
        timeout: cfg.job_timeout,
        ..DockerOptions::default()
    }));

    // RA 运行时。`new` 不起后台任务(测试要能造一个不碰 docker 的实例),
    // 这里显式起补池 / 回收。DISABLED 时整条链路关掉,ws 回错误。
    let lsp = if cfg.lsp_disabled {
        None
    } else {
        let svc = crate::lsp::service::LspService::new(Arc::clone(&cfg), Arc::clone(&baker));
        svc.spawn_background();
        Some(svc)
    };

    let state = Arc::new(AppState {
        queue: Arc::clone(&queue),
        baker: Arc::clone(&baker),
        jobs: Arc::clone(&jobs),
        repo: Arc::clone(&repo),
        registry: Arc::clone(&registry),
        runner,
        started: std::time::Instant::now(),
        cfg: Arc::clone(&cfg),
        lsp,
    });

    // 1) 启动时先看本地有没有现成的镜像:有就拿来用,不重复烘。
    //    这是"重启一下服务不该重编几分钟"的全部实现。
    tokio::spawn({
        let baker = Arc::clone(&baker);
        let cfg = Arc::clone(&cfg);
        let repo = Arc::clone(&repo);
        async move {
            let plan = startup_plan(&cfg, &repo).await;
            match plan.serving {
                Some(built) => {
                    tracing::info!(tag = %built.tag, "本地已有可用镜像,直接顶上");
                    baker.adopt(built);
                }
                None => tracing::info!("本地没有可用镜像,只能干等烘焙"),
            }
            if plan.needs_bake {
                tracing::info!("开始烘焙");
                baker.request(bake::BakeRequest {
                    sha: None,
                    note: Some("启动".into()),
                });
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

    #[test]
    fn candidates_come_back_newest_first_regardless_of_line_order() {
        // docker 的输出顺序不作数(而且这台宿主的 CLI 连 --sort 都不认),
        // 所以「谁最新」必须由我们自己从 CreatedAt 那一列判。
        let out = "rust2genshin-play:f1f75ac\t2026-10-18 02:00:00 +0000 UTC\n\
                   rust2genshin-play:1102e2b\t2026-10-18 03:00:00 +0000 UTC\n";
        assert_eq!(
            parse_image_candidates(out),
            vec![
                "rust2genshin-play:1102e2b".to_string(),
                "rust2genshin-play:f1f75ac".to_string()
            ]
        );
    }

    #[test]
    fn rows_without_a_usable_creation_time_are_dropped() {
        // `<none>` 的镜像是悬空的,`-` 是被删掉的 —— 两种都不该进候选,
        // 否则会拿一份根本起不来的镜像当 serving。
        let out = "rust2genshin-play:gone\t-\n\
                   rust2genshin-play:ok\t2026-10-18 02:00:00 +0000 UTC\n\
                   <none>\t<none>\n";
        assert_eq!(
            parse_image_candidates(out),
            vec!["rust2genshin-play:ok".to_string()]
        );
        assert!(parse_image_candidates("").is_empty());
        assert!(parse_image_candidates("rust2genshin-play:x\n").is_empty());
    }

    #[test]
    fn an_existing_image_is_served_without_baking() {
        // 启动计划的核心断言:**有镜像就用,不管 tag 对不对**,也不烘。
        let p = plan_reuse(crate::bake::BuiltImage {
            sha: "old".into(),
            tag: "img:old".into(),
        });
        assert_eq!(p.serving.unwrap().tag, "img:old");
        assert!(!p.needs_bake, "本地有镜像就不该再烘一遍");
    }

    #[test]
    fn a_cold_start_serves_nothing_and_bakes() {
        let p = plan_cold();
        assert!(p.serving.is_none());
        assert!(p.needs_bake);
    }

    #[tokio::test]
    #[ignore = "需要本机有 docker 且至少烘过一次"]
    async fn startup_image_uses_whatever_is_local_without_baking() {
        // 只要本机有带 commit label 的镜像就用它,不管 tag 是不是当前 HEAD,
        // 也不烘新的。
        let cfg = config::load().expect("读配置");
        let repo = Repo::new(cfg.repo_dir.clone());
        let plan = startup_plan(&cfg, &repo).await;
        let b = plan
            .serving
            .expect("本机一张镜像都没有,测试没起到守门作用");
        assert!(!b.sha.is_empty(), "adopt 的 sha 来自镜像 label");
        assert!(!plan.needs_bake, "本地有镜像就不该再烘");
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
        // force_rebake=1 必须让启动计划在碰 git/docker 之前就短路成「冷启动」
        // (不 adopt 任何镜像,只烘)。用一个真 git 仓库(head 能读出来)断言
        // `serving` 为 None:若短路被删掉,函数会继续去 `docker image ls`,
        // 本机没 docker 时同样没有 serving —— 所以这条主要钉住「force_rebake
        // 被读到、不再是无操作字段」,并在 docker 机上退化成强断言
        // (有镜像也必须不 adopt)。
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
        let plan = startup_plan(&cfg, &repo).await;
        assert!(plan.serving.is_none(), "force_rebake 不该 adopt 任何镜像");
        assert!(plan.needs_bake);
    }
}
