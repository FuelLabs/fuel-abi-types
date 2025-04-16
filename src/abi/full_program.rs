use std::collections::{BTreeMap, HashMap};

use crate::{abi::program::Attribute, utils::extract_custom_type_name};

use crate::{
    error::{error, Error, Result},
    utils::TypePath,
};

use super::program::ErrorDetails;
use super::unified_program::UnifiedMessageType;
use super::{
    program::Version,
    unified_program::{
        UnifiedABIFunction, UnifiedConfigurable, UnifiedLoggedType, UnifiedProgramABI,
        UnifiedTypeApplication, UnifiedTypeDeclaration,
    },
};

/// 'Full' versions of the ABI structures are needed to simplify duplicate
/// detection later on. The original ones([`UnifiedProgramABI`], [`UnifiedTypeApplication`],
/// [`UnifiedTypeDeclaration`] and others) are not suited for this due to their use of
/// ids, which might differ between contracts even though the type they
/// represent is virtually the same.
#[derive(Debug, Clone)]
pub struct FullProgramABI {
    pub program_type: String,
    pub spec_version: Version,
    pub encoding_version: Version,
    pub types: Vec<FullTypeDeclaration>,
    pub functions: Vec<FullABIFunction>,
    pub logged_types: Vec<FullLoggedType>,
    pub message_types: Vec<FullMessageType>,
    pub configurables: Vec<FullConfigurable>,
    pub error_codes: BTreeMap<u64, ErrorDetails>,
}

impl FullProgramABI {
    pub fn from_json_abi(abi: &str) -> Result<Self> {
        let unified_program_abi = UnifiedProgramABI::from_json_abi(abi)?;
        FullProgramABI::from_counterpart(&unified_program_abi)
    }

    fn from_counterpart(unified_program_abi: &UnifiedProgramABI) -> Result<FullProgramABI> {
        let lookup: HashMap<_, _> = unified_program_abi
            .types
            .iter()
            .map(|ttype| (ttype.type_id, ttype.clone()))
            .collect();

        let types = unified_program_abi
            .types
            .iter()
            .map(|ttype| FullTypeDeclaration::from_counterpart(ttype, &lookup))
            .collect();

        let functions = unified_program_abi
            .functions
            .iter()
            .map(|fun| FullABIFunction::from_counterpart(fun, &lookup))
            .collect::<Result<Vec<_>>>()?;

        let logged_types = unified_program_abi
            .logged_types
            .iter()
            .flatten()
            .map(|logged_type| FullLoggedType::from_counterpart(logged_type, &lookup))
            .collect();

        let message_types = unified_program_abi
            .messages_types
            .iter()
            .flatten()
            .map(|message_type| FullMessageType::from_counterpart(message_type, &lookup))
            .collect();

        let configurables = unified_program_abi
            .configurables
            .iter()
            .flatten()
            .map(|configurable| FullConfigurable::from_counterpart(configurable, &lookup))
            .collect();

        Ok(Self {
            program_type: unified_program_abi.program_type.clone(),
            spec_version: unified_program_abi.spec_version.clone(),
            encoding_version: unified_program_abi.encoding_version.clone(),
            types,
            functions,
            logged_types,
            message_types,
            configurables,
            error_codes: unified_program_abi.error_codes.clone().unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullABIFunction {
    name: String,
    inputs: Vec<FullTypeApplication>,
    output: FullTypeApplication,
    attributes: Vec<Attribute>,
}

impl FullABIFunction {
    pub fn new(
        name: String,
        inputs: Vec<FullTypeApplication>,
        output: FullTypeApplication,
        attributes: Vec<Attribute>,
    ) -> Result<Self> {
        if name.is_empty() {
            Err(error!("FullABIFunction's name cannot be empty!"))
        } else {
            Ok(Self {
                name,
                inputs,
                output,
                attributes,
            })
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn inputs(&self) -> &[FullTypeApplication] {
        self.inputs.as_slice()
    }

    pub fn output(&self) -> &FullTypeApplication {
        &self.output
    }

    pub fn is_payable(&self) -> bool {
        self.attributes.iter().any(|attr| attr.name == "payable")
    }

    pub fn doc_strings(&self) -> Result<Vec<String>> {
        self.attributes
            .iter()
            .filter(|attr| attr.name == "doc-comment")
            .map(|attr| {
                (attr.arguments.len() == 1)
                    .then_some(attr.arguments[0].clone())
                    .ok_or_else(|| {
                        Error("`doc-comment` attribute must have one argument".to_string())
                    })
            })
            .collect::<Result<Vec<String>>>()
    }

    pub fn from_counterpart(
        abi_function: &UnifiedABIFunction,
        types: &HashMap<usize, UnifiedTypeDeclaration>,
    ) -> Result<FullABIFunction> {
        let inputs = abi_function
            .inputs
            .iter()
            .map(|input| FullTypeApplication::from_counterpart(input, types))
            .collect();

        let attributes = abi_function
            .attributes
            .as_ref()
            .map_or(vec![], Clone::clone);
        FullABIFunction::new(
            abi_function.name.clone(),
            inputs,
            FullTypeApplication::from_counterpart(&abi_function.output, types),
            attributes,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FullTypeDeclaration {
    pub type_field: String,
    pub components: Vec<FullTypeApplication>,
    pub type_parameters: Vec<FullTypeDeclaration>,
}

impl FullTypeDeclaration {
    pub fn from_counterpart(
        type_decl: &UnifiedTypeDeclaration,
        types: &HashMap<usize, UnifiedTypeDeclaration>,
    ) -> FullTypeDeclaration {
        let components = type_decl
            .components
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|application| FullTypeApplication::from_counterpart(&application, types))
            .collect();
        let type_parameters = type_decl
            .type_parameters
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|id| FullTypeDeclaration::from_counterpart(types.get(&id).unwrap(), types))
            .collect();
        FullTypeDeclaration {
            type_field: type_decl.type_field.clone(),
            components,
            type_parameters,
        }
    }

    pub fn custom_type_path(&self) -> Result<TypePath> {
        let type_field = &self.type_field;
        let type_name = extract_custom_type_name(type_field)
            .ok_or_else(|| error!("Couldn't extract custom type path from '{type_field}'"))?;

        TypePath::new(type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FullTypeApplication {
    pub name: String,
    pub type_decl: FullTypeDeclaration,
    pub type_arguments: Vec<FullTypeApplication>,
}

impl FullTypeApplication {
    pub fn from_counterpart(
        type_application: &UnifiedTypeApplication,
        types: &HashMap<usize, UnifiedTypeDeclaration>,
    ) -> FullTypeApplication {
        let type_arguments = type_application
            .type_arguments
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|application| FullTypeApplication::from_counterpart(&application, types))
            .collect();

        let type_decl = FullTypeDeclaration::from_counterpart(
            types.get(&type_application.type_id).unwrap(),
            types,
        );

        FullTypeApplication {
            name: type_application.name.clone(),
            type_decl,
            type_arguments,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FullLoggedType {
    pub log_id: String,
    pub application: FullTypeApplication,
}

impl FullLoggedType {
    fn from_counterpart(
        logged_type: &UnifiedLoggedType,
        types: &HashMap<usize, UnifiedTypeDeclaration>,
    ) -> FullLoggedType {
        FullLoggedType {
            log_id: logged_type.log_id.clone(),
            application: FullTypeApplication::from_counterpart(&logged_type.application, types),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FullMessageType {
    pub log_id: String,
    pub application: FullTypeApplication,
}

impl FullMessageType {
    fn from_counterpart(
        message_type: &UnifiedMessageType,
        types: &HashMap<usize, UnifiedTypeDeclaration>,
    ) -> FullMessageType {
        FullMessageType {
            log_id: message_type.message_id.clone(),
            application: FullTypeApplication::from_counterpart(&message_type.application, types),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FullConfigurable {
    pub name: String,
    pub application: FullTypeApplication,
    pub offset: u64,
    pub indirect: bool,
}

impl FullConfigurable {
    pub fn from_counterpart(
        configurable: &UnifiedConfigurable,
        types: &HashMap<usize, UnifiedTypeDeclaration>,
    ) -> FullConfigurable {
        FullConfigurable {
            name: configurable.name.clone(),
            application: FullTypeApplication::from_counterpart(&configurable.application, types),
            offset: configurable.offset,
            indirect: configurable.indirect,
        }
    }
}

impl FullTypeDeclaration {
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn abi_function_cannot_have_an_empty_name() {
        let fn_output = FullTypeApplication {
            name: "".to_string(),
            type_decl: FullTypeDeclaration {
                type_field: "SomeType".to_string(),
                components: vec![],
                type_parameters: vec![],
            },
            type_arguments: vec![],
        };

        let err = FullABIFunction::new("".to_string(), vec![], fn_output, vec![])
            .expect_err("Should have failed.");

        assert_eq!(err.to_string(), "FullABIFunction's name cannot be empty!");
    }
    #[test]
    fn can_convert_into_full_type_decl() {
        // given
        let type_0 = UnifiedTypeDeclaration {
            type_id: 0,
            type_field: "type_0".to_string(),
            components: Some(vec![UnifiedTypeApplication {
                name: "type_0_component_a".to_string(),
                type_id: 1,
                type_arguments: Some(vec![UnifiedTypeApplication {
                    name: "type_0_type_arg_0".to_string(),
                    type_id: 2,
                    type_arguments: None,
                }]),
            }]),
            type_parameters: Some(vec![2]),
        };

        let type_1 = UnifiedTypeDeclaration {
            type_id: 1,
            type_field: "type_1".to_string(),
            components: None,
            type_parameters: None,
        };

        let type_2 = UnifiedTypeDeclaration {
            type_id: 2,
            type_field: "type_2".to_string(),
            components: None,
            type_parameters: None,
        };

        let types = [&type_0, &type_1, &type_2]
            .iter()
            .map(|&ttype| (ttype.type_id, ttype.clone()))
            .collect::<HashMap<_, _>>();

        // when
        let sut = FullTypeDeclaration::from_counterpart(&type_0, &types);

        // then
        let type_2_decl = FullTypeDeclaration {
            type_field: "type_2".to_string(),
            components: vec![],
            type_parameters: vec![],
        };
        assert_eq!(
            sut,
            FullTypeDeclaration {
                type_field: "type_0".to_string(),
                components: vec![FullTypeApplication {
                    name: "type_0_component_a".to_string(),
                    type_decl: FullTypeDeclaration {
                        type_field: "type_1".to_string(),
                        components: vec![],
                        type_parameters: vec![],
                    },
                    type_arguments: vec![FullTypeApplication {
                        name: "type_0_type_arg_0".to_string(),
                        type_decl: type_2_decl.clone(),
                        type_arguments: vec![],
                    },],
                },],
                type_parameters: vec![type_2_decl],
            }
        )
    }

    #[test]
    fn can_convert_into_full_type_appl() {
        let application = UnifiedTypeApplication {
            name: "ta_0".to_string(),
            type_id: 0,
            type_arguments: Some(vec![UnifiedTypeApplication {
                name: "ta_1".to_string(),
                type_id: 1,
                type_arguments: None,
            }]),
        };

        let type_0 = UnifiedTypeDeclaration {
            type_id: 0,
            type_field: "type_0".to_string(),
            components: None,
            type_parameters: None,
        };

        let type_1 = UnifiedTypeDeclaration {
            type_id: 1,
            type_field: "type_1".to_string(),
            components: None,
            type_parameters: None,
        };

        let types = [&type_0, &type_1]
            .into_iter()
            .map(|ttype| (ttype.type_id, ttype.clone()))
            .collect::<HashMap<_, _>>();

        // given
        let sut = FullTypeApplication::from_counterpart(&application, &types);

        // then
        assert_eq!(
            sut,
            FullTypeApplication {
                name: "ta_0".to_string(),
                type_decl: FullTypeDeclaration {
                    type_field: "type_0".to_string(),
                    components: vec![],
                    type_parameters: vec![],
                },
                type_arguments: vec![FullTypeApplication {
                    name: "ta_1".to_string(),
                    type_decl: FullTypeDeclaration {
                        type_field: "type_1".to_string(),
                        components: vec![],
                        type_parameters: vec![],
                    },
                    type_arguments: vec![],
                },],
            }
        )
    }
}
