use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::{Identifier, InterfaceMapping, PinSignature, identifier, node_interface};
use crate::asset::node_graph::decl::{DeclPin, NodeDecl};
use crate::asset::node_graph::{Connection, Link, NodeGraph, NodeKind, NodeRef, PinType};
use crate::asset::{Asset, AssetBundle, AssetRef};
use std::collections::BTreeMap;

pub struct RefData {
    pub id: Identifier,
    pub node: NodeKind,
}

pub struct CompositeNodeGraph {
    pub graph: NodeGraph,
    pub description: String,
    pub pins: BTreeMap<PinType, Vec<String>>,
}
impl Asset for CompositeNodeGraph {
    type RefData = RefData;

    fn apply(self, bundle: &mut AssetBundle) -> AssetRef<Self> {
        fn mapping(kind: PinType, id: usize, link: Connection) -> InterfaceMapping {
            let sig = |idx: usize| PinSignature {
                kind: kind as i32,
                index: idx as i32,
                source_ref: None,
            };
            InterfaceMapping {
                external_port: Some(sig(id)),
                // Link 的 NodeRef 是节点下标(0 起),穿透目标用图内 index(1 起)
                internal_target_node_handle: link.0.encode(),
                internal_port_shell: Some(sig(link.1)),
                internal_port_kernel: Some(sig(link.1)),
            }
        }
        let id = bundle.alloc(identifier::Category::ServerNodeGraph, identifier::AssetKind::Basic);
        let mut decl = NodeDecl {
            name: self.graph.name.clone(),
            description: self.description.clone(),
            pins: self.pins.iter().map(|(k, v)| (*k, v.iter().map(|x| DeclPin {
                name: x.clone(),
                kind: None,
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
        let mut pins_data: BTreeMap<PinType, Vec<Vec<Connection>>> = self.pins.iter().map(|(k, v)| (*k, vec![vec![]; v.len()])).collect();
        for (i, n) in &self.graph.nodes {
            for (j, links) in n.controls_in.iter().enumerate() {
                for k in links.iter().copied().flat_map(Link::export) {
                    pins_data.get_mut(&PinType::InControl).unwrap()[k].push(Connection(NodeRef::from(i), j));
                }
            }
            for (j, links) in n.controls_out.iter().enumerate() {
                for k in links.iter().copied().flat_map(Link::export) {
                    pins_data.get_mut(&PinType::OutControl).unwrap()[k].push(Connection(NodeRef::from(i), j));
                }
            }
            for (j, links) in n.values_in.iter().enumerate() {
                for k in links.link.iter().copied().flat_map(Link::export) {
                    pins_data.get_mut(&PinType::InValue).unwrap()[k].push(Connection(NodeRef::from(i), j));
                    decl.pins.get_mut(&PinType::InValue).unwrap()[k].kind = n.kind.values_in_types[j].clone().into();
                }
            }
            for (j, links) in n.values_out.iter().enumerate() {
                for k in links.iter().copied().flat_map(Link::export) {
                    pins_data.get_mut(&PinType::OutValue).unwrap()[k].push(Connection(NodeRef::from(i), j));
                    decl.pins.get_mut(&PinType::OutValue).unwrap()[k].kind = n.kind.values_out_types[j].clone().into();
                }
            }
        }
        let mut data = self.graph.apply(id);
        match data.payload.as_mut().unwrap() {
            Payload::GraphData(data) => {
                let data = data.inner.as_mut().unwrap().graph.as_mut().unwrap();
                data.id.as_mut().unwrap().kind = identifier::AssetKind::CompositeGraph as i32;
                for (&kind, p) in &pins_data {
                    for (i, links) in p.iter().enumerate() {
                        for link in links {
                            data.port_mapping.push(mapping(kind, i, *link));
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
        bundle.push(data);
        let decl = decl.apply(bundle);
        AssetRef::new(decl.root, RefData {
            id,
            node: decl.data,
        })
    }
}
impl CompositeNodeGraph {
    pub fn new(graph: NodeGraph) -> Self {
        let mut pins = BTreeMap::new();
        pins.insert(PinType::InControl, vec![]);
        pins.insert(PinType::OutControl, vec![]);
        pins.insert(PinType::InValue, vec![]);
        pins.insert(PinType::OutValue, vec![]);
        Self {
            graph,
            description: String::new(),
            pins,
        }
    }
}

pub fn node_composite(
    r: &AssetRef<CompositeNodeGraph>,
) -> NodeKind {
    r.data.node.clone()
}
