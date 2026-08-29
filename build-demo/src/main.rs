use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::ffi::OsStr;
use std::process::{Command, Stdio};

macro_rules! cargo {
    () => {
        $crate::cargo()
    };
    ($($x:expr),+ $(,)?) => {
        $crate::cargo(&[$($x),+])
    };
}

fn main() {
    // 1) 直接用 cargo 构建后端:core(rust2genshin)本身就是后端,
    //    产物是 target/debug/rust2genshin.dll(cdylib)
    cargo!("build", "-p", "rust2genshin");

    // 2) 清掉 demo 旧产物,保证这次真的用后端重新编译(否则 cargo 指纹命中会跳过)
    cargo!("clean", "-p", "rust2genshin-demo");

    // 3) 让真实 rustc 通过 -Zcodegen-backend 加载后端,只对目标叶 crate
    //    rust2genshin-demo 编译并导出 target/r2g/*.gia + *.txt
    //    (依赖含 proc-macro 照常由 LLVM 编译;关掉溢出检查避免 MIR 出现
    //    CheckedAdd 的 (i32, bool) 元组)
    let dylib = env!("CARGO_MANIFEST_DIR").to_string() + &format!("/../target/debug/{DLL_PREFIX}rust2genshin{DLL_SUFFIX}");
    let flag = format!("-Zcodegen-backend={dylib}");
    cargo!("test", "-p", "rust2genshin-demo");
    cargo!("rustc", "-p", "rust2genshin-demo", "--", "-Coverflow-checks=off", &flag);
}

/// 用 cargo 跑一个命令,继承 stdio,失败则退出非零。
fn cargo<I, S>(args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let status = Command::new("cargo")
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .unwrap();
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}
