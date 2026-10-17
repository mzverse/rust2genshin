//! RA 会话池 —— 见 `docs/superpowers/specs/2026-10-17-play-lsp-completion-design.md`。
//!
//! 池是**纯状态机 + 一个「造新会话」的闭包**,自己不碰 docker —— 容器
//! 生命周期由 `exec::RaProcess` 承担(`kill` 在 `Drop` 里被异步调度)。
//!
//! 不变量:
//!
//! - 空闲数 <= `min`:不销毁(没「超出」可言,留待命容器)。
//! - 空闲数 > `min`:销毁多出来的,池回到 `min` 留待命。
//! - 取用时先用 handle 自己的存活探针查活,坏的就丢,坏光了就调
//!   `new_session` 现造;造不出来返回 [`PoolError::Create`]。
//!
//! 为什么下限通常是 1:服务器 15 GB 内存 + 0 swap,RA 进程做完一次
//! `didOpen` 常驻 839 MB,所以「多于 1」是负债不是资产 —— 下一个请求来时
//! 再起一个,比一直保两个活省一半内存。下限由 `Config::lsp_pool_min`
//! (默认 1)决定。

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use thiserror::Error;
use tokio::sync::Mutex;

use crate::lsp::exec::RaProcess;
use crate::lsp::session::Session;

/// 存活性探针。**可替换**:池的单元测试可以直接说「活着」,而真实会话
/// 必须用 [`Session::ping`] —— 裸管道 ping 会和后台 reader 抢输出。
type Liveness =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync + 'static>;

/// 取不到会话的两种修法不一样:池里没有 / 全坏时现造,造不出就是这条。
#[derive(Debug, Error)]
pub enum PoolError {
    #[error("起不了 RA 会话(镜像未就绪或 docker 不可用)")]
    Create,
}

/// 池里装的一个会话。带销毁钩子 —— `Handle` 被 drop 时调用,真实
/// 实现里它是 `docker rm -f <container>`,假实现里它数计数。
pub struct Handle {
    process: Arc<RaProcess>,
    session: Arc<Session>,
    /// 默认探针是 ping 裸管道 —— 只给池的单元测试用(那里的假 handle
    /// 没有 reader 竞争)。**真实会话必须 `with_liveness` 换成
    /// [`Session::ping`]。**
    liveness: Liveness,
    /// 上次回到空闲队列的时间,回收循环据此判断是否超龄。
    pub(crate) idle_since: Instant,
    /// `Drop` 里取走;`None` 表示已被消费过。
    destroy: Option<Box<dyn FnOnce() + Send + 'static>>,
}

impl Handle {
    pub fn new(
        process: Arc<RaProcess>,
        session: Arc<Session>,
        destroy: Box<dyn FnOnce() + Send + 'static>,
    ) -> Self {
        let p = Arc::clone(&process);
        Self {
            process,
            session,
            liveness: Box::new(move || {
                let p = Arc::clone(&p);
                Box::pin(async move { p.ping(Duration::from_secs(1)).await.is_ok() })
            }),
            idle_since: Instant::now(),
            destroy: Some(destroy),
        }
    }

    /// 换掉存活探针。真实会话传 [`Session::ping`] 的包装,绕开裸管道。
    pub fn with_liveness(
        mut self,
        f: impl Fn() -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync + 'static,
    ) -> Self {
        self.liveness = Box::new(f);
        self
    }

    pub fn session(&self) -> &Arc<Session> {
        &self.session
    }

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
    pub fn session(&self) -> &Arc<Session> {
        &self
            .handle
            .as_ref()
            .expect("Acquire 已 release,不能再用 session()")
            .session
    }

    /// 显式归还。`alive=false` 时会话直接销毁(不进池);`alive=true`
    /// 时回到空闲队列,空闲数 > `min` 会顺带销毁多出的。
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
/// 已经装好 `destroy` 钩子;`None` = 造不出来(镜像没就绪 / docker 不可用)。
pub struct Pool {
    idle: Mutex<VecDeque<Handle>>,
    in_use: AtomicUsize,
    /// 至少留几个空闲会话,来自 `Config::lsp_pool_min`。
    min: usize,
    new_session: Box<
        dyn Fn() -> Pin<Box<dyn Future<Output = Option<Handle>> + Send>> + Send + Sync + 'static,
    >,
}

impl Pool {
    pub fn new<F, Fut>(min: usize, new_session: F) -> Arc<Self>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Option<Handle>> + Send + 'static,
    {
        Arc::new(Self {
            idle: Mutex::new(VecDeque::new()),
            in_use: AtomicUsize::new(0),
            min,
            new_session: Box::new(move || Box::pin(new_session())),
        })
    }

    pub fn in_use(&self) -> usize {
        self.in_use.load(Ordering::SeqCst)
    }

    pub async fn idle_count(&self) -> usize {
        self.idle.lock().await.len()
    }

    /// 取用一个会话。空闲队列里有的话,先用 handle 自己的探针查存活性;
    /// 坏的就丢(触发 destroy 钩子),继续往下 pop,至多尝试「进入时的
    /// 空闲数」次 —— 全坏时不能死循环。空闲弹光了就调 `new_session`
    /// 现造;造不出来返回 [`PoolError::Create`]。
    pub async fn acquire(self: &Arc<Self>) -> Result<Acquire, PoolError> {
        // 1. 先看空闲队列
        let initial_idle = self.idle.lock().await.len();
        let mut tries = initial_idle;
        while tries > 0 {
            let candidate = {
                let mut idle = self.idle.lock().await;
                idle.pop_back()
            };
            let Some(handle) = candidate else { break };
            // 探针在锁外做 —— 锁里 await 会让其他 release 卡住。
            let alive = (handle.liveness)().await;
            if alive {
                self.in_use.fetch_add(1, Ordering::SeqCst);
                return Ok(Acquire {
                    handle: Some(handle),
                    pool: Arc::clone(self),
                });
            }
            // 死了 —— drop handle,触发 destroy 钩子
            drop(handle);
            tries -= 1;
        }

        // 2. 空闲弹光或全坏:调 new_session 现造一个
        let handle = (self.new_session)().await.ok_or(PoolError::Create)?;
        self.in_use.fetch_add(1, Ordering::SeqCst);
        Ok(Acquire {
            handle: Some(handle),
            pool: Arc::clone(self),
        })
    }

    /// 销毁空闲超过 `ttl` 的会话,但至少留 `min` 个。返回销毁数。
    ///
    /// `ttl == 0` 是「不回收」的哨兵(`Config` 里 0 表示关掉这条策略),
    /// 而不是「立刻全部回收」。从队首(最老)开始删,留下的自然是最新的。
    pub async fn sweep_idle(&self, ttl: Duration) -> usize {
        if ttl.is_zero() {
            return 0;
        }
        let mut doomed = Vec::new();
        {
            let mut idle = self.idle.lock().await;
            let mut i = 0;
            while i < idle.len() {
                if idle.len() <= self.min || idle[i].idle_since.elapsed() < ttl {
                    i += 1;
                } else {
                    doomed.push(idle.remove(i).expect("i 在界内"));
                }
            }
        }
        let n = doomed.len();
        drop(doomed); // 触发 destroy
        n
    }

    async fn release_handle(&self, mut handle: Handle, alive: bool) {
        self.in_use.fetch_sub(1, Ordering::SeqCst);
        if !alive {
            // 直接 drop,触发 destroy 钩子
            drop(handle);
            return;
        }
        // alive:push 回空闲队列,然后看是否要销毁多出的
        handle.idle_since = Instant::now();
        let to_destroy = {
            let mut idle = self.idle.lock().await;
            idle.push_back(handle);
            // 「超过」要严格大于 —— 等于 `min` 时那个是留待命容器,
            // 不能销毁。
            if idle.len() > self.min {
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

    /// 假 docker —— 只记录 spawn/kill,不提供 stdout。池的单元测试
    /// 通过 `with_liveness` 直接给探针结论,不依赖管道。
    struct FakeDocker {
        killed: AtomicBool,
        _stdout_tx: StdMutex<Option<mpsc::UnboundedSender<Vec<u8>>>>,
    }

    impl FakeDocker {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                killed: AtomicBool::new(false),
                _stdout_tx: StdMutex::new(None),
            })
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
                ._stdout_tx
                .lock()
                .expect("Mutex 不该中毒:持锁段不 panic") = Some(tx);
            Some(rx)
        }
    }

    /// 造一个存活探针直接返回 `healthy` 的 handle。真实会话还要起 reader,
    /// 裸管道 ping 会和它抢帧 —— 池的测试只关心状态机,探针在这里注入。
    /// 每次都 +1 `state.created`,销毁钩子 +1 `state.destroyed`。
    async fn make_handle(state: &Arc<FakeState>, healthy: bool) -> Handle {
        let docker = FakeDocker::new();
        let cmd: Arc<dyn RaCommand> = Arc::clone(&docker) as Arc<dyn RaCommand>;
        let p = RaProcess::start(cmd, "test")
            .await
            .expect("FakeDocker::spawn 不该失败");
        state.created.fetch_add(1, Ordering::SeqCst);
        let s = Arc::clone(state);
        let destroy: Box<dyn FnOnce() + Send + 'static> = Box::new(move || {
            s.destroyed.fetch_add(1, Ordering::SeqCst);
        });
        let proc = Arc::new(p);
        let session = Arc::new(Session::new(Arc::clone(&proc)));
        Handle::new(proc, session, destroy).with_liveness(move || Box::pin(async move { healthy }))
    }

    fn new_pool(state: Arc<FakeState>, healthy: bool) -> Arc<Pool> {
        let s = Arc::clone(&state);
        Pool::new(1, move || {
            let s2 = Arc::clone(&s);
            let h = healthy;
            async move { Some(make_handle(&s2, h).await) }
        })
    }

    /// 池空时取用一次,会调一次 `new_session`。
    #[tokio::test]
    async fn empty_pool_yields_a_new_session() {
        let state = FakeState::new();
        let pool = new_pool(Arc::clone(&state), true);
        let acq = pool.acquire().await.expect("该造出一个新会话");
        assert_eq!(state.created(), 1, "取用一次该造一个新会话");
        assert_eq!(state.destroyed(), 0, "取用还没释放,不该销毁");
        acq.release(true).await;
        assert_eq!(state.destroyed(), 0, "放回活的,不该销毁");
        // 池里现在有 1 个空闲,正好是 min —— 不销毁
        assert_eq!(pool.idle.lock().await.len(), 1);
    }

    /// min = 1,池里有 1 个空闲;取用 → 0;放回 → 1,什么都不销毁。
    #[tokio::test]
    async fn idle_never_drops_below_min() {
        let state = FakeState::new();
        let pool = new_pool(Arc::clone(&state), true);
        // 手动注入 1 个空闲 entry —— 不调 new_session,所以 created=1
        let h = make_handle(&state, true).await;
        assert_eq!(state.created(), 1);
        pool.idle.lock().await.push_back(h);
        assert_eq!(pool.idle.lock().await.len(), 1);

        let acq = pool.acquire().await.expect("池里有一个活的");
        assert_eq!(pool.idle.lock().await.len(), 0, "取用后空闲该 0");
        assert_eq!(pool.in_use(), 1, "取用后在用数该 1");
        assert_eq!(state.created(), 1, "有空就不该再造");
        assert_eq!(state.destroyed(), 0, "取用不该销毁");

        acq.release(true).await;
        assert_eq!(pool.idle.lock().await.len(), 1, "放回后空闲该 1");
        assert_eq!(pool.in_use(), 0, "放回后在用数该归 0");
        assert_eq!(state.destroyed(), 0, "min 时不该销毁");
    }

    /// min = 1,池里有 2 个空闲;取用一个再放回 → 空闲数 = 2 > 1
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

        let acq = pool.acquire().await.expect("池里有一个活的");
        assert_eq!(pool.idle.lock().await.len(), 1, "取用一个后空闲该 1");
        assert_eq!(state.destroyed(), 0);

        acq.release(true).await;
        assert_eq!(pool.idle.lock().await.len(), 1, "销毁后该回到 min(=1)");
        assert_eq!(state.destroyed(), 1, "空闲 > min 该销毁 1 个");
    }

    /// 探针失败的 RA 释放时不回池,直接销毁;同时验证 acquire 拿到一个
    /// 探针失败的 entry 时不会把它交给调用方 —— 会丢弃并继续试。
    #[tokio::test]
    async fn dead_session_is_not_returned_to_the_pool() {
        let state = FakeState::new();
        let pool = new_pool(Arc::clone(&state), true); // new_session 出来的都是 healthy
        // 池里塞一个探针失败的 entry
        let dead = make_handle(&state, false).await;
        assert_eq!(state.created(), 1);
        pool.idle.lock().await.push_back(dead);

        // 取用:pop 那个坏的 → 探针失败 → drop + new_session 造一个新的
        let acq = pool.acquire().await.expect("坏的光了之后该现造");
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

    /// 存活性检查必须用 handle 自己的探针,而不是裸管道 ping:真实会话
    /// 的 reader 会和裸管道抢帧。fake 的裸管道没人回,只有探针能说「活」。
    #[tokio::test]
    async fn acquire_uses_the_handles_liveness_probe() {
        let state = FakeState::new();
        let pool = new_pool(Arc::clone(&state), true);
        let h = make_handle(&state, true).await; // 裸管道 ping 会失败(没人回)
        let probed = Arc::new(AtomicUsize::new(0));
        let p = Arc::clone(&probed);
        let h = h.with_liveness(move || {
            let p = Arc::clone(&p);
            Box::pin(async move {
                p.fetch_add(1, Ordering::SeqCst);
                true
            })
        });
        pool.idle.lock().await.push_back(h);

        let acq = pool.acquire().await.expect("liveness 说活就该拿到");
        assert_eq!(probed.load(Ordering::SeqCst), 1, "必须用 handle 自己的探针");
        assert_eq!(state.created(), 1, "不该因为裸管道 ping 不通就重造");
        acq.release(true).await;
    }

    /// `new_session` 返回 `None`(镜像没就绪 / docker 起不来)时,acquire
    /// 必须报错而不是交出一个假的会话。
    #[tokio::test]
    async fn acquire_reports_a_failed_creation() {
        let pool = Pool::new(1, || async { None });
        assert!(pool.acquire().await.is_err());
    }

    /// 回收只动空闲且超龄的,且保留下限。
    #[tokio::test]
    async fn sweep_idle_destroys_only_expired_entries_above_the_minimum() {
        let state = FakeState::new();
        let pool = new_pool(Arc::clone(&state), true);
        pool.idle.lock().await.push_back(make_handle(&state, true).await);
        pool.idle.lock().await.push_back(make_handle(&state, true).await);
        assert_eq!(state.created(), 2);

        // TTL = 0 是「不回收」的哨兵,一个都不删。
        assert_eq!(pool.sweep_idle(Duration::ZERO).await, 0);
        assert_eq!(pool.idle.lock().await.len(), 2);

        // 给两条都标成很久以前,再放一条新的 —— 超出 min=1 的才被删。
        {
            let mut idle = pool.idle.lock().await;
            for h in idle.iter_mut() {
                h.idle_since = Instant::now() - Duration::from_secs(3600);
            }
        }
        let fresh = make_handle(&state, true).await;
        {
            let mut idle = pool.idle.lock().await;
            idle.push_back(fresh);
        }
        assert_eq!(pool.sweep_idle(Duration::from_secs(90)).await, 2);
        assert_eq!(pool.idle.lock().await.len(), 1, "至少留 min 个");
        assert_eq!(state.destroyed(), 2);
    }
}
