//! 摆弄场的后端服务。结构和取舍见
//! `docs/superpowers/specs/2026-10-16-playground-design.md`。

mod config;
mod docker;
mod gc;
mod image;
mod job;
mod protocol;
mod repo;

fn main() {
    println!("{}", config::load().is_ok());
}
