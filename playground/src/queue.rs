//! 并发额度 + 每 IP 一个 + 排队上限。
//!
//! 两条规则来自需求:同时最多 `max_jobs` 个任务;同一个 IP 同时只能有一个
//! (跑着或排着都算),第二次直接拒。NAT 后面多人共用一个出口 IP 会被误伤
//! —— 认了,回一句"请求过多，请稍后重试"。
//!
//! 额度的释放靠 `Ticket` 的 Drop:连接断开、任务失败、进程 panic 折叠栈
//! 都会走到它。**没有任何一条路径需要手工 `release()`**。

use std::{
    collections::{HashSet, VecDeque},
    net::IpAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use tokio::sync::Notify;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitError {
    IpBusy,
    QueueFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub running: usize,
    pub waiting: usize,
    pub max_jobs: usize,
}

/// IPv6 按 /64 归并:一个 /64 里的地址随便换就绕过了"每 IP 一个"。
/// IPv4 按 /32(即原样)。
pub fn client_key(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        // `::ffff:a.b.c.d` 是 IPv4-mapped: dual-stack 监听下内核会把 v4 对端
        // 报成这个形态。不在这里拆回 v4,清零后 4 段会得到 `"::/64"` ——
        // 于是全部 v4 客户端坍缩成同一个 key(还会和 `::`/`::1` 撞),
        // "每 IP 一个"退化成全局串行。
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let mut seg = v6.segments();
                for s in &mut seg[4..] {
                    *s = 0;
                }
                format!("{}/64", std::net::Ipv6Addr::from(seg))
            }
        },
    }
}

#[derive(Debug, Default)]
struct TicketState {
    promoted: AtomicBool,
    notify: Notify,
}

#[derive(Debug, Default)]
struct Inner {
    running: usize,
    /// (客户端 key, 票的状态)。FIFO,谁先排谁先拿额度。
    waiting: VecDeque<(String, Arc<TicketState>)>,
    /// 占着额度的客户端 key —— 跑着和排着都算。
    ips: HashSet<String>,
}

#[derive(Debug)]
pub struct Queue {
    max_jobs: usize,
    max_queue: usize,
    inner: Mutex<Inner>,
}

impl Queue {
    pub fn new(max_jobs: usize, max_queue: usize) -> Arc<Self> {
        Arc::new(Self {
            max_jobs: max_jobs.max(1),
            max_queue,
            inner: Mutex::new(Inner::default()),
        })
    }

    pub fn try_acquire(self: &Arc<Self>, ip: &str) -> Result<Ticket, SubmitError> {
        let mut inner = self.inner.lock().expect("队列锁中毒");
        if inner.ips.contains(ip) {
            return Err(SubmitError::IpBusy);
        }
        let state = Arc::new(TicketState::default());
        if inner.running < self.max_jobs {
            inner.running += 1;
            state.promoted.store(true, Ordering::SeqCst);
        } else {
            if inner.waiting.len() >= self.max_queue {
                return Err(SubmitError::QueueFull);
            }
            inner.waiting.push_back((ip.to_string(), Arc::clone(&state)));
        }
        inner.ips.insert(ip.to_string());
        Ok(Ticket {
            queue: Arc::clone(self),
            ip: ip.to_string(),
            state,
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        let inner = self.inner.lock().expect("队列锁中毒");
        Snapshot {
            running: inner.running,
            waiting: inner.waiting.len(),
            max_jobs: self.max_jobs,
        }
    }

    fn promote(&self, inner: &mut Inner) {
        while inner.running < self.max_jobs {
            let Some((_, st)) = inner.waiting.pop_front() else {
                break;
            };
            inner.running += 1;
            // 先置位再唤醒:等待方在 await 之前先查这个位,不会丢唤醒。
            st.promoted.store(true, Ordering::SeqCst);
            st.notify.notify_one();
        }
    }
}

#[derive(Debug)]
pub struct Ticket {
    queue: Arc<Queue>,
    ip: String,
    state: Arc<TicketState>,
}

impl Ticket {
    pub fn ip(&self) -> &str {
        &self.ip
    }

    pub fn is_running(&self) -> bool {
        self.state.promoted.load(Ordering::SeqCst)
    }

    /// 排队中的位次(从 1 起);已经在跑返回 0。
    pub fn position(&self) -> usize {
        let inner = self.queue.inner.lock().expect("队列锁中毒");
        inner
            .waiting
            .iter()
            .position(|(_, st)| Arc::ptr_eq(st, &self.state))
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    /// 等到拿到并发额度才返回。
    pub async fn wait_started(&self) {
        loop {
            if self.state.promoted.load(Ordering::SeqCst) {
                return;
            }
            self.state.notify.notified().await;
        }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let mut inner = self.queue.inner.lock().expect("队列锁中毒");
        inner.ips.remove(&self.ip);
        if self.state.promoted.load(Ordering::SeqCst) {
            inner.running -= 1;
            self.queue.promote(&mut inner);
        } else {
            inner
                .waiting
                .retain(|(_, st)| !Arc::ptr_eq(st, &self.state));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn q(jobs: usize, queue: usize) -> Arc<Queue> {
        Queue::new(jobs, queue)
    }

    #[test]
    fn ipv4_keys_are_per_address() {
        let a: IpAddr = "1.2.3.4".parse().unwrap();
        let b: IpAddr = "1.2.3.5".parse().unwrap();
        assert_ne!(client_key(a), client_key(b));
        assert_eq!(client_key(a), client_key(a));
    }

    #[test]
    fn ipv6_keys_merge_within_a_64() {
        let a: IpAddr = "2001:db8:1:2::1".parse().unwrap();
        let b: IpAddr = "2001:db8:1:2:ffff:ffff:ffff:ffff".parse().unwrap();
        let c: IpAddr = "2001:db8:1:3::1".parse().unwrap();
        assert_eq!(client_key(a), client_key(b), "同一个 /64 要算同一个客户端");
        assert_ne!(client_key(a), client_key(c));
    }

    #[test]
    fn ipv4_mapped_v6_keys_like_its_v4_self() {
        let v4: IpAddr = "1.2.3.4".parse().unwrap();
        let mapped: IpAddr = "::ffff:1.2.3.4".parse().unwrap();
        assert_eq!(
            client_key(mapped),
            client_key(v4),
            "mapped 形态必须归到它自己的 v4 上,不能落到 ::/64"
        );
        // 不同 v4 不能因为都走 mapped 而一起坍缩
        let other: IpAddr = "::ffff:1.2.3.5".parse().unwrap();
        assert_ne!(client_key(other), client_key(v4));
        // 更不能和未指定地址撞 key
        let unspecified: IpAddr = "::".parse().unwrap();
        assert_ne!(client_key(unspecified), client_key(v4));
    }

    #[test]
    fn under_the_limit_runs_immediately() {
        let q = q(2, 4);
        let a = q.try_acquire("1.1.1.1").unwrap();
        assert!(a.is_running());
        assert_eq!(a.position(), 0);
        let b = q.try_acquire("2.2.2.2").unwrap();
        assert!(b.is_running());
        assert_eq!(q.snapshot().running, 2);
    }

    #[test]
    fn same_ip_twice_is_rejected() {
        let q = q(8, 8);
        let _a = q.try_acquire("1.1.1.1").unwrap();
        assert_eq!(q.try_acquire("1.1.1.1").unwrap_err(), SubmitError::IpBusy);
    }

    #[test]
    fn queued_ticket_still_counts_as_that_ip() {
        let q = q(1, 4);
        let _a = q.try_acquire("1.1.1.1").unwrap();
        let b = q.try_acquire("2.2.2.2").unwrap();
        assert!(!b.is_running());
        assert_eq!(q.try_acquire("2.2.2.2").unwrap_err(), SubmitError::IpBusy);
    }

    #[test]
    fn release_frees_both_the_ip_and_the_slot() {
        let q = q(1, 4);
        let a = q.try_acquire("1.1.1.1").unwrap();
        drop(a);
        assert_eq!(q.snapshot().running, 0);
        assert!(q.try_acquire("1.1.1.1").is_ok(), "同一 IP 释放后要能再提交");
    }

    #[test]
    fn position_is_one_based_and_snapshot_counts() {
        let q = q(1, 4);
        let _a = q.try_acquire("1.1.1.1").unwrap();
        let b = q.try_acquire("2.2.2.2").unwrap();
        let c = q.try_acquire("3.3.3.3").unwrap();
        assert_eq!(b.position(), 1);
        assert_eq!(c.position(), 2);
        let s = q.snapshot();
        assert_eq!((s.running, s.waiting, s.max_jobs), (1, 2, 1));
    }

    #[test]
    fn max_queue_is_enforced() {
        let q = q(1, 1);
        let _a = q.try_acquire("1.1.1.1").unwrap();
        let _b = q.try_acquire("2.2.2.2").unwrap();
        assert_eq!(q.try_acquire("3.3.3.3").unwrap_err(), SubmitError::QueueFull);
        // 被拒的 IP 不该留下痕迹
        assert_eq!(q.snapshot().waiting, 1);
        // 被拒的 IP 一个字段都不该留下 —— 再提一次要还是 QueueFull。
        // (如果它被误插进了 ips 表,这一次会变成 IpBusy,断言就会红。)
        assert_eq!(q.try_acquire("3.3.3.3").unwrap_err(), SubmitError::QueueFull);
    }

    #[test]
    fn dropping_a_queued_ticket_leaves_no_trace() {
        let q = q(1, 1);
        let _a = q.try_acquire("1.1.1.1").unwrap();
        let b = q.try_acquire("2.2.2.2").unwrap();
        drop(b);
        assert_eq!(q.snapshot().waiting, 0);
        assert!(q.try_acquire("2.2.2.2").is_ok());
        assert!(q.try_acquire("3.3.3.3").is_ok(), "额度空出来了,应该能排进来");
    }

    #[tokio::test]
    async fn waiting_ticket_starts_when_a_slot_frees() {
        let q = q(1, 1);
        let a = q.try_acquire("1.1.1.1").unwrap();
        let b = Arc::new(q.try_acquire("2.2.2.2").unwrap());
        let waiter = {
            let b = Arc::clone(&b);
            tokio::spawn(async move { b.wait_started().await })
        };
        drop(a);
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("额度交接必须唤醒排队者")
            .unwrap();
        assert!(b.is_running());
        assert_eq!(q.snapshot().running, 1);
    }

    #[tokio::test]
    async fn wait_started_returns_immediately_when_not_queued() {
        let q = q(4, 4);
        let a = q.try_acquire("1.1.1.1").unwrap();
        tokio::time::timeout(Duration::from_millis(50), a.wait_started())
            .await
            .expect("已在跑的票不该等");
    }
}
