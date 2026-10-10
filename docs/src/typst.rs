use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::{env, iter};

fn walk(dir: PathBuf) -> impl Iterator<Item = PathBuf> {
    dir.read_dir().unwrap().map(Result::unwrap).flat_map(|x| {
        let p = x.path();
        if p.is_dir() {
            Box::new(walk(p).map(move |name| Path::new(&x.file_name()).join(name))) as Box<dyn Iterator<Item = PathBuf>>
        } else {
            Box::new(iter::once(PathBuf::from(x.file_name())))
        }
    })
}

pub static SRC: LazyLock<PathBuf> = LazyLock::new(|| Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("typst"));

pub fn build() {
    let mut file = File::create(SRC.join("main.typ")).unwrap();
    writeln!(file, r###"#import "/lib/lib.typ": *;"###).unwrap();
    for mut name in walk(SRC.clone()) {
        if name.as_os_str() == "main.typ" {
            continue;
        }
        if name.to_str().unwrap().ends_with("~") {
            continue;
        }
        if name.extension() == Some("typ".as_ref()) {
            write!(file, "#bundle_document").unwrap();
            name = name.with_extension("");
        } else {
            write!(file, "#bundle_asset").unwrap();
        }
        writeln!(
            file,
            r#"("/{}")"#,
            name.to_str().unwrap().replace("\\", "/"),
        ).unwrap();
    }
}

pub fn run(cmd: &str) -> i32 {
    let output = env::current_exe().unwrap().parent().unwrap().join("docs");
    Command::new("typst")
        .arg(cmd)
        .args(["--root", SRC.to_str().unwrap()])
        .args(["./docs/typst/main.typ", output.to_str().unwrap()])
        .args(["--features", "bundle,html"])
        .args(["--format", "bundle"])
        .args(env::var("BASE_PATH").into_iter().flat_map(|x| ["--input".to_string(), format!("base_path={x}")]))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status().unwrap().code().unwrap()
}

