use rustc_errors::MultiSpan;
use rustc_hir::def::DefKind;
use rustc_middle::ty::{EarlyBinder, FnSig, Instance, Ty, TyCtxt, TypingEnv, Unnormalized};

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
        let sig = match self.0.def_kind(func.def_id()) {
            DefKind::Closure => Unnormalized::new(func.args.as_closure().sig()),
            _ => self.0.fn_sig(func.def_id()).instantiate(self.0, func.args),
        };
        let sig = self.0.normalize_erasing_regions(TypingEnv::fully_monomorphized(), sig);
        self.0.normalize_erasing_late_bound_regions(TypingEnv::fully_monomorphized(), sig)
    }

    pub fn fn_params(self, func: Instance<'tcx>, sig: FnSig<'tcx>) -> &'tcx [Ty<'tcx>] {
        let result = sig.inputs();
        if matches!(self.0.def_kind(func.def_id()), DefKind::Closure) {
            let mut vec = vec![Ty::new_closure(self.0, func.def_id(), func.args)];
            vec.extend_from_slice(result);
            self.0.mk_type_list(&vec)
        } else {
            result
        }
    }
}
