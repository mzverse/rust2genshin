use std::collections::HashMap;
use std::ops::Deref;
use tap::Tap;
use crate::asset::generated::{node_interface, PinSignature, AssetData, identifier, PinInterface, pin_interface, asset_data, NodeInterfaceContainer, node_interface_container, NodeInterface, pin_signature};
use crate::asset::{Asset, AssetBundle, AssetRef, Identifier};
use crate::asset::node_graph::{NodeKind, PinType};
use crate::asset::value::AnyValue;

pub struct DeclPin {
    pub name: String,
    pub kind: Option<AnyValue>,
    pub meta: Option<pin_signature::Kind>,
}

pub struct NodeDecl {
    pub name: String,
    pub description: String,
    pub pins: HashMap<PinType, Vec<DeclPin>>,
    pub implementation: node_interface::Implementation,
    pub template_root: node_interface::TemplateRoot,
    pub template_sub: node_interface::TemplateSub,
    pub references: Vec<Identifier>,
}

impl Asset for NodeDecl {
    type RefData = NodeKind;

    fn apply(self, bundle: &mut AssetBundle) -> AssetRef<Self> {
        let id = bundle.alloc(identifier::Category::NodeDecl, identifier::AssetKind::Basic);
        let id_sig = Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: identifier::AssetKind::GeneratedStub as i32,
            guid: 0,
            runtime_id: id.guid,
        };
        let mut persistent_uid = 0;
        let mut encode_pin = |kind: PinType, index: i32, pin: &DeclPin| {
            PinInterface {
                name: pin.name.clone(),
                visibility_mask: 1,
                sig: Some(PinSignature {
                    kind: kind as i32,
                    index,
                    source_ref: None,
                }),
                r#type: pin.kind.as_ref().map(|x| pin_interface::TypeInfo {
                    ui_class: Some(x.get_widget_type() as i32),
                    var_type_shell: Some(x.get_server_type() as i32),
                    var_type_kernel: Some(x.get_server_type() as i32),
                    placeholder: None,
                    display_state: None,
                    detail: x.encode_type_detail(),
                }),
                meta_sig_type: pin.meta.map(|m| PinSignature {
                    kind: m as i32,
                    index,
                    source_ref: None,
                }),
                persistent_pin_uid: persistent_uid + 1,
            }.tap(|_| persistent_uid += 1)
        };
        let def = Default::default();
        bundle.push(AssetData {
            id: id.into(),
            name: self.name.clone(),
            r#type: asset_data::Type::CompositeNodeDecl as i32,
            payload: asset_data::Payload::InterfaceData(NodeInterfaceContainer {
                inner: node_interface_container::InnerWrapper {
                    interface: NodeInterface {
                        id: node_interface::Signature {
                            shell_ref: id_sig.into(),
                            kernel_ref: id_sig.into(),
                            graph_ref: self.references.first().filter(|x| x.category == identifier::Category::ServerNodeGraph as i32).map(|x| Identifier {
                                source: identifier::Source::UserDefined as i32,
                                category: identifier::Category::ServerBasic as i32,
                                kind: identifier::AssetKind::CompositeGraph as i32,
                                guid: 0,
                                runtime_id: x.guid,
                            }).unwrap_or(Identifier {
                                source: 0,
                                category: 0,
                                kind: 0,
                                guid: 0,
                                runtime_id: 0,
                            }).into(),
                            signal_version: None,
                        }.into(),
                        inflows: self.pins.get(&PinType::InControl).unwrap_or(&def).iter().enumerate().map(|(i, pin)| encode_pin(PinType::InControl, i as i32, pin)).collect(),
                        outflows: self.pins.get(&PinType::OutControl).unwrap_or(&def).iter().enumerate().map(|(i, pin)| encode_pin(PinType::OutControl, i as i32, pin)).collect(),
                        inputs: self.pins.get(&PinType::InValue).unwrap_or(&def).iter().enumerate().map(|(i, pin)| encode_pin(PinType::InValue, i as i32, pin)).collect(),
                        outputs: self.pins.get(&PinType::OutValue).unwrap_or(&def).iter().enumerate().map(|(i, pin)| encode_pin(PinType::OutValue, i as i32, pin)).collect(),
                        meta_pins: vec![], // TODO
                        r#impl: self.implementation.clone().into(),
                        name: self.name.clone(),
                        description: self.description.clone(),
                        template_root: self.template_root as i32,
                        template_sub: self.template_sub as i32,
                    }.into(),
                }.into(),
            }).into(),
            references: self.references,
        });
        AssetRef::new(id, node_decl(id.guid,
                                    self.pins.get(&PinType::InControl).map(Vec::len).unwrap_or(0),
                                    self.pins.get(&PinType::OutControl).map(Vec::len).unwrap_or(0),
                                    self.pins.get(&PinType::InValue).map(Deref::deref).map(<[_]>::iter).unwrap_or_default().map(|x| x.kind.as_ref().cloned()).collect(),
                                    self.pins.get(&PinType::OutValue).map(Deref::deref).map(<[_]>::iter).unwrap_or_default().map(|x| x.kind.as_ref().unwrap().clone()).collect(),
        ))
    }
}

pub fn node_decl(
    id: i64,
    controls_in_num: usize,
    controls_out_num: usize,
    values_in_types: Vec<Option<AnyValue>>,
    values_out_types: Vec<AnyValue>,
) -> NodeKind {
    let mut result = NodeKind::full(id, controls_in_num, controls_out_num, values_in_types, values_out_types);
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
