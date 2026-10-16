//! 真正去 `docker build` 的 `ImageBuilder`。
//!
//! 唯一需要小心的是 tag 的算法要和镜像里的 label 对齐:tag 带 `-dirty`
//! 后缀、label 只带 sha,清理和"本地有没有现成的"两处都靠这两个值互相找。

use std::path::PathBuf;

use crate::{
    bake::{BuiltImage, ImageBuilder},
    image::{tag, LABEL_COMMIT, LABEL_PLAY},
    repo::Repo,
};

/// `docker build` 的全部参数。`--platform` 单列出来:目标机是 amd64 而开发机
/// 是 arm 时,不钉住会烘出一个跑不起来的镜像,而且症状很难认。
///
/// `--network=host` 是被部署机逼出来的:那台机的 dockerd 配了
/// `"iptables": false`,容器默认没有出网,layered RUN 里的 `apt-get` /
/// `curl rustup` / `cargo fetch` 全会挂在 DNS 解析上。走宿主网络才下得动。
/// 这是**烘焙独有**的放宽 —— 任务容器仍然是 `--network none`。
///
/// **不要**加 `--progress=plain`:它是 BuildKit 的旗标,而这台机只有
/// docker 19.03 的 legacy builder(见 `build()` 里钉的 `DOCKER_BUILDKIT=0`)。
pub fn build_args(
    platform: &str,
    image_tag: &str,
    commit: &str,
    context: &std::path::Path,
) -> Vec<String> {
    vec![
        "build".into(),
        format!("--platform={platform}"),
        "--network=host".into(),
        "--file".into(),
        // **必须是绝对路径**:`--file` 按 docker 客户端的 CWD 解析,不是按
        // context 解析。context 用的是绝对路径(`repo_dir`),而服务在容器里
        // 的 CWD 是 `/` 之类的地方,相对 `--file` 会直接报
        // `unable to evaluate symlinks in Dockerfile path: lstat /playground`。
        // 过去靠「碰巧在仓库根起服务」才没炸,那是部署姿势在替代码兜底。
        context.join("playground/Dockerfile").display().to_string(),
        format!("--tag={image_tag}"),
        format!("--label={LABEL_PLAY}=1"),
        format!("--label={LABEL_COMMIT}={commit}"),
        context.display().to_string(),
    ]
}

pub struct DockerImageBuilder {
    repo_dir: PathBuf,
    image: String,
    platform: String,
    repo: Repo,
}

impl DockerImageBuilder {
    /// `build_cache_max` 不在这里 —— 它是 gc 那边 `docker builder prune`
    /// 的参数(见 `gc_loop::sweep_images`),builder 自己不管预算。
    pub fn new(repo_dir: PathBuf, image: String) -> Self {
        Self {
            repo: Repo::new(repo_dir.clone()),
            repo_dir,
            image,
            platform: "linux/amd64".into(),
        }
    }
}

#[async_trait::async_trait]
impl ImageBuilder for DockerImageBuilder {
    async fn build(
        &self,
        sha: Option<&str>,
        log: &(dyn for<'a> Fn(&'a str) + Send + Sync),
    ) -> Result<BuiltImage, String> {
        // 镜像内容来自工作区目录,所以"烘哪个 commit"= 当前 HEAD。
        // 传进来的 sha 只用来对账,不切分支 —— 服务只服务 main。
        let head = self
            .repo
            .head()
            .await
            .map_err(|e| format!("读 HEAD 失败:{e}"))?;
        if let Some(want) = sha
            && !head.starts_with(want)
        {
            log(&format!("注意:工作区 HEAD 是 {head},不是 {want},按工作区烘"));
        }
        let dirty = self.repo.is_dirty().await.unwrap_or(true);
        let built_tag = tag(&self.image, &head, dirty);

        let mut child = tokio::process::Command::new("docker")
            // 钉死 legacy builder。部署机是 docker 19.03 且没装 buildx,
            // 而 BuildKit 一旦被打开,输出格式就换了(没有 `Step k/n`),
            // 日志解析和"进度看哪一行"全得跟着变。显式关掉,让两台机器
            // (开发机没有 docker,部署机是老版本)的行为一致。
            .env("DOCKER_BUILDKIT", "0")
            .args(build_args(
                &self.platform,
                &built_tag,
                &head,
                self.repo_dir.as_path(),
            ))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("起 docker build 失败:{e}"))?;

        // legacy builder 的进度(`Step k/n : …`)走 stderr;stdout 基本是
        // 空的,但两边都读,免得管道写满把 docker 卡住。
        //
        // 三个 future 必须**一起** join:`read_lines` 是惰性的,单独 `wait()`
        // 期间不 poll 它们就等于没读,`Step k/n` 一多(编一百多个 crate 时
        // 轻松超过管道缓冲)docker 就写阻塞、`wait()` 永不返回 —— 死锁。
        let err_lines = read_lines(child.stderr.take().expect("piped"), log);
        let out_lines = read_lines(child.stdout.take().expect("piped"), log);
        let (_, _, waited) = tokio::join!(err_lines, out_lines, child.wait());
        let status = waited.map_err(|e| format!("等 docker build 失败:{e}"))?;

        if !status.success() {
            return Err(format!(
                "docker build 失败,退出码 {:?}。上面最后几行是原因。",
                status.code()
            ));
        }
        log(&format!("烘好了:{built_tag}"));
        Ok(BuiltImage {
            sha: head,
            tag: built_tag,
        })
    }
}

async fn read_lines<R>(r: R, log: &(dyn for<'a> Fn(&'a str) + Send + Sync))
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut lines = BufReader::new(r).lines();
    while let Ok(Some(l)) = lines.next_line().await {
        log(&l);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_pin_the_dockerfile_and_labels() {
        let a = build_args(
            "linux/amd64",
            "rust2genshin-play:abc1234",
            "abc1234",
            PathBuf::from("/repo").as_path(),
        );
        assert_eq!(a[0], "build");
        assert!(a.contains(&"--file".to_string()));
        assert!(
            a.contains(&PathBuf::from("/repo")
                .join("playground/Dockerfile")
                .display()
                .to_string()),
            "--file 必须是绝对路径:它按客户端 CWD 解析,不按 context:{a:?}"
        );
        assert!(a.contains(&"--tag=rust2genshin-play:abc1234".to_string()));
        assert!(a.contains(&format!("--label={LABEL_PLAY}=1")));
        assert!(a.contains(&format!("--label={LABEL_COMMIT}=abc1234")));
        // 部署机的 dockerd 是 `iptables: false`,容器默认没网 —— 烘焙那几层
        // RUN 全靠这条才下得动 rustup / apt / cargo 依赖。
        assert!(a.contains(&"--network=host".to_string()));
        // `--progress=plain` 是 BuildKit 的旗标:加了它,legacy builder
        // (docker 19.03,部署机上没有 buildx)会直接报未知旗标。
        assert!(
            !a.iter().any(|x| x.starts_with("--progress")),
            "legacy builder 不认 --progress:{a:?}"
        );
        assert_eq!(a.last().unwrap(), "/repo", "最后一个参数是 context");
    }

    #[test]
    fn labels_and_tags_agree_on_the_commit() {
        // 全限定写 `label_filter`:它只在测试里用,放文件头的 `use` 会让
        // 非测试构建吃一个 unused_imports。
        let t = tag("play", "abcdef1234567890", false);
        let f = crate::image::label_filter("play", Some("abcdef1234567890"));
        assert!(f.contains("abcdef1234567890"), "按 commit 找镜像要用全长 sha:{f}");
        assert!(t.starts_with("play:abcdef1"), "{t}");
    }

    #[test]
    fn dockerfile_puts_cargo_on_the_task_containers_path() {
        // 任务容器的命令是裸 `sh -c "cargo build …"`(见 `docker.rs`),
        // 非登录非交互 shell 不 source `$HOME/.cargo/env` —— cargo 在不在
        // PATH 上,全靠镜像里那行 `ENV PATH=`。
        let dockerfile = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Dockerfile"),
        )
        .expect("读 playground/Dockerfile");
        let lines: Vec<&str> = dockerfile.lines().collect();

        let env_path = lines
            .iter()
            .find(|l| l.starts_with("ENV PATH="))
            .unwrap_or_else(|| {
                panic!("Dockerfile 没有 `ENV PATH=` 行,任务容器 sh -c cargo 会以 127 失败")
            });
        assert!(
            env_path.contains("/home/builder/.cargo/bin"),
            "ENV PATH 必须含 rustup 装工具链的目录:{env_path}"
        );

        // `rustup show` 靠 CWD 找 `rust-toolchain.toml`,所以 WORKDIR 要在
        // 它那条 RUN 之前 —— 否则 CWD 是 `/`,看不到工具链文件。
        let workdir = lines
            .iter()
            .position(|l| l.starts_with("WORKDIR /opt/rust2genshin"))
            .expect("没有 `WORKDIR /opt/rust2genshin`");
        let rustup = lines
            .iter()
            .position(|l| l.contains("sh.rustup.rs"))
            .expect("没有 rustup 安装那条 RUN");
        assert!(
            workdir < rustup,
            "WORKDIR 落在 rustup RUN 之后(rustup show 的 CWD 是 /,看不到 rust-toolchain.toml):\
             workdir={workdir} rustup={rustup}"
        );
    }
}
