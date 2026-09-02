use std::sync::LazyLock;
use crate::asset::node_graph::NodeKind;
use crate::asset::value::{AnyValue, ValueBool, ValueDefault, ValueInt, ValueIntList, ValueString, ValueStringList};

/// 条件分支(ID 2):condition(Bool) → 2 个流出分支(true/false)
pub static NODE_IF: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(2, 1, 2, vec![ValueBool::def()], vec![])
});

/// 闭区间循环(ID 5):begin/end(Int) → body/next 两个流出;输出当前值(Int)
pub static NODE_FOR_CLOSED: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(5, 2, 2, vec![ValueInt::def(), ValueInt::def()], vec![ValueInt::def()])
});

/// 跳出循环(ID 6):1 个流出(cycle)
pub static NODE_BREAK: LazyLock<NodeKind> = LazyLock::new(|| {
    NodeKind::new(6, 1, 1, vec![], vec![])
});

/// 多分支选择(ID 3,泛型变体):shell 固定 3,kernel 随类型(Int→3、Str→4)。
/// key 匹配 cases 列表中的一项,跳转到对应分支;无匹配走 default。
/// 分支数量随 cases 动态变化;此处 controls_out 按 1 个 default 分支声明,
/// 编译期按需扩展。
pub fn node_switch(ty: AnyValue, cases: usize) -> NodeKind {
    let mut result = NodeKind::new(3, 1, 1 + cases, vec![ty.clone(), ty.clone()], vec![]);
    let selected;
    if ty.is::<ValueInt>() {
        result.kernel_id = 3;
        selected = 0;
        result.values_in_types[1] = ValueIntList::def();
    } else if ty.is::<ValueString>() {
        result.kernel_id = 4;
        selected = 1;
        result.values_in_types[1] = ValueStringList::def();
    } else {
        panic!("Unsupported type: {ty:?}");
    }
    result.selectors_in[0] = selected.into();
    result.selectors_in[1] = selected.into();
    result
}
