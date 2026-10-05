use crate::asset::generated::{AssetData, NodeInterface, NodeInterfaceContainer, PinInterface, PinSignature, asset_data, identifier, node_interface, node_interface_container, pin_interface, pin_signature};
use crate::asset::{AssetBundle, Identifier};
use crate::node::{NativeNodeId, NodeKind, PinType};
use crate::value::NativeKind;
use std::collections::HashMap;
use std::ops::Deref;
use tap::Tap;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct DeclPin {
    pub name: String,
    pub kind: Option<NativeKind>,
    pub meta: Option<pin_signature::Kind>,
}

pub fn decl_pins_value_out(kinds: &[NativeKind]) -> Vec<DeclPin> {
    kinds.iter().map(|kind| DeclPin {
        name: "".to_string(),
        kind: kind.clone().into(),
        meta: None,
    }).collect()
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct NodeDecl {
    pub name: String,
    pub description: String,
    pub pins: HashMap<PinType, Vec<DeclPin>>,
    pub implementation: node_interface::Implementation,
    pub template_root: node_interface::TemplateRoot,
    pub template_sub: node_interface::TemplateSub,
    pub references: Vec<Identifier>,
}

impl NodeDecl {
    pub fn encode(&self, id: Option<node_interface::Signature>) -> NodeInterface {
        let mut persistent_uid = 0;
        let mut encode_pin = |kind: Option<PinType>, index: i32, pin: &DeclPin| {
            PinInterface {
                name: pin.name.clone(),
                visibility_mask: kind.is_some() as i32,
                sig: kind.map(|kind| PinSignature {
                    kind: kind as i32,
                    index,
                    source_ref: None,
                }),
                r#type: pin.kind.as_ref().map(|x| pin_interface::TypeInfo {
                    ui_class: x.get_widget_type().map(|x| x as i32),
                    var_type_shell: Some(x.get_server_shell_type()),
                    var_type_kernel: Some(x.get_server_id() as i32),
                    placeholder: None,
                    display_state: None,
                    detail: x.encode_type_detail(),
                }),
                meta_sig_type: pin.meta.map(|m| PinSignature {
                    kind: m as i32,
                    index: 0,
                    source_ref: None,
                }),
                persistent_pin_uid: persistent_uid + 1,
            }.tap(|_| persistent_uid += 1)
        };
        let def = Default::default();
        NodeInterface {
            id,
            inflows: self.pins.get(&PinType::InControl).unwrap_or(&def).iter().enumerate().map(|(i, pin)| encode_pin(PinType::InControl.into(), i as i32, pin)).collect(),
            outflows: self.pins.get(&PinType::OutControl).unwrap_or(&def).iter().enumerate().map(|(i, pin)| encode_pin(PinType::OutControl.into(), i as i32, pin)).collect(),
            inputs: vec![].tap_mut(|inputs| {
                let mut i = 0;
                for pin in self.pins.get(&PinType::InValue).unwrap_or(&def).iter() {
                    inputs.push(encode_pin(pin.kind.is_some().then_some(PinType::InValue), i, pin));
                    if pin.kind.is_some() {
                        i += 1;
                    }
                }
            }),
            outputs: self.pins.get(&PinType::OutValue).unwrap_or(&def).iter().enumerate().map(|(i, pin)| encode_pin(PinType::OutValue.into(), i as i32, pin)).collect(),
            meta_pins: vec![], // TODO
            r#impl: self.implementation.clone().into(),
            name: self.name.clone(),
            description: self.description.clone(),
            template_root: self.template_root as i32,
            template_sub: self.template_sub as i32,
        }
    }
}

impl NodeDecl {
    pub fn apply(self, bundle: &mut AssetBundle) -> (Identifier, NodeKind) {
        let id = bundle.alloc(identifier::Category::NodeDecl, identifier::AssetKind::Basic);
        let id_sig = Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::ServerBasic as i32,
            kind: identifier::AssetKind::GeneratedStub as i32,
            guid: 0,
            runtime_id: id.guid,
        };
        bundle.push(AssetData {
            id: id.into(),
            name: self.name.clone(),
            r#type: asset_data::Type::CompositeNodeDecl as i32,
            payload: asset_data::Payload::InterfaceData(NodeInterfaceContainer {
                inner: node_interface_container::InnerWrapper {
                    interface: self.encode(node_interface::Signature {
                        shell_ref: id_sig.into(),
                        kernel_ref: id_sig.into(),
                        graph_ref: self.references.first().filter(|x| x.category == identifier::Category::ServerNodeGraph as i32).map(|x| Identifier {
                            source: identifier::Source::UserDefined as i32,
                            category: identifier::Category::ServerBasic as i32,
                            kind: identifier::AssetKind::CompositeGraph as i32,
                            guid: 0,
                            runtime_id: x.guid,
                        }).unwrap_or_default().into(),
                        signal_version: None,
                    }.into()).into(),
                }.into(),
            }).into(),
            references: self.references,
        });
        (id, node_declared(
            id,
            self.pins.get(&PinType::InControl).map(Vec::len).unwrap_or(0),
            self.pins.get(&PinType::OutControl).map(Vec::len).unwrap_or(0),
            self.pins.get(&PinType::InValue).map(Deref::deref).map(<[_]>::iter).unwrap_or_default().map(|x| x.kind.as_ref().cloned()).collect(),
            self.pins.get(&PinType::OutValue).map(Deref::deref).map(<[_]>::iter).unwrap_or_default().map(|x| x.kind.clone().unwrap()).collect(),
        ))
    }
}

pub fn node_declared(
    id: Identifier,
    controls_in_num: usize,
    controls_out_num: usize,
    values_in_types: Vec<Option<NativeKind>>,
    values_out_types: Vec<NativeKind>,
) -> NodeKind {
    // let AssetData {
    //     payload: Some(
    //         asset_data::Payload::InterfaceData(
    //             NodeInterfaceContainer {
    //                 inner: Some(
    //                     node_interface_container::InnerWrapper {
    //                         interface: Some(node_interface), ..
    //                     }), ..
    //             })), ..
    // } = assets.get(id).unwrap() else { panic!() };
    NodeKind::new(NativeNodeId {
        kind: identifier::AssetKind::GeneratedStub,
        id: id.guid,
        kernel: id.guid,
    }, controls_in_num, controls_out_num, values_in_types, values_out_types).tap_mut(|node| node.references = vec![id])
}
