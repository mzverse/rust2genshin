use crate::node::NodeKind;
use crate::value::NativeKind::{self, *};
use std::sync::LazyLock;

/// 条件分支(ID 2):condition(Bool) → 2 个流出分支(true/false)
pub static NODE_IF: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(2, 1, 2, vec![Bool], vec![])
});

/// 闭区间循环(ID 5):begin/end(Int) → body/next 两个流出;输出当前值(Int)
pub static NODE_FOR_CLOSED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(5, 2, 2, vec![Int, Int], vec![Int])
});

/// 跳出循环(ID 6):1 个流出(cycle)
pub static NODE_BREAK: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::simple(6, 1, 1, vec![], vec![])
});

/// 多分支选择(ID 3,泛型变体):shell 固定 3,kernel 随类型(Int→3、Str→4)。
/// key 匹配 cases 列表中的一项,跳转到对应分支;无匹配走 default。
/// 分支数量随 cases 动态变化;此处 controls_out 按 1 个 default 分支声明,
/// 编译期按需扩展。
pub fn node_switch(kind: NativeKind, cases: usize) -> NodeKind {
    assert!(cases <= 10);
    let (kernel_id, selected) = match kind {
        Int => (3, 0),
        String => (4, 1),
        _ => panic!("Unsupported type for switch: {kind:?}"),
    };
    let mut result = NodeKind::simple(3, 1, 1 + cases, vec![kind.clone(), List(Box::new(kind))], vec![]);
    result.id.kernel = kernel_id;
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}
