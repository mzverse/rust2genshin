use crate::asset::AssetBundle;
use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::{Identifier, identifier, node_interface};
use crate::node::decl::{DeclPin, NodeDecl};
use crate::node::{NodeGraph, NodeKind};

pub struct CompositeNodeGraph {
    pub graph: NodeGraph,
    pub description: String,
}
impl CompositeNodeGraph {
    pub fn apply(self, bundle: &mut AssetBundle) -> (Identifier, NodeKind) {
        let id = bundle.alloc(identifier::Category::ServerNodeGraph, identifier::AssetKind::Basic);
        let decl = NodeDecl {
            name: self.graph.name.clone(),
            description: self.description.clone(),
            pins: self.graph.exports.iter().map(|(k, v)| (*k, v.iter().map(|x| DeclPin {
                name: x.name.clone(),
                kind: x.kind.clone(),
                meta: None,
            }).collect())).collect(),
            implementation: node_interface::Implementation {
                category: node_interface::implementation::Category::Composite as i32,
                template: None,
            },
            template_root: node_interface::TemplateRoot::UserComposite,
            template_sub: node_interface::TemplateSub::None,
            references: vec![id],
        };
        let mut data = self.graph.apply(id);
        match data.payload.as_mut().unwrap() {
            Payload::GraphData(data) => {
                let data = data.inner.as_mut().unwrap().graph.as_mut().unwrap();
                data.id.as_mut().unwrap().kind = identifier::AssetKind::CompositeGraph as i32;
            }
            _ => unreachable!(),
        }
        bundle.push(data);
        decl.apply(bundle)
    }
}
impl CompositeNodeGraph {
    pub fn new(graph: NodeGraph) -> Self {
        Self {
            graph,
            description: String::new(),
        }
    }
}
