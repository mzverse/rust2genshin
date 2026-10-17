use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::{env, iter};

/// 递归列出 `dir` 下的所有条目,返回**相对路径**,按每个目录内的文件名排序。
///
/// 必须排序:`read_dir` 的顺序跟着文件系统走(NTFS 大致是名字的哈希序、ext4 是
/// 目录项的物理顺序),不同机器、不同文件系统上结果不一样。顺序会一路传到
/// `main.typ` 的 `#bundle_asset(...)`,最后变成页面上 `query(asset)` 的顺序 ——
/// 比如「摆弄」页那个样例下拉框取的就是它的第一项当默认值。
///
/// 比较用 `as_encoded_bytes()`(WTF-8 字节)而不是 `OsStr: Ord`:后者在 Windows
/// 上按 WTF-16 码元比,在 Unix 上按字节比,同一份内容跨平台会得出不同顺序。
fn walk(dir: PathBuf) -> impl Iterator<Item = PathBuf> {
    let mut entries = dir.read_dir().unwrap().map(Result::unwrap).collect::<Vec<_>>();
    entries.sort_unstable_by_key(|x| x.file_name().as_encoded_bytes().to_vec());
    entries.into_iter().flat_map(|x| {
        let p = x.path();
        if p.is_dir() {
            Box::new(walk(p).map(move |name| Path::new(&x.file_name()).join(name))) as Box<dyn Iterator<Item = PathBuf>>
        } else {
            Box::new(iter::once(PathBuf::from(x.file_name())))
        }
    })
}

pub static SRC: LazyLock<PathBuf> = LazyLock::new(|| Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("typst"));

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

