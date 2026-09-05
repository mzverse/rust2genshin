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

    fn target_cpu(&self, _sess: &Session) -> String {
        todo!()
    }

    fn codegen_crate<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Box<dyn Any> {
        Box::new(rustc_codegen_ssa::base::codegen_crate(Self, tcx))
    }

    fn join_codegen(&self, _ongoing_codegen: Box<dyn Any>, _sess: &Session, _incr_comp_session: Option<&IncrCompSession>, _outputs: &OutputFilenames, _crate_info: &CrateInfo) -> (CompiledModules, WorkProductMap) {
        todo!()
    }

    fn link(&self, _sess: &Session, _compiled_modules: CompiledModules, _crate_info: CrateInfo, _metadata: EncodedMetadata, _outputs: &OutputFilenames) {
    }
}

impl ExtraBackendMethods for R2gCodegenBackend {
    type Module = ();

    fn codegen_allocator<'tcx>(&self, _tcx: TyCtxt<'tcx>, _module_name: &str, _methods: &[AllocatorMethod]) -> Self::Module {
        todo!()
    }

    fn compile_codegen_unit(&self, _tcx: TyCtxt<'_>, _cgu_name: Symbol) -> (ModuleCodegen<Self::Module>, u64) {
        let start_time = Instant::now();



        let _time_to_codegen = start_time.elapsed();
        // The `0` is unreachable since `todo!()` diverges; clippy flags it.
        // We need *some* value of type `u64` here so the function compiles, and `0` is
        // the natural placeholder. Suppress the lint at the expression site.
        #[allow(unreachable_code)]
        (todo!(), 0)
    }
}

impl WriteBackendMethods for R2gCodegenBackend {
    type Module = ();
    type TargetMachine = ();
    type ModuleBuffer = ModuleBuffer;
    type ThinData = ();

    fn target_machine_factory(&self, _sess: &Session, _opt_level: OptLevel, _target_features: &[String]) -> TargetMachineFactoryFn<Self> {
        todo!()
    }

    fn optimize_and_codegen_fat_lto(_sess: &Session, _cgcx: &CodegenContext, _shared_emitter: &SharedEmitter, _tm_factory: TargetMachineFactoryFn<Self>, _exported_symbols_for_lto: &[String], _each_linked_rlib_for_lto: &[PathBuf], _modules: Vec<FatLtoInput<Self>>) -> CompiledModule {
        todo!()
    }

    fn run_thin_lto(_cgcx: &CodegenContext, _prof: &rustc_data_structures::profiling::SelfProfilerRef, _dcx: DiagCtxtHandle<'_>, _exported_symbols_for_lto: &[String], _each_linked_rlib_for_lto: &[PathBuf], _modules: Vec<ThinLtoInput<Self>>) -> (Vec<ThinModule<Self>>, Vec<WorkProduct>) {
        todo!()
    }

    fn optimize(_cgcx: &CodegenContext, _prof: &rustc_data_structures::profiling::SelfProfilerRef, _shared_emitter: &SharedEmitter, _module: &mut ModuleCodegen<Self::Module>, _config: &ModuleConfig) {
        todo!()
    }

    fn optimize_and_codegen_thin(_cgcx: &CodegenContext, _prof: &rustc_data_structures::profiling::SelfProfilerRef, _shared_emitter: &SharedEmitter, _tm_factory: TargetMachineFactoryFn<Self>, _thin: ThinModule<Self>) -> CompiledModule {
        todo!()
    }

    fn codegen(_cgcx: &CodegenContext, _prof: &rustc_data_structures::profiling::SelfProfilerRef, _shared_emitter: &SharedEmitter, _module: ModuleCodegen<Self::Module>, _config: &ModuleConfig) -> CompiledModule {
        todo!()
    }

    fn serialize_module(_module: Self::Module, _is_thin: bool) -> Self::ModuleBuffer {
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

