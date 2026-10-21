//! 会话级占用:**同一个 IP 同时只能有一条活连接**。
//!
//! 为什么要放在连接这一层,而不是编译那一层:编译限额只管得住「同时
//! 几个 cargo 在跑」,管不住 **rust-analyzer**。代码分析那条路不经过
//! 队列 —— `lsp_open` 直接从会话池取一个 RA 会话,池空就现造一个容器
//! (`docker run -d` + `docker exec -i rust-analyzer`)。一个 RA 进程做完
//! 一次 `didOpen` 常驻 839 MB,而会话池只有「空闲下限」没有「在用上限」。
//! 于是同一个 IP 开 N 个标签页就能起 N 个 RA 容器,「每 IP 一个」这条
//! 限流对真正贵的资源形同虚设。
//!
//! 把限制定在**连接**上,一 IP 一连接 ⇒ 一 IP 一个 RA 会话、一个在途编译。
//!
//! 两条规矩:
//!
//! - key 用 [`crate::server::client_key_of`](super::client_key_of) 算出来的
//!   客户端身份(IPv6 按 /64 归并、IPv4-mapped 归一),不是裸 IP 字符串 ——
//!   否则在 /64 里换地址就能绕过。
//! - 占用随**连接结束**归还,靠 `SessionGuard` 的 `Drop`。没有手工
//!   `release()` 的路径,连接断开 / 协议错 / 任务收尾都自动还。
//!
//! 为什么不在**握手**阶段就 403:那样前端的自动重连会变成
//! 「403 → 退避 → 再连 → 再 403」的死循环。改成「握手成功 → 收到一条
//! 明确的 `session_busy` 错误 → 前端停止自动重连」,用户看得懂发生了什么。

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

/// 这个 IP 已经有别的连接在用了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionBusy;

/// 活着的连接表。`HashSet` 而不是计数:一个 IP 同时只允许**一条**连接,
/// 计数没有意义。
#[derive(Debug, Default)]
pub struct SessionGate {
    inner: Mutex<HashSet<String>>,
}

impl SessionGate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 占一个坑。成功时返回的 guard 负责归还。
    pub fn acquire(self: &Arc<Self>, key: &str) -> Result<SessionGuard, SessionBusy> {
        let mut inner = self.inner.lock().expect("会话表锁中毒");
        if inner.contains(key) {
            return Err(SessionBusy);
        }
        inner.insert(key.to_string());
        Ok(SessionGuard {
            gate: Arc::clone(self),
            key: key.to_string(),
        })
    }

    /// 现在有几条活连接。给 `/status` 与测试看。
    pub fn active(&self) -> usize {
        self.inner.lock().expect("会话表锁中毒").len()
    }

    fn release(&self, key: &str) {
        let mut inner = self.inner.lock().expect("会话表锁中毒");
        inner.remove(key);
    }
}

/// 一个连接持有的占用。**drop 即归还** —— 连接结束(无论是正常断开、
/// 协议错还是任务收尾)都会走到这里。
#[derive(Debug)]
pub struct SessionGuard {
    gate: Arc<SessionGate>,
    key: String,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.gate.release(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_connection_from_the_same_key_is_rejected() {
        let g = SessionGate::new();
        let _a = g.acquire("1.2.3.4").expect("第一个该拿到");
        assert_eq!(g.acquire("1.2.3.4").unwrap_err(), SessionBusy);
    }

    #[test]
    fn different_keys_do_not_interfere() {
        let g = SessionGate::new();
        let _a = g.acquire("1.2.3.4").unwrap();
        let _b = g.acquire("5.6.7.8").expect("别的 IP 不该被拒");
        assert_eq!(g.active(), 2);
    }

    #[test]
    fn dropping_the_guard_frees_the_key() {
        let g = SessionGate::new();
        let a = g.acquire("1.2.3.4").unwrap();
        assert!(g.acquire("1.2.3.4").is_err());
        drop(a);
        // 拿着 guard 才有占用 —— 直接 `is_ok()` 的话临时值当场就被 drop 了。
        let b = g.acquire("1.2.3.4").expect("连接断了就该还回去");
        assert_eq!(g.active(), 1);
        drop(b);
    }

    #[test]
    fn a_key_can_be_reacquired_after_the_guard_is_gone() {
        // 归还之后同一个 key 要能再拿 —— 连接断开 → 用户刷新 → 再连。
        let g = SessionGate::new();
        {
            let _a = g.acquire("2001:db8::/64").unwrap();
            assert!(g.acquire("2001:db8::/64").is_err());
        }
        assert!(g.acquire("2001:db8::/64").is_ok());
    }
}
