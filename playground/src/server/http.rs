//! HTTP 面:健康检查、状态、样例源码、产物下载、重建。
//!
//! CORS 只对浏览器生效;WebSocket 那侧不吃 CORS,得自己查 Origin(见 ws.rs)。
//! 这一层不做业务判断 —— 所有"能不能跑"都在 `queue` 和 `bake` 里。

use std::{
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    bake::BakeRequest,
    config::Config,
    image::ImageRegistry,
    job::JobStore,
    queue::Queue,
    repo::Repo,
    server::webhook::{is_push_to_main, verify_signature},
};

pub struct AppState {
    pub cfg: Arc<Config>,
    pub queue: Arc<Queue>,
    pub baker: Arc<crate::bake::Baker>,
    pub jobs: Arc<JobStore>,
    pub repo: Arc<Repo>,
    pub registry: Arc<ImageRegistry>,
    /// 做成 trait object 是这一层全部可测性的来源:HTTP 和 WS 的测试
    /// 换成 `testing::FakeRunner`,不需要 docker。
    pub runner: Arc<dyn crate::runner::JobRunner>,
    pub started: Instant,
    /// RA 运行时。`None` = `PLAYGROUND_LSP_DISABLED` 或被测试关掉 ——
    /// `hello` 据此报 `lsp_supported: false`,前端退回纯编译界面。
    pub lsp: Option<Arc<crate::lsp::service::LspService>>,
}

/// `/status` 的响应体。字段是给前端和运维看的,名字直接抄过去的
/// 就是这些 —— 改字段名等于改前端。
#[derive(Debug, Clone, Serialize)]
pub struct StatusBody {
    pub ready: bool,
    pub serving: Option<String>,
    pub serving_tag: Option<String>,
    pub baking: Option<crate::bake::BakingStatus>,
    pub running: usize,
    pub waiting: usize,
    pub max_jobs: usize,
    pub head: Option<String>,
    pub dirty: Option<bool>,
    pub uptime_secs: u64,
    /// 最近一次 webhook 触发时 `git pull` 失败的原因;`None` = 没失败(或没触发)。
    pub pull_failure: Option<String>,
}

/// 用户源码在仓库里的路径。样例和任务用的是同一个文件 —— 用户点
/// "载入样例"拿到的就是仓库当前那份 `demo/src/lib.rs`。
pub fn sample_path(cfg: &Config) -> PathBuf {
    cfg.repo_dir.join(crate::protocol::SOURCE_FILE)
}

/// 浏览器解析 `.gia` 用的 schema。跟着仓库工作区走，不 embed ——
/// 与 `/sample` 同一个理由：部署的就是当前仓库，进程启动时冻结反而会漂。
pub fn proto_path(cfg: &Config) -> PathBuf {
    cfg.repo_dir.join("core/proto/asset.proto")
}

/// `/sample` 上用两个响应头捎回「样例出自哪个 commit / 镜像出自哪个
/// commit」。不用 JSON 包一层是因为正文要原样进编辑器 —— 包了的话前端
/// 每次都得剥一层。名字定成常量,`router` 里的 expose 列表和这里必须
/// 是同一份,写死字符串迟早对不上。
pub const HEADER_HEAD: &str = "x-playground-head";
pub const HEADER_BAKED: &str = "x-playground-baked";

pub fn router(state: Arc<AppState>) -> Router {
    let origins: Vec<HeaderValue> = state
        .cfg
        .allowed_origins
        .iter()
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect();
    let cors = if origins.is_empty() {
        // 显式配成空 = 谁都不许。比"不小心开了 Any"安全。
        CorsLayer::new()
    } else if state.cfg.allowed_origins.iter().any(|o| o == "*") {
        // `PLAYGROUND_ALLOWED_ORIGINS=*` —— 任意站点可用。WS 那侧
        // `origin_allowed` 认同一个 `*`,两边不会一松一紧。
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
            .expose_headers([
                HeaderName::from_static(HEADER_HEAD),
                HeaderName::from_static(HEADER_BAKED),
            ])
    } else {
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any)
            // 跨域下自定义响应头默认**读不到**,/sample 的 head / baked
            // 必须在这里列出来,否则前端那句"样例比镜像新"的提示是死的。
            .expose_headers([
                HeaderName::from_static(HEADER_HEAD),
                HeaderName::from_static(HEADER_BAKED),
            ])
    };

    Router::new()
        .route("/healthz", get(healthz))
        .route("/status", get(status))
        .route("/sample", get(sample))
        .route("/proto/asset.proto", get(asset_proto))
        .route("/admin/rebuild", post(admin_rebuild))
        .route("/ws", get(crate::server::ws::ws_upgrade))
        .layer(cors)
        .with_state(state)
}

async fn healthz(State(st): State<Arc<AppState>>) -> Response {
    let body = serde_json::json!({
        "ready": st.baker.ready(),
        "serving": st.baker.status().serving,
        "tail": st.baker.tail(),
    });
    let code = if st.baker.ready() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(body)).into_response()
}

async fn status(State(st): State<Arc<AppState>>) -> Json<StatusBody> {
    let bake = st.baker.status();
    let snap = st.queue.snapshot();
    // git 探测失败不是错误:head/dirty 是给运维看的锦上添花。
    let head = st.repo.head().await.ok();
    let dirty = st.repo.is_dirty().await.ok();
    Json(StatusBody {
        ready: bake.serving.is_some(),
        serving: bake.serving,
        serving_tag: bake.serving_tag,
        baking: bake.baking,
        running: snap.running,
        waiting: snap.waiting,
        max_jobs: snap.max_jobs,
        head,
        dirty,
        uptime_secs: st.started.elapsed().as_secs(),
        pull_failure: bake.pull_failure,
    })
}

async fn sample(State(st): State<Arc<AppState>>) -> Response {
    let Ok(bytes) = tokio::fs::read(sample_path(&st.cfg)).await else {
        return (StatusCode::NOT_FOUND, "样例源码不在仓库里").into_response();
    };
    let mut res = ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], bytes).into_response();
    let h = res.headers_mut();
    // 两个头都是**有才发**。工作区不是 git 仓库时 head 探测会失败,那时
    // 发一个空串只会让前端以为"HEAD 是空的",不如不发。
    if let Ok(head) = st.repo.head().await {
        if let Ok(v) = HeaderValue::from_str(&head) {
            h.insert(HeaderName::from_static(HEADER_HEAD), v);
        }
    }
    if let Some(sha) = st.baker.status().serving {
        if let Ok(v) = HeaderValue::from_str(&sha) {
            h.insert(HeaderName::from_static(HEADER_BAKED), v);
        }
    }
    res
}

async fn asset_proto(State(st): State<Arc<AppState>>) -> Response {
    let Ok(bytes) = tokio::fs::read(proto_path(&st.cfg)).await else {
        return (StatusCode::NOT_FOUND, "asset.proto 不在仓库里").into_response();
    };
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], bytes).into_response()
}

/// webhook 路径上的 `git pull`。返回 `Some(原因)` 表示 pull 没成功(但烘焙
/// 照常,烘的是当前工作区),`None` 表示没失败 —— 包括 `git_ref` 钉成 sha 时
/// 整条跳过(定点部署不该被 pull 改掉 ref)。
async fn webhook_pull(repo: &Repo, cfg: &Config) -> Option<String> {
    if crate::repo::is_sha_like(&cfg.git_ref) {
        return None;
    }
    match repo.pull_ff_only(&cfg.remote, &cfg.git_ref).await {
        Ok(crate::repo::Pull::Skipped(why)) => Some(why),
        Ok(_) => None,
        Err(e) => Some(e.to_string()),
    }
}

/// 两条进得来:GitHub webhook(HMAC)和运维手工触发(Bearer token)。
/// 两个都没配 = 403,不做"没配就放行"这种事。
async fn admin_rebuild(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let sig = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok());
    let event = headers.get("x-github-event").and_then(|v| v.to_str().ok());

    let authed_by_webhook = match (&st.cfg.webhook_secret, sig) {
        (Some(secret), Some(s)) => {
            if !verify_signature(secret, &body, Some(s)) {
                return (StatusCode::FORBIDDEN, "签名不对").into_response();
            }
            true
        }
        _ => false,
    };

    if !authed_by_webhook {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        let ok = match (&st.cfg.admin_token, token) {
            (Some(want), Some(got)) => want == got,
            _ => false,
        };
        if !ok {
            return (StatusCode::FORBIDDEN, "没带凭证或者凭证不对").into_response();
        }
    }

    // webhook 这条路只认 main 的 push。不是 push、不是 main,一律 202
    // 说"跳过" —— 回 4xx 会让 GitHub 反复重投,而它本来就不该触发。
    if authed_by_webhook && !is_push_to_main(event, &body) {
        return (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({ "accepted": false, "reason": "忽略:不是 main 的 push" })),
        )
            .into_response();
    }

    // 只有 webhook 这条路才 pull —— 手工 /admin/rebuild 烘的是工作区当前
    // 状态,不该顺手 pull。pull 失败/分叉不阻断,原因记进 /status 的
    // `pull_failure`(下次 pull 或成功时覆盖),烘焙照常继续。
    if authed_by_webhook {
        st.baker.set_pull_failure(webhook_pull(&st.repo, &st.cfg).await);
    }

    st.baker.request(BakeRequest {
        sha: None,
        note: Some(if authed_by_webhook { "webhook".into() } else { "手工触发".into() }),
    });
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "accepted": true })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;
    use std::path::Path;
    use crate::{
        bake::Baker,
        config::resolve,
        gc::DiskGate,
        image::ImageRegistry,
        job::JobStore,
        queue::Queue,
        repo::Repo,
        testing::{FakeBuilder, OkGate},
    };

    fn cfg_of(repo: &Path) -> Config {
        let mut c = resolve(&|_| None, &repo.join("playground")).unwrap();
        c.repo_dir = repo.to_path_buf();
        c.data_dir = repo.join("data");
        c
    }

    /// 一个"已经烘好 `sha`"的 baker。`sha` 传空 vec 的 builder 就是一个
    /// 从没成功过的 baker —— 这条路径 `/healthz` 要报 503。
    async fn baked(results: Vec<Result<crate::bake::BuiltImage, String>>) -> Arc<Baker> {
        let baker = Baker::new(
            FakeBuilder::new(results),
            Arc::new(OkGate) as Arc<dyn DiskGate>,
        );
        baker.request(BakeRequest::default());
        baker.step().await;
        baker
    }

    fn baked_one(sha: &str) -> crate::bake::BuiltImage {
        crate::bake::BuiltImage {
            sha: sha.into(),
            tag: format!("rust2genshin-play:{sha}"),
        }
    }

    /// 测试里的 AppState 装配点只此一处 —— 三个测试模块(这里、ws、将来的)
    /// 都挂在这上面,加一个字段就只用改这里。
    fn state(repo: &Path, baker: Arc<Baker>) -> Arc<AppState> {
        let cfg = Arc::new(cfg_of(repo));
        Arc::new(AppState {
            queue: Queue::new(cfg.max_jobs, cfg.max_queue),
            baker,
            jobs: Arc::new(JobStore::new(cfg.data_dir.join("tasks"))),
            repo: Arc::new(Repo::new(cfg.repo_dir.clone())),
            registry: ImageRegistry::new(),
            runner: crate::testing::FakeRunner::new(crate::testing::Behavior::Ok {
                lines: vec![],
                bytes: 1,
            }),
            started: Instant::now(),
            cfg,
            lsp: None,
        })
    }

    /// 白名单为 `*` 的 state。`PLAYGROUND_ALLOWED_ORIGINS=*` 时用的就是它。
    async fn any_origin_state(repo: &Path) -> Arc<AppState> {
        let st = ready_state(repo).await;
        let mut c = (*st.cfg).clone();
        c.allowed_origins = vec!["*".into()];
        Arc::new(AppState {
            queue: Queue::new(c.max_jobs, c.max_queue),
            baker: st.baker.clone(),
            jobs: st.jobs.clone(),
            repo: st.repo.clone(),
            registry: ImageRegistry::new(),
            runner: st.runner.clone(),
            started: st.started,
            cfg: Arc::new(c),
            lsp: st.lsp.clone(),
        })
    }

    /// "已经烘好 aaa1111"的状态,HTTP 面的测试几乎都用它。
    async fn ready_state(repo: &Path) -> Arc<AppState> {
        state(repo, baked(vec![Ok(baked_one("aaa1111"))]).await)
    }

    async fn body_json(res: Response) -> serde_json::Value {
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn get(state: Arc<AppState>, uri: &str) -> Response {
        router(state)
            .oneshot(axum::http::Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    /// 带 `Origin` 的 GET。CORS 中间件只在有 Origin 时才加回响应头,
    /// 查 `access-control-expose-headers` 必须走这个。
    async fn get_with_origin(state: Arc<AppState>, uri: &str, origin: &str) -> Response {
        router(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri(uri)
                    .header(header::ORIGIN, origin)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn healthz_is_ok_when_ready() {
        let d = tempfile::tempdir().unwrap();
        let res = get(ready_state(d.path()).await, "/healthz").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_json(res).await["ready"], true);
    }

    #[tokio::test]
    async fn healthz_is_503_before_the_first_bake() {
        // 从未烘成功过时进程还活着,但**不算健康** —— 带着日志尾巴报 503,
        // 前端能把原因显示出来,比进程直接退出强。
        let d = tempfile::tempdir().unwrap();
        let res = get(state(d.path(), baked(vec![]).await), "/healthz").await;
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body_json(res).await["ready"], false);
    }

    #[tokio::test]
    async fn status_reports_the_serving_image_and_queue() {
        let d = tempfile::tempdir().unwrap();
        let res = get(ready_state(d.path()).await, "/status").await;
        assert_eq!(res.status(), StatusCode::OK);
        let j = body_json(res).await;
        assert_eq!(j["serving"], "aaa1111");
        assert_eq!(j["serving_tag"], "rust2genshin-play:aaa1111");
        assert_eq!(j["max_jobs"], 8);
        assert_eq!(j["running"], 0);
        assert_eq!(j["waiting"], 0);
    }

    #[tokio::test]
    async fn status_survives_a_directory_that_is_not_a_git_repo() {
        // /status 不该因为 `git rev-parse` 失败而 500:运维面板在仓库被搬走、
        // git 没装好的时候也得打得开。
        let d = tempfile::tempdir().unwrap();
        let res = get(ready_state(d.path()).await, "/status").await;
        let j = body_json(res).await;
        assert_eq!(j["head"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn sample_returns_the_repo_file() {
        let d = tempfile::tempdir().unwrap();
        let lib = d.path().join(crate::protocol::SOURCE_FILE);
        std::fs::create_dir_all(lib.parent().unwrap()).unwrap();
        std::fs::write(&lib, "fn main() {}").unwrap();
        let res = get(ready_state(d.path()).await, "/sample").await;
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"fn main() {}");
    }

    #[tokio::test]
    async fn sample_is_404_when_the_file_is_gone() {
        let d = tempfile::tempdir().unwrap();
        let res = get(ready_state(d.path()).await, "/sample").await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn proto_is_served_from_the_repo() {
        let d = tempfile::tempdir().unwrap();
        let proto = d.path().join("core/proto/asset.proto");
        std::fs::create_dir_all(proto.parent().unwrap()).unwrap();
        let body = b"package rust2genshin;\nmessage AssetBundleData {}\n";
        std::fs::write(&proto, body).unwrap();
        let res = get(ready_state(d.path()).await, "/proto/asset.proto").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers()[header::CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], body);
    }

    #[tokio::test]
    async fn proto_is_404_when_missing() {
        let d = tempfile::tempdir().unwrap();
        let res = get(ready_state(d.path()).await, "/proto/asset.proto").await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn sample_carries_the_baked_sha_and_exposes_it_to_the_browser() {
        // 样例来自工作区、镜像来自某个 sha,两者可能不是同一个 commit
        // (典型:给 lib/ 加了 API 还没重烘焙)。前端要能看出这一点并提示,
        // 所以 `baked` 得随 /sample 回去 —— 光靠 /status 不够,那是两次
        // 请求,中间会变。
        let d = tempfile::tempdir().unwrap();
        let lib = d.path().join(crate::protocol::SOURCE_FILE);
        std::fs::create_dir_all(lib.parent().unwrap()).unwrap();
        std::fs::write(&lib, "fn main() {}").unwrap();
        let st = ready_state(d.path()).await;
        let res = get_with_origin(st, "/sample", "https://mzverse.github.io").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get("x-playground-baked").unwrap(),
            "aaa1111",
            "serving 的 sha 要跟着样例一起回去"
        );
        // 跨域下浏览器默认读不到自定义响应头,除非服务端显式 expose。
        // 忘了这一条,前端那句"样例比镜像新"的提示永远不会触发。
        let expose = res.headers()[header::ACCESS_CONTROL_EXPOSE_HEADERS]
            .to_str()
            .unwrap()
            .to_string();
        assert!(expose.contains("x-playground-baked"), "expose 里有:{expose}");
        assert!(expose.contains("x-playground-head"), "expose 里有:{expose}");
    }

    #[tokio::test]
    async fn a_star_allowlist_reflects_any_origin_back() {
        // 回的是字面量 `*`,不是回显调用方。`CorsLayer::allow_origin(Any)`
        // 就是这个行为,这里不用改 —— 因为整条链不开凭证(没有
        // `allow_credentials`),无凭证请求下 `*` 是合法的,浏览器照认。
        // 一旦哪天要加 cookie,这里必须改成回显 origin,否则浏览器直接拒绝。
        let d = tempfile::tempdir().unwrap();
        let res =
            get_with_origin(any_origin_state(d.path()).await, "/healthz", "https://any.example").await;
        assert_eq!(res.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    }

    #[tokio::test]
    async fn the_default_allowlist_does_not_reflect_a_foreign_origin() {
        let d = tempfile::tempdir().unwrap();
        let res = get_with_origin(ready_state(d.path()).await, "/healthz", "https://evil.example").await;
        assert!(
            res.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none(),
            "默认白名单不该给陌生站点回 ACAO",
        );
    }

    #[tokio::test]
    async fn rebuild_needs_a_token() {
        let d = tempfile::tempdir().unwrap();
        let res = router(ready_state(d.path()).await)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/admin/rebuild")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "没配 token 就该整条关掉");
    }

    #[tokio::test]
    async fn rebuild_is_403_when_neither_secret_nor_token_is_set() {
        // 行为固化:`webhook_secret` 与 `admin_token` 都为 `None` 时,两条进
        // 来的路都是关的 —— 不配凭证等于端点整条关掉,不做"没配就放行"。
        // (config 已把空串/全空白归一成 `None`,所以这条是真正的守门。)
        let d = tempfile::tempdir().unwrap();
        let st = ready_state(d.path()).await;
        assert!(st.cfg.webhook_secret.is_none());
        assert!(st.cfg.admin_token.is_none());
        let res = router(st)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/admin/rebuild")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    // ── webhook 路径的 pull ───────────────────────────────────────────

    /// 和 `verify_signature` 用同一套算法签一个 body,好让 webhook 请求过鉴权。
    fn sign(secret: &str, body: &[u8]) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
    }

    /// 带 webhook 配置的 AppState,`builder` 交出来好断言"烘焙确实被触发"。
    /// `repo` 必须是真 git 仓库(webhook 这条路径要 pull)。
    fn webhook_state(repo: &Path, secret: &str, git_ref: &str) -> (Arc<AppState>, Arc<FakeBuilder>) {
        let mut cfg = cfg_of(repo);
        cfg.webhook_secret = Some(secret.to_string());
        cfg.git_ref = git_ref.to_string();
        cfg.remote = "origin".to_string();
        let cfg = Arc::new(cfg);
        let builder = FakeBuilder::new(vec![Ok(baked_one("bbb2222"))]);
        let image_builder: Arc<dyn crate::bake::ImageBuilder> = builder.clone();
        let baker = Baker::new(image_builder, Arc::new(OkGate) as Arc<dyn DiskGate>);
        let st = Arc::new(AppState {
            queue: Queue::new(cfg.max_jobs, cfg.max_queue),
            baker,
            jobs: Arc::new(JobStore::new(cfg.data_dir.join("tasks"))),
            repo: Arc::new(Repo::new(cfg.repo_dir.clone())),
            registry: ImageRegistry::new(),
            runner: crate::testing::FakeRunner::new(crate::testing::Behavior::Ok {
                lines: vec![],
                bytes: 1,
            }),
            started: Instant::now(),
            cfg,
            lsp: None,
        });
        (st, builder)
    }

    async fn post_webhook(state: Arc<AppState>, secret: &str) -> Response {
        let body = br#"{"ref":"refs/heads/main"}"#;
        router(state)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/admin/rebuild")
                    .header("x-hub-signature-256", sign(secret, body))
                    .header("x-github-event", "push")
                    .body(Body::from(body.to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn webhook_pull_skips_a_pinned_sha() {
        // git_ref 是 sha → 整条跳过,不去 pull。用非 git 目录:若真去 pull,
        // git 会失败并返回 Some(原因),这里返回 None 才证明 is_sha_like 短路生效。
        let d = tempfile::tempdir().unwrap();
        let repo = Repo::new(d.path().to_path_buf());
        let mut cfg = cfg_of(d.path());
        cfg.git_ref = "0123456789abcdef".to_string();
        assert_eq!(webhook_pull(&repo, &cfg).await, None);
    }

    #[tokio::test]
    async fn a_failed_pull_still_bakes_and_records_the_reason() {
        if !crate::testing::git_available() {
            return;
        }
        let secret = "s3cret";
        let (remote, _a) = crate::testing::git_remote_with_commit("a");
        let local = crate::testing::git_local_clone(remote.path());
        // 分叉:本地 + 远端各 commit → `pull --ff-only` 必然 Skipped。
        std::fs::write(local.path().join("local.txt"), "x").unwrap();
        crate::testing::git(local.path(), &["add", "."]);
        crate::testing::git(local.path(), &["commit", "-m", "local"]);
        std::fs::write(remote.path().join("f.txt"), "b").unwrap();
        crate::testing::git(remote.path(), &["add", "."]);
        crate::testing::git(remote.path(), &["commit", "-m", "remote"]);

        let (st, builder) = webhook_state(local.path(), secret, "main");
        let res = post_webhook(Arc::clone(&st), secret).await;
        assert_eq!(res.status(), StatusCode::ACCEPTED);

        // pull 失败不阻断烘焙,原因要能从 /status 查到。
        assert!(st.baker.status().pull_failure.is_some(), "pull 失败原因要能查到");
        assert!(st.baker.step().await, "pull 失败仍该触发烘焙");
        assert_eq!(builder.calls().len(), 1, "烘焙确实被触发了");
    }

    #[tokio::test]
    async fn a_successful_pull_advances_to_the_new_head() {
        if !crate::testing::git_available() {
            return;
        }
        let secret = "s3cret";
        let (remote, _a) = crate::testing::git_remote_with_commit("a");
        let local = crate::testing::git_local_clone(remote.path());
        let repo = Repo::new(local.path().to_path_buf());
        let before = repo.head().await.unwrap();
        // 远端前进一个 commit。
        std::fs::write(remote.path().join("f.txt"), "b").unwrap();
        crate::testing::git(remote.path(), &["add", "."]);
        crate::testing::git(remote.path(), &["commit", "-m", "b"]);
        let b_sha = crate::testing::git(remote.path(), &["rev-parse", "HEAD"]);

        let (st, _builder) = webhook_state(local.path(), secret, "main");
        let res = post_webhook(Arc::clone(&st), secret).await;
        assert_eq!(res.status(), StatusCode::ACCEPTED);

        // pull 成功:pull_failure 无,HEAD 前进到远端新 sha。
        assert_eq!(st.baker.status().pull_failure, None);
        let after = st.repo.head().await.unwrap();
        assert_eq!(after, b_sha, "pull 之后 HEAD 要前进到远端");
        assert_ne!(after, before);
    }
}
