use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use new_string_template::template::Template;
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};

pub fn cargo<I, S>(args: I)
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

#[macro_export]
macro_rules! cargo {
    ($($x:expr),+ $(,)?) => {
        $crate::cargo([$($x),+])
    };
}

pub fn gen_target(name: &str) {
    let target = PathBuf::from(format!("{}/../{name}.json", env!("CARGO_MANIFEST_DIR")));
    let mut data = HashMap::new();
    data.insert("backend", serde_json::to_string(fs::canonicalize(format!("{}/../target/debug/{DLL_PREFIX}rust2genshin{DLL_SUFFIX}", env!("CARGO_MANIFEST_DIR"))).unwrap().to_str().unwrap()).unwrap());
    fs::write(&target, Template::new(fs::read_to_string(target.with_added_extension("template")).unwrap()).render(&data).unwrap()).unwrap();
}

pub fn build(target: &str) {
    cargo!("build", "-p", "rust2genshin");

    gen_target(target);

    cargo!("clean", "-p", "rust2genshin-demo");
    cargo!("clean", "-p", "rust2genshin-lib");

    cargo!("build", "--release", "-p", "rust2genshin-demo", "--target", &format!("./{target}.json"), "-Zunstable-options", "-Zjson-target-spec", "-Zbuild-std=core");
}
