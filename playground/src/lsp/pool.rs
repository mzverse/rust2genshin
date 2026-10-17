//! RA 会话池 —— 见 `docs/superpowers/specs/2026-10-17-play-lsp-completion-design.md`。
//!
//! 池是**纯状态机 + 一个「造新会话」的闭包**,自己不碰 docker —— 容器
//! 生命周期由 `exec::RaProcess` 承担(`kill` 在 `Drop` 里被异步调度)。
//!
//! 不变量:
//!
//! - 空闲数 < `POOL_MIN`:不销毁(没「超出」可言,留待命容器)。
//! - 空闲数 > `POOL_MIN`:销毁多出来的,池回到 `POOL_MIN` 留待命。
//! - 取用时先 ping 查存活性,坏的就丢,坏光了就调 `new_session` 现造。
//!
//! 为什么下限是 1:服务器 15 GB 内存 + 0 swap,RA 进程做完一次 `didOpen`
//! 常驻 839 MB,所以「多于 1」是负债不是资产 —— 下一个请求来时再起
//! 一个,比一直保两个活省一半内存。`POOL_MIN` 现在写死,Task 7 才会
//! 把它做成配置项。

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::Mutex;

use crate::lsp::exec::RaProcess;

/// 池的最小空闲数。空闲数低于它时不销毁(没「超出」可言,留待命容器);
/// 等于它时同样不销毁(「超过」要严格大于);高于它时才销毁多出的。
///
/// Task 7 才会把这个做成配置项 —— 现在先写死。
const POOL_MIN: usize = 1;

/// 取用时拿到的 RA 会话。带销毁钩子 —— `Handle` 被 drop 时调用,真实
/// 实现里它是 `docker rm -f <container>`,假实现里它数计数。
pub struct Handle {
    process: Arc<RaProcess>,
    /// `Drop` 里取走;`None` 表示已被消费过。
    destroy: Option<Box<dyn FnOnce() + Send + 'static>>,
}

impl Handle {
    pub fn process(&self) -> &Arc<RaProcess> {
        &self.process
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if let Some(d) = self.destroy.take() {
            d();
        }
    }
}

/// RAII 句柄:`acquire()` 时拿到,`release(alive)` 时归还。Drop 不显式
/// release 时,异步任务把会话当 `alive=true` 丢回池 —— 调用方忘了释放
/// 不该让会话白白丢掉。
pub struct Acquire {
    handle: Option<Handle>,
    pool: Arc<Pool>,
}

impl Acquire {
    pub fn process(&self) -> &Arc<RaProcess> {
        &self
            .handle
            .as_ref()
            .expect("Acquire 已 release,不能再用 process()")
            .process
    }

    /// 显式归还。`alive=false` 时会话直接销毁(不进池);`alive=true`
    /// 时回到空闲队列,空闲数 > POOL_MIN 会顺带销毁多出的。
    pub async fn release(mut self, alive: bool) {
        let handle = self
            .handle
            .take()
            .expect("Acquire 不该被 double release");
        self.pool.release_handle(handle, alive).await;
    }
}

impl Drop for Acquire {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            // 没显式 release —— 当作 alive=true 丢回池。Drop 不能 await,
            // 只能 spawn;Runtime 关闭瞬间(`#[tokio::test]` 跑完)这条任务
            // 可能被吃掉,真实路径里 `RaProcess::Drop` 会兜底再调一次 kill。
            let pool = Arc::clone(&self.pool);
            tokio::spawn(async move {
                pool.release_handle(handle, true).await;
            });
        }
    }
}

/// 池。`new_session` 是「造一个新会话」的闭包 —— 它返回的 `Handle`
/// 已经装好 `destroy` 钩子。
pub struct Pool {
    idle: Mutex<VecDeque<Handle>>,
    in_use: AtomicUsize,
    new_session: Box<
        dyn Fn() -> Pin<Box<dyn Future<Output = Handle> + Send>> + Send + Sync + 'static,
    >,
}

impl Pool {
    pub fn new<F, Fut>(new_session: F) -> Arc<Self>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Handle> + Send + 'static,
    {
        Arc::new(Self {
            idle: Mutex::new(VecDeque::new()),
            in_use: AtomicUsize::new(0),
            new_session: Box::new(move || Box::pin(new_session())),
        })
    }

    pub fn in_use(&self) -> usize {
        self.in_use.load(Ordering::SeqCst)
    }

    /// 取用一个会话。空闲队列里有的话,先 ping 查存活性;坏的就丢
    /// (触发 destroy 钩子),继续往下 pop,至多尝试「进入时的空闲数」
    /// 次 —— 全坏时不能死循环。空闲弹光了就调 `new_session` 现造。
    pub async fn acquire(self: &Arc<Self>) -> Acquire {
        // 1. 先看空闲队列
        let initial_idle = self.idle.lock().await.len();
        let mut tries = initial_idle;
        while tries > 0 {
            let candidate = {
                let mut idle = self.idle.lock().await;
                idle.pop_back()
            };
            let Some(handle) = candidate else { break };
            // ping 在锁外做 —— 锁里 await 会让其他 release 卡住。
            let alive = handle
                .process
                .ping(std::time::Duration::from_secs(1))
                .await
                .is_ok();
            if alive {
                self.in_use.fetch_add(1, Ordering::SeqCst);
                return Acquire {
                    handle: Some(handle),
                    pool: Arc::clone(self),
                };
            }
            // 死了 —— drop handle,触发 destroy 钩子
            drop(handle);
            tries -= 1;
        }

        // 2. 空闲弹光或全坏:调 new_session 现造一个
        let handle = (self.new_session)().await;
        self.in_use.fetch_add(1, Ordering::SeqCst);
        Acquire {
            handle: Some(handle),
            pool: Arc::clone(self),
        }
    }

    async fn release_handle(&self, handle: Handle, alive: bool) {
        self.in_use.fetch_sub(1, Ordering::SeqCst);
        if !alive {
            // 直接 drop,触发 destroy 钩子
            drop(handle);
            return;
        }
        // alive:push 回空闲队列,然后看是否要销毁多出的
        let to_destroy = {
            let mut idle = self.idle.lock().await;
            idle.push_back(handle);
            // 「超过」要严格大于 —— 等于 POOL_MIN 时那个是留待命容器,
            // 不能销毁。
            if idle.len() > POOL_MIN {
                idle.pop_front()
            } else {
                None
            }
        };
        if let Some(extra) = to_destroy {
            drop(extra);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex as StdMutex};

    use async_trait::async_trait;
    use tokio::sync::mpsc;

    use crate::lsp::exec::{ExecError, RaCommand};
    use crate::lsp::frame;

    /// 假 RA 进程的「测试夹具」—— 每次 `new_session` +1 created,
    /// 每次 handle 被销毁(Handle::Drop)都 +1 destroyed。
    struct FakeState {
        created: AtomicUsize,
        destroyed: AtomicUsize,
    }

    impl FakeState {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                created: AtomicUsize::new(0),
                destroyed: AtomicUsize::new(0),
            })
        }
        fn created(&self) -> usize {
            self.created.load(Ordering::SeqCst)
        }
        fn destroyed(&self) -> usize {
            self.destroyed.load(Ordering::SeqCst)
        }
    }

    /// 假 docker —— 与 `exec.rs` 测试里的 `FakeDocker` 同形,只多了
    /// 「可选地预先喂一条 ping 响应」让 ping 健康可控。
    struct FakeDocker {
        stdout_tx: StdMutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
        killed: AtomicBool,
    }

    impl FakeDocker {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                stdout_tx: StdMutex::new(None),
                killed: AtomicBool::new(false),
            })
        }

        /// 喂一条 ping 会等待的响应。必须在 `RaProcess::start` 之后调
        /// (那时 `take_stdout` 已经把 sender 存进 `stdout_tx`)。
        fn feed_ping_response(&self) {
            let tx_opt = self
                .stdout_tx
                .lock()
                .expect("Mutex 不该中毒:持锁段不 panic")
                .clone();
            if let Some(tx) = tx_opt {
                let resp = frame::encode(r#"{"jsonrpc":"2.0","id":1,"result":null}"#);
                tx.send(resp)
                    .expect("UnboundedSender 不该 send 失败");
            }
        }
    }

    #[async_trait]
    impl RaCommand for FakeDocker {
        async fn spawn(&self, _args: &[String]) -> Result<(), ExecError> {
            Ok(())
        }
        async fn write(&self, _bytes: &[u8]) -> Result<(), ExecError> {
            Ok(())
        }
        async fn kill(&self) {
            self.killed.store(true, Ordering::SeqCst);
        }
        fn take_stdout(&self) -> Option<mpsc::UnboundedReceiver<Vec<u8>>> {
            let (tx, rx) = mpsc::unbounded_channel();
            *self
                .stdout_tx
                .lock()
                .expect("Mutex 不该中毒:持锁段不 panic") = Some(tx);
            Some(rx)
        }
    }

    /// 造一个会 ping 通(`healthy=true`)或不通(`healthy=false`)的 handle。
    /// 每次都 +1 `state.created`,销毁钩子 +1 `state.destroyed`。
    async fn make_handle(state: &Arc<FakeState>, healthy: bool) -> Handle {
        let docker = FakeDocker::new();
        let cmd: Arc<dyn RaCommand> =
            Arc::clone(&docker) as Arc<dyn RaCommand>;
        let p = RaProcess::start(cmd, "test")
            .await
            .expect("FakeDocker::spawn 不该失败");
        if healthy {
            docker.feed_ping_response();
        }
        state.created.fetch_add(1, Ordering::SeqCst);
        let s = Arc::clone(state);
        let destroy: Box<dyn FnOnce() + Send + 'static> = Box::new(move || {
            s.destroyed.fetch_add(1, Ordering::SeqCst);
        });
        Handle {
            process: Arc::new(p),
            destroy: Some(destroy),
        }
    }

    fn new_pool(state: Arc<FakeState>, healthy: bool) -> Arc<Pool> {
        let s = Arc::clone(&state);
        Pool::new(move || {
            let s2 = Arc::clone(&s);
            let h = healthy;
            async move { make_handle(&s2, h).await }
        })
    }

    /// 池空时取用一次,会调一次 `new_session`。
    #[tokio::test]
    async fn empty_pool_yields_a_new_session() {
        let state = FakeState::new();
        let pool = new_pool(Arc::clone(&state), true);
        let acq = pool.acquire().await;
        assert_eq!(state.created(), 1, "取用一次该造一个新会话");
        assert_eq!(state.destroyed(), 0, "取用还没释放,不该销毁");
        acq.release(true).await;
        assert_eq!(state.destroyed(), 0, "放回活的,不该销毁");
        // 池里现在有 1 个空闲,正好是 POOL_MIN —— 不销毁
        assert_eq!(pool.idle.lock().await.len(), 1);
    }

    /// POOL_MIN = 1,池里有 1 个空闲;取用 → 0;放回 → 1,什么都不销毁。
    #[tokio::test]
    async fn idle_never_drops_below_min() {
        let state = FakeState::new();
        let pool = new_pool(Arc::clone(&state), true);
        // 手动注入 1 个空闲 entry —— 不调 new_session,所以 created=1
        let h = make_handle(&state, true).await;
        assert_eq!(state.created(), 1);
        pool.idle.lock().await.push_back(h);
        assert_eq!(pool.idle.lock().await.len(), 1);

        let acq = pool.acquire().await;
        assert_eq!(pool.idle.lock().await.len(), 0, "取用后空闲该 0");
        assert_eq!(pool.in_use(), 1, "取用后在用数该 1");
        assert_eq!(state.created(), 1, "有空就不该再造");
        assert_eq!(state.destroyed(), 0, "取用不该销毁");

        acq.release(true).await;
        assert_eq!(pool.idle.lock().await.len(), 1, "放回后空闲该 1");
        assert_eq!(pool.in_use(), 0, "放回后在用数该归 0");
        assert_eq!(state.destroyed(), 0, "POOL_MIN 时不该销毁");
    }

    /// POOL_MIN = 1,池里有 2 个空闲;取用一个再放回 → 空闲数 = 2 > 1
    /// → 销毁 1 个。
    #[tokio::test]
    async fn idle_above_min_is_reclaimed() {
        let state = FakeState::new();
        let pool = new_pool(Arc::clone(&state), true);
        // 注入 2 个空闲 entry
        let h1 = make_handle(&state, true).await;
        let h2 = make_handle(&state, true).await;
        assert_eq!(state.created(), 2);
        pool.idle.lock().await.push_back(h1);
        pool.idle.lock().await.push_back(h2);
        assert_eq!(pool.idle.lock().await.len(), 2);

        let acq = pool.acquire().await;
        assert_eq!(pool.idle.lock().await.len(), 1, "取用一个后空闲该 1");
        assert_eq!(state.destroyed(), 0);

        acq.release(true).await;
        assert_eq!(
            pool.idle.lock().await.len(),
            1,
            "销毁后该回到 POOL_MIN(=1)"
        );
        assert_eq!(state.destroyed(), 1, "空闲 > POOL_MIN 该销毁 1 个");
    }

    /// Review Focus 第 2 条:ping 失败的 RA 释放时不回池,直接销毁;
    /// 同时验证 acquire 拿到一个 ping 失败的 entry 时不会把它交给
    /// 调用方 —— 会丢弃并继续试。
    #[tokio::test]
    async fn dead_session_is_not_returned_to_the_pool() {
        let state = FakeState::new();
        let pool = new_pool(Arc::clone(&state), true); // new_session 出来的都是 healthy
        // 池里塞一个 ping 不通的 entry(healthy=false)
        let dead = make_handle(&state, false).await;
        assert_eq!(state.created(), 1);
        pool.idle.lock().await.push_back(dead);

        // 取用:pop 那个坏的 → ping 失败 → drop + new_session 造一个新的
        let acq = pool.acquire().await;
        assert_eq!(state.created(), 2, "坏 entry 该被替换成新造的");
        assert_eq!(state.destroyed(), 1, "坏 entry 该被销毁");
        assert_eq!(pool.in_use(), 1, "取用后在用数该 1");

        // release(alive=false) → 该销毁,不入池
        acq.release(false).await;
        assert_eq!(state.destroyed(), 2);
        assert_eq!(pool.idle.lock().await.len(), 0, "alive=false 不该回池");
        // 会话死了不等于计数能漏 —— 在用数必须归 0,否则下一个请求
        // 看到的「在用数」会一直虚高。
        assert_eq!(pool.in_use(), 0, "释放死会话后在用数也得归 0");
    }
}