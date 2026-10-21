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
//! 4. `initialize` → `didOpen(PLACEHOLDER)`;
//! 5. 服务端用自己的 id 拉一次 `textDocument/diagnostic`，在
//!    `lsp_boot_timeout` 内拿到响应即算就绪。
//!
//! 第 4 步灌一份占位内容是为了把 crate graph 拉热。第 5 步只看「有响应」，
//! 不看诊断内容：拿到响应就说明 RA 已经能应答分析请求。请求在交棒前一定
//! `await` 完，交棒后 `Pending` 必为空。

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::bake::Baker;
use crate::config::Config;
use crate::docker::{RA_WORKDIR, RunSpec, rm_args, run_detached_args};
use crate::lsp::docker_exec::DockerExec;
use crate::lsp::exec::{RaCommand, RaProcess};
use crate::lsp::pool::{Acquire, Handle, Pool, PoolError};
use crate::lsp::session::{DOC_URI, Session};

/// 待命会话打开文档用的占位内容。它只负责把 crate graph 拉热 —— 交棒
/// 前服务端会补一条 `didClose`,用户的源码随后由浏览器自己的 `didOpen`
/// 打开。
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

    /// 取一个会话。池里有活的直接给，没有就现造；造不出来是
    /// [`PoolError::Create`](crate::lsp::pool::PoolError::Create)。
    ///
    /// 返回前保证**没有打开的文档**：池里的会话可能还带着上一份占位或
    /// 上一个浏览器的 overlay。补一条 `didClose` 之后，浏览器的 `didOpen`
    /// 永远合法，也不需要把 didOpen 改写成 didChange。
    pub async fn acquire(&self) -> Result<Acquire, PoolError> {
        let acq = self.pool.acquire().await?;
        if let Err(e) = acq.session().did_close().await {
            tracing::warn!("交棒前 didClose 失败,丢弃这个会话:{e}");
            acq.release(false).await;
            return Err(PoolError::Create);
        }
        Ok(acq)
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

    /// 清掉**上一实例遗留**的 RA 容器。返回清掉几个。
    ///
    /// 为什么必须清:RA 容器的编号是进程内计数器(`SessionMaker.next`),
    /// 服务一重启就从头数。上一实例留下的 `r2g-ra-0` 之类还在,新实例
    /// 造同名容器时 `docker run` 会因重名失败 —— RA 就再也起不来,编辑框
    /// 永远停在「分析中…」。而池的空闲回收(`Pool::sweep_idle`)只认内存里
    /// 的会话,看不见上一实例的容器,靠它清不掉。
    ///
    /// **必须在补池之前调用**:正在用的容器不能删。
    pub async fn cleanup_stale_containers(&self) -> usize {
        let listed = match run_docker_capture(crate::docker::ra_container_ls_args()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("列遗留 RA 容器失败,跳过清理:{e}");
                return 0;
            }
        };
        let names = stale_ra_names(&listed.lines().map(|s| s.to_string()).collect::<Vec<_>>());
        let mut n = 0;
        for name in names {
            match run_docker(crate::docker::rm_args(&name)).await {
                Ok(()) => {
                    n += 1;
                    tracing::info!("清掉上一实例遗留的 RA 容器 {name}");
                }
                Err(e) => tracing::warn!("删遗留 RA 容器 {name} 失败:{e}"),
            }
        }
        n
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

        // 2. 待命容器。不挂 demo/src:RA 的 workspace 在镜像内,
        //    源码由 `write_source` 灌进去。
        let spec = RunSpec {
            name: name.clone(),
            image: tag,
            src_dir: PathBuf::new(),
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
            // 撞名 = 上一实例留下的同名容器还占着位置。删掉它再试一次 ——
            // 启动清理漏了这一手时(比如清理和补池并行、或有人手工起过
            // 同名容器),这一句就是最后的兜底:RA 照样能起来。
            if !is_name_conflict(&e) {
                tracing::warn!("起 RA 容器失败:{e}");
                return None;
            }
            tracing::warn!("RA 容器 {name} 撞名,删掉旧的重试");
            remove_container(&name).await;
            if let Err(e) = run_docker(run_detached_args(&spec)).await {
                tracing::warn!("起 RA 容器失败(清掉同名容器后仍失败):{e}");
                return None;
            }
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

        // 4. 灌一份占位内容把 crate graph 拉热 —— 见文件头。
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

        // 5. 就绪判据：服务端用自己的 id 拉一次诊断，拿到响应即说明 crate
        //    graph 已加载。这里只等「有响应」，不看诊断内容；请求在交棒前
        //    一定 await 完，交棒后 Pending 必为空。
        if let Err(e) = session
            .request(
                "textDocument/diagnostic",
                json!({ "textDocument": { "uri": DOC_URI } }),
                self.cfg.lsp_boot_timeout,
            )
            .await
        {
            tracing::warn!("RA 就绪探测失败,丢弃这次会话:{e}");
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

/// 跑一条 docker 命令并把 stdout 收回来。
async fn run_docker_capture(args: Vec<String>) -> Result<String, String> {
    let out = Command::new("docker")
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("docker 起不来:{e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(format!(
            "docker {} 失败:{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// 从 docker 列出的容器名里挑出**该清的**。
///
/// docker 的 `--filter name=` 是正则匹配,理论上会漏(比如名字里带正则
/// 元字符);这里再按前缀过一道,确保只碰 `r2g-ra-` 开头的,别人的容器
/// 一个都不动。
pub fn stale_ra_names(listed: &[String]) -> Vec<String> {
    listed
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter(|s| s.starts_with(crate::docker::RA_CONTAINER_PREFIX))
        .map(str::to_string)
        .collect()
}

/// docker 的「容器重名」报错。文本是 daemon 说的,不同版本措辞略有出入,
/// 所以按两个片段一起认。
pub fn is_name_conflict(stderr: &str) -> bool {
    let s = stderr.to_ascii_lowercase();
    s.contains("already in use") || s.contains("name conflict")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 启动清理只碰 `r2g-ra-` 开头的容器 —— 别人的容器一个都不动。
    #[test]
    fn stale_ra_names_only_keeps_our_own_containers() {
        let listed = vec![
            "r2g-ra-0".to_string(),
            "r2g-ra-13".to_string(),
            "r2g-playground".to_string(),
            "my-r2g-ra-x".to_string(),
            "dbg-ra".to_string(),
        ];
        let got = stale_ra_names(&listed);
        assert_eq!(got, vec!["r2g-ra-0".to_string(), "r2g-ra-13".to_string()]);
    }

    /// docker 输出里会有空行和空白 —— 不该被当成容器名。
    #[test]
    fn stale_ra_names_skips_blank_lines() {
        let listed = vec![String::new(), "  ".to_string(), "  r2g-ra-2  ".to_string()];
        assert_eq!(stale_ra_names(&listed), vec!["r2g-ra-2".to_string()]);
    }

    /// 空列表不该 panic,也不该给出任何 victims。
    #[test]
    fn stale_ra_names_on_an_empty_listing_is_empty() {
        assert!(stale_ra_names(&[]).is_empty());
    }

    /// docker 的重名报错要认出来 —— 认错了就起不了 RA 容器。
    #[test]
    fn name_conflicts_are_recognized() {
        assert!(is_name_conflict(
            "docker: Error response from daemon: Conflict. The container name \"/r2g-ra-0\" is already in use by container \"abc\". You have to remove (or rename) that container to be able to reuse that name."
        ));
        assert!(is_name_conflict("Conflict. The name is already in use"));
        assert!(is_name_conflict("name conflict"));
    }

    /// 别的失败不能被当成重名 —— 否则会白删一个容器再重试一次。
    #[test]
    fn other_failures_are_not_name_conflicts() {
        assert!(!is_name_conflict("docker: Error response from daemon: no such image: rust2genshin-play:xxx"));
        assert!(!is_name_conflict("docker 起不来:program not found"));
        assert!(!is_name_conflict(""));
    }
}
