//! 复合节点定义 (CompositeNode)
//!
//! 对标 GIA 工具集的 `ComponentDecl` + `NodeInterface`:
//! ```ts
//!   function ComponentName(arg_name: type) {
//!     // LocalVarDecl + ExecutionBlocks
//!     return ExecFun<{out_name: type}>(outBranchId)
//!   }
//! ```
//! 复合节点 = 一段拥有自定义入口/出口引脚的"子图",可被当作一个节点调用。
//! 这里实现的是**对外签名**部分(NodeInterface):内部逻辑图由调用方以
//! RawNodeGraph(AssetKind::CompositeGraph)另行提供,通过 `graph_ref` 关联。
//!
//! 序列化:AssetData(Type = COMPOSITE_NODE_DECL, payload = NodeInterfaceContainer)。

use crate::asset::generated::asset_data::Payload;
use crate::asset::generated::*;
use crate::asset::IAsset;

/// 复合节点的对外引脚(对标 GIA `PinInterface`)
pub struct CompositePin {
    pub name: String,
    pub kind: pin_signature::Kind,
    /// 第几个该类型的引脚(同一 kind 内递增)
    pub index: i32,
    /// 引脚数据类型(服务端类型 ID;流引脚为 None)
    pub var_type: Option<ServerTypeId>,
    /// 结构体引脚:指向结构体定义 schema_id
    pub struct_id: Option<i64>,
}

impl CompositePin {
    pub fn flow(name: impl Into<String>, kind: pin_signature::Kind, index: i32) -> Self {
        Self { name: name.into(), kind, index, var_type: None, struct_id: None }
    }

    pub fn param(
        name: impl Into<String>,
        kind: pin_signature::Kind,
        index: i32,
        var_type: ServerTypeId,
    ) -> Self {
        Self { name: name.into(), kind, index, var_type: Some(var_type), struct_id: None }
    }

    pub fn struct_param(
        name: impl Into<String>,
        kind: pin_signature::Kind,
        index: i32,
        struct_id: i64,
    ) -> Self {
        Self { name: name.into(), kind, index, var_type: Some(ServerTypeId::SStruct), struct_id: Some(struct_id) }
    }

    fn encode_pin(&self) -> PinInterface {
        PinInterface {
            name: self.name.clone(),
            visibility_mask: 0,
            sig: Some(PinSignature {
                kind: self.kind as i32,
                index: self.index,
                source_ref: None,
            }),
            r#type: (self.var_type.is_some() || self.struct_id.is_some()).then(|| pin_interface::TypeInfo {
                ui_class: None,
                var_type_shell: self.var_type.map(|t| t as i32),
                var_type_kernel: self.var_type.map(|t| t as i32),
                placeholder: None,
                display_state: None,
                detail: self.struct_id.map(|id| {
                    pin_interface::type_info::Detail::StructId(pin_interface::type_info::StructId { val: id })
                }),
            }),
            meta_sig_type: None,
            persistent_pin_uid: self.index,
        }
    }
}

/// 复合节点定义(对标 GIA `ComponentDecl`)
pub struct CompositeNode {
    pub name: String,
    pub description: String,
    /// 内部逻辑图引用(用户子图);系统生成节点为 None
    pub graph_ref: Option<Identifier>,
    /// 对外引脚(InFlow/OutFlow/InParam/OutParam/结构体操作等)
    pub pins: Vec<CompositePin>,
    /// 实现类别(COMPOSITE 用户子图 / STRUCT_ASSEMBLY 拼装结构体等)
    pub implementation: Option<node_interface::implementation::Category>,
}

impl CompositeNode {
    /// 构造一个用户自定义复合节点(内部逻辑图由 graph_ref 关联)
    pub fn new_user_composite(name: impl Into<String>, graph_ref: Identifier) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            graph_ref: Some(graph_ref),
            pins: vec![],
            implementation: Some(node_interface::implementation::Category::Composite),
        }
    }
}

impl IAsset for CompositeNode {
    fn encode(&self, id: i64) -> AssetData {
        let node_id = Identifier {
            source: identifier::Source::SystemDefined as i32,
            category: identifier::Category::NodeDecl as i32,
            kind: identifier::AssetKind::GeneratedStub as i32,
            guid: 0,
            runtime_id: id,
        };
        // 按引脚类别分流到 NodeInterface 的五个引脚组
        let mut inflows = vec![];
        let mut outflows = vec![];
        let mut inputs = vec![];
        let mut outputs = vec![];
        let mut meta_pins = vec![];
        for p in &self.pins {
            let pi = p.encode_pin();
            match p.kind {
                pin_signature::Kind::InFlow => inflows.push(pi),
                pin_signature::Kind::OutFlow => outflows.push(pi),
                pin_signature::Kind::InParam => inputs.push(pi),
                pin_signature::Kind::OutParam => outputs.push(pi),
                _ => meta_pins.push(pi), // META_* / STRUCT_* 等特殊引脚
            }
        }

        let interface = NodeInterface {
            id: Some(node_interface::Signature {
                shell_ref: Some(node_id.clone()),
                kernel_ref: Some(node_id),
                graph_ref: self.graph_ref.clone(),
                signal_version: None,
            }),
            inflows,
            outflows,
            inputs,
            outputs,
            meta_pins,
            r#impl: self.implementation.map(|c| node_interface::Implementation {
                category: c as i32,
                template: None,
            }),
            name: self.name.clone(),
            description: self.description.clone(),
            template_root: node_interface::TemplateRoot::UserComposite as i32,
            template_sub: node_interface::TemplateSub::None as i32,
        };

        AssetData {
            id: Some(Identifier {
                source: identifier::Source::UserDefined as i32,
                category: identifier::Category::NodeDecl as i32,
                kind: identifier::AssetKind::CompositeGraph as i32,
                guid: id,
                runtime_id: id,
            }),
            reference: vec![],
            name: self.name.clone(),
            r#type: asset_data::Type::CompositeNodeDecl as i32,
            payload: Some(Payload::InterfaceData(NodeInterfaceContainer {
                inner: Some(node_interface_container::InnerWrapper {
                    interface: Some(interface),
                }),
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_node_encodes() {
        let graph_ref = Identifier {
            source: identifier::Source::UserDefined as i32,
            category: identifier::Category::ServerNodeGraph as i32,
            kind: identifier::AssetKind::CompositeGraph as i32,
            guid: 200,
            runtime_id: 200,
        };
        let mut c = CompositeNode::new_user_composite("MyComponent", graph_ref);
        c.pins = vec![
            CompositePin::flow("start", pin_signature::Kind::InFlow, 0),
            CompositePin::flow("done", pin_signature::Kind::OutFlow, 0),
            CompositePin::param("value", pin_signature::Kind::InParam, 0, ServerTypeId::SInt),
            CompositePin::param("result", pin_signature::Kind::OutParam, 0, ServerTypeId::SString),
        ];
        let data = c.encode(200);
        assert_eq!(data.r#type, asset_data::Type::CompositeNodeDecl as i32);
        assert_eq!(data.id.as_ref().unwrap().kind, identifier::AssetKind::CompositeGraph as i32);
        let Payload::InterfaceData(container) = data.payload.unwrap() else {
            panic!("payload is not interface data");
        };
        let iface = container.inner.unwrap().interface.unwrap();
        assert_eq!(iface.inflows.len(), 1);
        assert_eq!(iface.outflows.len(), 1);
        assert_eq!(iface.inputs.len(), 1);
        assert_eq!(iface.outputs.len(), 1);
        assert_eq!(iface.inputs[0].name, "value");
        assert_eq!(
            iface.inputs[0].r#type.as_ref().unwrap().var_type_shell,
            Some(ServerTypeId::SInt as i32)
        );
        let sig = iface.id.unwrap();
        assert!(sig.graph_ref.is_some());
        assert_eq!(iface.template_root, node_interface::TemplateRoot::UserComposite as i32);
        assert_eq!(
            iface.r#impl.unwrap().category,
            node_interface::implementation::Category::Composite as i32
        );
    }
}
