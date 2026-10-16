//! HTTP / WS 服务的装配点。`server` 只依赖 `queue` / `job` / `runner`,
//! 反向不成立 —— 将来加 rust-analyzer 的长驻会话,是往这一层加 `session`
//! 模块,不动下面。

pub mod http;
pub mod webhook;

pub use http::AppState;

use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

/// 客户端 IP。**只有** `PLAYGROUND_TRUST_PROXY=1` 时才看
/// `X-Forwarded-For` —— 否则谁都能伪造这个头绕过"每 IP 一个"。
/// 取**最右一个可解析的 IP**:XFF 是追加式列表,最右那项才是最近的代理
/// 亲眼看到的对端;取第一跳等于让客户端自己填身份(它能在最前面塞值)。
pub fn client_ip(headers: &HeaderMap, peer: SocketAddr, trust_proxy: bool) -> IpAddr {
    if trust_proxy
        && let Some(ip) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                v.split(',')
                    .filter_map(|s| s.trim().parse::<IpAddr>().ok())
                    .next_back()
            })
    {
        return ip;
    }
    peer.ip()
}

/// 客户端身份 key:先取 IP,再归一化(IPv4 原样 / IPv6 按 /64)。
/// **握手侧必须用这个、不要用 `client_ip(..).to_string()`** ——
/// 后者会漏掉 /64 归并,一个 /64 里换地址就能绕过"每 IP 一个"。
pub fn client_key_of(headers: &HeaderMap, peer: SocketAddr, trust_proxy: bool) -> String {
    crate::queue::client_key(client_ip(headers, peer, trust_proxy))
}

/// WebSocket **不受 CORS 保护**,握手必须自己查 Origin。
/// 没有 Origin 头(非浏览器客户端:curl、测试)放行 —— 它没有"被第三方
/// 页面借用身份"的问题,而浏览器一定会带。
pub fn origin_allowed(origin: Option<&str>, allowed: &[String]) -> bool {
    match origin {
        None => true,
        Some(o) => allowed.iter().any(|a| a == o),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn peer(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    fn headers_with(name: &str, value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            name.parse::<axum::http::HeaderName>().unwrap(),
            HeaderValue::from_str(value).unwrap(),
        );
        h
    }

    #[test]
    fn without_trust_proxy_the_header_is_ignored() {
        let h = headers_with("x-forwarded-for", "9.9.9.9");
        assert_eq!(
            client_ip(&h, peer("1.2.3.4:5000"), false),
            "1.2.3.4".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn with_trust_proxy_the_nearest_hop_wins() {
        let h = headers_with("x-forwarded-for", "9.9.9.9, 8.8.8.8");
        assert_eq!(
            client_ip(&h, peer("1.2.3.4:5000"), true),
            "8.8.8.8".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn with_trust_proxy_a_single_hop_is_used() {
        let h = headers_with("x-forwarded-for", "9.9.9.9");
        assert_eq!(
            client_ip(&h, peer("1.2.3.4:5000"), true),
            "9.9.9.9".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn with_trust_proxy_but_no_header_falls_back_to_peer() {
        let h = HeaderMap::new();
        assert_eq!(
            client_ip(&h, peer("1.2.3.4:5000"), true),
            "1.2.3.4".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn garbage_forwarded_for_falls_back_to_peer() {
        let h = headers_with("x-forwarded-for", "not-an-ip");
        assert_eq!(
            client_ip(&h, peer("1.2.3.4:5000"), true),
            "1.2.3.4".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn origins() {
        let allowed = vec!["https://mzverse.github.io".to_string()];
        assert!(origin_allowed(Some("https://mzverse.github.io"), &allowed));
        assert!(!origin_allowed(Some("https://evil.example"), &allowed));
        assert!(origin_allowed(None, &allowed), "非浏览器客户端没有 Origin");
        assert!(!origin_allowed(
            Some("https://mzverse.github.io.evil.com"),
            &allowed
        ));
    }

    #[test]
    fn client_key_of_merges_v6_within_a_64() {
        // 同一个 /64 里的两个地址必须算同一个客户端 —— 否则在 /64 里
        // 换地址就能绕过"每 IP 一个"。
        let a = headers_with("x-forwarded-for", "2001:db8:1:2::1");
        let b = headers_with("x-forwarded-for", "2001:db8:1:2:ffff:ffff:ffff:ffff");
        assert_eq!(
            client_key_of(&a, peer("1.2.3.4:5000"), true),
            client_key_of(&b, peer("1.2.3.4:5000"), true)
        );
    }

    #[test]
    fn client_key_of_maps_v4_mapped_to_v4() {
        // IPv4-mapped 形态必须归到它自己的 v4 上,不能和纯 v4 被算成两个人。
        let mapped = headers_with("x-forwarded-for", "::ffff:1.2.3.4");
        let plain = headers_with("x-forwarded-for", "1.2.3.4");
        assert_eq!(
            client_key_of(&mapped, peer("9.9.9.9:5000"), true),
            client_key_of(&plain, peer("9.9.9.9:5000"), true)
        );
    }
}
