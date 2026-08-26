//! rust2genshin:以库方式调用 rustc(rustc_driver)编译目标 crate,
//! 在 `after_analysis` 拿到 TyCtxt 后遍历 HIR 输出结构(不做 codegen/link)。
//!
//! 项目分析完全交给 cargo:
//!   - `cargo metadata` 拿包名、edition、入口文件;
//!   - `cargo check` 编译目标项目的依赖,rustc 用 `-L dependency=`
//!     目录自动按 crate 名解析(不需要手工收集 --extern);
//!   - `#![feature(rustc_private)]` 下 rustc 自动从 sysroot 解析
//!     rustc_* crate,不需要 --extern / --sysroot / 手工定位 toolchain。

#![feature(rustc_private)]

pub mod asset;
pub mod parser;

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::asset::generated::ServerTypeId;
use crate::asset::raw_node_graph::{StructField, StructureDefinition};
use crate::asset::value::{ValueFloat, ValueGuid, ValueInt, ValueString, ValueVector};
use crate::asset::{Asset, AssetBundle, GameMode};
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
    /// 不预先执行 `cargo check` 编译依赖,直接用已有 target/debug/deps
    #[arg(long)]
    no_cargo: bool,
    /// 打印更详细的信息(函数参数、结构体字段、枚举变体等)
    #[arg(short, long)]
    verbose: bool,
    /// 示例 .gia 输出路径;缺省为 ./target/<项目名>.gia
    #[arg(short, long)]
    output: Option<PathBuf>,
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

// ---------- 依赖编译(cargo check) ----------

/// 用 `cargo check -p` 编译目标包的依赖,并从 JSON 输出收集它们的
/// rlib/rmeta 路径作为 `--extern`。
///
/// 为什么需要 --extern:cargo 的产物带 hash 且分散在各自的
/// build/<pkg>/<hash>/out 目录,rustc 无法靠 `-L dependency=` 自动匹配。
/// rustc_* crate 则由 `#![feature(rustc_private)]` 自动从 sysroot 解析,
/// 不需要收集。
///
/// `-p` 只检查目标包:目标项目可能是某个更大 workspace 的成员,不带 -p
/// 会连带检查所有成员(如 demo 连 core 一起查,而 core 依赖 rustc_driver,
/// 普通 cargo 编不了,必然失败)。
fn collect_deps(project: &Path, package_name: &str) -> Vec<(String, PathBuf)> {
    if !project.join("Cargo.toml").is_file() {
        return vec![]; // 单文件输入,无依赖
    }
    let output = match Command::new("cargo")
        .args(["check", "--quiet", "--message-format=json", "-p", package_name])
        .current_dir(project)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[warn] cannot run `cargo check`: {e}");
            return vec![];
        }
    };
    if !output.status.success() {
        eprintln!("[warn] `cargo check` failed; continuing with whatever was built already");
    }

    let mut externs: Vec<(String, PathBuf)> = vec![];
    for line in output.stdout.split(|&b| b == b'\n') {
        let line = String::from_utf8_lossy(line).trim().to_string();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if v["reason"].as_str() != Some("compiler-artifact") {
            continue;
        }
        let Some(name) = v["target"]["name"].as_str() else {
            continue;
        };
        if name == package_name {
            continue; // 根 crate 自身不传 --extern
        }
        let is_proc = v["target"]["kind"]
            .as_array()
            .is_some_and(|k| k.iter().any(|x| x.as_str() == Some("proc-macro")));
        let files = || {
            v["filenames"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|f| f.as_str())
        };
        // 分析只需要元数据:cargo check 产 rmeta;proc-macro 的代码在 dll 里
        let chosen = if is_proc {
            files().find(|f| f.ends_with(".dll") || f.ends_with(".so") || f.ends_with(".dylib"))
        } else {
            files().find(|f| f.ends_with(".rmeta") || f.ends_with(".rlib"))
        };
        if let Some(p) = chosen {
            if !externs.iter().any(|(n, _)| n == name) {
                externs.push((name.replace('-', "_"), PathBuf::from(p)));
            }
        }
    }
    externs
}

// ---------- 主流程 ----------

/// 组装传给 `rustc_driver::run_compiler` 的参数。
/// rustc_private 让 rustc 自动从 sysroot 解析 rustc_* crate;
/// 目标项目的普通依赖由 `--extern` 显式给出(来自 cargo check 的输出)。
fn build_rustc_args(info: &ProjectInfo, externs: &[(String, PathBuf)]) -> Vec<String> {
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
    for (name, path) in externs {
        args.push("--extern".to_string());
        args.push(format!("{name}={}", path.display()));
    }
    args.push(info.source_path.display().to_string());
    args
}

/// 生成一个简单的示例 .gia(AssetBundle 封装示例)。
/// 输出路径取 `--output`,缺省为 `./target/<项目名>.gia`(<项目名> 取输入的
/// 包的 name,如 `rust2genshin-demo`)。
/// 与 rustc 分析相互独立:失败只警告,不影响主流程。
fn write_example_gia(info: &ProjectInfo, output: Option<&Path>) -> std::io::Result<()> {
    let path = match output {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from("target").join(format!("{}.gia", info.package_name)),
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let bundle = AssetBundle::new(
        GameMode::Overlimit,
        vec![Asset::StructureDefinition(StructureDefinition {
            name: "Player".to_string(),
            version: 1,
            fields: vec![
                StructField {
                    name: "hp".to_string(),
                    var_type: ServerTypeId::SInt,
                    default: Some(ValueInt(100).into()),
                },
                StructField {
                    name: "name".to_string(),
                    var_type: ServerTypeId::SString,
                    default: Some(ValueString("alice".to_string()).into()),
                },
                StructField {
                    name: "pos".to_string(),
                    var_type: ServerTypeId::SVector,
                    default: Some(ValueVector(114.0, 514.0, 191.0).into()),
                },
                StructField {
                    name: "uid".to_string(),
                    var_type: ServerTypeId::SGuid,
                    default: Some(ValueGuid(46456416).into()),
                },
                StructField {
                    name: "ratio".to_string(),
                    var_type: ServerTypeId::SFloat,
                    default: Some(ValueFloat(5.145).into()),
                },
            ],
        })],
        vec![0],
    );
    bundle.save(&path)?;
    eprintln!("wrote example asset bundle to {}", path.display());
    Ok(())
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

    // 生成示例 .gia(不依赖 rustc 分析结果)
    if let Err(e) = write_example_gia(&info, args.output.as_deref()) {
        eprintln!("warning: cannot write example .gia: {e}");
    }

    // cargo check 编译目标项目依赖,收集 --extern
    let externs = if args.no_cargo {
        vec![]
    } else {
        collect_deps(&args.path, &info.package_name)
    };

    let rustc_args = build_rustc_args(&info, &externs);
    let mut callbacks = AnalysisCallbacks { verbose: args.verbose };
    rustc_driver::catch_with_exit_code(|| rustc_driver::run_compiler(&rustc_args, &mut callbacks))
}
