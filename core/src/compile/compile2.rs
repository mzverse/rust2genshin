use std::any::Any;
use std::path::PathBuf;
use std::time::Instant;
use rustc_ast::expand::allocator::AllocatorMethod;
use rustc_codegen_ssa::{CompiledModule, CompiledModules, CrateInfo, ModuleCodegen};
use rustc_codegen_ssa::back::lto::ThinModule;
use rustc_codegen_ssa::back::write::{CodegenContext, FatLtoInput, ModuleConfig, SharedEmitter, TargetMachineFactoryFn, ThinLtoInput};
use rustc_codegen_ssa::traits::{CodegenBackend, ExtraBackendMethods, ModuleBufferMethods, WriteBackendMethods};
use rustc_errors::DiagCtxtHandle;
use rustc_metadata::EncodedMetadata;
use rustc_middle::dep_graph::{WorkProduct, WorkProductMap};
use rustc_middle::ty::TyCtxt;
use rustc_session::{IncrCompSession, Session};
use rustc_session::config::{OptLevel, OutputFilenames};
use rustc_span::Symbol;

#[derive(Clone)]
pub struct R2gCodegenBackend;

impl CodegenBackend for R2gCodegenBackend {
    fn name(&self) -> &'static str {
        "rust2genshin"
    }

    fn target_cpu(&self, sess: &Session) -> String {
        todo!()
    }

    fn codegen_crate<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Box<dyn Any> {
        Box::new(rustc_codegen_ssa::base::codegen_crate(Self, tcx))
    }

    fn join_codegen(&self, ongoing_codegen: Box<dyn Any>, sess: &Session, incr_comp_session: Option<&IncrCompSession>, outputs: &OutputFilenames, crate_info: &CrateInfo) -> (CompiledModules, WorkProductMap) {
        todo!()
    }

    fn link(&self, sess: &Session, compiled_modules: CompiledModules, crate_info: CrateInfo, metadata: EncodedMetadata, outputs: &OutputFilenames) {
    }
}

impl ExtraBackendMethods for R2gCodegenBackend {
    type Module = ();

    fn codegen_allocator<'tcx>(&self, tcx: TyCtxt<'tcx>, module_name: &str, methods: &[AllocatorMethod]) -> Self::Module {
        todo!()
    }

    fn compile_codegen_unit(&self, tcx: TyCtxt<'_>, cgu_name: Symbol) -> (ModuleCodegen<Self::Module>, u64) {
        let start_time = Instant::now();



        let time_to_codegen = start_time.elapsed();
        let cost = time_to_codegen.as_nanos() as u64;
        (todo!(), cost)
    }
}

impl WriteBackendMethods for R2gCodegenBackend {
    type Module = ();
    type TargetMachine = ();
    type ModuleBuffer = ModuleBuffer;
    type ThinData = ();

    fn target_machine_factory(&self, sess: &Session, opt_level: OptLevel, target_features: &[String]) -> TargetMachineFactoryFn<Self> {
        todo!()
    }

    fn optimize_and_codegen_fat_lto(sess: &Session, cgcx: &CodegenContext, shared_emitter: &SharedEmitter, tm_factory: TargetMachineFactoryFn<Self>, exported_symbols_for_lto: &[String], each_linked_rlib_for_lto: &[PathBuf], modules: Vec<FatLtoInput<Self>>) -> CompiledModule {
        todo!()
    }

    fn run_thin_lto(cgcx: &CodegenContext, prof: &rustc_data_structures::profiling::SelfProfilerRef, dcx: DiagCtxtHandle<'_>, exported_symbols_for_lto: &[String], each_linked_rlib_for_lto: &[PathBuf], modules: Vec<ThinLtoInput<Self>>) -> (Vec<ThinModule<Self>>, Vec<WorkProduct>) {
        todo!()
    }

    fn optimize(cgcx: &CodegenContext, prof: &rustc_data_structures::profiling::SelfProfilerRef, shared_emitter: &SharedEmitter, module: &mut ModuleCodegen<Self::Module>, config: &ModuleConfig) {
        todo!()
    }

    fn optimize_and_codegen_thin(cgcx: &CodegenContext, prof: &rustc_data_structures::profiling::SelfProfilerRef, shared_emitter: &SharedEmitter, tm_factory: TargetMachineFactoryFn<Self>, thin: ThinModule<Self>) -> CompiledModule {
        todo!()
    }

    fn codegen(cgcx: &CodegenContext, prof: &rustc_data_structures::profiling::SelfProfilerRef, shared_emitter: &SharedEmitter, module: ModuleCodegen<Self::Module>, config: &ModuleConfig) -> CompiledModule {
        todo!()
    }

    fn serialize_module(module: Self::Module, is_thin: bool) -> Self::ModuleBuffer {
        todo!()
    }
}

pub struct ModuleBuffer;
impl ModuleBufferMethods for ModuleBuffer {
    fn data(&self) -> &[u8] {
        todo!()
    }
}

// impl CodegenMethods for

