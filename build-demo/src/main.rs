use new_string_template::template::Template;
use std::collections::HashMap;
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

macro_rules! cargo {
    ($s:expr, $($x:expr),+ $(,)?) => {
        $crate::cargo($s, [$($x),+])
    };
}

fn main() {
    // 1) 直接用 cargo 构建后端:core(rust2genshin)本身就是后端,
    //    产物是 target/debug/rust2genshin.dll(cdylib)
    let status = Command::new("cargo")
        .args(["build", "-p", "rust2genshin"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status().unwrap();
    if !status.success() {
        std::process::exit(status.code().unwrap());
    }

    let target = PathBuf::from(format!("{}/../genshin-unknown-server.json", env!("CARGO_MANIFEST_DIR")));
    let mut data = HashMap::new();
    data.insert("backend", serde_json::to_string(fs::canonicalize(format!("{}/../target/debug/{DLL_PREFIX}rust2genshin{DLL_SUFFIX}", env!("CARGO_MANIFEST_DIR"))).unwrap().to_str().unwrap()).unwrap());
    fs::write(&target, Template::new(fs::read_to_string(target.with_added_extension("template")).unwrap()).render(&data).unwrap()).unwrap();

    // 2) 清掉 demo 旧产物,保证这次真的用后端重新编译(否则 cargo 指纹命中会跳过)
    cargo!("clean", "-p", "rust2genshin-demo");
    cargo!("clean", "-p", "rust2genshin-lib");

    // 3) 让真实 rustc 通过 -Zcodegen-backend 加载后端,只对目标叶 crate
    //    rust2genshin-demo 编译并导出 target/r2g/*.gia + *.txt
    //    (依赖含 proc-macro 照常由 LLVM 编译;关掉溢出检查避免 MIR 出现
    //    CheckedAdd 的 (i32, bool) 元组)
    // cargo!("test", "-p", "rust2genshin-demo");
    // cargo!("rustc", "--release", "-p", "rust2genshin-lib", "--", "-Coverflow-checks=off");
    cargo!("rustc", "--release", "-p", "rust2genshin-demo", "--", "-Coverflow-checks=off");
}

/// 用 cargo 跑一个命令,继承 stdio,失败则退出非零。
fn cargo<I, S>(s: S, args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let status = Command::new("cargo")
        .arg(s)
        .arg("-Zunstable-options")
        .args(["--target", "./genshin-unknown-server.json"])
        .arg("-Zjson-target-spec")
        .arg("-Zbuild-std=core")
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .unwrap();
    if !status.success() {
        std::process::exit(status.code().unwrap());
    }
}
