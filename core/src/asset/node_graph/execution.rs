use crate::asset::node_graph::{ControlOut, INode, LogType, NodeRef, Simulation, ValueIn};
use crate::asset::raw_node_graph::NodeType;
use crate::asset::value::{AnyValue, ValueString};
use anyhow::{anyhow, bail};

pub struct NodeLog {
    value: ValueIn,
    next: ControlOut,
}
impl INode for NodeLog {
    fn get_controls_in(&self) -> i32 {
        1
    }
    fn get_controls_out(&self) -> Vec<ControlOut> {
        vec![self.next.clone()]
    }
    fn get_values_in(&self) -> Vec<&ValueIn> {
        vec![&self.value]
    }
    fn get_values_out(&self) -> Vec<AnyValue> {
        vec![]
    }

    fn execute(&mut self, context: &mut Simulation) -> anyhow::Result<Vec<NodeRef>> {
        let Ok(value) = self.value.get(context)?.downcast::<ValueString>() else {
            return Err(anyhow!("Log must be String"));
        };
        context.logs.push((LogType::Info, value.0));
        Ok(self.next.clone())
    }

    fn get_value(&self, _index: i32, _context: &Simulation) -> anyhow::Result<AnyValue> {
        bail!("No value")
    }

    fn get_type(&self) -> NodeType {
        NodeType::simple(1)
    }
}
