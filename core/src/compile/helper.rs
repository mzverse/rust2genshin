use rustc_attr_ir::LangItem;
use rustc_errors::MultiSpan;
use rustc_hir::def::DefKind;
use rustc_middle::ty::{EarlyBinder, FnSig, GenSig, Instance, Mutability, Ty, TyCtxt, TyKind, TypingEnv, Unnormalized};
use rustc_span::DUMMY_SP;

#[derive(Clone, Copy)]
pub struct Helper<'tcx>(pub TyCtxt<'tcx>);

impl<'tcx> Helper<'tcx> {
    pub fn err<T>(self, msg: impl Into<rustc_errors::DiagMessage>) -> crate::compile::Result<T> {
        Err(self.0.dcx().err(msg.into()))
    }

    pub fn span_err<T>(self, span: impl Into<MultiSpan>, msg: impl Into<rustc_errors::DiagMessage>) -> crate::compile::Result<T> {
        Err(self.0.dcx().span_err(span, msg.into()))
    }

    pub fn span_fatal(self, span: impl Into<MultiSpan>, msg: impl Into<rustc_errors::DiagMessage>) -> ! {
        self.0.dcx().span_fatal(span, msg)
    }

    pub fn monomorphize(self, func: Instance<'tcx>, ty: Ty<'tcx>) -> Ty<'tcx> {
        func.instantiate_mir_and_normalize_erasing_regions(self.0, TypingEnv::fully_monomorphized(), EarlyBinder::bind(self.0, ty))
    }

    pub fn fn_sig(self, func: Instance<'tcx>) -> FnSig<'tcx> {
        fn co_state<'tcx>(tcx: TyCtxt<'tcx>, cor: GenSig<TyCtxt<'tcx>>) -> Ty<'tcx> {
            Ty::new_adt(tcx, tcx.adt_def(tcx.require_lang_item(LangItem::CoroutineState, DUMMY_SP)), tcx.mk_args(&[cor.yield_ty.into(), cor.return_ty.into()]))
        }
        let sig = match self.0.def_kind(func.def_id()) {
            DefKind::Closure => Unnormalized::new({
                let ty = func.instantiate_mir_and_normalize_erasing_regions(self.0, TypingEnv::fully_monomorphized(), self.0.type_of(func.def_id()));
                match ty.kind() {
                    TyKind::Closure(..) => func.args.as_closure().sig(),
                    TyKind::Coroutine(..) => {
                        let sig = func.args.as_coroutine().sig();
                        return FnSig { inputs_and_output: self.0.mk_type_list(&[Ty::new_pinned_ref(self.0, self.0.lifetimes.re_erased, ty, Mutability::Mut), sig.resume_ty, co_state(self.0, sig)]), fn_sig_kind: Default::default() }
                    },
                    other => panic!("{other:?}"),
                }
            }),
            _ => self.0.fn_sig(func.def_id()).instantiate(self.0, func.args),
        };
        let sig = self.0.normalize_erasing_regions(TypingEnv::fully_monomorphized(), sig);
        self.0.normalize_erasing_late_bound_regions(TypingEnv::fully_monomorphized(), sig)
    }

    pub fn fn_params(self, func: Instance<'tcx>, sig: FnSig<'tcx>) -> &'tcx [Ty<'tcx>] {
        let result = sig.inputs();
        if matches!(self.0.def_kind(func.def_id()), DefKind::Closure) && let ty = func.instantiate_mir_and_normalize_erasing_regions(self.0, TypingEnv::fully_monomorphized(), self.0.type_of(func.def_id())) && matches!(ty.kind(), TyKind::Closure(..)) {
            let mut vec = vec![ty];
            vec.extend_from_slice(result);
            self.0.mk_type_list(&vec)
        } else {
            result
        }
    }
}
