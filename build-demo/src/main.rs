use std::process::{Command, Stdio};

fn main() {
    Command::new("cargo")
        .args(vec!["run", "-p", "rust2genshin", "--", "demo", "-v"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn().unwrap()
        .wait().unwrap();
}
