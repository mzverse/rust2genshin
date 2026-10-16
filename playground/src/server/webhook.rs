//! GitHub webhook 的签名校验。
//!
//! 签名是 `HMAC-SHA256(secret, 原始 body)`,放进 `X-Hub-Signature-256`。
//! 必须拿**未解析的原始字节**去算 —— 解析再重新序列化会得到不同的字节,
//! 签名必然对不上。

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// 常量时间比较(hmac 的 `verify_slice` 自带),不给时序攻击留缝。
pub fn verify_signature(secret: &str, body: &[u8], header: Option<&str>) -> bool {
    let Some(header) = header else { return false };
    let Some(hex_sig) = header.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(raw) = hex::decode(hex_sig.trim()) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&raw).is_ok()
}

/// 只认 `push` 事件且 ref 是 `refs/heads/main`。
pub fn is_push_to_main(event: Option<&str>, body: &[u8]) -> bool {
    if event != Some("push") {
        return false;
    }
    #[derive(serde::Deserialize)]
    struct Payload {
        #[serde(default)]
        r#ref: String,
    }
    serde_json::from_slice::<Payload>(body)
        .map(|p| p.r#ref == "refs/heads/main")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 这两条向量是用 python 的 hmac 独立算出来的,不是本函数算的。
    const SECRET: &str = "It's a Secret to Everybody";
    const BODY: &[u8] = b"Hello, World!";
    const SIG: &str = "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";

    #[test]
    fn correct_signature_passes() {
        assert!(verify_signature(SECRET, BODY, Some(SIG)));
    }

    #[test]
    fn wrong_secret_fails() {
        assert!(!verify_signature("wrong", BODY, Some(SIG)));
    }

    #[test]
    fn modified_body_fails() {
        assert!(!verify_signature(SECRET, b"Hello, World?", Some(SIG)));
    }

    #[test]
    fn missing_or_malformed_header_fails() {
        assert!(!verify_signature(SECRET, BODY, None));
        assert!(!verify_signature(SECRET, BODY, Some("deadbeef")));
        assert!(!verify_signature(SECRET, BODY, Some("sha1=abcd")));
        assert!(!verify_signature(SECRET, BODY, Some("sha256=zzzz")));
    }

    #[test]
    fn empty_secret_never_matches() {
        // GITHUB_WEBHOOK_SECRET 为空时端点必须整条 403 —— 这一层只保证
        // 空 secret 不会意外放行(空 key 的 HMAC 是算得出来的)。
        assert!(!verify_signature("", BODY, Some(SIG)));
    }

    #[test]
    fn push_to_main_is_accepted() {
        let body = br#"{"ref":"refs/heads/main","after":"abc"}"#;
        assert!(is_push_to_main(Some("push"), body));
    }

    #[test]
    fn other_branches_and_events_are_rejected() {
        let body = br#"{"ref":"refs/heads/main"}"#;
        assert!(!is_push_to_main(Some("pull_request"), body));
        assert!(!is_push_to_main(None, body));
        assert!(!is_push_to_main(Some("push"), br#"{"ref":"refs/heads/dev"}"#));
        assert!(!is_push_to_main(Some("push"), br#"{"ref":"refs/tags/v1"}"#));
        assert!(!is_push_to_main(Some("push"), b"not json"));
    }
}
