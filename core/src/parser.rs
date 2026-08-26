//! 遍历目标 crate 的 HIR(rustc_hir)并打印结构。

use rustc_hir as hir;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::LOCAL_CRATE;
use rustc_span::Span;

/// 在 `after_analysis` 回调中调用:遍历整个 crate 的 HIR 并打印。
pub fn dump_hir<'tcx>(tcx: TyCtxt<'tcx>, verbose: bool) {
    println!("== HIR dump of crate `{}` ==", tcx.crate_name(LOCAL_CRATE));
    let mut visitor = Dumper { tcx, depth: 0, verbose };
    tcx.hir_walk_toplevel_module(&mut visitor);
    println!("== end ==");
}

struct Dumper<'tcx> {
    tcx: TyCtxt<'tcx>,
    depth: usize, // 词法嵌套深度,用于缩进
    verbose: bool,
}

impl<'tcx> Dumper<'tcx> {
    fn indent(&self) -> String {
        "  ".repeat(self.depth)
    }
    fn span_str(&self, span: Span) -> String {
        self.tcx.sess.source_map().span_to_diagnostic_string(span)
    }
    /// verbose 模式:按 ItemKind 输出补充细节
    fn print_details(&self, item: &hir::Item<'tcx>) {
        match &item.kind {
            hir::ItemKind::Fn { sig, .. } => {
                println!(
                    "{}  params: {} -> {}",
                    self.indent(),
                    sig.decl.inputs.len(),
                    fn_ret(&sig.decl.output)
                );
            }
            hir::ItemKind::Struct(_, _, vdata) | hir::ItemKind::Union(_, _, vdata) => {
                for field in vdata.fields() {
                    println!("{}  field {}: {:?}", self.indent(), field.ident, field.ty);
                }
            }
            hir::ItemKind::Enum(_, _, def) => {
                for variant in def.variants {
                    println!("{}  variant {}", self.indent(), variant.ident);
                }
            }
            hir::ItemKind::Impl(imp) => {
                if let Some(t) = &imp.of_trait {
                    println!("{}  of_trait: {:?}", self.indent(), t.trait_ref);
                }
                println!("{}  self_ty: {:?}", self.indent(), imp.self_ty);
            }
            hir::ItemKind::Trait { items, .. } => {
                println!("{}  trait items: {}", self.indent(), items.len());
            }
            _ => {}
        }
    }
}

impl<'tcx> Visitor<'tcx> for Dumper<'tcx> {
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
        println!(
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
