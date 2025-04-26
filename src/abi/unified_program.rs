use std::collections::{BTreeMap, HashMap};

use crate::{
    abi::program::{
        ABIFunction, Attribute, Configurable, LoggedType, ProgramABI, TypeApplication,
        TypeConcreteDeclaration, TypeMetadataDeclaration,
    },
    utils::extract_custom_type_name,
};

use crate::{
    error::{error, Result},
    utils::TypePath,
};

use super::program::{self, ConcreteTypeId, ErrorDetails, MessageType, TypeId, Version};

/// 'Unified' versions of the ABI structures removes concrete types and types metadata and unifies them under a single types declarations array.
#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct UnifiedProgramABI {
    pub program_type: String,
    pub spec_version: Version,
    pub encoding_version: Version,
    pub types: Vec<UnifiedTypeDeclaration>,
    pub functions: Vec<UnifiedABIFunction>,
    pub logged_types: Option<Vec<UnifiedLoggedType>>,
    pub messages_types: Option<Vec<UnifiedMessageType>>,
    pub configurables: Option<Vec<UnifiedConfigurable>>,
    pub error_codes: Option<BTreeMap<u64, ErrorDetails>>,
}

impl UnifiedProgramABI {
    pub fn from_json_abi(abi: &str) -> Result<Self> {
        let parsed_abi: ProgramABI = serde_json::from_str(abi)?;
        UnifiedProgramABI::from_counterpart(&parsed_abi)
    }

    pub fn from_counterpart(program_abi: &ProgramABI) -> Result<UnifiedProgramABI> {
        let mut extended_concrete_types = program_abi.concrete_types.clone();
        let mut extended_metadata_types = program_abi.metadata_types.clone();
        let mut next_metadata_type_id = extended_metadata_types
            .iter()
            .map(|v| v.metadata_type_id.0)
            .max()
            .unwrap_or(0)
            + 1;

        // Ensure every concrete type has an associated type metadata.
        for concrete_type_decl in extended_concrete_types.iter_mut() {
            if concrete_type_decl.metadata_type_id.is_none() {
                extended_metadata_types.push(TypeMetadataDeclaration {
                    type_field: concrete_type_decl.type_field.clone(),
                    metadata_type_id: program::MetadataTypeId(next_metadata_type_id),
                    components: None,
                    type_parameters: None,
                });
                concrete_type_decl.metadata_type_id =
                    Some(program::MetadataTypeId(next_metadata_type_id));
                next_metadata_type_id += 1;
            }
        }

        let concrete_types_lookup: HashMap<_, _> = extended_concrete_types
            .iter()
            .map(|ttype| (ttype.concrete_type_id.clone(), ttype.clone()))
            .collect();

        let types = extended_metadata_types
            .iter()
            .map(|ttype| UnifiedTypeDeclaration::from_counterpart(ttype, &concrete_types_lookup))
            .collect();

        let functions = program_abi
            .functions
            .iter()
            .map(|fun| UnifiedABIFunction::from_counterpart(fun, &concrete_types_lookup))
            .collect::<Result<Vec<_>>>()?;

        let logged_types: Vec<UnifiedLoggedType> = program_abi
            .logged_types
            .iter()
            .flatten()
            .map(|logged_type| {
                UnifiedLoggedType::from_counterpart(logged_type, &concrete_types_lookup)
            })
            .collect();

        let configurables: Vec<UnifiedConfigurable> = program_abi
            .configurables
            .iter()
            .flatten()
            .map(|configurable| {
                UnifiedConfigurable::from_counterpart(configurable, &concrete_types_lookup)
            })
            .collect();

        let messages_types: Vec<UnifiedMessageType> = program_abi
            .messages_types
            .iter()
            .flatten()
            .map(|message_types| {
                UnifiedMessageType::from_counterpart(message_types, &concrete_types_lookup)
            })
            .collect();

        Ok(Self {
            program_type: program_abi.program_type.clone(),
            spec_version: program_abi.spec_version.clone(),
            encoding_version: program_abi.encoding_version.clone(),
            types,
            functions,
            logged_types: if logged_types.is_empty() {
                None
            } else {
                Some(logged_types)
            },
            messages_types: if messages_types.is_empty() {
                None
            } else {
                Some(messages_types)
            },
            configurables: if configurables.is_empty() {
                None
            } else {
                Some(configurables)
            },
            error_codes: program_abi.error_codes.clone(),
        })
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct UnifiedABIFunction {
    pub name: String,
    pub inputs: Vec<UnifiedTypeApplication>,
    pub output: UnifiedTypeApplication,
    pub attributes: Option<Vec<Attribute>>,
}

impl UnifiedABIFunction {
    pub fn new(
        name: String,
        inputs: Vec<UnifiedTypeApplication>,
        output: UnifiedTypeApplication,
        attributes: Vec<Attribute>,
    ) -> Result<Self> {
        if name.is_empty() {
            Err(error!("UnifiedABIFunction's name cannot be empty!"))
        } else {
            Ok(Self {
                name,
                inputs,
                output,
                attributes: Some(attributes),
            })
        }
    }

    pub fn from_counterpart(
        abi_function: &ABIFunction,
        concrete_types_lookup: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> Result<UnifiedABIFunction> {
        let inputs = abi_function
            .inputs
            .iter()
            .map(|input| {
                UnifiedTypeApplication::from_concrete_type_id(
                    input.name.clone(),
                    input.concrete_type_id.clone(),
                    concrete_types_lookup,
                )
            })
            .collect();

        let attributes = abi_function
            .attributes
            .as_ref()
            .map_or(vec![], Clone::clone);

        UnifiedABIFunction::new(
            abi_function.name.clone(),
            inputs,
            UnifiedTypeApplication::from_concrete_type_id(
                "".to_string(),
                abi_function.output.clone(),
                concrete_types_lookup,
            ),
            attributes,
        )
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnifiedTypeDeclaration {
    pub type_id: usize,
    pub type_field: String,
    pub components: Option<Vec<UnifiedTypeApplication>>,
    pub type_parameters: Option<Vec<usize>>,
}

impl UnifiedTypeDeclaration {
    pub fn from_counterpart(
        type_decl: &TypeMetadataDeclaration,
        concrete_types_lookup: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> UnifiedTypeDeclaration {
        let components: Vec<UnifiedTypeApplication> = type_decl
            .components
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|application| {
                UnifiedTypeApplication::from_counterpart(&application, concrete_types_lookup)
            })
            .collect();
        let type_parameters: Vec<usize> = type_decl
            .type_parameters
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|id| id.0)
            .collect();
        UnifiedTypeDeclaration {
            type_id: type_decl.metadata_type_id.0,
            type_field: type_decl.type_field.clone(),
            components: if components.is_empty() {
                None
            } else {
                Some(components)
            },
            type_parameters: if type_parameters.is_empty() {
                None
            } else {
                Some(type_parameters)
            },
        }
    }

    pub fn custom_type_path(&self) -> Result<TypePath> {
        let type_field = &self.type_field;
        let type_name = extract_custom_type_name(type_field)
            .ok_or_else(|| error!("Couldn't extract custom type path from '{type_field}'"))?;

        TypePath::new(type_name)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnifiedTypeApplication {
    pub name: String,
    pub type_id: usize,
    pub error_message: Option<String>,
    pub type_arguments: Option<Vec<UnifiedTypeApplication>>,
}

impl UnifiedTypeApplication {
    pub fn from_counterpart(
        type_application: &TypeApplication,
        concrete_types_lookup: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> UnifiedTypeApplication {
        let (metadata_type_id, type_arguments) = match type_application.type_id.clone() {
            TypeId::Concrete(concrete_type_id) => (
                concrete_types_lookup
                    .get(&concrete_type_id)
                    .unwrap()
                    .metadata_type_id
                    .clone()
                    .unwrap(),
                concrete_types_lookup
                    .get(&concrete_type_id)
                    .unwrap()
                    .type_arguments
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|concrete_type_id| {
                        UnifiedTypeApplication::from_concrete_type_id(
                            "".to_string(),
                            concrete_type_id,
                            concrete_types_lookup,
                        )
                    })
                    .collect::<Vec<UnifiedTypeApplication>>(),
            ),
            TypeId::Metadata(metadata_type_id) => (
                metadata_type_id,
                type_application
                    .type_arguments
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|application| {
                        UnifiedTypeApplication::from_counterpart(
                            &application,
                            concrete_types_lookup,
                        )
                    })
                    .collect(),
            ),
        };

        UnifiedTypeApplication {
            name: type_application.name.clone(),
            type_id: metadata_type_id.0,
            error_message: type_application.error_message.clone(),
            type_arguments: if type_arguments.is_empty() {
                None
            } else {
                Some(type_arguments)
            },
        }
    }

    pub fn from_concrete_type_id(
        name: String,
        concrete_type_id: ConcreteTypeId,
        concrete_types_lookup: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> UnifiedTypeApplication {
        let concrete_type_decl = concrete_types_lookup
            .get(&concrete_type_id)
            .unwrap()
            .clone();
        let type_arguments: Vec<UnifiedTypeApplication> = concrete_type_decl
            .type_arguments
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|concrete_type_id| {
                UnifiedTypeApplication::from_concrete_type_id(
                    "".to_string(),
                    concrete_type_id,
                    concrete_types_lookup,
                )
            })
            .collect();

        let metadata_type_id = concrete_type_decl.metadata_type_id.unwrap();

        UnifiedTypeApplication {
            name,
            type_id: metadata_type_id.0,
            // `from_concrete_type_id` is always used to describe either
            // a type or, mostly, type arguments. It is never used for
            // enum fields, and, thus, can never return a `UnifiedTypeApplication`
            // with an `error_message`.
            error_message: None,
            type_arguments: if type_arguments.is_empty() {
                None
            } else {
                Some(type_arguments)
            },
        }
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct UnifiedLoggedType {
    pub log_id: String,
    pub application: UnifiedTypeApplication,
}

impl UnifiedLoggedType {
    fn from_counterpart(
        logged_type: &LoggedType,
        concrete_types_lookup: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> UnifiedLoggedType {
        UnifiedLoggedType {
            log_id: logged_type.log_id.clone(),
            application: UnifiedTypeApplication::from_concrete_type_id(
                "".to_string(),
                logged_type.concrete_type_id.clone(),
                concrete_types_lookup,
            ),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnifiedConfigurable {
    pub name: String,
    pub application: UnifiedTypeApplication,
    pub offset: u64,
    pub indirect: bool,
}

impl UnifiedConfigurable {
    pub fn from_counterpart(
        configurable: &Configurable,
        concrete_types_lookup: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> UnifiedConfigurable {
        UnifiedConfigurable {
            name: configurable.name.clone(),
            application: UnifiedTypeApplication::from_concrete_type_id(
                "".to_string(),
                configurable.concrete_type_id.clone(),
                concrete_types_lookup,
            ),
            offset: configurable.offset,
            indirect: configurable.indirect,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnifiedMessageType {
    pub message_id: String,
    pub application: UnifiedTypeApplication,
}

impl UnifiedMessageType {
    pub fn from_counterpart(
        message_type: &MessageType,
        concrete_types_lookup: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> UnifiedMessageType {
        UnifiedMessageType {
            message_id: message_type.message_id.clone(),
            application: UnifiedTypeApplication::from_concrete_type_id(
                "".to_string(),
                message_type.concrete_type_id.clone(),
                concrete_types_lookup,
            ),
        }
    }
}

impl UnifiedTypeDeclaration {
    pub fn is_custom_type(&self) -> bool {
        self.is_struct_type() || self.is_enum_type()
    }

    pub fn is_enum_type(&self) -> bool {
        self.type_field.starts_with("enum ")
    }

    pub fn is_struct_type(&self) -> bool {
        self.type_field.starts_with("struct ")
    }
}
