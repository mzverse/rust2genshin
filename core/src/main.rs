//! rust2genshin:以库方式调用 rustc(rustc_driver)编译目标 crate,
//! 在 `after_analysis` 拿到 TyCtxt 后遍历 HIR 输出结构(不做 codegen/link)。
//!
//! rustc_driver 等 crate 只存在于 nightly sysroot,cargo 无法从 crates.io
//! 解析;构建时由 build.ps1 通过 RUSTFLAGS --extern 指向 sysroot 里的
//! rmeta/dll(并写入 .cargo/config.toml,之后可直接 `cargo build`)。

#![feature(rustc_private)]

mod asset;
mod parser;

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::Parser;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::ty::TyCtxt;

// ---------- CLI ----------

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// 目标 Rust 项目路径(含 Cargo.toml 的目录),或单个 .rs 文件
    path: PathBuf,
    /// 不预先执行 `cargo check` 收集依赖(--extern),直接跑 rustc
    #[arg(long)]
    no_cargo: bool,
    /// 打印更详细的信息(函数参数、结构体字段、枚举变体等)
    #[arg(short, long)]
    verbose: bool,
}

// ---------- rustc 回调 ----------

struct AnalysisCallbacks {
    verbose: bool,
}

impl Callbacks for AnalysisCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        parser::dump_hir(tcx, self.verbose);
        Compilation::Stop // 只做分析,不需要 codegen/link
    }
}

// ---------- 目标项目信息 ----------

struct ProjectInfo {
    /// 清单里的包名(可能带短横线),用于 `cargo check -p`
    package_name: String,
    crate_name: String,
    edition: String,
    crate_type: String, // "lib" | "bin"
    source_path: PathBuf,
}

/// 解析出入口信息:
/// - 单个 .rs 文件:直接从文件名推 crate 名(没有 Cargo.toml,cargo 无从下手);
/// - 项目目录:全部交给 `cargo metadata`(包名、edition、入口文件一次拿全)。
fn load_project_info(path: &Path) -> Result<ProjectInfo, Box<dyn Error>> {
    if path.is_file() {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("lib.rs");
        let stem = file_name.trim_end_matches(".rs");
        let crate_name = if stem.is_empty() { "crate" } else { stem }.replace('-', "_");
        let crate_type = if file_name == "main.rs" { "bin" } else { "lib" };
        return Ok(ProjectInfo {
            package_name: crate_name.clone(),
            crate_name,
            edition: "2021".to_string(),
            crate_type: crate_type.to_string(),
            source_path: path.to_path_buf(),
        });
    }
    if !path.is_dir() {
        return Err(format!("path does not exist: {}", path.display()).into());
    }

    load_from_cargo_metadata(path).map_err(|e| e.into())
}

/// 用 `cargo metadata` 一次性拿包名、edition、入口文件。目标项目嵌套在别的
/// workspace 里且没有 `[workspace]` 表时 cargo 会拒绝执行,错误信息原样透传。
fn load_from_cargo_metadata(project: &Path) -> Result<ProjectInfo, String> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(project)
        .output()
        .map_err(|e| format!("cannot run `cargo metadata`: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`cargo metadata` failed:\n{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("cannot parse `cargo metadata` output: {e}"))?;
    let manifest = project.join("Cargo.toml");
    let manifest = manifest
        .canonicalize()
        .map_err(|_| format!("cannot find Cargo.toml under {}", project.display()))?;
    // workspace 成员可能有多个包,按 manifest 路径匹配目标项目
    let pkg = v["packages"]
        .as_array()
        .and_then(|pkgs| {
            pkgs.iter().find(|p| {
                p["manifest_path"]
                    .as_str()
                    .map(Path::new)
                    .and_then(|p| p.canonicalize().ok())
                    == Some(manifest.clone())
            })
        })
        .ok_or_else(|| {
            format!("no package with manifest {} found in `cargo metadata` output", manifest.display())
        })?;
    let name = pkg["name"].as_str().ok_or("package has no `name`")?.to_string();
    let edition = pkg["edition"].as_str().unwrap_or("2021").to_string();
    let targets = pkg["targets"].as_array().ok_or("package has no targets")?;
    let is_kind = |t: &serde_json::Value, k: &str| {
        t["kind"].as_array().is_some_and(|kinds| kinds.iter().any(|x| x.as_str() == Some(k)))
    };
    let (target, crate_type) = match targets.iter().find(|t| is_kind(t, "lib")) {
        Some(t) => (t, "lib"),
        None => (targets.iter().find(|t| is_kind(t, "bin")).ok_or("no lib or bin target")?, "bin"),
    };
    let source_path = PathBuf::from(target["src_path"].as_str().ok_or("target has no src_path")?);
    if !source_path.is_file() {
        return Err(format!("entry file does not exist: {}", source_path.display()));
    }
    // metadata 给的是绝对路径;rustc 会把 cwd 下的绝对路径相对化显示成
    // `.\...`,而相对路径会按绝对路径显示——转回相对形式,保持 span 显示
    // 与直接传路径参数时一致
    let source_path = std::env::current_dir()
        .ok()
        .and_then(|cwd| source_path.strip_prefix(&cwd).ok().map(|r| r.to_path_buf()))
        .unwrap_or(source_path);
    Ok(ProjectInfo {
        package_name: name.clone(),
        crate_name: name.replace('-', "_"),
        edition,
        crate_type: crate_type.to_string(),
        source_path,
    })
}

// ---------- 依赖收集(cargo check) ----------

/// 依赖元数据:直接依赖的 --extern 列表 + -L dependency 目录
#[derive(Default)]
struct DepInfo {
    externs: Vec<(String, PathBuf)>,
    dep_dirs: Vec<PathBuf>,
}

/// 在目标项目里执行 `cargo check --message-format=json` 收集依赖的
/// rlib/rmeta 路径。用与 driver 相同的 nightly rustc(RUSTC 环境变量)
/// 保证元数据版本兼容;`-p 包名` 只检查目标包——目标项目可能是某个更大
/// workspace 的成员,不带 -p 会连带检查所有成员(如 demo 连 core 一起查,
/// 而 core 依赖 rustc_driver,普通 cargo 编不了,必然失败)。
fn collect_deps(project: &Path, package_name: &str, root_crate: &str, sysroot: Option<&Path>) -> DepInfo {
    let mut info = DepInfo::default();
    let manifest = project.join("Cargo.toml");
    if !manifest.is_file() {
        return info;
    }

    // 优先用 sysroot 里的真实 cargo.exe(与 RUSTC 同工具链、绕过 rustup shim),
    // 找不到再退回 PATH 上的 cargo
    let mut cmd = Command::new("cargo");
    if let Some(sysroot) = sysroot {
        let cargo = sysroot.join("bin").join(if cfg!(windows) { "cargo.exe" } else { "cargo" });
        if cargo.is_file() {
            cmd = Command::new(cargo);
        }
    }
    cmd.args(["check", "--quiet", "--message-format=json", "-p", package_name])
        .current_dir(project);
    if let Some(sysroot) = sysroot {
        let rustc = sysroot.join("bin").join(if cfg!(windows) { "rustc.exe" } else { "rustc" });
        if rustc.is_file() {
            cmd.env("RUSTC", &rustc);
        }
    }

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[warn] cannot run `cargo check`: {e}");
            return fallback_dep_dir(project, info);
        }
    };
    if !output.status.success() {
        eprintln!(
            "[warn] `cargo check` failed (exit {}); continuing with whatever was built already",
            output.status
        );
    }

    for line in output.stdout.split(|&b| b == b'\n') {
        let line = String::from_utf8_lossy(line);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["reason"].as_str() != Some("compiler-artifact") || v["target"]["name"].as_str() == Some(root_crate) {
            continue; // 只关心编译产物;根 crate 自身不传 --extern
        }
        let Some(name) = v["target"]["name"].as_str() else {
            continue;
        };
        let kinds: Vec<&str> = v["target"]["kind"]
            .as_array()
            .map(|k| k.iter().filter_map(|x| x.as_str()).collect())
            .unwrap_or_default();
        let is_lib = kinds
            .iter()
            .any(|k| matches!(*k, "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro"));
        if !is_lib {
            continue;
        }
        let is_proc = kinds.iter().any(|k| *k == "proc-macro");
        let Some(files) = v["filenames"].as_array() else {
            continue;
        };
        // 只做分析(codegen 被跳过),依赖用 rlib 或 rmeta 均可:cargo check 产
        // rmeta,cargo build 产 rlib,优先 rlib;proc-macro 取动态库
        let mut chosen: Option<PathBuf> = None;
        let mut chosen_dir: Option<PathBuf> = None;
        for f in files.iter().filter_map(|f| f.as_str()) {
            let p = PathBuf::from(f);
            let ext = p.extension().and_then(|e| e.to_str());
            let (good, prefer) = if is_proc {
                (matches!(ext, Some("dll" | "so" | "dylib")), true)
            } else {
                (matches!(ext, Some("rlib" | "rmeta")), ext == Some("rlib"))
            };
            if good {
                if chosen.is_none() || prefer {
                    chosen = Some(p.clone());
                    chosen_dir = p.parent().map(|d| d.to_path_buf());
                }
                if prefer {
                    break;
                }
            }
        }
        if let Some(p) = chosen {
            if !info.externs.iter().any(|(n, _)| n == name) {
                info.externs.push((name.replace('-', "_"), p));
            }
            if let Some(dir) = chosen_dir {
                if !info.dep_dirs.contains(&dir) {
                    info.dep_dirs.push(dir);
                }
            }
        }
    }

    fallback_dep_dir(project, info)
}

/// 兜底:cargo 信息不全时,把项目里已有的 target/debug/deps 作为 -L 目录
fn fallback_dep_dir(project: &Path, mut info: DepInfo) -> DepInfo {
    let deps = project.join("target").join("debug").join("deps");
    if deps.is_dir() && !info.dep_dirs.contains(&deps) {
        info.dep_dirs.push(deps);
    }
    info
}

// ---------- sysroot ----------

/// sysroot 下所有 `lib/rustlib/<triple>/lib` 目录(里面是 rustc 的 rmeta/dll)
fn rustlib_lib_dirs(sysroot: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(sysroot.join("lib").join("rustlib")) else {
        return vec![];
    };
    entries
        .flatten()
        .map(|e| e.path().join("lib"))
        .filter(|d| d.is_dir())
        .collect()
}

/// 校验 sysroot 里带 rustc-dev(librustc_driver-*.rmeta),否则 ABI 可能不匹配
fn sysroot_has_rustc_driver(sysroot: &Path) -> bool {
    rustlib_lib_dirs(sysroot).into_iter().any(|dir| {
        std::fs::read_dir(&dir).is_ok_and(|files| {
            files
                .flatten()
                .any(|f| f.file_name().to_string_lossy().starts_with("librustc_driver-"))
        })
    })
}

/// 依次尝试:1) 环境变量 SYSROOT 2) `rustc --print sysroot`(cwd 内
/// rust-toolchain.toml 生效)3) `rustup run nightly rustc --print sysroot`
fn locate_sysroot() -> Option<PathBuf> {
    if let Some(p) = env::var_os("SYSROOT").map(PathBuf::from) {
        if sysroot_has_rustc_driver(&p) {
            return Some(p);
        }
    }
    probe_sysroot(&["rustc", "--print", "sysroot"])
        .or_else(|| probe_sysroot(&["rustup", "run", "nightly", "rustc", "--print", "sysroot"]))
}

fn probe_sysroot(args: &[&str]) -> Option<PathBuf> {
    let out = Command::new(args[0]).args(&args[1..]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    sysroot_has_rustc_driver(&p).then_some(p)
}

/// 把 sysroot 的 bin 与 rustlib/*/lib 目录加进 PATH,让运行时能加载
/// rustc_driver-*.dll / libLLVM-*.dll
fn prepend_dll_dirs(sysroot: &Path) {
    let mut dirs: Vec<String> = vec![];
    let bin = sysroot.join("bin");
    if bin.is_dir() {
        dirs.push(bin.display().to_string());
    }
    dirs.extend(rustlib_lib_dirs(sysroot).iter().map(|d| d.display().to_string()));
    if dirs.is_empty() {
        return;
    }
    if let Some(old) = env::var_os("PATH") {
        dirs.push(old.to_string_lossy().into_owned());
    }
    set_env("PATH", &dirs.join(";"));
}

/// edition 2024 里 `env::set_var` 是 unsafe,集中到一处
fn set_env(name: &str, value: &str) {
    unsafe { env::set_var(name, value) };
}

// ---------- 主流程 ----------

/// 组装传给 `rustc_driver::run_compiler` 的参数
fn build_rustc_args(info: &ProjectInfo, deps: &DepInfo, sysroot: Option<&Path>) -> Vec<String> {
    let mut args = vec![
        "rust2genshin".to_string(),
        "--crate-type".to_string(),
        info.crate_type.clone(),
        "--edition".to_string(),
        info.edition.clone(),
        "--crate-name".to_string(),
        info.crate_name.clone(),
        "--cap-lints".to_string(),
        "allow".to_string(), // 分析工具不需要触发目标 crate 的 deny 类 lint
    ];
    if let Some(sysroot) = sysroot {
        args.push("--sysroot".to_string());
        args.push(sysroot.display().to_string());
    }
    for dir in &deps.dep_dirs {
        args.push("-L".to_string());
        args.push(format!("dependency={}", dir.display()));
    }
    for (name, path) in &deps.externs {
        args.push("--extern".to_string());
        args.push(format!("{name}={}", path.display()));
    }
    args.push(info.source_path.display().to_string());
    args
}

fn main() -> ExitCode {
    let args = Args::parse();

    let info = match load_project_info(&args.path) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    // 定位并配置 sysroot(PATH 注入 dll 目录、SYSROOT 环境变量、--sysroot 参数)
    let sysroot = locate_sysroot();
    if let Some(sysroot) = &sysroot {
        set_env("SYSROOT", &sysroot.display().to_string());
        prepend_dll_dirs(sysroot);
    } else {
        eprintln!(
            "warning: cannot locate a nightly sysroot with rustc-dev; \
             set SYSROOT env var or put a nightly toolchain with rustc-dev in PATH"
        );
    }

    // 先 `cargo check` 收集目标项目依赖的 rlib/rmeta 路径
    let deps = if args.no_cargo {
        DepInfo::default()
    } else {
        collect_deps(&args.path, &info.package_name, &info.crate_name, sysroot.as_deref())
    };

    let rustc_args = build_rustc_args(&info, &deps, sysroot.as_deref());
    let mut callbacks = AnalysisCallbacks { verbose: args.verbose };
    rustc_driver::catch_with_exit_code(|| rustc_driver::run_compiler(&rustc_args, &mut callbacks))
}
