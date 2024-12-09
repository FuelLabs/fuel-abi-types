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
    pub program_type: String,
    pub spec_version: Version,
    pub encoding_version: Version,
    pub concrete_types: Vec<TypeConcreteDeclaration>,
    pub metadata_types: Vec<TypeMetadataDeclaration>,
    pub functions: Vec<ABIFunction>,
    pub logged_types: Option<Vec<LoggedType>>,
    pub messages_types: Option<Vec<MessageType>>,
    pub configurables: Option<Vec<Configurable>>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version(pub String);

impl From<&str> for Version {
    fn from(value: &str) -> Self {
        Version(value.into())
    }
}

impl Version {
    pub fn major(&self) -> Option<&str> {
        let s = self.0.split('.').next().map(|x| x.trim());
        match s {
            Some("") => None,
            s => s,
        }
    }

    pub fn minor(&self) -> Option<&str> {
        let s = self.0.split('.').nth(1).map(|x| x.trim());
        match s {
            Some("") => None,
            s => s,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ConcreteTypeId(pub String);

impl From<&str> for ConcreteTypeId {
    fn from(value: &str) -> Self {
        ConcreteTypeId(value.into())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct MetadataTypeId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(untagged)]
pub enum TypeId {
    Concrete(ConcreteTypeId),
    Metadata(MetadataTypeId),
}

impl Default for TypeId {
    fn default() -> Self {
        TypeId::Metadata(MetadataTypeId(usize::MAX))
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ABIFunction {
    pub inputs: Vec<TypeConcreteParameter>,
    pub name: String,
    pub output: ConcreteTypeId,
    pub attributes: Option<Vec<Attribute>>,
}

impl ABIFunction {
    pub fn is_payable(&self) -> bool {
        self.attributes
            .iter()
            .flatten()
            .any(|attr| attr.name == "payable")
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeMetadataDeclaration {
    #[serde(rename = "type")]
    pub type_field: String,
    pub metadata_type_id: MetadataTypeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<TypeApplication>>, // Used for custom types
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<Vec<MetadataTypeId>>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeConcreteDeclaration {
    #[serde(rename = "type")]
    pub type_field: String,
    pub concrete_type_id: ConcreteTypeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_type_id: Option<MetadataTypeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<Vec<ConcreteTypeId>>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeConcreteParameter {
    pub name: String,
    pub concrete_type_id: ConcreteTypeId,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeApplication {
    pub name: String,
    pub type_id: TypeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_arguments: Option<Vec<TypeApplication>>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggedType {
    pub log_id: String,
    pub concrete_type_id: ConcreteTypeId,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageType {
    pub message_id: String,
    pub concrete_type_id: ConcreteTypeId,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configurable {
    pub name: String,
    pub concrete_type_id: ConcreteTypeId,
    pub offset: u64,
    #[serde(default)]
    pub indirect: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attribute {
    pub name: String,
    pub arguments: Vec<String>,
}

#[test]
fn version_extraction_test() {
    let v = Version("1.2".to_string());
    assert_eq!(v.major(), Some("1"));
    assert_eq!(v.minor(), Some("2"));

    let v = Version("1".to_string());
    assert_eq!(v.major(), Some("1"));
    assert_eq!(v.minor(), None);

    let v = Version("".to_string());
    assert_eq!(v.major(), None);
    assert_eq!(v.minor(), None);
}
