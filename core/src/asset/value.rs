use downcast::{downcast, Any};
use crate::asset::generated::{type_definition, TypeDefinition, TypedValue, ServerTypeId, ClientTypeId};
use crate::asset::generated::typed_value::WidgetType;

#[derive(Clone, Copy)]
pub enum Side {
    Server,
    Client,
}

pub type AnyValue = Box<dyn Value>;
impl<T: Value> From<T> for AnyValue {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}
pub trait CloneValue {
    fn clone(&self) -> AnyValue;
}
pub trait Value: Any + CloneValue {
    fn encode(&self, is_set: bool, side: Side) -> TypedValue {
        TypedValue {
            widget: self.get_widget_type() as i32,
            is_set,
            r#type: Some(TypeDefinition {
                backend: match side {
                    Side::Server => type_definition::Backend::Server as i32,
                    Side::Client => type_definition::Backend::Client as i32,
                },
                type_detail: Some(match side {
                    Side::Server => type_definition::TypeDetail::ServerSide(type_definition::ServerType {
                        type_tag: self.get_server_type() as i32,
                        r#impl: 0,
                        schema: None,
                    }),
                    Side::Client => type_definition::TypeDetail::ClientSide(type_definition::ClientType {
                        type_tag: self.get_client_type() as i32,
                    }),
                }),
            }),
            tracker: None,
            storage: None,
        }
    }

    fn get_widget_type(&self) -> WidgetType {
        WidgetType::Unknown
    }

    fn get_server_type(&self) -> ServerTypeId;
    fn get_client_type(&self) -> ClientTypeId;
}
impl Clone for AnyValue {
    fn clone(&self) -> Self {
        CloneValue::clone(self.as_ref())
    }
}
downcast!(dyn Value);

pub trait ValueDefault: Value + Default {
    fn def() -> Box<Self> {
        Self::default().into()
    }
}
impl<T: Default + Value> ValueDefault for T {
}

trait ValueClone: Value + Clone {
}
impl<T: Value + Clone> ValueClone for T {
}
impl<T: ValueClone> CloneValue for T {
    fn clone(&self) -> AnyValue {
        Clone::clone(self).into()
    }
}

#[derive(Clone)]
pub struct ValueBool(pub bool);
impl Default for ValueBool {
    fn default() -> Self {
        Self(Default::default())
    }
}
impl Value for ValueBool {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SBoolean
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CBoolean
    }
}

#[derive(Clone)]
pub struct ValueInt(pub i32);
impl Default for ValueInt {
    fn default() -> Self {
        Self(Default::default())
    }
}
impl Value for ValueInt {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SInt
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CInt
    }
}

#[derive(Clone)]
pub struct ValueString(pub String);
impl Default for ValueString {
    fn default() -> Self {
        Self(Default::default())
    }
}
impl Value for ValueString {
    fn get_server_type(&self) -> ServerTypeId {
        ServerTypeId::SString
    }
    fn get_client_type(&self) -> ClientTypeId {
        ClientTypeId::CString
    }
}
// TODO: other types
