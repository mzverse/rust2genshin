//! 摆弄场的后端服务。结构和取舍见
//! `docs/superpowers/specs/2026-10-16-playground-design.md`。

mod bake;
mod config;
mod docker;
mod gc;
mod image;
mod image_builder;
mod job;
mod protocol;
mod queue;
mod repo;
mod runner;
mod server;
#[cfg(test)]
mod testing;

fn main() {
    println!("{}", config::load().is_ok());
}
