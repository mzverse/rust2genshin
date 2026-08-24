use anyhow::{anyhow, bail};
use crate::asset::node_graph::{ControlOut, INode, LogType, NodeRef, Simulation, Value, ValueIn};

pub struct NodeLog {
    value: ValueIn,
    next: ControlOut,
}
impl INode for NodeLog {
    fn get_controls_in(&self) -> u32 {
        1
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.next.clone()]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value]
    }
    fn get_values_out(&self) -> Vec<Value> {
        vec![]
    }

    fn execute(&mut self, context: &mut Simulation) -> anyhow::Result<Vec<NodeRef>> {
        let Value::String(value) = self.value.get(context)? else {
            return Err(anyhow!("Log must be String"));
        };
        context.logs.push((LogType::Info, value));
        Ok(self.next.clone())
    }

    fn get_value(&self, index: u32, context: &Simulation) -> anyhow::Result<Value> {
        bail!("No value")
    }
}
