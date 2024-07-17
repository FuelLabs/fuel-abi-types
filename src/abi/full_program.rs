use std::collections::HashMap;

use crate::{
    abi::program::{
        ABIFunction, Attribute, Configurable, LoggedType, ProgramABI, TypeApplication,
        TypeMetadataDeclaration,
    },
    utils::extract_custom_type_name,
};

use crate::{
    error::{error, Error, Result},
    utils::TypePath,
};

use super::program::{ConcreteTypeId, TypeConcreteDeclaration, TypeId, Version};

/// 'Full' versions of the ABI structures are needed to simplify duplicate
/// detection later on. The original ones([`ProgramABI`], [`TypeApplication`],
/// [`TypeDeclaration`] and others) are not suited for this due to their use of
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
    pub configurables: Vec<FullConfigurable>,
}

impl FullProgramABI {
    pub fn from_json_abi(abi: &str) -> Result<Self> {
        let parsed_abi: ProgramABI = serde_json::from_str(abi)?;
        FullProgramABI::from_counterpart(&parsed_abi)
    }

    fn from_counterpart(program_abi: &ProgramABI) -> Result<FullProgramABI> {
        let mut metadata_types_lookup: HashMap<_, _> = program_abi
            .types_metadata
            .iter()
            .map(|ttype| {
                (
                    TypeId::Metadata(ttype.metadata_type_id.clone()),
                    ttype.clone(),
                )
            })
            .collect();

        let original_metadata_types_lookup = metadata_types_lookup.clone();

        //Extends lookup table with TypeMetadataDeclaration for concrete types.
        metadata_types_lookup.extend(program_abi.concrete_types.iter().map(|ctype| {
            if let Some(metadata_type_id) = &ctype.metadata_type_id {
                (
                    TypeId::Concrete(ctype.concrete_type_id.clone()),
                    original_metadata_types_lookup
                        .get(&TypeId::Metadata(metadata_type_id.clone()))
                        .unwrap()
                        .clone(),
                )
            } else {
                (
                    TypeId::Concrete(ctype.concrete_type_id.clone()),
                    TypeMetadataDeclaration {
                        type_field: ctype.type_field.clone(),
                        metadata_type_id: Default::default(), //This should not be used anymore.
                        components: None,
                        type_parameters: None,
                    },
                )
            }
        }));

        let concrete_types_lookup: HashMap<_, _> = program_abi
            .concrete_types
            .iter()
            .map(|ctype| (ctype.concrete_type_id.clone(), ctype.clone()))
            .collect();

        let mut types: Vec<FullTypeDeclaration> = program_abi
            .types_metadata
            .iter()
            .map(|ttype| {
                FullTypeDeclaration::from_metadata_declaration(
                    ttype,
                    &metadata_types_lookup,
                    &concrete_types_lookup,
                )
            })
            .collect();

        //Extends lookup table with TypeMetadataDeclaration for concrete types.
        //This will add TypeMetadataDeclaration for built in types.
        types.extend(program_abi.concrete_types.iter().filter_map(|ctype| {
            if ctype.metadata_type_id.is_none() {
                Some(FullTypeDeclaration::from_metadata_declaration(
                    &TypeMetadataDeclaration {
                        type_field: ctype.type_field.clone(),
                        metadata_type_id: Default::default(), //This should not be used anymore.
                        components: None,
                        type_parameters: None,
                    },
                    &metadata_types_lookup,
                    &concrete_types_lookup,
                ))
            } else {
                None
            }
        }));

        let functions = program_abi
            .functions
            .iter()
            .map(|fun| {
                FullABIFunction::from_counterpart(
                    fun,
                    &metadata_types_lookup,
                    &concrete_types_lookup,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        let logged_types = program_abi
            .logged_types
            .iter()
            .flatten()
            .map(|logged_type| {
                FullLoggedType::from_counterpart(
                    logged_type,
                    &metadata_types_lookup,
                    &concrete_types_lookup,
                )
            })
            .collect();

        let configurables = program_abi
            .configurables
            .iter()
            .flatten()
            .map(|configurable| {
                FullConfigurable::from_counterpart(
                    configurable,
                    &metadata_types_lookup,
                    &concrete_types_lookup,
                )
            })
            .collect();

        Ok(Self {
            encoding_version: program_abi.encoding_version.clone(),
            spec_version: program_abi.spec_version.clone(),
            program_type: program_abi.program_type.clone(),
            types,
            functions,
            logged_types,
            configurables,
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
        abi_function: &ABIFunction,
        metadata_types: &HashMap<TypeId, TypeMetadataDeclaration>,
        concrete_types: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> Result<FullABIFunction> {
        let inputs = abi_function
            .inputs
            .iter()
            .map(|input| {
                FullTypeApplication::from_concrete_type_id(
                    input.name.clone(),
                    &input.concrete_type_id,
                    metadata_types,
                    concrete_types,
                )
            })
            .collect();

        let attributes = abi_function
            .attributes
            .as_ref()
            .map_or(vec![], Clone::clone);
        FullABIFunction::new(
            abi_function.name.clone(),
            inputs,
            FullTypeApplication::from_concrete_type_id(
                "".to_string(),
                &abi_function.output,
                metadata_types,
                concrete_types,
            ),
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
    pub fn from_metadata_declaration(
        type_decl: &TypeMetadataDeclaration,
        metadata_types: &HashMap<TypeId, TypeMetadataDeclaration>,
        concrete_types: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> FullTypeDeclaration {
        let components = type_decl
            .components
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|application| {
                FullTypeApplication::from_counterpart(&application, metadata_types, concrete_types)
            })
            .collect();
        let type_parameters = type_decl
            .type_parameters
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|id| {
                FullTypeDeclaration::from_metadata_declaration(
                    metadata_types.get(&TypeId::Metadata(id)).unwrap(),
                    metadata_types,
                    concrete_types,
                )
            })
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
        type_application: &TypeApplication,
        metadata_types: &HashMap<TypeId, TypeMetadataDeclaration>,
        concrete_types: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> FullTypeApplication {
        let type_arguments = type_application
            .type_arguments
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|application| {
                FullTypeApplication::from_counterpart(&application, metadata_types, concrete_types)
            })
            .collect();

        let type_decl = FullTypeDeclaration::from_metadata_declaration(
            metadata_types.get(&type_application.type_id).unwrap(),
            metadata_types,
            concrete_types,
        );

        FullTypeApplication {
            name: type_application.name.clone(),
            type_decl,
            type_arguments,
        }
    }

    pub fn from_concrete_type_id(
        name: String,
        concrete_type_id: &ConcreteTypeId,
        metadata_types: &HashMap<TypeId, TypeMetadataDeclaration>,
        concrete_types: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> FullTypeApplication {
        let concrete_decl = concrete_types.get(concrete_type_id).unwrap();

        if concrete_decl.metadata_type_id.is_none() {
            assert!(concrete_decl.type_arguments.is_none(),"When concrete_decl.metadata_type_id is none, concrete_decl.type_arguments must also be none.");
            return FullTypeApplication {
                name,
                type_decl: FullTypeDeclaration::from_metadata_declaration(
                    metadata_types
                        .get(&TypeId::Concrete(concrete_type_id.clone()))
                        .unwrap(),
                    metadata_types,
                    concrete_types,
                ),
                type_arguments: vec![],
            };
        }

        let type_arguments = concrete_decl
            .type_arguments
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|ctype_id| {
                FullTypeApplication::from_concrete_type_id(
                    "".to_string(),
                    &ctype_id,
                    metadata_types,
                    concrete_types,
                )
            })
            .collect();

        let type_decl = FullTypeDeclaration::from_metadata_declaration(
            metadata_types
                .get(&TypeId::Metadata(
                    concrete_decl.metadata_type_id.clone().unwrap(),
                ))
                .unwrap(),
            metadata_types,
            concrete_types,
        );

        FullTypeApplication {
            name,
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
        logged_type: &LoggedType,
        metadata_types: &HashMap<TypeId, TypeMetadataDeclaration>,
        concrete_types: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> FullLoggedType {
        FullLoggedType {
            log_id: logged_type.log_id.clone(),
            application: FullTypeApplication::from_concrete_type_id(
                "".to_string(),
                &logged_type.concrete_type_id,
                metadata_types,
                concrete_types,
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FullConfigurable {
    pub name: String,
    pub application: FullTypeApplication,
    pub offset: u64,
}

impl FullConfigurable {
    pub fn from_counterpart(
        configurable: &Configurable,
        metadata_types: &HashMap<TypeId, TypeMetadataDeclaration>,
        concrete_types: &HashMap<ConcreteTypeId, TypeConcreteDeclaration>,
    ) -> FullConfigurable {
        FullConfigurable {
            name: configurable.name.clone(),
            application: FullTypeApplication::from_concrete_type_id(
                configurable.name.clone(),
                &configurable.concrete_type_id,
                metadata_types,
                concrete_types,
            ),
            offset: configurable.offset,
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

    use crate::abi::program::MetadataTypeId;

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
        let type_0 = TypeMetadataDeclaration {
            metadata_type_id: MetadataTypeId(0),
            type_field: "type_0".to_string(),
            components: Some(vec![TypeApplication {
                name: "type_0_component_a".to_string(),
                type_id: TypeId::Metadata(MetadataTypeId(1)),
                type_arguments: Some(vec![TypeApplication {
                    name: "type_0_type_arg_0".to_string(),
                    type_id: TypeId::Metadata(MetadataTypeId(2)),
                    type_arguments: None,
                }]),
            }]),
            type_parameters: Some(vec![MetadataTypeId(2)]),
        };

        let type_1 = TypeMetadataDeclaration {
            metadata_type_id: MetadataTypeId(1),
            type_field: "type_1".to_string(),
            components: None,
            type_parameters: None,
        };

        let type_2 = TypeMetadataDeclaration {
            metadata_type_id: MetadataTypeId(2),
            type_field: "type_2".to_string(),
            components: None,
            type_parameters: None,
        };

        let metadata_types = [&type_0, &type_1, &type_2]
            .iter()
            .map(|&ttype| {
                (
                    TypeId::Metadata(ttype.metadata_type_id.clone()),
                    ttype.clone(),
                )
            })
            .collect::<HashMap<_, _>>();

        // when
        let sut = FullTypeDeclaration::from_metadata_declaration(
            &type_0,
            &metadata_types,
            &HashMap::<ConcreteTypeId, TypeConcreteDeclaration>::new(),
        );

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
        let application = TypeApplication {
            name: "ta_0".to_string(),
            type_id: TypeId::Metadata(MetadataTypeId(0)),
            type_arguments: Some(vec![TypeApplication {
                name: "ta_1".to_string(),
                type_id: TypeId::Metadata(MetadataTypeId(1)),
                type_arguments: None,
            }]),
        };

        let type_0 = TypeMetadataDeclaration {
            metadata_type_id: MetadataTypeId(0),
            type_field: "type_0".to_string(),
            components: None,
            type_parameters: None,
        };

        let type_1 = TypeMetadataDeclaration {
            metadata_type_id: MetadataTypeId(1),
            type_field: "type_1".to_string(),
            components: None,
            type_parameters: None,
        };

        let types = [&type_0, &type_1]
            .into_iter()
            .map(|ttype| {
                (
                    TypeId::Metadata(ttype.metadata_type_id.clone()),
                    ttype.clone(),
                )
            })
            .collect::<HashMap<_, _>>();

        // given
        let sut = FullTypeApplication::from_counterpart(
            &application,
            &types,
            &HashMap::<ConcreteTypeId, TypeConcreteDeclaration>::new(),
        );

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
