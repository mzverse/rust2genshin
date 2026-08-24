use crate::asset::node_graph::{ControlOut, INode, NodeRef, Simulation, Value, ValueIn};
use anyhow::{bail, Result};

pub struct NodeIf {
    condition: ValueIn,
    branch_true: ControlOut,
    branch_false: ControlOut,
}
impl Default for NodeIf {
    fn default() -> Self {
        Self {
            condition: ValueIn::new(Value::Bool(false)),
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
    fn get_values_out(&self) -> Vec<Value> {
        vec![]
    }

    fn execute(&mut self, context: &mut Simulation) -> Result<Vec<NodeRef>> {
        let Value::Bool(value) = self.condition.get(context)? else {
            bail!("Condition must be bool");
        };
        if value {
            Ok(self.branch_true.clone())
        } else {
            Ok(self.branch_false.clone())
        }
    }

    fn get_value(&self, _index: u32, _context: &Simulation) -> Result<Value> {
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
    fn get_values_out(&self) -> Vec<Value> {
        vec![Value::Int(0)] // current
    }

    fn execute(&mut self, context: &mut Simulation) -> Result<Vec<NodeRef>> {
        if self.state.is_none() {
            self.state = Some(match self.begin.get(context)? {
                Value::Int(it) => it,
                _ => bail!("begin must be int"),
            });
            self.state_end = match self.end.get(context)? {
                Value::Int(it) => it,
                _ => bail!("end must be int"),
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

    fn get_value(&self, index: u32, _context: &Simulation) -> Result<Value> {
        match index {
            0 => Ok(Value::Int(self.state.map(|it| it - 1).unwrap_or(0))),
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
    fn get_values_out(&self) -> Vec<Value> {
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

    fn get_value(&self, _index: u32, _context: &Simulation) -> Result<Value> {
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
