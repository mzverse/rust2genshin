//! docker CLI 的薄封装。
//!
//! 不引 `bollard`(docker API 的 Rust 客户端):它会带来一大堆类型,而 shell 出去
//! 的 argv 就是人能照着敲的那条命令,调 docker 问题时这个值钱。
//!
//! 代价是 stdout/stderr 的分帧要自己解:`docker run` 不带 `-t` 时它俩是
//! 8 字节头 + 载荷的 multiplexed stream(`header[0]` 1=stdout 2=stderr,
//! `header[4..8]` 是载荷长度,大端)。不用 `-t` 是因为 `-t` 会把 stderr 并进
//! stdout、并把行尾变成 `\r\n` —— 前端就拿不到 stream 区分了。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 容器输出流的种类。正常日志里只会出现 stdout(1) 和 stderr(2);
/// docker 自己保留的 stream id 3(dockerd 的内部错误流)不在其中,
/// 会走 `FrameError::BadStream`,按终止处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub stream: Stream,
    pub data: Vec<u8>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("未知的流字节 {0}(只认 1=stdout / 2=stderr)")]
    BadStream(u8),
    #[error("帧载荷 {0} 字节超出上限")]
    TooLarge(u32),
    /// 读容器输出时 IO 出错。存成 String 而不是 `io::Error`,是为了让
    /// 这个枚举能继续派生 `PartialEq` / `Eq`(测试里要 `assert_eq!`)——
    /// `io::Error` 两者都没有。
    #[error("读容器输出失败:{0}")]
    Io(String),
}

/// 单帧上限 4 MiB:正常编译输出远小于它,设上限是为了一个坏掉的
/// 头(比如没带 `--rm` 时 docker 打了别的东西)不会让服务去分配几个 GB。
const MAX_FRAME: u32 = 4 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂一块读到的字节,返回这块里已经完整解出的帧。
    /// 跨 read 边界断开的帧留在内部缓冲里,等下一次 push。
    ///
    /// **粘性错误:一旦 `push` 返回 `Err`,这个 decoder 就作废了**。
    /// `Err` 路径不会清空 `self.buf`,坏字节留在里面,再 push 一次
    /// 会在同一处原封不动地再失败一次 —— 这是有意的:头被破坏意味
    /// 着流后续不可信,继续喂就是死循环 `Err`,而静默丢弃后面的日志
    /// 又太轻率。调用方应当在第一次 `Err` 时停止喂数据,把这条日志流
    /// 当作终止。
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<Frame>, FrameError> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        loop {
            if self.buf.len() < 8 {
                break;
            }
            let stream = match self.buf[0] {
                1 => Stream::Stdout,
                2 => Stream::Stderr,
                other => return Err(FrameError::BadStream(other)),
            };
            let len = u32::from_be_bytes([self.buf[4], self.buf[5], self.buf[6], self.buf[7]]);
            if len > MAX_FRAME {
                return Err(FrameError::TooLarge(len));
            }
            if self.buf.len() < 8 + len as usize {
                break;
            }
            let data = self.buf[8..8 + len as usize].to_vec();
            self.buf.drain(..8 + len as usize);
            out.push(Frame { stream, data });
        }
        Ok(out)
    }
}

/// 跑一个任务容器要的全部参数。做成结构体而不是一堆函数参数,
/// 是因为任务容器那串加固参数一个都不能少 —— 显式列出比默认值安全。
#[derive(Debug, Clone)]
pub struct RunSpec {
    pub name: String,
    pub image: String,
    pub src_dir: PathBuf,
    pub out_dir: PathBuf,
    pub workdir: String,
    /// 目标平台:`beyond-server`(超限)或 `classic-server`(经典)。
    /// 两个 spec 的 triple 不同,cargo 在 `target/` 下按 triple 分目录,
    /// 所以路径里的这一段必须跟着它走。
    pub target: String,
    /// `None` = **不下** `--cpus`。有些宿主内核没挂 cgroup 的 cpu 控制器
    /// (整个系统跑在容器里、或者内核没编 `CFS_BANDWIDTH`),这时 docker 会直接
    /// 拒掉整个 `run`:`NanoCPUs can not be set, as your kernel does not
    /// support CPU cfs period/quota or the cgroup is not mounted`。留个
    /// 能关掉的开关,免得在那种机器上一个任务都跑不起来。
    pub cpus: Option<u32>,
    /// `None` = **不下** `--memory` / `--memory-swap`。cgroup v1 的 memory
    /// 控制器不支持 `kmem.limit_in_bytes` 时(常见于内核没编
    /// `CONFIG_MEMCG_KMEM` 的机器),runc 会在建容器时炸:
    /// `failed to write 1 to memory.kmem.limit_in_bytes: operation not
    /// supported`,`docker run` 整个失败,一个任务都跑不起来。
    pub memory: Option<String>,
    pub pids_limit: u32,
    pub user: String,
    /// 用户代码在仓库里的相对路径(`demo/src/lib.rs`),只写进日志。
    pub source_file: String,
}

/// 容器里跑的那条命令。`--offline` 是关键:依赖在烘焙时 fetch 齐了,
/// 容器又是 `--network none`,让"万一要联网"变成一个明确的错误而不是超时。
pub fn container_command(spec: &RunSpec) -> String {
    // `--target` 与 `cp` 的路径必须来自同一个变量:写死其中一处,选经典
    // 时就会去超限的目录里拷产物,cp 失败、任务判失败。
    let t = &spec.target;
    format!(
        "cargo build --release --offline -p rust2genshin-demo \
         --target ./genshin-unknown-{t}.json \
         -Zunstable-options -Zjson-target-spec -Zbuild-std=core \
         && cp target/genshin-unknown-{t}/release/rust2genshin_demo.gia /out/"
    )
}

/// RA 容器在镜像内的 workspace 根目录。源码由 `exec` 在容器内写进这个目录
/// 不挂 `demo/src`,所以 workdir 必须在镜像里已经存在 —— 否则 `docker run`
/// 会以 `/opt/ra/ws` 不存在为由失败。
pub const RA_WORKDIR: &str = "/opt/ra/ws";

/// 资源限额与加固旗标。任务容器与待命 RA 容器都吃同一份:docker CLI 对限额
/// 旗标的相对顺序不敏感(`-d` / `-v` / `--rm` 这些**是**敏感的,所以那些留在
/// `run_args` / `run_detached_args` 自己拼)。
///
/// 抽出来的目的:让 `run_detached_args` 复用这堆限额,不在 image 之后塞 `-d`。
fn limit_args(spec: &RunSpec) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    let pids = spec.pids_limit.to_string();
    for s in [
        "--network", "none",
        "--pids-limit", &pids,
        "--user", &spec.user,
        "--tmpfs", "/tmp",
    ] {
        a.push(s.into());
    }
    // `--memory-swap` 与 `--memory` 同进同出:单给 swap 而不给 memory 的话
    // docker 会拿 memory 的默认值当上限,那不是部署想表达的意思。
    if let Some(memory) = &spec.memory {
        a.push("--memory".into());
        a.push(memory.clone());
        a.push("--memory-swap".into());
        a.push(memory.clone());
    }
    // 限额是有还是没有**由 spec 说了算**,这里只是别把 `None` 拼成 "None"。
    if let Some(cpus) = spec.cpus {
        a.push("--cpus".into());
        a.push(cpus.to_string());
    }
    a.push("--cap-drop".into());
    a.push("ALL".into());
    a.push("--security-opt".into());
    a.push("no-new-privileges".into());
    a
}

/// 不挂 `target/` —— 挂了会盖掉烘焙进去的预热缓存,任务就必然超时。
/// 挂目录而不是挂单个文件:单文件 bind mount 在 Docker Desktop 的
/// 9p / virtiofs 共享上最容易出怪问题,而 `demo/src/` 里只有一个文件。
pub fn run_args(spec: &RunSpec) -> Vec<String> {
    let mut a: Vec<String> = vec!["run".into()];
    a.extend(limit_args(spec));
    a.push("--rm".into());
    a.push("--name".into());
    a.push(spec.name.clone());
    a.push("-v".into());
    a.push(format!(
        "{}:{}/{}:ro",
        spec.src_dir.display(),
        spec.workdir,
        "demo/src"
    ));
    a.push("-v".into());
    a.push(format!("{}:/out:rw", spec.out_dir.display()));
    a.push("--workdir".into());
    a.push(spec.workdir.clone());
    a.push(spec.image.clone());
    a.push("sh".into());
    a.push("-c".into());
    a.push(container_command(spec));
    a
}

/// `--keep-storage` 是容量,不是次数:保留最近用到的层,超预算的丢最旧的。
/// **有副作用**:builder cache 全局共享、没有 label,这台机器上别的项目的
/// 构建缓存会被一起裁(下次变慢,不会出错)。写进部署文档。
pub fn prune_args(keep_storage: u64) -> Vec<String> {
    vec![
        "builder".into(),
        "prune".into(),
        "-f".into(),
        "--keep-storage".into(),
        keep_storage.to_string(),
    ]
}

/// 镜像 prune,只碰带指定 label 的镜像。不带 `-a`(那会连未使用的 tagged
/// 镜像一起删,可能误删 serving)—— 这里只清 dangling 层,builder prune
/// 才管得住大头。
pub fn image_prune_args(filter: &str) -> Vec<String> {
    vec![
        "image".into(),
        "prune".into(),
        "-f".into(),
        "--filter".into(),
        filter.into(),
    ]
}

pub fn kill_args(name: &str) -> Vec<String> {
    vec!["kill".into(), name.into()]
}

/// 删掉一个容器。RA 会话的销毁钩子用它:容器一没,里面的 exec 和
/// rust-analyzer 一起走。
pub fn rm_args(name: &str) -> Vec<String> {
    vec!["rm".into(), "-f".into(), name.into()]
}

/// RA 会话的 exec 参数。
///
/// **`-i` 必须在、`-t` 必须不在。** `-i` 开 stdin/stdout 管道 —— 而这条管道
/// 和 `docker run` 的 attach 协议不一样:exec 走的是**裸字节管道**,宿主机到
/// 容器之间不套那 8 字节复用头,所以 LSP 的 `Content-Length` 帧能原样穿过
/// (实测回包头完好)。`-t` 会分配伪终端、输出带 `\r\n`,拆帧就废了。
pub fn exec_args(name: &str, cmd: &str) -> Vec<String> {
    vec![
        "exec".into(),
        "-i".into(),
        name.into(),
        "sh".into(),
        "-c".into(),
        cmd.into(),
    ]
}

/// 待命 RA 容器:`-d` 后台,不 attach。
///
/// 形状必须是 `... limit_args --rm --name X --workdir DIR -d IMAGE sleep`:
/// `-d` 是 `docker run` 的旗标(IMAGE 之前),而 image 之后是容器要执行的
/// `COMMAND [ARG...]` —— 末尾的 `sleep` 担当那个 COMMAND,撑住前台进程。
///
/// 不挂 `demo/src` 与 `/out`:RA 的 workspace 在镜像内,源码由 `exec_args`
/// 把 `cargo build` 的产物灌进去;不挂出 `target/` 也意味着容器不会有宿主机
/// 路径 —— 全程容器内文件系统,挂载少一份故障面。
///
/// 末尾的 `sleep infinity` 是必须的:容器必须有前台常驻进程才不会被判定退出
/// (镜像的 CMD 是 bash,不显式给命令的话它会起一个没有 tty 的 bash 然后
/// 立刻 exit 0 —— 看起来「起来了」,实际什么都没跑)。RA 本身由
/// `exec_args` 在这个容器里另起。
///
/// 参数 `infinity` 不是可有可无的:裸 `sleep` 缺操作数,coreutils 会立刻
/// 报错退出,`--rm` 随即将容器删掉 —— 后续 `docker exec` 全都打不进去。
pub fn run_detached_args(spec: &RunSpec) -> Vec<String> {
    let mut a: Vec<String> = vec!["run".into()];
    a.extend(limit_args(spec));
    a.push("--rm".into());
    a.push("--name".into());
    a.push(spec.name.clone());
    a.push("--workdir".into());
    a.push(RA_WORKDIR.into());
    a.push("-d".into());
    a.push(spec.image.clone());
    a.push("sleep".into());
    a.push("infinity".into());
    a
}

pub fn image_ls_args(filter: &str) -> Vec<String> {
    vec![
        "image".into(),
        "ls".into(),
        "--filter".into(),
        filter.into(),
        "--format".into(),
        "{{.Repository}}:{{.Tag}}".into(),
    ]
}

/// 列**我们自己的**全部镜像:tag + 创建时间,按调用方给的顺序无关(不排序)。
///
/// 部署用的那台宿主上的 docker CLI 有两个坑,这里一个都不踩:
///   - `image ls` 不认 `--sort`(退出码 125),所以排序在客户端做;
///   - `image ls` 的模板里**没有 `.Label` 字段**(`can't evaluate field
///     Label`),所以 commit label 拿不到 —— 改用 [`image_inspect_commit_args`]。
pub fn newest_labeled_image_args(image: &str) -> Vec<String> {
    vec![
        "image".into(),
        "ls".into(),
        "--filter".into(),
        crate::image::label_filter(image, None),
        "--format".into(),
        "{{.Repository}}:{{.Tag}}\t{{.CreatedAt}}".into(),
    ]
}

/// 读单个镜像的 commit label 全长 sha。
///
/// `image inspect` 的模板走的是 `index .Config.Labels`,这条路在老 docker 上
/// 是通的(`image ls` 的 `.Label` 不是)。
pub fn image_inspect_commit_args(tag: &str) -> Vec<String> {
    vec![
        "image".into(),
        "inspect".into(),
        tag.into(),
        "--format".into(),
        format!("{{{{index .Config.Labels \"{}\"}}}}", crate::image::LABEL_COMMIT),
    ]
}

pub fn system_df_args() -> Vec<String> {    vec![
        "system".into(),
        "df".into(),
        "--format".into(),
        "{{json .}}".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(stream: u8, payload: &[u8]) -> Vec<u8> {
        let mut v = vec![stream, 0, 0, 0];
        v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        v.extend_from_slice(payload);
        v
    }

    fn decode_all(mut d: FrameDecoder, chunks: &[Vec<u8>]) -> Vec<Frame> {
        let mut out = Vec::new();
        for c in chunks {
            out.extend(d.push(c).unwrap());
        }
        out
    }

    #[test]
    fn single_stdout_frame() {
        let d = FrameDecoder::new();
        let got = decode_all(d, &[frame(1, b"Compiling demo\n")]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].stream, Stream::Stdout);
        assert_eq!(got[0].data, b"Compiling demo\n");
    }

    #[test]
    fn two_frames_in_one_chunk() {
        let mut chunk = frame(1, b"out");
        chunk.extend(frame(2, b"err"));
        let got = decode_all(FrameDecoder::new(), &[chunk]);
        assert_eq!(
            got,
            vec![
                Frame { stream: Stream::Stdout, data: b"out".to_vec() },
                Frame { stream: Stream::Stderr, data: b"err".to_vec() },
            ]
        );
    }

    #[test]
    fn frame_split_inside_header() {
        let whole = frame(1, b"hello");
        let got = decode_all(
            FrameDecoder::new(),
            &[whole[..3].to_vec(), whole[3..].to_vec()],
        );
        assert_eq!(got, vec![Frame { stream: Stream::Stdout, data: b"hello".to_vec() }]);
    }

    #[test]
    fn frame_split_inside_payload() {
        let whole = frame(2, b"0123456789");
        let got = decode_all(
            FrameDecoder::new(),
            &[whole[..12].to_vec(), whole[12..].to_vec()],
        );
        assert_eq!(got, vec![Frame { stream: Stream::Stderr, data: b"0123456789".to_vec() }]);
    }

    #[test]
    fn empty_chunks_are_harmless() {
        let mut d = FrameDecoder::new();
        assert!(d.push(&[]).unwrap().is_empty());
        let got = d.push(&frame(1, b"x")).unwrap();
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn zero_length_frame() {
        let got = decode_all(FrameDecoder::new(), &[frame(1, b"")]);
        assert_eq!(got, vec![Frame { stream: Stream::Stdout, data: vec![] }]);
    }

    #[test]
    fn bad_stream_byte_is_an_error() {
        let mut d = FrameDecoder::new();
        assert_eq!(d.push(&frame(3, b"x")), Err(FrameError::BadStream(3)));
    }

    #[test]
    fn oversized_frame_is_rejected() {
        let mut v = vec![1u8, 0, 0, 0];
        v.extend_from_slice(&(MAX_FRAME + 1).to_be_bytes());
        let mut d = FrameDecoder::new();
        assert_eq!(d.push(&v), Err(FrameError::TooLarge(MAX_FRAME + 1)));
    }

    fn spec() -> RunSpec {
        RunSpec {
            name: "r2g-task1".into(),
            image: "rust2genshin-play:0123456".into(),
            src_dir: PathBuf::from("/data/tasks/task1/src"),
            out_dir: PathBuf::from("/data/tasks/task1/out"),
            workdir: "/opt/rust2genshin".into(),
            target: "beyond-server".into(),
            cpus: Some(2),
            memory: Some("1536m".into()),
            pids_limit: 256,
            user: "1001".into(),
            source_file: "demo/src/lib.rs".into(),
        }
    }

    #[test]
    fn container_command_uses_the_selected_target() {
        let mut s = spec();
        s.target = "classic-server".into();
        let cmd = container_command(&s);
        assert!(cmd.contains("--target ./genshin-unknown-classic-server.json"), "{cmd}");
        assert!(
            cmd.contains("cp target/genshin-unknown-classic-server/release/rust2genshin_demo.gia /out/"),
            "{cmd}"
        );
        assert!(!cmd.contains("beyond-server"), "{cmd}");
    }

    #[test]
    fn container_command_defaults_to_beyond() {
        let cmd = container_command(&spec());
        assert!(cmd.contains("--target ./genshin-unknown-beyond-server.json"), "{cmd}");
        assert!(
            cmd.contains("cp target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia /out/"),
            "{cmd}"
        );
    }

    #[test]
    fn run_args_hardening_is_complete() {
        let a = run_args(&spec());
        for want in [
            "--network", "none",
            "--memory", "1536m",
            "--cpus", "2",
            "--pids-limit", "256",
            "--user", "1001",
            "--tmpfs", "/tmp",
            "--rm",
        ] {
            assert!(a.contains(&want.to_string()), "argv 缺 {want}:{a:?}");
        }
        assert!(a.windows(2).any(|w| w == ["--cap-drop", "ALL"]));
        assert!(a
            .windows(2)
            .any(|w| w == ["--security-opt", "no-new-privileges"]));
        assert!(a.windows(2).any(|w| w == ["--memory-swap", "1536m"]));
        assert!(!a.contains(&"--read-only".to_string()), "cargo 要写 target/,不能只读");
        assert!(!a.iter().any(|s| s.contains("/var/run/docker.sock")));
    }

    #[test]
    fn no_cpu_limit_means_no_cpus_flag() {
        // `--cpus` 整个旗标都不出现,而不是拼成 `--cpus None` 或 `--cpus 0`。
        let mut s = spec();
        s.cpus = None;
        let a = run_args(&s);
        assert!(!a.iter().any(|x| x == "--cpus"), "{a:?}");
        assert!(!a.iter().any(|x| x == "None"), "{a:?}");
    }

    #[test]
    fn no_memory_limit_drops_memory_and_its_swap_twin() {
        // kmem 不被支持时,`--memory` 和 `--memory-swap` 都要消失;留下
        // `--memory-swap` 一个孤儿旗标毫无意义。
        let mut s = spec();
        s.memory = None;
        let a = run_args(&s);
        assert!(!a.iter().any(|x| x == "--memory"), "{a:?}");
        assert!(!a.iter().any(|x| x == "--memory-swap"), "{a:?}");
        assert!(!a.iter().any(|x| x == "None"), "{a:?}");
    }

    #[test]
    fn run_args_mounts_only_src_and_out() {
        let a = run_args(&spec());
        let vols: Vec<String> = a
            .windows(2)
            .filter(|w| w[0] == "-v")
            .map(|w| w[1].clone())
            .collect();
        assert_eq!(
            vols,
            vec![
                "/data/tasks/task1/src:/opt/rust2genshin/demo/src:ro".to_string(),
                "/data/tasks/task1/out:/out:rw".to_string(),
            ]
        );
        assert!(
            !vols.iter().any(|v| v.contains("target")),
            "挂 target/ 会盖掉烘进去的预热缓存"
        );
    }

    #[test]
    fn run_args_names_container_and_ends_with_shell_command() {
        let a = run_args(&spec());
        assert!(a.windows(2).any(|w| w == ["--name", "r2g-task1"]));
        let img = a
            .iter()
            .position(|s| s == "rust2genshin-play:0123456")
            .unwrap();
        assert_eq!(a[img - 2], "--workdir");
        assert_eq!(a[img - 1], "/opt/rust2genshin");
        assert_eq!(a[img + 1], "sh");
        assert_eq!(a[img + 2], "-c");
        let cmd = &a[img + 3];
        assert!(cmd.contains("--offline"));
        assert!(cmd.contains("-Zbuild-std=core"));
        assert!(cmd.contains("cp target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia /out/"));
        assert_eq!(a.len(), img + 4, "命令之后不该再有别的参数");
    }

    #[test]
    fn prune_keep_storage() {
        assert_eq!(
            prune_args(10 * 1024 * 1024 * 1024),
            vec!["builder", "prune", "-f", "--keep-storage", "10737418240"]
        );
    }

    #[test]
    fn image_prune_is_label_scoped() {
        assert_eq!(
            image_prune_args("label=rust2genshin.play=1"),
            vec!["image", "prune", "-f", "--filter", "label=rust2genshin.play=1"]
        );
    }

    #[test]
    fn misc_args() {
        assert_eq!(kill_args("r2g-t1"), vec!["kill", "r2g-t1"]);
        assert_eq!(
            image_ls_args("label=rust2genshin.play=1"),
            vec![
                "image", "ls", "--filter", "label=rust2genshin.play=1",
                "--format", "{{.Repository}}:{{.Tag}}",
            ]
        );
        assert_eq!(system_df_args(), vec!["system", "df", "--format", "{{json .}}"]);
    }

    #[test]
    fn newest_labeled_image_args_carry_the_creation_time_and_nothing_they_cannot_do() {
        let a = newest_labeled_image_args("rust2genshin-play");
        // 部署用的那台宿主上 docker CLI 不认 --sort(退出码 125),
        // 排序在客户端做 —— 这里就不许再出现它。
        assert!(!a.iter().any(|x| x.contains("sort")), "{a:?}");
        let fmt = &a[a.iter().position(|x| x == "--format").unwrap() + 1];
        // `.Label` 在这台 docker 的 image ls 模板里不存在(模板解析直接报错),
        // commit 只能另走 image inspect。
        assert!(!fmt.contains("Label"), "{fmt}");
        assert!(fmt.contains("CreatedAt"), "{fmt}");
        assert!(
            a.iter().any(|x| x == "label=rust2genshin.play=1"),
            "必须只列我们自己的镜像:{a:?}"
        );
    }

    #[test]
    fn image_inspect_commit_args_read_the_label_by_index() {
        let a = image_inspect_commit_args("rust2genshin-play:1102e2b");
        assert_eq!(a[0], "image");
        assert_eq!(a[1], "inspect");
        assert!(a.contains(&"rust2genshin-play:1102e2b".to_string()), "{a:?}");
        let fmt = &a[a.iter().position(|x| x == "--format").unwrap() + 1];
        assert_eq!(fmt, "{{index .Config.Labels \"rust2genshin.commit\"}}");
    }

    #[test]
    fn exec_has_i_and_not_t() {
        let a = exec_args("r2g-ra-1", "rust-analyzer");
        assert!(a.contains(&"-i".to_string()));
        assert!(!a.contains(&"-t".to_string()));
    }

    #[test]
    fn exec_shape_is_run_subcommand() {
        let a = exec_args("r2g-ra-1", "rust-analyzer");
        assert_eq!(
            a,
            vec!["exec", "-i", "r2g-ra-1", "sh", "-c", "rust-analyzer"]
        );
    }

    fn ra_spec() -> RunSpec {
        RunSpec {
            name: "r2g-ra-1".into(),
            image: "img:tag".into(),
            src_dir: PathBuf::from("/src"),
            out_dir: PathBuf::from("/out"),
            workdir: "/w".into(),
            target: "beyond-server".into(),
            cpus: Some(2),
            memory: Some("2g".into()),
            pids_limit: 128,
            user: "1001".into(),
            source_file: "demo/src/lib.rs".into(),
        }
    }

    #[test]
    fn run_detached_puts_d_before_the_image() {
        let a = run_detached_args(&ra_spec());
        let d_pos = a.iter().position(|x| x == "-d").unwrap();
        let img_pos = a.iter().position(|x| x == "img:tag").unwrap();
        assert!(d_pos < img_pos, "-d 必须在 image 之前,当前 argv: {a:?}");
    }

    #[test]
    fn run_detached_keeps_the_container_alive() {
        let a = run_detached_args(&ra_spec());
        let img_pos = a.iter().position(|x| x == "img:tag").unwrap();
        let after: Vec<&String> = a.iter().skip(img_pos + 1).collect();
        // 必须是 `sleep infinity`:裸 `sleep` 缺操作数会立刻退出,`--rm`
        // 随即删掉容器,后续 docker exec 全部打不进去。
        assert_eq!(
            after,
            vec![&"sleep".to_string(), &"infinity".to_string()],
            "image 之后必须是 sleep infinity,实际:{after:?}"
        );
        assert!(
            !a.iter().any(|s| s.contains("cargo build")),
            "不能含 cargo build,实际 argv:{a:?}"
        );
    }

    #[test]
    fn run_detached_has_no_attach_flags_and_no_mounts() {
        let a = run_detached_args(&ra_spec());
        assert!(!a.contains(&"-i".to_string()), "不能有 -i,实际 argv:{a:?}");
        assert!(!a.contains(&"-t".to_string()), "不能有 -t,实际 argv:{a:?}");
        assert!(
            !a.iter().any(|s| s.contains("demo/src")),
            "不能挂 demo/src,实际 argv:{a:?}"
        );
        assert!(
            !a.iter().any(|s| s.contains("/out")),
            "不能挂 /out,实际 argv:{a:?}"
        );
    }
}
