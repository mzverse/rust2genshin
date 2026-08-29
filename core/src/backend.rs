//! rust2genshin 的 codegen 后端实现(本包 `rust2genshin` 自身就是后端)。
//!
//! 用 `rustc -Zcodegen-backend=<本 dll 路径>` 加载:rustc 通过
//! `__rustc_codegen_backend` 符号拿到 `Box<dyn CodegenBackend>`,在
//! `codegen_crate(tcx)` 里(analysis 之后)导出 `.gia` + extern 方法列表。
//!
//! 本后端不产生真实机器码(没有 LLVM 可委托):`codegen_crate` 返回空
//! `CompiledModules`,`link` 走 rustc 共享的 `link_binary`,产物是
//! **仅含 rmeta 的 rlib**(与 rustc 自带的 `DummyCodegenBackend` 同款行为)。
//! 因此:
//! - `cargo build`(需要 codegen)才会触发导出;`cargo check` 不会调用后端;
//! - 依赖 proc-macro / 需要链接可执行文件的 crate 会失败(建议只对目标叶 crate
//!   用 `cargo rustc -p <pkg> -- -Zcodegen-backend=<path>`,依赖照常由 LLVM 编译)。
//!
//! 链接说明:rustup 工具链里 rustc_* crate 只发布 rmeta(代码都在
//! `rustc_driver.dll` 里),所以本 crate 必须 `extern crate rustc_driver`,
//! 其余 rustc crate 的符号通过该 dylib 解析(与 miri 同款做法);
//! 运行时 rustc 进程已加载该 dll,加载本后端时会复用。

use rustc_codegen_ssa::back::archive::ArArchiveBuilderBuilder;
use rustc_codegen_ssa::back::link::link_binary;
use rustc_codegen_ssa::target_features::internal_target_features;
use rustc_codegen_ssa::traits::CodegenBackend;
use rustc_codegen_ssa::{CompiledModules, CrateInfo, TargetConfig};
use rustc_metadata::EncodedMetadata;
use rustc_middle::dep_graph::WorkProductMap;
use rustc_middle::ty::TyCtxt;
use rustc_session::config::OutputFilenames;
use rustc_session::{IncrCompSession, Session};
use rustc_structures::CrateType;
use std::any::Any;
use crate::compile;

pub struct R2gCodegenBackend;

impl CodegenBackend for R2gCodegenBackend {
    fn name(&self) -> &'static str {
        "rust2genshin"
    }

    fn target_config(&self, sess: &Session) -> TargetConfig {
        // 与 dummy 后端一致:把 ABI 必需特性填进 internal_target_features,
        // 否则前端会警告 x87/sse2 等目标特性未启用
        let abi_required_features = sess.target.abi_required_features();
        let internal_target_features = internal_target_features::<0>(
            sess,
            |_feature| Default::default(),
            |feature| abi_required_features.required.contains(&feature),
        );
        TargetConfig {
            internal_target_features,
            has_reliable_f16: true,
            has_reliable_f16_math: true,
            has_reliable_f128: true,
            has_reliable_f128_math: true,
        }
    }

    fn supported_crate_types(&self, _sess: &Session) -> Vec<CrateType> {
        // 与 dummy 后端一致:只认 rlib/可执行;库类 crate cargo 会退化为 rlib,
        // 可执行保留前端处理但会在 link 步报错(本后端不产机器码)。
        vec![CrateType::Rlib, CrateType::Executable]
    }

    fn target_cpu(&self, _sess: &Session) -> String {
        String::new()
    }

    fn codegen_crate<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Box<dyn Any> {
        // 编译进程内的后端钩子:analysis 已完成,HIR 可用 → 导出 .gia + extern 方法列表。
        // (proc-macro / #[test] 过滤在 export 内部处理)
        // crate::export::maybe_export_crate(tcx, &cfg);
        _ = compile::compile(tcx);
        Box::new(CompiledModules { modules: vec![], allocator_module: None })
    }

    fn join_codegen(
        &self,
        ongoing_codegen: Box<dyn Any>,
        _sess: &Session,
        _incr_comp_session: Option<&IncrCompSession>,
        _outputs: &OutputFilenames,
        _crate_info: &CrateInfo,
    ) -> (CompiledModules, WorkProductMap) {
        (*ongoing_codegen.downcast().unwrap(), WorkProductMap::default())
    }

    fn link(
        &self,
        sess: &Session,
        compiled_modules: CompiledModules,
        crate_info: CrateInfo,
        metadata: EncodedMetadata,
        outputs: &OutputFilenames,
    ) {
        // 非 rlib 且需要链接的 crate 类型(dummy 后端同款行为):给出明确报错
        if let Some(&crate_type) =
            crate_info.crate_types.iter().find(|&&crate_type| crate_type != CrateType::Rlib)
            && outputs.outputs.should_link()
        {
            sess.dcx().fatal(format!(
                "crate type {crate_type} not supported by the rust2genshin codegen backend \
                 (it does not produce machine code)"
            ));
        }

        link_binary(
            sess,
            &ArArchiveBuilderBuilder,
            compiled_modules,
            crate_info,
            metadata,
            outputs,
            self.name(),
        );
    }
}

/// rustc 加载 codegen 后端的入口符号(见 rustc_interface::util::get_codegen_backend)。
#[unsafe(no_mangle)]
pub fn __rustc_codegen_backend() -> Box<dyn CodegenBackend> {
    Box::new(R2gCodegenBackend)
}
