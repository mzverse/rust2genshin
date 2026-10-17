//! RA 会话。见 `docs/superpowers/specs/2026-10-17-play-lsp-completion-design.md`。
//!
//! 这里的东西**与编译任务那条路完全无关**:编译走 `docker run` 的 attach 协议
//! (8 字节复用头,见 `docker::FrameDecoder`),RA 走 `docker exec -i` 的裸管道。

pub mod docker_exec;
pub mod exec;
pub mod frame;
pub mod pool;
pub mod service;
pub mod session;
