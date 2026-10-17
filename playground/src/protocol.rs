//! WebSocket 的信封消息。
//!
//! **可扩展 + 连接长开**是两条硬约束,不是为了好看:将来要加
//! `{"type":"lsp", ...}` 的双向转发(rust-analyzer 代码提示),不需要动
//! 现有分支。写成"一提交一连接就关"就得推倒重来。

use serde::{Deserialize, Serialize};

use crate::docker::Stream;

pub const PROTOCOL_VERSION: u32 = 2;
/// `hello` 里报的路径,将来 LSP 的 `file://` URI 才有对应物。
pub const SOURCE_FILE: &str = "demo/src/lib.rs";
/// 源码上限 1 MiB。正常一份 `demo/src/lib.rs` 是几 KB;设上限是为了
/// 一次心跳式的大 payload 不会让服务去分配几百 MB。
///
/// **按字节算**(`str::len()`),不是按字符数:UTF-8 下一个中文字符
/// 占 3 字节。前端若拿 JS 的 `source.length`(UTF-16 code unit)或
/// `chars().count()` 去比这个上限,中文源码的判定就会和后端对不上。
pub const MAX_SOURCE_BYTES: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Submit { source: String, target: Option<String> },
    Cancel {},
    /// 请求开一个 RA 会话;服务端就绪后回 `lsp_ready`。
    LspOpen {},
    /// 编辑器里的源码变了。服务端据此给 RA 发 `did_change`。
    LspChange { source: String },
    /// 补全是请求-应答,而 `ServerMsg` 是单向广播流 —— 没有这条消息,
    /// 服务端就没有任何入口知道「用户要补全了」,`ServerMsg::LspCompletion`
    /// 永远发不出来。`request_id` 由前端产生,服务端原样用作 JSON-RPC 的
    /// `id` 并原样回显。
    LspCompletion { request_id: u32, line: u32, column: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Hello {
        protocol: u32,
        image_commit: Option<String>,
        file: String,
        max_jobs: usize,
        max_queue: usize,
        targets: Vec<String>,
        /// 旧后端没有 LSP,前端看到 `false` 就退回今天的行为。
        lsp_supported: bool,
    },
    Queued {
        position: usize,
        queue: usize,
        max_jobs: usize,
    },
    Started {
        task: String,
        image_commit: String,
    },
    Log {
        task: String,
        stream: Stream,
        text: String,
    },
    Finished {
        task: String,
        ok: bool,
        bytes: u64,
        download: String,
    },
    /// 客户端点了「取消」,服务端已经把那一单 abort 掉了。
    ///
    /// **这条不能省**:取消在服务端就是 `abort`,而 abort 不会产生任何
    /// 消息。不发这一条,前端会一直停在「编译中…」,取消按钮也一直亮着
    /// —— 它只在 `finished`/`failed`/`error` 时才收摊。
    Cancelled {
        task: String,
    },
    Failed {
        task: String,
        exit_code: Option<i32>,
        message: String,
    },
    Error {
        code: ErrorCode,
        message: String,
    },
    /// RA 会话就绪。`file` 是 `file://` URI 对应的那个路径。
    LspReady { file: String },
    /// RA 的 semanticTokens delta 数组,原样透传。**不在服务端拆** ——
    /// 拆成 (start, len, type, modifiers) 是前端高亮扩展的活。
    LspTokens { data: Vec<u32> },
    LspDiagnostic {
        line: u32,
        column: u32,
        end_line: u32,
        end_column: u32,
        severity: u8,
        message: String,
    },
    LspCompletion { request_id: u32, items: Vec<CompletionItem> },
}

/// 补全列表的一项。`insertText` 先不做 —— RA 默认给的与 `label` 一致。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<u32>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotReady,
    IpBusy,
    QueueFull,
    BadRequest,
    Internal,
}

impl ErrorCode {
    /// NAT 后面多人共用一个出口 IP 会被误伤 —— 认了,但要把话说清楚。
    pub fn ip_busy_message() -> &'static str {
        "请求过多，请稍后重试"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(v: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let s = serde_json::to_string(v).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn submit_round_trip_keeps_target_optional() {
        let m = ClientMsg::Submit { source: "fn main() {}".into(), target: None };
        assert_eq!(round_trip(&m), m);
        // 线格式:`target` 必须**存在且为 null**,不是被省略 —— round-trip
        // 对 "null" 和 "省略字段" 两种形状都成立,钉不住真正发到线上的样子。
        // Task 16 的 JS 照这个形状读,`null` 与 `undefined` 在那边不一样。
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({ "type": "submit", "source": "fn main() {}", "target": null })
        );

        let m = ClientMsg::Submit { source: "x".into(), target: Some("beyond-server".into()) };
        assert_eq!(round_trip(&m), m);
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({ "type": "submit", "source": "x", "target": "beyond-server" })
        );
    }

    #[test]
    fn cancel_round_trip() {
        assert_eq!(round_trip(&ClientMsg::Cancel {}), ClientMsg::Cancel {});
        assert_eq!(
            serde_json::to_value(&ClientMsg::Cancel {}).unwrap(),
            serde_json::json!({ "type": "cancel" })
        );
    }

    #[test]
    fn hello_json_shape() {
        let m = ServerMsg::Hello {
            protocol: PROTOCOL_VERSION,
            image_commit: Some("abc1234".into()),
            file: SOURCE_FILE.into(),
            max_jobs: 8,
            max_queue: 64,
            targets: vec!["beyond-server".into()],
            lsp_supported: false,
        };
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({
                "type": "hello",
                "protocol": 2,
                "image_commit": "abc1234",
                "file": "demo/src/lib.rs",
                "max_jobs": 8,
                "max_queue": 64,
                "targets": ["beyond-server"],
                "lsp_supported": false,
            })
        );
    }

    #[test]
    fn queued_json_shape() {
        let m = ServerMsg::Queued { position: 2, queue: 5, max_jobs: 8 };
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({ "type": "queued", "position": 2, "queue": 5, "max_jobs": 8 })
        );
    }

    #[test]
    fn log_carries_stream() {
        let m = ServerMsg::Log {
            task: "t1".into(),
            stream: Stream::Stderr,
            text: "error[E0308]\n".into(),
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["type"], "log");
        assert_eq!(v["stream"], "stderr");
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn finished_and_failed_round_trip() {
        let m = ServerMsg::Finished {
            task: "t1".into(),
            ok: true,
            bytes: 15860,
            download: "/artifact/t1".into(),
        };
        assert_eq!(round_trip(&m), m);
        // `bytes` 必须是 **JSON 数字**(不是字符串),`ok` 是 bool ——
        // 前端要拿 `bytes` 显示体积、拿 `ok` 判断成功,类型错了就静默出错。
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({
                "type": "finished",
                "task": "t1",
                "ok": true,
                "bytes": 15860,
                "download": "/artifact/t1",
            })
        );
        assert!(serde_json::to_value(&m).unwrap()["bytes"].is_number());

        let m = ServerMsg::Failed {
            task: "t1".into(),
            exit_code: Some(101),
            message: "编译失败".into(),
        };
        assert_eq!(round_trip(&m), m);
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({
                "type": "failed",
                "task": "t1",
                "exit_code": 101,
                "message": "编译失败",
            })
        );

        // `exit_code` 为 `None` 时线上就是 `null`,不是被省略。
        let m = ServerMsg::Failed {
            task: "t1".into(),
            exit_code: None,
            message: "容器没起来".into(),
        };
        assert_eq!(round_trip(&m), m);
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({
                "type": "failed",
                "task": "t1",
                "exit_code": null,
                "message": "容器没起来",
            })
        );
    }

    #[test]
    fn cancelled_json_shape() {
        let m = ServerMsg::Cancelled { task: "t1".into() };
        assert_eq!(round_trip(&m), m);
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({ "type": "cancelled", "task": "t1" })
        );
    }

    #[test]
    fn started_json_shape() {
        let m = ServerMsg::Started {
            task: "t1".into(),
            image_commit: "abc1234".into(),
        };
        assert_eq!(round_trip(&m), m);
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({ "type": "started", "task": "t1", "image_commit": "abc1234" })
        );
    }

    #[test]
    fn error_json_shape_embeds_code() {
        let m = ServerMsg::Error {
            code: ErrorCode::IpBusy,
            message: ErrorCode::ip_busy_message().into(),
        };
        assert_eq!(round_trip(&m), m);
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            serde_json::json!({ "type": "error", "code": "ip_busy", "message": "请求过多，请稍后重试" })
        );
    }

    #[test]
    fn error_codes_serialize_snake_case() {
        for (code, s) in [
            (ErrorCode::NotReady, "not_ready"),
            (ErrorCode::IpBusy, "ip_busy"),
            (ErrorCode::QueueFull, "queue_full"),
            (ErrorCode::BadRequest, "bad_request"),
            (ErrorCode::Internal, "internal"),
        ] {
            assert_eq!(serde_json::to_value(code).unwrap(), serde_json::json!(s));
        }
    }

    #[test]
    fn ip_busy_message_is_exact() {
        assert_eq!(ErrorCode::ip_busy_message(), "请求过多，请稍后重试");
    }

    #[test]
    fn unknown_type_is_rejected_not_ignored() {
        assert!(serde_json::from_str::<ClientMsg>(r#"{"type":"nope"}"#).is_err());
    }

    #[test]
    fn lsp_open_roundtrips() {
        let m = ClientMsg::LspOpen {};
        let j = serde_json::to_string(&m).unwrap();
        assert_eq!(j, r#"{"type":"lsp_open"}"#);
        assert_eq!(serde_json::from_str::<ClientMsg>(&j).unwrap(), m);
    }

    #[test]
    fn lsp_completion_reports_the_cursor() {
        let m = ClientMsg::LspCompletion { request_id: 7, line: 3, column: 12 };
        let j = serde_json::to_string(&m).unwrap();
        assert_eq!(j, r#"{"type":"lsp_completion","request_id":7,"line":3,"column":12}"#);
        assert_eq!(serde_json::from_str::<ClientMsg>(&j).unwrap(), m);
    }

    #[test]
    fn lsp_tokens_is_a_raw_u32_array() {
        let m = ServerMsg::LspTokens { data: vec![0, 0, 3, 6, 0] };
        let j = serde_json::to_string(&m).unwrap();
        assert!(j.contains(r#""data":[0,0,3,6,0]"#), "{j}");
    }

    #[test]
    fn protocol_version_is_2() {
        assert_eq!(PROTOCOL_VERSION, 2);
    }
}