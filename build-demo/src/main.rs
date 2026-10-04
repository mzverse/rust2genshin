use new_string_template::template::Template;
use std::collections::HashMap;
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

macro_rules! cargo {
    ($($x:expr),+ $(,)?) => {
        $crate::cargo([$($x),+])
    };
}

fn main() {
    cargo!("build", "-p", "rust2genshin");

    let target = PathBuf::from(format!("{}/../genshin-unknown-server.json", env!("CARGO_MANIFEST_DIR")));
    let mut data = HashMap::new();
    data.insert("backend", serde_json::to_string(fs::canonicalize(format!("{}/../target/debug/{DLL_PREFIX}rust2genshin{DLL_SUFFIX}", env!("CARGO_MANIFEST_DIR"))).unwrap().to_str().unwrap()).unwrap());
    fs::write(&target, Template::new(fs::read_to_string(target.with_added_extension("template")).unwrap()).render(&data).unwrap()).unwrap();

    cargo!("clean", "-p", "rust2genshin-demo");
    cargo!("clean", "-p", "rust2genshin-lib");

    cargo!("build", "--release", "-p", "rust2genshin-demo", "--target", "./genshin-unknown-server.json", "-Zunstable-options", "-Zjson-target-spec", "-Zbuild-std=core");
}

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
        std::process::exit(status.code().unwrap());
    }
}
