use crate::asset::generated::{AssetData, Identifier, InterfaceMapping, NodeGraphData, NodeInterface, NodeInterfaceContainer, PinInterface, PinSignature, asset_data, identifier, node_interface, node_interface_container, pin_interface};
use crate::asset::node_graph::{Connection, Link, NodeGraph, NodeGraphExtra, NodeKind, NodeRef, PinType};
use crate::asset::value::{AnyValue};
use std::collections::BTreeMap;
use tap::Tap;

pub struct NodeGraphExtraEncoding {
    name: String,
    pins: BTreeMap<PinType, Vec<(String, Option<AnyValue>)>>,
}

pub struct NodeGraphComposite {
    pub(crate) description: String,
    pub(crate) pins: BTreeMap<PinType, Vec<String>>,
}
impl NodeGraphExtra for NodeGraphComposite {
    type Data = NodeGraphExtraEncoding;
    fn encode(&self, graph: &NodeGraph<Self>, data: &mut NodeGraphData) -> Self::Data {
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
        let mut extra = NodeGraphExtraEncoding {
            name: graph.name.clone(),
            pins: self.pins.iter().map(|(k, v)| (*k, v.iter().map(|x| (x.clone(), None)).collect())).collect(),
        };
        let mut pins_data: BTreeMap<PinType, Vec<Vec<Connection>>> = self.pins.iter().map(|(k, v)| (*k, vec![vec![]; v.len()])).collect();
        for (i, n) in &graph.nodes {
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
                    extra.pins.get_mut(&PinType::InValue).unwrap()[k].1 = n.kind.values_in_types[j].clone().into();
                }
            }
            for (j, links) in n.values_out.iter().enumerate() {
                for k in links.iter().copied().flat_map(Link::export) {
                    pins_data.get_mut(&PinType::OutValue).unwrap()[k].push(Connection(NodeRef::from(i), j));
                    extra.pins.get_mut(&PinType::OutValue).unwrap()[k].1 = n.kind.values_out_types[j].clone().into();
                }
            }
        }
        for (&kind, p) in &pins_data {
            for (i, links) in p.iter().enumerate() {
                for link in links {
                    data.port_mapping.push(mapping(kind, i, *link));
                }
            }
        }
        extra
    }
    /// 复合接口声明(替代独立的 CompositeNode):由本图(接口引脚)编码为资产。
    fn encode_extra(&self, data: Self::Data, id: i64) -> Vec<AssetData> {
        let decl_id = Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: identifier::AssetKind::GeneratedStub as i32,
            guid: 0,
            runtime_id: id + Self::DECL_OFFSET,
        };
        let mut persistent_uid = 0;
        let mut encode_pin = |kind: PinType, index: i32, name: String, tar: &Option<AnyValue>| {
            PinInterface {
                name: name.clone(),
                // 对齐参考导出:对外引脚 visibility=1
                visibility_mask: 1,
                sig: Some(PinSignature {
                    kind: kind as i32,
                    index,
                    source_ref: None,
                }),
                r#type: tar.clone().map(|x| pin_interface::TypeInfo {
                    ui_class: Some(x.get_widget_type() as i32),
                    var_type_shell: Some(x.get_server_type() as i32),
                    var_type_kernel: Some(x.get_server_type() as i32),
                    placeholder: None,
                    display_state: None,
                    detail: x.encode_type_detail(), // TODO: enum ...
                }),
                meta_sig_type: None,
                persistent_pin_uid: persistent_uid,
            }.tap(|_| persistent_uid += 1)
        };
        vec![AssetData {
            id: Some(Identifier {
                source: 0,
                category: identifier::Category::NodeDecl as i32,
                kind: 0,
                guid: id + Self::DECL_OFFSET,
                runtime_id: 0,
            }),
            references: vec![Identifier {
                source: 0,
                category: identifier::Category::ServerNodeGraph as i32,
                kind: 0,
                guid: id,
                runtime_id: 0,
            }],
            name: data.name.clone(),
            r#type: asset_data::Type::CompositeNodeDecl as i32,
            payload: Some(asset_data::Payload::InterfaceData(NodeInterfaceContainer {
                inner: Some(node_interface_container::InnerWrapper {
                    interface: Some(NodeInterface {
                        id: Some(node_interface::Signature {
                            shell_ref: Some(decl_id),
                            kernel_ref: Some(decl_id),
                            graph_ref: Some(Identifier {
                                source: identifier::Source::UserDefined as i32,
                                category: identifier::Category::ServerBasic as i32,
                                kind: identifier::AssetKind::CompositeGraph as i32,
                                guid: 0,
                                runtime_id: id,
                            }),
                            signal_version: None,
                        }),
                        inflows: data.pins[&PinType::InControl].iter().enumerate().map(|(i, (name, ty))| encode_pin(PinType::InControl, i as i32, name.clone(), ty)).collect(),
                        outflows: data.pins[&PinType::OutControl].iter().enumerate().map(|(i, (name, ty))| encode_pin(PinType::OutControl, i as i32, name.clone(), ty)).collect(),
                        inputs: data.pins[&PinType::InValue].iter().enumerate().map(|(i, (name, ty))| encode_pin(PinType::InValue, i as i32, name.clone(), ty)).collect(),
                        outputs: data.pins[&PinType::OutValue].iter().enumerate().map(|(i, (name, ty))| encode_pin(PinType::OutValue, i as i32, name.clone(), ty)).collect(),
                        meta_pins: vec![], // TODO
                        r#impl: Some(node_interface::Implementation {
                            category: node_interface::implementation::Category::Composite as i32,
                            template: None,
                        }),
                        name: data.name,
                        description: self.description.clone(),
                        template_root: node_interface::TemplateRoot::UserComposite as i32,
                        template_sub: node_interface::TemplateSub::None as i32,
                    }),
                }),
            })),
        }]
    }
}
impl NodeGraphComposite {
    pub const DECL_OFFSET: i64 = 0x100000;

    pub fn new() -> Self {
        let mut pins = BTreeMap::new();
        pins.insert(PinType::InControl, vec![]);
        pins.insert(PinType::OutControl, vec![]);
        pins.insert(PinType::InValue, vec![]);
        pins.insert(PinType::OutValue, vec![]);
        Self {
            description: String::new(),
            pins,
        }
    }
}

impl Default for NodeGraphComposite {
    fn default() -> Self {
        Self::new()
    }
}

pub fn node_composite(
    id: i64,
    controls_in_num: usize,
    controls_out_num: usize,
    values_in_types: Vec<AnyValue>,
    values_out_types: Vec<AnyValue>,
) -> NodeKind {
    let mut result = NodeKind::new(id + NodeGraphComposite::DECL_OFFSET, controls_in_num, controls_out_num, values_in_types, values_out_types);
    result.asset_kind = identifier::AssetKind::GeneratedStub;
    result.references = vec![Identifier {
        source: 0,
        category: identifier::Category::NodeDecl as i32,
        kind: 0,
        guid: id,
        runtime_id: 0,
    }];
    result
}
