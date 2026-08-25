use crate::asset::node_graph::{ControlOut, INode, NodeRef, Simulation, ValueIn};
use crate::asset::value::{AnyValue, ValueBool, ValueDefault, ValueInt};
use anyhow::{Result, bail};

pub struct NodeIf {
    condition: ValueIn,
    branch_true: ControlOut,
    branch_false: ControlOut,
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
impl INode for NodeIf {
    fn get_controls_in(&self) -> u32 {
        1
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.branch_true.clone(), self.branch_false.clone()]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.condition]
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

    fn get_value(&self, _index: u32, _context: &Simulation) -> Result<AnyValue> {
        bail!("No value")
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
impl INode for NodeForClosed {
    fn get_controls_in(&self) -> u32 {
        2
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.body.clone(), self.next.clone()]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.begin, &self.end]
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

    fn get_value(&self, index: u32, _context: &Simulation) -> Result<AnyValue> {
        match index {
            0 => Ok(ValueInt(self.state.map(|it| it - 1).unwrap_or(0)).into()),
            _ => bail!("No value"),
        }
    }
}

pub struct NodeBreak {
    cycle: ControlOut,
}
impl INode for NodeBreak {
    fn get_controls_in(&self) -> u32 {
        1
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.cycle.clone()]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
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

    fn get_value(&self, _index: u32, _context: &Simulation) -> Result<AnyValue> {
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
}
