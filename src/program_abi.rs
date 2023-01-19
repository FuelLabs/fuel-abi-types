//! Defines a set of serializable types required for the Fuel VM ABI.

use serde::{Deserialize, Serialize};

/// FuelVM ABI representation in JSON, originally specified
/// [here](https://github.com/FuelLabs/fuel-specs/blob/master/specs/protocol/abi.md).
///
/// This type may be used by compilers and related tooling to convert an ABI
/// representation into native Rust structs and vice-versa.
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramABI {
    pub types: Vec<TypeDeclaration>,
    pub functions: Vec<ABIFunction>,
    pub logged_types: Option<Vec<LoggedType>>,
    pub messages_types: Option<Vec<MessageType>>,
    pub configurables: Option<Vec<Configurable>>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ABIFunction {
    pub inputs: Vec<TypeApplication>,
    pub name: String,
    pub output: TypeApplication,
    pub attributes: Option<Vec<Attribute>>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDeclaration {
    pub type_id: usize,
    #[serde(rename = "type")]
    pub type_field: String,
    pub components: Option<Vec<TypeApplication>>, // Used for custom types
    pub type_parameters: Option<Vec<usize>>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeApplication {
    pub name: String,
    #[serde(rename = "type")]
    pub type_id: usize,
    pub type_arguments: Option<Vec<TypeApplication>>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggedType {
    pub log_id: u64,
    #[serde(rename = "loggedType")]
    pub application: TypeApplication,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageType {
    pub message_id: u64,
    #[serde(rename = "messageType")]
    pub application: TypeApplication,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configurable {
    pub name: String,
    #[serde(rename = "configurableType")]
    pub application: TypeApplication,
    pub offset: u64,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attribute {
    pub name: String,
    pub arguments: Vec<String>,
}
