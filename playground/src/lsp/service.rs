//! RA 服务的运行时:把「造会话」「池」「真实 docker exec」接起来。
//!
//! Task 9b 的 WebSocket 层只用这里:`LspService::acquire` 拿会话、
//! `subscribe` 收事件、`write_source` 把用户源码落进容器。本文件不碰
//! `AppState` / ws 协议。
//!
//! 会话的创建流程(顺序不能乱):
//!
//! 1. 从 [`Baker`] 拿 serving 镜像的 tag —— 没有就说明还没烘好,返回 `None`;
//! 2. `docker run -d` 起一个待命容器;
//! 3. `docker exec -i ... rust-analyzer` 起 RA([`DockerExec`],裸管道);
//! 4. `initialize` → 订阅事件 → `didOpen(PLACEHOLDER)`;
//! 5. 在 `lsp_boot_timeout` 内收到第一条诊断或 token 才算就绪。
//!
//! 第 4 步**必须先订阅再 didOpen**:RA 可能立刻推诊断,晚一步就漏掉
//! 就绪信号。第 5 步认两种事件:计划原先只认 Diagnostics,但占位内容
//! 零错误时 RA 未必发它;`didOpen` 会主动请求 token,Tokens 是更可靠的
//! 兜底。

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::broadcast;

use crate::bake::Baker;
use crate::config::Config;
use crate::docker::{RA_WORKDIR, RunSpec, rm_args, run_detached_args};
use crate::lsp::docker_exec::DockerExec;
use crate::lsp::exec::{RaCommand, RaProcess};
use crate::lsp::pool::{Acquire, Handle, Pool, PoolError};
use crate::lsp::session::{Event, Session};

/// 待命会话打开文档用的占位内容。用户源码在 `LspChange` 时用 didChange
/// 全文替换;同一个 URI 再 didOpen 一次是 LSP 协议违规。
const PLACEHOLDER: &str = "// playground: waiting for user source\n";

/// 服务进程里唯一的 RA 运行时。`new` 不启后台任务(测试要能造一个不碰
/// docker 的实例),补池与回收由 `main.rs` 显式调 `spawn_background`。
pub struct LspService {
    cfg: Arc<Config>,
    pool: Arc<Pool>,
}

impl LspService {
    pub fn new(cfg: Arc<Config>, baker: Arc<Baker>) -> Arc<Self> {
        // 闭包不能捕获 `Arc<LspService>`(池是它的字段,会循环),所以
        // 造会话需要的东西放进独立的 `SessionMaker`,池只持有它。
        let maker = Arc::new(SessionMaker {
            cfg: Arc::clone(&cfg),
            baker,
            next: AtomicU64::new(0),
        });
        let pool = Pool::new(cfg.lsp_pool_min, move || {
            let maker = Arc::clone(&maker);
            async move { maker.make().await }
        });
        Arc::new(Self { cfg, pool })
    }

    /// 取一个会话。池里有活的直接给,没有就现造;造不出来是
    /// [`PoolError::Create`](crate::lsp::pool::PoolError::Create)。
    pub async fn acquire(&self) -> Result<Acquire, PoolError> {
        self.pool.acquire().await
    }

    /// 把用户源码写进容器里的 RA workspace。
    ///
    /// **必须在 `did_change` 之前 await 完成**:否则 RA 可能先看到
    /// didChange、再从磁盘读到旧内容。
    pub async fn write_source(&self, container: &str, source: &str) -> Result<(), String> {
        let target = format!("{RA_WORKDIR}/{}", crate::protocol::SOURCE_FILE);
        let cmd = format!("cat > {target}");
        let mut child = Command::new("docker")
            .args(["exec", "-i", container, "sh", "-c", cmd.as_str()])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("写源码的 docker exec 起不来:{e}"))?;
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| "docker exec 没有 stdin 管道".to_string())?;
            stdin
                .write_all(source.as_bytes())
                .await
                .map_err(|e| format!("写源码 stdin 失败:{e}"))?;
            stdin
                .flush()
                .await
                .map_err(|e| format!("flush 源码 stdin 失败:{e}"))?;
        } // drop stdin → EOF,容器里的 `cat` 才会退出
        let out = child
            .wait_with_output()
            .await
            .map_err(|e| format!("等 docker exec 退出失败:{e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "写源码失败:{}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    /// 起两个后台循环:补池(维持 `lsp_pool_min` 个空闲)与回收
    /// (销毁空闲超过 `lsp_idle_ttl` 的,`ttl == 0` 时整条策略关闭)。
    pub fn spawn_background(self: &Arc<Self>) {
        let svc = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                while svc.pool.idle_count().await < svc.cfg.lsp_pool_min {
                    match svc.pool.acquire().await {
                        // 走 acquire + release,不必给池开一个「直接塞一个」
                        // 的口子;release 会顺带做下限裁剪。
                        Ok(acq) => acq.release(true).await,
                        Err(e) => {
                            tracing::warn!("补 RA 会话失败,5s 后重试:{e}");
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });

        let svc = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                if svc.cfg.lsp_idle_ttl.is_zero() {
                    continue;
                }
                let n = svc.pool.sweep_idle(svc.cfg.lsp_idle_ttl).await;
                if n > 0 {
                    tracing::debug!("回收了 {n} 个空闲 RA 会话");
                }
            }
        });
    }
}

/// 需要跨 `Pool::new` 的闭包共享的造会话状态。
struct SessionMaker {
    cfg: Arc<Config>,
    baker: Arc<Baker>,
    next: AtomicU64,
}

impl SessionMaker {
    async fn make(&self) -> Option<Handle> {
        // 1. 没有 serving 镜像 = 还没烘好,等下一轮补池。
        let tag = self.baker.status().serving_tag?;
        let name = format!("r2g-ra-{}", self.next.fetch_add(1, Ordering::SeqCst));

        // 2. 待命容器。不挂 demo/src、不挂 /out:RA 的 workspace 在镜像内,
        //    源码由 `write_source` 灌进去。
        let spec = RunSpec {
            name: name.clone(),
            image: tag,
            src_dir: PathBuf::new(),
            out_dir: PathBuf::new(),
            workdir: RA_WORKDIR.into(),
            // RA 容器只跑 `dd`/`sleep`,不跑 `container_command` ——
            // 目标选择对分析工作区没有意义,给个默认值满足结构体。
            target: "beyond-server".into(),
            // 走配置,别写死:宿主 cgroup 没挂 cpu 控制器时 `--cpus` 会让
            // docker 拒掉整个 run(`NanoCPUs can not be set`),待命池就永远
            // 补不上 —— 症状是编辑框一直「分析中…」而编译功能一切正常。
            cpus: self.cfg.lsp_cpus,
            memory: self.cfg.lsp_memory.clone(),
            pids_limit: 256,
            user: "1001:1001".into(),
            source_file: crate::protocol::SOURCE_FILE.into(),
        };
        if let Err(e) = run_docker(run_detached_args(&spec)).await {
            tracing::warn!("起 RA 容器失败:{e}");
            return None;
        }

        // 3. 容器里起 RA(裸管道,见 docker_exec)。
        let exec: Arc<dyn RaCommand> = Arc::new(DockerExec::new());
        let proc = match RaProcess::start(exec, &name).await {
            Ok(p) => Arc::new(p),
            Err(e) => {
                tracing::warn!("起 RA exec 失败:{e}");
                remove_container(&name).await;
                return None;
            }
        };
        let session = Arc::new(Session::new(Arc::clone(&proc)));

        // 4. 先订阅再 didOpen —— 见文件头。
        let mut events = session.subscribe();
        if let Err(e) = session.initialize().await {
            tracing::warn!("RA initialize 失败:{e}");
            teardown(&name, &session).await;
            return None;
        }
        if let Err(e) = session.did_open(PLACEHOLDER).await {
            tracing::warn!("RA didOpen 失败:{e}");
            teardown(&name, &session).await;
            return None;
        }

        // 5. 第一条 Diagnostics 或 Tokens 即就绪。收不到 = 这次冷启动
        //    失败,清理后让补池循环重建。
        let ready = tokio::time::timeout(self.cfg.lsp_boot_timeout, async {
            loop {
                match events.recv().await {
                    Ok(Event::Diagnostics(_) | Event::Tokens(_)) => return true,
                    // 补全响应等其它事件与「就绪」无关。
                    Ok(_) => continue,
                    // 慢消费者丢几条不影响就绪判断。
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    // reader 退出 = 会话死了。
                    Err(broadcast::error::RecvError::Closed) => return false,
                }
            }
        })
        .await;
        if !matches!(ready, Ok(true)) {
            tracing::warn!(
                timeout = ?self.cfg.lsp_boot_timeout,
                "RA 就绪超时,丢弃这次会话"
            );
            teardown(&name, &session).await;
            return None;
        }

        // 容器一删,exec 与 RA 一起没。`Handle::Drop` 里同步调用,所以
        // 真正删容器的工作 spawn 出去。
        let destroy_name = name.clone();
        let destroy: Box<dyn FnOnce() + Send + 'static> = Box::new(move || {
            tokio::spawn(async move { remove_container(&destroy_name).await });
        });
        // 真实会话的存活探针必须走 `Session::ping`(pending 表),不能用
        // `Handle::new` 默认的裸管道 ping —— 那条管道上有 reader。
        let live = Arc::clone(&session);
        Some(
            Handle::new(proc, session, destroy)
                .with_liveness(move || {
                    let s = Arc::clone(&live);
                    Box::pin(async move { s.ping(Duration::from_secs(2)).await.is_ok() })
                })
                // ws 层要用它把用户源码写进这个容器(`LspService::write_source`)。
                .with_container(name),
        )
    }
}

/// 会话没造成功时的清理。先杀 exec(收尸),再删容器。
async fn teardown(name: &str, session: &Session) {
    session.shutdown().await;
    remove_container(name).await;
}

async fn remove_container(name: &str) {
    if let Err(e) = run_docker(rm_args(name)).await {
        tracing::warn!("删 RA 容器 {name} 失败:{e}");
    }
}

async fn run_docker(args: Vec<String>) -> Result<(), String> {
    let out = Command::new("docker")
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("docker 起不来:{e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "docker {} 失败:{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
