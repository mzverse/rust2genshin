//! 摆弄场的后端服务。结构和取舍见
//! `docs/superpowers/specs/2026-10-16-playground-design.md`。

mod config;

fn main() {
    println!("{}", config::load().is_ok());
}
