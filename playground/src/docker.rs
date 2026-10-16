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
    pub cpus: u32,
    pub memory: String,
    pub pids_limit: u32,
    pub user: String,
    /// 用户代码在仓库里的相对路径(`demo/src/lib.rs`),只写进日志。
    pub source_file: String,
}

/// 容器里跑的那条命令。`--offline` 是关键:依赖在烘焙时 fetch 齐了,
/// 容器又是 `--network none`,让"万一要联网"变成一个明确的错误而不是超时。
pub fn container_command(spec: &RunSpec) -> String {
    format!(
        "cargo build --release --offline -p rust2genshin-demo \
         --target ./genshin-unknown-beyond-server.json \
         -Zunstable-options -Zjson-target-spec -Zbuild-std=core \
         && cp target/genshin-unknown-beyond-server/release/rust2genshin_demo.gia /out/"
    )
}

/// 不挂 `target/` —— 挂了会盖掉烘焙进去的预热缓存,任务就必然超时。
/// 挂目录而不是挂单个文件:单文件 bind mount 在 Docker Desktop 的
/// 9p / virtiofs 共享上最容易出怪问题,而 `demo/src/` 里只有一个文件。
pub fn run_args(spec: &RunSpec) -> Vec<String> {
    let mut a: Vec<String> = vec!["run".into()];
    let cpus = spec.cpus.to_string();
    let pids = spec.pids_limit.to_string();
    for s in [
        "--network", "none",
        "--memory", &spec.memory,
        "--memory-swap", &spec.memory,
        "--cpus", &cpus,
        "--pids-limit", &pids,
        "--user", &spec.user,
        "--tmpfs", "/tmp",
    ] {
        a.push(s.into());
    }
    a.push("--cap-drop".into());
    a.push("ALL".into());
    a.push("--security-opt".into());
    a.push("no-new-privileges".into());
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

pub fn system_df_args() -> Vec<String> {
    vec![
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
            cpus: 2,
            memory: "1536m".into(),
            pids_limit: 256,
            user: "1001".into(),
            source_file: "demo/src/lib.rs".into(),
        }
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
}
