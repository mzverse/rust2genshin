use crate::asset::node_graph::{ControlOut, Node, NodeRef, Simulation, ValueIn};
use crate::asset::value::{AnyValue, ValueBool, ValueDefault, ValueInt, ValueIntList};
use anyhow::{Result, bail};
use crate::asset::raw_node_graph::NodeType;

pub struct NodeIf {
    pub condition: ValueIn,
    pub branch_true: ControlOut,
    pub branch_false: ControlOut,
}
impl Default for NodeIf {
    fn default() -> Self {
        Self {
            condition: ValueIn::new(ValueBool::def()),
            branch_true: vec![],
            branch_false: vec![],
        }
    }
}
impl Node for NodeIf {
    fn get_controls_in(&self) -> i32 {
        1
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.branch_true.clone(), self.branch_false.clone()]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.condition.clone()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![]
    }

    fn execute(&mut self, context: &mut Simulation) -> Result<Vec<NodeRef>> {
        let Ok(value) = self.condition.get(context)?.downcast::<ValueBool>() else {
            bail!("Condition must be bool");
        };
        if value.0 {
            Ok(self.branch_true.clone())
        } else {
            Ok(self.branch_false.clone())
        }
    }

    fn get_value(&self, _index: i32, _context: &Simulation) -> Result<AnyValue> {
        bail!("No value")
    }

    fn get_type(&self) -> NodeType {
        NodeType::simple(2)
    }

    fn get_controls_out_mut(&mut self) -> Vec<&mut ControlOut> {
        vec![&mut self.branch_true, &mut self.branch_false]
    }
}

pub struct NodeForClosed {
    body: ControlOut,
    next: ControlOut,
    begin: ValueIn,
    end: ValueIn,
    state: Option<i32>,
    state_end: i32,
    break_tag: bool,
}
impl Node for NodeForClosed {
    fn get_controls_in(&self) -> i32 {
        2
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.body.clone(), self.next.clone()]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.begin.clone(), self.end.clone()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![ValueInt::def()] // current
    }

    fn execute(&mut self, context: &mut Simulation) -> Result<Vec<NodeRef>> {
        if self.state.is_none() {
            self.state = Some(if let Ok(it) = self.begin.get(context)?.downcast::<ValueInt>() {
                it.0
            } else {
                bail!("begin must be int")
            });
            self.state_end = if let Ok(it) = self.end.get(context)?.downcast::<ValueInt>() {
                it.0
            } else {
                bail!("end must be int")
            };
            self.break_tag = false;
        }
        if self.break_tag {
            self.state = None;
            return Ok(self.next.clone());
        }
        if self.state.unwrap() > self.state_end {
            self.state = None;
            Ok(self.next.clone())
        } else {
            self.state = Some(self.state.unwrap() + 1);
            let mut result = self.body.clone();
            result.push(context.current.unwrap());
            Ok(result)
        }
    }

    fn get_value(&self, index: i32, _context: &Simulation) -> Result<AnyValue> {
        match index {
            0 => Ok(ValueInt(self.state.map(|it| it - 1).unwrap_or(0)).into()),
            _ => bail!("No value"),
        }
    }

    fn get_type(&self) -> NodeType {
        NodeType::simple(5)
    }
}

pub struct NodeBreak {
    cycle: ControlOut,
}
impl Node for NodeBreak {
    fn get_controls_in(&self) -> i32 {
        1
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.cycle.clone()]
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![]
    }

    fn execute(&mut self, context: &mut Simulation) -> Result<Vec<NodeRef>> {
        for i in &mut self.cycle {
            let target = context.get_node_mut(*i);
            if let Ok(target) = target.downcast_mut::<NodeForClosed>() {
                target.break_tag = true;
            } else { // TODO
                bail!("Target is not loop");
            }
        }
        Ok(vec![])
    }

    fn get_value(&self, _index: i32, _context: &Simulation) -> Result<AnyValue> {
        bail!("No value")
    }

    fn verify(&self, context: &Simulation) -> Result<()> {
        for i in &self.cycle {
            let target = context.get_node(*i);
            if target.get_controls_in() != 2 {
                bail!("Target is not loop");
            }
        }
        Ok(())
    }

    fn get_type(&self) -> NodeType {
        NodeType::simple(6)
    }
}

/// 多分支选择(Control.General.Switch,ID 3)
///
/// `key` 匹配 `cases` 列表中的某一项,跳转到对应的 `case_branches` 分支;
/// 无匹配时走 `default_branch`。分支数量随 cases 动态变化,不做穷举。
pub struct NodeSwitch {
    /// 匹配键(Int 或 Str)
    key: ValueIn,
    /// 候选值列表(与 case_branches 一一对应)
    cases: ValueIn,
    /// 默认分支(无匹配时)
    default_branch: ControlOut,
    /// 各 case 分支(数量与 cases 动态对应)
    case_branches: Vec<ControlOut>,
}
impl Default for NodeSwitch {
    fn default() -> Self {
        Self {
            key: ValueIn::new(ValueInt::def()),
            cases: ValueIn::new(ValueIntList::def()),
            default_branch: vec![],
            case_branches: vec![],
        }
    }
}
impl NodeSwitch {
    /// 设置默认分支(无匹配时)
    pub fn set_default(&mut self, branch: ControlOut) {
        self.default_branch = branch;
    }
    /// 添加一个 case 分支(与 cases 列表中的一项对应)
    pub fn add_case(&mut self, branch: ControlOut) {
        self.case_branches.push(branch);
    }
}
impl Node for NodeSwitch {
    fn get_controls_in(&self) -> i32 {
        1
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        let mut v = vec![self.default_branch.clone()];
        v.extend(self.case_branches.iter().cloned());
        v
    }
    fn get_values_in(&self) -> Vec<ValueIn> {
        vec![self.key.clone(), self.cases.clone()]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![]
    }

    fn execute(&mut self, context: &mut Simulation) -> Result<Vec<NodeRef>> {
        let Ok(key) = self.key.get(context)?.downcast::<ValueInt>() else {
            bail!("Switch key must be Int")
        };
        let cases = self.cases.get(context)?;
        if let Ok(list) = cases.downcast::<ValueIntList>() {
            if let Some(i) = list.0.iter().position(|&x| x == key.0) {
                if let Some(branch) = self.case_branches.get(i) {
                    return Ok(branch.clone());
                }
            }
        }
        Ok(self.default_branch.clone())
    }

    fn get_value(&self, _index: i32, _context: &Simulation) -> Result<AnyValue> {
        bail!("No value")
    }

    fn get_type(&self) -> NodeType {
        NodeType::simple(3)
    }
}
