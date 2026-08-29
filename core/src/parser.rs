//! 遍历目标 crate 的 HIR(rustc_hir)并输出结构。
//!
//! 作为编译器后端,输出不再走 stdout(cargo 会解析 rustc 的 stdout JSON),
//! 而是由 `after_analysis` / `codegen_crate` 回调把文本写入导出目录。

use rustc_hir as hir;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::LOCAL_CRATE;
use rustc_span::Span;
use std::fmt::Write as _;
use std::path::Path;

/// 在 `after_analysis` 回调中调用:遍历整个 crate 的 HIR,返回文本。
pub fn dump_hir_to_string<'tcx>(tcx: TyCtxt<'tcx>, verbose: bool) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "== HIR dump of crate `{}` ==", tcx.crate_name(LOCAL_CRATE));
    let mut visitor = Dumper { tcx, depth: 0, verbose, out: &mut out };
    tcx.hir_walk_toplevel_module(&mut visitor);
    let _ = writeln!(out, "== end ==");
    out
}

/// 把 HIR dump 写入文件(编译后端产物之一)。
pub fn dump_hir_to_file<'tcx>(tcx: TyCtxt<'tcx>, verbose: bool, path: &Path) -> std::io::Result<()> {
    std::fs::write(path, dump_hir_to_string(tcx, verbose))
}

/// 收集整个 crate 的入口点:`#[event_listener]` 标注的 pub fn,返回文本,每行一条。
pub fn collect_entry_points_to_string<'tcx>(tcx: TyCtxt<'tcx>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "== entry points of crate `{}` ==", tcx.crate_name(LOCAL_CRATE));
    let mut visitor = EntryPointCollector { tcx, out: &mut out };
    tcx.hir_walk_toplevel_module(&mut visitor);
    let _ = writeln!(out, "== end ==");
    out
}

/// 把入口点列表写入文件(编译后端产物之一)。
pub fn collect_entry_points_to_file<'tcx>(tcx: TyCtxt<'tcx>, path: &Path) -> std::io::Result<()> {
    std::fs::write(path, collect_entry_points_to_string(tcx))
}

/// 收集入口点的 def_id(编译入口:从这些函数开始遍历调用图)。
pub fn collect_entry_point_def_ids<'tcx>(tcx: TyCtxt<'tcx>) -> Vec<rustc_span::def_id::LocalDefId> {
    struct Collector<'tcx> {
        tcx: TyCtxt<'tcx>,
        out: Vec<rustc_span::def_id::LocalDefId>,
    }
    impl<'tcx> Visitor<'tcx> for Collector<'tcx> {
        type NestedFilter = nested_filter::All;
        fn maybe_tcx(&mut self) -> TyCtxt<'tcx> {
            self.tcx
        }
        fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) {
            if let hir::ItemKind::Fn { .. } = &item.kind
                && is_event_listener(item.span, self.tcx)
            {
                self.out.push(item.owner_id.def_id);
            }
            intravisit::walk_item(self, item);
        }
    }
    let mut c = Collector { tcx, out: Vec::new() };
    tcx.hir_walk_toplevel_module(&mut c);
    c.out
}

struct EntryPointCollector<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    out: &'a mut String,
}

impl<'a, 'tcx> EntryPointCollector<'a, 'tcx> {
    fn push_entry(&mut self, name: &str, sig: &hir::FnSig<'tcx>, span: Span) {
        let params = sig
            .decl
            .inputs
            .iter()
            .map(|ty| self.ty_str(ty))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            self.out,
            "#[event_listener] fn {name}({params}) -> {} @ {}",
            self.ret_str(&sig.decl.output),
            self.tcx.sess.source_map().span_to_diagnostic_string(span),
        );
    }

    /// 类型取源码文本(比 `{:?}` 的 Ty 内部结构可读得多)
    fn ty_str(&self, ty: &hir::Ty<'tcx>) -> String {
        self.tcx
            .sess
            .source_map()
            .span_to_snippet(ty.span)
            .unwrap_or_else(|_| format!("{ty:?}"))
    }

    fn ret_str(&self, ret: &hir::FnRetTy<'tcx>) -> String {
        match ret {
            hir::FnRetTy::DefaultReturn(_) => "()".to_string(),
            hir::FnRetTy::Return(ty) => self.ty_str(ty),
        }
    }
}

/// 通过展开上下文识别 `#[event_listener]` 宏产生的函数:
/// 属性本身在展开时被宏消费(hir_attrs 里没有),但宏用标准写法(syn/quote)
/// 重新生成函数头 token(call_site span),item span 因此带有宏展开上下文,
/// 即 `ExpnKind::Macro(MacroKind::Attr, "event_listener")`。
fn is_event_listener(span: Span, tcx: TyCtxt<'_>) -> bool {
    use rustc_span::hygiene::{ExpnKind, MacroKind};
    use rustc_span::Symbol;
    matches!(
        span.ctxt().outer_expn().expn_data().kind,
        ExpnKind::Macro(MacroKind::Attr, name) if name == Symbol::intern("event_listener")
    )
}

impl<'a, 'tcx> Visitor<'tcx> for EntryPointCollector<'a, 'tcx> {
    // 嵌套 item(mod 里的 fn 等)也进入遍历
    type NestedFilter = nested_filter::All;

    fn maybe_tcx(&mut self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) {
        if let hir::ItemKind::Fn { ident, sig, .. } = &item.kind
            && is_event_listener(item.span, self.tcx)
        {
            self.push_entry(&ident.to_string(), sig, item.span);
        }
        intravisit::walk_item(self, item);
    }
}

struct Dumper<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    depth: usize, // 词法嵌套深度,用于缩进
    verbose: bool,
    out: &'a mut String,
}

impl<'a, 'tcx> Dumper<'a, 'tcx> {
    fn indent(&self) -> String {
        "  ".repeat(self.depth)
    }
    fn span_str(&self, span: Span) -> String {
        self.tcx.sess.source_map().span_to_diagnostic_string(span)
    }
    /// verbose 模式:按 ItemKind 输出补充细节
    fn print_details(&mut self, item: &hir::Item<'tcx>) {
        match &item.kind {
            hir::ItemKind::Fn { sig, .. } => {
                let _ = writeln!(
                    self.out,
                    "{}  params: {} -> {}",
                    self.indent(),
                    sig.decl.inputs.len(),
                    fn_ret(&sig.decl.output)
                );
            }
            hir::ItemKind::Struct(_, _, vdata) | hir::ItemKind::Union(_, _, vdata) => {
                for field in vdata.fields() {
                    let _ = writeln!(self.out, "{}  field {}: {:?}", self.indent(), field.ident, field.ty);
                }
            }
            hir::ItemKind::Enum(_, _, def) => {
                for variant in def.variants {
                    let _ = writeln!(self.out, "{}  variant {}", self.indent(), variant.ident);
                }
            }
            hir::ItemKind::Impl(imp) => {
                if let Some(t) = &imp.of_trait {
                    let _ = writeln!(self.out, "{}  of_trait: {:?}", self.indent(), t.trait_ref);
                }
                let _ = writeln!(self.out, "{}  self_ty: {:?}", self.indent(), imp.self_ty);
            }
            hir::ItemKind::Trait { items, .. } => {
                let _ = writeln!(self.out, "{}  trait items: {}", self.indent(), items.len());
            }
            _ => {}
        }
    }
}

impl<'a, 'tcx> Visitor<'tcx> for Dumper<'a, 'tcx> {
    // 嵌套 item(mod 里的 struct、impl 里的方法等)也进入遍历,同时保留词法嵌套
    type NestedFilter = nested_filter::All;

    fn maybe_tcx(&mut self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) {
        // 跳过编译器注入的合成 item(如 extern crate std),它们没有源码位置
        if self.span_str(item.span) == "no-location" {
            return;
        }
        let name = item.kind.ident().map(|i| i.to_string()).unwrap_or_default();
        let _ = writeln!(
            self.out,
            "{}{} {} @ {}",
            self.indent(),
            kind_name(&item.kind),
            name,
            self.span_str(item.span)
        );
        if self.verbose {
            self.print_details(item);
        }
        self.depth += 1;
        intravisit::walk_item(self, item);
        self.depth -= 1;
    }
}

/// ItemKind 的可读名称(1.100 已移除 `ItemKind::descr`,这里显式映射)
fn kind_name(kind: &hir::ItemKind<'_>) -> &'static str {
    match kind {
        hir::ItemKind::ExternCrate(..) => "extern_crate",
        hir::ItemKind::Use(..) => "use",
        hir::ItemKind::Static(..) => "static",
        hir::ItemKind::Const(..) => "const",
        hir::ItemKind::Fn { .. } => "fn",
        hir::ItemKind::Macro(..) => "macro",
        hir::ItemKind::Mod(..) => "mod",
        hir::ItemKind::ForeignMod { .. } => "extern",
        hir::ItemKind::GlobalAsm { .. } => "global_asm",
        hir::ItemKind::TyAlias(..) => "type",
        hir::ItemKind::Enum(..) => "enum",
        hir::ItemKind::Struct(..) => "struct",
        hir::ItemKind::Union(..) => "union",
        hir::ItemKind::Trait { .. } => "trait",
        hir::ItemKind::TraitAlias(..) => "trait_alias",
        hir::ItemKind::Impl(..) => "impl",
        hir::ItemKind::TestBinderConstraints { .. } => "test_binder_constraints", // 新 nightly 新增
    }
}

fn fn_ret(ret: &hir::FnRetTy<'_>) -> String {
    match ret {
        hir::FnRetTy::DefaultReturn(_) => "()".to_string(),
        hir::FnRetTy::Return(ty) => format!("{ty:?}"),
    }
}
