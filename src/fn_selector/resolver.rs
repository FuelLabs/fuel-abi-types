use std::collections::HashMap;

use itertools::Itertools;
use sha2::{Digest, Sha256};

use crate::error_codes;
use crate::fn_selector::resolved_type::ResolvedType;
use crate::program_abi::{TypeApplication, TypeDeclaration};
use crate::utils::extract_array_len;

/// Hashes an encoded function selector using SHA256 and returns the first 4 bytes.
/// The function selector has to have been already encoded following the ABI specs defined
/// [here](https://github.com/FuelLabs/fuel-specs/blob/1be31f70c757d8390f74b9e1b3beb096620553eb/specs/protocol/abi.md)
pub fn first_four_bytes_of_sha256_hash(string: &str) -> [u8; 4] {
    let string_as_bytes = string.as_bytes();
    let mut hasher = Sha256::default();
    hasher.update(string_as_bytes);
    let result = hasher.finalize();
    let mut output = [0; 4];
    output[4..].copy_from_slice(&result[..4]);
    output
}

pub fn resolve_fn_selector(
    name: &str,
    arguments: &[TypeApplication],
    type_lookup: &HashMap<usize, TypeDeclaration>,
) -> error_codes::Result<[u8; 4]> {
    let fn_signature = resolve_fn_signature(name, arguments, type_lookup)?;

    Ok(first_four_bytes_of_sha256_hash(&fn_signature))
}

/// For when you need to convert a ABI JSON's TypeApplication into a ParamType.
///
/// # Arguments
///
/// * `type_application`: The TypeApplication you wish to convert into a ParamType
/// * `type_lookup`: A HashMap of TypeDeclarations mentioned in the
///                  TypeApplication where the type id is the key.
fn resolve_fn_signature(
    name: &str,
    arguments: &[TypeApplication],
    type_lookup: &HashMap<usize, TypeDeclaration>,
) -> error_codes::Result<String> {
    let types = arguments
        .iter()
        .map(|ta| ResolvedType::try_from(ta, type_lookup))
        .map_ok(|t| fnselectify(&t))
        .collect::<error_codes::Result<Vec<_>>>()?
        .join(",");

    Ok(format!("{name}({types})"))
}

fn fnselectify(ttype: &ResolvedType) -> String {
    match ttype.type_field.as_ref() {
        "raw untyped slice" => "rawslice".into(),
        "raw untyped ptr" => "rawptr".into(),
        struct_typef if struct_typef.starts_with("struct ") => {
            let strings = ttype.components.iter().map(fnselectify).join(",");
            let generics = ttype.generic_params.iter().map(fnselectify).join(",");

            let generics_str = if generics.is_empty() {
                "".into()
            } else {
                format!("<{generics}>")
            };

            format!("s{generics_str}({strings})")
        }
        enum_typef if enum_typef.starts_with("enum ") => {
            let strings = ttype.components.iter().map(fnselectify).join(",");
            let generics = ttype.generic_params.iter().map(fnselectify).join(",");

            let generics_str = if generics.is_empty() {
                "".into()
            } else {
                format!("<{generics}>")
            };

            format!("e{generics_str}({strings})")
        }
        tuple_typef if tuple_typef.starts_with('(') && tuple_typef.ends_with(')') => {
            let strings = ttype.components.iter().map(fnselectify).join(",");

            format!("({strings})")
        }
        array_typef if array_typef.starts_with('[') && array_typef.ends_with(']') => {
            let inner_array_type = ttype
                .components
                .iter()
                .map(fnselectify)
                .next()
                .expect("should have");

            //TODO: when moved to fuel-abi-types refactor this to avoid matching array twice
            let len = extract_array_len(&ttype.type_field).expect("should be ok");

            format!("a[{inner_array_type};{len}]")
        }
        _ => ttype.type_field.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::error_codes::Result;
    use crate::fn_selector::resolver::resolve_fn_signature;
    use crate::program_abi::{TypeApplication, TypeDeclaration};

    #[test]
    fn handles_simple_types() -> Result<()> {
        let selector_for = |fn_name: &str, type_field: &str| {
            let type_application = TypeApplication {
                name: "arg0".to_string(),
                type_id: 0,
                type_arguments: None,
            };

            let declarations = [TypeDeclaration {
                type_id: 0,
                type_field: type_field.to_string(),
                components: None,
                type_parameters: None,
            }];

            let type_lookup = declarations
                .into_iter()
                .map(|decl| (decl.type_id, decl))
                .collect::<HashMap<_, _>>();

            resolve_fn_signature(fn_name, &[type_application], &type_lookup)
        };

        assert_eq!(selector_for("some_fn", "u8")?, "some_fn(u8)");
        assert_eq!(selector_for("some_fn", "u16")?, "some_fn(u16)");
        assert_eq!(selector_for("some_fn", "u32")?, "some_fn(u32)");
        assert_eq!(selector_for("some_fn", "u64")?, "some_fn(u64)");
        assert_eq!(selector_for("some_fn", "bool")?, "some_fn(bool)");
        assert_eq!(selector_for("some_fn", "b256")?, "some_fn(b256)");
        assert_eq!(selector_for("some_fn", "()")?, "some_fn(())");
        assert_eq!(selector_for("some_fn", "str[21]")?, "some_fn(str[21])");

        Ok(())
    }

    #[test]
    fn handles_arrays() -> Result<()> {
        // given
        let type_application = TypeApplication {
            name: "arg0".to_string(),
            type_id: 0,
            type_arguments: None,
        };

        let declarations = [
            TypeDeclaration {
                type_id: 0,
                type_field: "[_; 10]".to_string(),
                components: Some(vec![TypeApplication {
                    name: "__array_element".to_string(),
                    type_id: 1,
                    type_arguments: None,
                }]),
                type_parameters: None,
            },
            TypeDeclaration {
                type_id: 1,
                type_field: "u8".to_string(),
                components: None,
                type_parameters: None,
            },
        ];

        let type_lookup = declarations
            .into_iter()
            .map(|decl| (decl.type_id, decl))
            .collect::<HashMap<_, _>>();

        // when
        let selector = resolve_fn_signature("some_fun", &[type_application], &type_lookup)?;

        // then
        assert_eq!(selector, "some_fun(a[u8;10])");

        Ok(())
    }

    // #[test]
    // fn handles_vectors() -> Result<()> {
    //     // given
    //     let declarations = [
    //         TypeDeclaration {
    //             type_id: 1,
    //             type_field: "generic T".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 2,
    //             type_field: "raw untyped ptr".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 3,
    //             type_field: "struct std::vec::RawVec".to_string(),
    //             components: Some(vec![
    //                 TypeApplication {
    //                     name: "ptr".to_string(),
    //                     type_id: 2,
    //                     type_arguments: None,
    //                 },
    //                 TypeApplication {
    //                     name: "cap".to_string(),
    //                     type_id: 5,
    //                     type_arguments: None,
    //                 },
    //             ]),
    //             type_parameters: Some(vec![1]),
    //         },
    //         TypeDeclaration {
    //             type_id: 4,
    //             type_field: "struct std::vec::Vec".to_string(),
    //             components: Some(vec![
    //                 TypeApplication {
    //                     name: "buf".to_string(),
    //                     type_id: 3,
    //                     type_arguments: Some(vec![TypeApplication {
    //                         name: "".to_string(),
    //                         type_id: 1,
    //                         type_arguments: None,
    //                     }]),
    //                 },
    //                 TypeApplication {
    //                     name: "len".to_string(),
    //                     type_id: 5,
    //                     type_arguments: None,
    //                 },
    //             ]),
    //             type_parameters: Some(vec![1]),
    //         },
    //         TypeDeclaration {
    //             type_id: 5,
    //             type_field: "u64".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 6,
    //             type_field: "u8".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //     ];

    //     let type_application = TypeApplication {
    //         name: "arg".to_string(),
    //         type_id: 4,
    //         type_arguments: Some(vec![TypeApplication {
    //             name: "".to_string(),
    //             type_id: 6,
    //             type_arguments: None,
    //         }]),
    //     };

    //     let type_lookup = declarations
    //         .into_iter()
    //         .map(|decl| (decl.type_id, decl))
    //         .collect::<HashMap<_, _>>();

    //     // when
    //     let result = ParamType::try_from_type_application(&type_application, &type_lookup)?;

    //     // then
    //     assert_eq!(result, ParamType::Vector(Box::new(ParamType::U8)));

    //     Ok(())
    // }

    #[test]
    fn handles_vectors_2() -> Result<()> {
        // given
        let declarations = [
            TypeDeclaration {
                type_id: 1,
                type_field: "generic T".to_string(),
                components: None,
                type_parameters: None,
            },
            TypeDeclaration {
                type_id: 2,
                type_field: "raw untyped ptr".to_string(),
                components: None,
                type_parameters: None,
            },
            TypeDeclaration {
                type_id: 3,
                type_field: "struct std::vec::RawVec".to_string(),
                components: Some(vec![
                    TypeApplication {
                        name: "ptr".to_string(),
                        type_id: 2,
                        type_arguments: None,
                    },
                    TypeApplication {
                        name: "cap".to_string(),
                        type_id: 5,
                        type_arguments: None,
                    },
                ]),
                type_parameters: Some(vec![1]),
            },
            TypeDeclaration {
                type_id: 4,
                type_field: "struct std::vec::Vec".to_string(),
                components: Some(vec![
                    TypeApplication {
                        name: "buf".to_string(),
                        type_id: 3,
                        type_arguments: Some(vec![TypeApplication {
                            name: "".to_string(),
                            type_id: 1,
                            type_arguments: None,
                        }]),
                    },
                    TypeApplication {
                        name: "len".to_string(),
                        type_id: 5,
                        type_arguments: None,
                    },
                ]),
                type_parameters: Some(vec![1]),
            },
            TypeDeclaration {
                type_id: 5,
                type_field: "u64".to_string(),
                components: None,
                type_parameters: None,
            },
            TypeDeclaration {
                type_id: 6,
                type_field: "u8".to_string(),
                components: None,
                type_parameters: None,
            },
        ];

        let type_application = TypeApplication {
            name: "arg".to_string(),
            type_id: 4,
            type_arguments: Some(vec![TypeApplication {
                name: "".to_string(),
                type_id: 6,
                type_arguments: None,
            }]),
        };

        let type_lookup = declarations
            .into_iter()
            .map(|decl| (decl.type_id, decl))
            .collect::<HashMap<_, _>>();

        // when
        let selector = resolve_fn_signature("some_fun", &[type_application], &type_lookup)?;
        dbg!(&selector);

        // then
        assert_eq!(selector, "some_fun(s<u8>(s<u8>(rawptr,u64),u64))");

        Ok(())
    }

    #[test]
    fn handles_structs() -> Result<()> {
        // given
        let custom_type = CustomType::new_struct("SomeStruct");
        // when
        let signature =
            resolve_fn_signature("some_fn", &custom_type.application, &custom_type.lookup())?;

        // then
        assert_eq!(signature, "some_fn(s(u8))");

        Ok(())
    }

    #[test]
    fn handles_generic_structs() -> Result<()> {
        // given
        let declarations = [
            TypeDeclaration {
                type_id: 1,
                type_field: "generic T".to_string(),
                components: None,
                type_parameters: None,
            },
            TypeDeclaration {
                type_id: 2,
                type_field: "struct SomeStruct".to_string(),
                components: Some(vec![TypeApplication {
                    name: "field".to_string(),
                    type_id: 1,
                    type_arguments: None,
                }]),
                type_parameters: Some(vec![1]),
            },
            TypeDeclaration {
                type_id: 3,
                type_field: "u8".to_string(),
                components: None,
                type_parameters: None,
            },
        ];

        let type_application = TypeApplication {
            name: "arg".to_string(),
            type_id: 2,
            type_arguments: Some(vec![TypeApplication {
                name: "".to_string(),
                type_id: 3,
                type_arguments: None,
            }]),
        };

        let type_lookup = declarations
            .into_iter()
            .map(|decl| (decl.type_id, decl))
            .collect::<HashMap<_, _>>();

        // when
        let signature = resolve_fn_signature("some_fn", &[type_application], &type_lookup)?;

        // then
        assert_eq!(signature, "some_fn(s<u8>(u8))");

        Ok(())
    }

    struct CustomType {
        decl: Vec<TypeDeclaration>,
        application: Vec<TypeApplication>,
    }

    impl CustomType {
        fn new_struct(name: &str) -> Self {
            let declarations = vec![
                TypeDeclaration {
                    type_id: 1,
                    type_field: "u8".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 2,
                    type_field: format!("struct {name}"),
                    components: Some(vec![TypeApplication {
                        name: "_0".to_string(),
                        type_id: 1,
                        type_arguments: None,
                    }]),
                    type_parameters: None,
                },
            ];

            let type_application = TypeApplication {
                name: "arg".to_string(),
                type_id: 2,
                type_arguments: Some(vec![TypeApplication {
                    name: "".to_string(),
                    type_id: 3,
                    type_arguments: None,
                }]),
            };

            Self {
                decl: declarations,
                application: vec![type_application],
            }
        }

        fn lookup(&self) -> HashMap<usize, TypeDeclaration> {
            self.decl
                .iter()
                .map(|decl| (decl.type_id, decl.clone()))
                .collect()
        }
    }

    #[test]
    fn handles_enums() -> Result<()> {
        // given
        let declarations = [
            TypeDeclaration {
                type_id: 1,
                type_field: "u8".to_string(),
                components: None,
                type_parameters: None,
            },
            TypeDeclaration {
                type_id: 2,
                type_field: "enum SomeEnum".to_string(),
                components: Some(vec![TypeApplication {
                    name: "field".to_string(),
                    type_id: 1,
                    type_arguments: None,
                }]),
                type_parameters: None,
            },
        ];

        let type_application = TypeApplication {
            name: "arg".to_string(),
            type_id: 2,
            type_arguments: None,
        };

        let type_lookup = declarations
            .into_iter()
            .map(|decl| (decl.type_id, decl))
            .collect::<HashMap<_, _>>();

        // when
        let signature = resolve_fn_signature("some_fn", &[type_application], &type_lookup)?;

        // then
        assert_eq!(signature, "some_fn(e(u8))");

        Ok(())
    }

    #[test]
    fn handles_generic_enums() -> Result<()> {
        // given
        let declarations = [
            TypeDeclaration {
                type_id: 1,
                type_field: "generic T".to_string(),
                components: None,
                type_parameters: None,
            },
            TypeDeclaration {
                type_id: 2,
                type_field: "enum SomeEnum".to_string(),
                components: Some(vec![TypeApplication {
                    name: "field".to_string(),
                    type_id: 1,
                    type_arguments: None,
                }]),
                type_parameters: Some(vec![1]),
            },
            TypeDeclaration {
                type_id: 3,
                type_field: "u8".to_string(),
                components: None,
                type_parameters: None,
            },
        ];

        let type_application = TypeApplication {
            name: "arg".to_string(),
            type_id: 2,
            type_arguments: Some(vec![TypeApplication {
                name: "".to_string(),
                type_id: 3,
                type_arguments: None,
            }]),
        };

        let type_lookup = declarations
            .into_iter()
            .map(|decl| (decl.type_id, decl))
            .collect::<HashMap<_, _>>();

        // when
        let signature = resolve_fn_signature("some_fn", &[type_application], &type_lookup)?;

        // then
        assert_eq!(signature, "some_fn(e<u8>(u8))");

        Ok(())
    }

    // #[test]
    // fn handles_tuples() -> Result<()> {
    //     // given
    //     let declarations = [
    //         TypeDeclaration {
    //             type_id: 1,
    //             type_field: "(_, _)".to_string(),
    //             components: Some(vec![
    //                 TypeApplication {
    //                     name: "__tuple_element".to_string(),
    //                     type_id: 3,
    //                     type_arguments: None,
    //                 },
    //                 TypeApplication {
    //                     name: "__tuple_element".to_string(),
    //                     type_id: 2,
    //                     type_arguments: None,
    //                 },
    //             ]),
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 2,
    //             type_field: "str[15]".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 3,
    //             type_field: "u8".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //     ];

    //     let type_application = TypeApplication {
    //         name: "arg".to_string(),
    //         type_id: 1,
    //         type_arguments: None,
    //     };
    //     let type_lookup = declarations
    //         .into_iter()
    //         .map(|decl| (decl.type_id, decl))
    //         .collect::<HashMap<_, _>>();

    //     // when
    //     let result = ParamType::try_from_type_application(&type_application, &type_lookup)?;

    //     // then
    //     assert_eq!(
    //         result,
    //         ParamType::Tuple(vec![ParamType::U8, ParamType::String(15)])
    //     );

    //     Ok(())
    // }

    // #[test]
    // fn ultimate_example() -> Result<()> {
    //     // given
    //     let declarations = [
    //         TypeDeclaration {
    //             type_id: 1,
    //             type_field: "(_, _)".to_string(),
    //             components: Some(vec![
    //                 TypeApplication {
    //                     name: "__tuple_element".to_string(),
    //                     type_id: 11,
    //                     type_arguments: None,
    //                 },
    //                 TypeApplication {
    //                     name: "__tuple_element".to_string(),
    //                     type_id: 11,
    //                     type_arguments: None,
    //                 },
    //             ]),
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 2,
    //             type_field: "(_, _)".to_string(),
    //             components: Some(vec![
    //                 TypeApplication {
    //                     name: "__tuple_element".to_string(),
    //                     type_id: 4,
    //                     type_arguments: None,
    //                 },
    //                 TypeApplication {
    //                     name: "__tuple_element".to_string(),
    //                     type_id: 24,
    //                     type_arguments: None,
    //                 },
    //             ]),
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 3,
    //             type_field: "(_, _)".to_string(),
    //             components: Some(vec![
    //                 TypeApplication {
    //                     name: "__tuple_element".to_string(),
    //                     type_id: 5,
    //                     type_arguments: None,
    //                 },
    //                 TypeApplication {
    //                     name: "__tuple_element".to_string(),
    //                     type_id: 13,
    //                     type_arguments: None,
    //                 },
    //             ]),
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 4,
    //             type_field: "[_; 1]".to_string(),
    //             components: Some(vec![TypeApplication {
    //                 name: "__array_element".to_string(),
    //                 type_id: 8,
    //                 type_arguments: Some(vec![TypeApplication {
    //                     name: "".to_string(),
    //                     type_id: 22,
    //                     type_arguments: Some(vec![TypeApplication {
    //                         name: "".to_string(),
    //                         type_id: 21,
    //                         type_arguments: Some(vec![TypeApplication {
    //                             name: "".to_string(),
    //                             type_id: 18,
    //                             type_arguments: Some(vec![TypeApplication {
    //                                 name: "".to_string(),
    //                                 type_id: 13,
    //                                 type_arguments: None,
    //                             }]),
    //                         }]),
    //                     }]),
    //                 }]),
    //             }]),
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 5,
    //             type_field: "[_; 2]".to_string(),
    //             components: Some(vec![TypeApplication {
    //                 name: "__array_element".to_string(),
    //                 type_id: 14,
    //                 type_arguments: None,
    //             }]),
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 6,
    //             type_field: "[_; 2]".to_string(),
    //             components: Some(vec![TypeApplication {
    //                 name: "__array_element".to_string(),
    //                 type_id: 10,
    //                 type_arguments: None,
    //             }]),
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 7,
    //             type_field: "b256".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 8,
    //             type_field: "enum EnumWGeneric".to_string(),
    //             components: Some(vec![
    //                 TypeApplication {
    //                     name: "a".to_string(),
    //                     type_id: 25,
    //                     type_arguments: None,
    //                 },
    //                 TypeApplication {
    //                     name: "b".to_string(),
    //                     type_id: 12,
    //                     type_arguments: None,
    //                 },
    //             ]),
    //             type_parameters: Some(vec![12]),
    //         },
    //         TypeDeclaration {
    //             type_id: 9,
    //             type_field: "generic K".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 10,
    //             type_field: "generic L".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 11,
    //             type_field: "generic M".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 12,
    //             type_field: "generic N".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 13,
    //             type_field: "generic T".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 14,
    //             type_field: "generic U".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 15,
    //             type_field: "raw untyped ptr".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 16,
    //             type_field: "str[2]".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 17,
    //             type_field: "struct MegaExample".to_string(),
    //             components: Some(vec![
    //                 TypeApplication {
    //                     name: "a".to_string(),
    //                     type_id: 3,
    //                     type_arguments: None,
    //                 },
    //                 TypeApplication {
    //                     name: "b".to_string(),
    //                     type_id: 23,
    //                     type_arguments: Some(vec![TypeApplication {
    //                         name: "".to_string(),
    //                         type_id: 2,
    //                         type_arguments: None,
    //                     }]),
    //                 },
    //             ]),
    //             type_parameters: Some(vec![13, 14]),
    //         },
    //         TypeDeclaration {
    //             type_id: 18,
    //             type_field: "struct PassTheGenericOn".to_string(),
    //             components: Some(vec![TypeApplication {
    //                 name: "one".to_string(),
    //                 type_id: 20,
    //                 type_arguments: Some(vec![TypeApplication {
    //                     name: "".to_string(),
    //                     type_id: 9,
    //                     type_arguments: None,
    //                 }]),
    //             }]),
    //             type_parameters: Some(vec![9]),
    //         },
    //         TypeDeclaration {
    //             type_id: 19,
    //             type_field: "struct std::vec::RawVec".to_string(),
    //             components: Some(vec![
    //                 TypeApplication {
    //                     name: "ptr".to_string(),
    //                     type_id: 15,
    //                     type_arguments: None,
    //                 },
    //                 TypeApplication {
    //                     name: "cap".to_string(),
    //                     type_id: 25,
    //                     type_arguments: None,
    //                 },
    //             ]),
    //             type_parameters: Some(vec![13]),
    //         },
    //         TypeDeclaration {
    //             type_id: 20,
    //             type_field: "struct SimpleGeneric".to_string(),
    //             components: Some(vec![TypeApplication {
    //                 name: "single_generic_param".to_string(),
    //                 type_id: 13,
    //                 type_arguments: None,
    //             }]),
    //             type_parameters: Some(vec![13]),
    //         },
    //         TypeDeclaration {
    //             type_id: 21,
    //             type_field: "struct StructWArrayGeneric".to_string(),
    //             components: Some(vec![TypeApplication {
    //                 name: "a".to_string(),
    //                 type_id: 6,
    //                 type_arguments: None,
    //             }]),
    //             type_parameters: Some(vec![10]),
    //         },
    //         TypeDeclaration {
    //             type_id: 22,
    //             type_field: "struct StructWTupleGeneric".to_string(),
    //             components: Some(vec![TypeApplication {
    //                 name: "a".to_string(),
    //                 type_id: 1,
    //                 type_arguments: None,
    //             }]),
    //             type_parameters: Some(vec![11]),
    //         },
    //         TypeDeclaration {
    //             type_id: 23,
    //             type_field: "struct std::vec::Vec".to_string(),
    //             components: Some(vec![
    //                 TypeApplication {
    //                     name: "buf".to_string(),
    //                     type_id: 19,
    //                     type_arguments: Some(vec![TypeApplication {
    //                         name: "".to_string(),
    //                         type_id: 13,
    //                         type_arguments: None,
    //                     }]),
    //                 },
    //                 TypeApplication {
    //                     name: "len".to_string(),
    //                     type_id: 25,
    //                     type_arguments: None,
    //                 },
    //             ]),
    //             type_parameters: Some(vec![13]),
    //         },
    //         TypeDeclaration {
    //             type_id: 24,
    //             type_field: "u32".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //         TypeDeclaration {
    //             type_id: 25,
    //             type_field: "u64".to_string(),
    //             components: None,
    //             type_parameters: None,
    //         },
    //     ];

    //     let type_lookup = declarations
    //         .into_iter()
    //         .map(|decl| (decl.type_id, decl))
    //         .collect::<HashMap<_, _>>();

    //     let type_application = TypeApplication {
    //         name: "arg1".to_string(),
    //         type_id: 17,
    //         type_arguments: Some(vec![
    //             TypeApplication {
    //                 name: "".to_string(),
    //                 type_id: 16,
    //                 type_arguments: None,
    //             },
    //             TypeApplication {
    //                 name: "".to_string(),
    //                 type_id: 7,
    //                 type_arguments: None,
    //             },
    //         ]),
    //     };

    //     // when
    //     let result = ParamType::try_from_type_application(&type_application, &type_lookup)?;

    //     // then
    //     let expected_param_type = {
    //         let fields = vec![ParamType::Struct {
    //             fields: vec![ParamType::String(2)],
    //             generics: vec![ParamType::String(2)],
    //         }];
    //         let pass_the_generic_on = ParamType::Struct {
    //             fields,
    //             generics: vec![ParamType::String(2)],
    //         };

    //         let fields = vec![ParamType::Array(Box::from(pass_the_generic_on.clone()), 2)];
    //         let struct_w_array_generic = ParamType::Struct {
    //             fields,
    //             generics: vec![pass_the_generic_on],
    //         };

    //         let fields = vec![ParamType::Tuple(vec![
    //             struct_w_array_generic.clone(),
    //             struct_w_array_generic.clone(),
    //         ])];
    //         let struct_w_tuple_generic = ParamType::Struct {
    //             fields,
    //             generics: vec![struct_w_array_generic],
    //         };

    //         let types = vec![ParamType::U64, struct_w_tuple_generic.clone()];
    //         let fields = vec![
    //             ParamType::Tuple(vec![
    //                 ParamType::Array(Box::from(ParamType::B256), 2),
    //                 ParamType::String(2),
    //             ]),
    //             ParamType::Vector(Box::from(ParamType::Tuple(vec![
    //                 ParamType::Array(
    //                     Box::from(ParamType::Enum {
    //                         variants: EnumVariants::new(types).unwrap(),
    //                         generics: vec![struct_w_tuple_generic],
    //                     }),
    //                     1,
    //                 ),
    //                 ParamType::U32,
    //             ]))),
    //         ];
    //         ParamType::Struct {
    //             fields,
    //             generics: vec![ParamType::String(2), ParamType::B256],
    //         }
    //     };

    //     assert_eq!(result, expected_param_type);

    //     Ok(())
    // }
    // #[test]
    // fn contains_nested_heap_types_false_on_simple_types() -> Result<()> {
    //     // Simple types cannot have nested heap types
    //     assert!(!ParamType::Unit.contains_nested_heap_types());
    //     assert!(!ParamType::U8.contains_nested_heap_types());
    //     assert!(!ParamType::U16.contains_nested_heap_types());
    //     assert!(!ParamType::U32.contains_nested_heap_types());
    //     assert!(!ParamType::U64.contains_nested_heap_types());
    //     assert!(!ParamType::Bool.contains_nested_heap_types());
    //     assert!(!ParamType::B256.contains_nested_heap_types());
    //     assert!(!ParamType::String(10).contains_nested_heap_types());
    //     assert!(!ParamType::RawSlice.contains_nested_heap_types());
    //     assert!(!ParamType::Bytes.contains_nested_heap_types());
    //     Ok(())
    // }
    //
    // #[test]
    // fn test_complex_types_for_nested_heap_types_containing_vectors() -> Result<()> {
    //     let base_vector = ParamType::Vector(Box::from(ParamType::U8));
    //     let param_types_no_nested_vec = vec![ParamType::U64, ParamType::U32];
    //     let param_types_nested_vec = vec![ParamType::Unit, ParamType::Bool, base_vector.clone()];
    //
    //     let is_nested = |param_type: ParamType| assert!(param_type.contains_nested_heap_types());
    //     let not_nested = |param_type: ParamType| assert!(!param_type.contains_nested_heap_types());
    //
    //     not_nested(base_vector.clone());
    //     is_nested(ParamType::Vector(Box::from(base_vector.clone())));
    //
    //     not_nested(ParamType::Array(Box::from(ParamType::U8), 10));
    //     is_nested(ParamType::Array(Box::from(base_vector), 10));
    //
    //     not_nested(ParamType::Tuple(param_types_no_nested_vec.clone()));
    //     is_nested(ParamType::Tuple(param_types_nested_vec.clone()));
    //
    //     not_nested(ParamType::Struct {
    //         generics: param_types_no_nested_vec.clone(),
    //         fields: param_types_no_nested_vec.clone(),
    //     });
    //     is_nested(ParamType::Struct {
    //         generics: param_types_nested_vec.clone(),
    //         fields: param_types_no_nested_vec.clone(),
    //     });
    //     is_nested(ParamType::Struct {
    //         generics: param_types_no_nested_vec.clone(),
    //         fields: param_types_nested_vec.clone(),
    //     });
    //
    //     not_nested(ParamType::Enum {
    //         variants: EnumVariants::new(param_types_no_nested_vec.clone())?,
    //         generics: param_types_no_nested_vec.clone(),
    //     });
    //     is_nested(ParamType::Enum {
    //         variants: EnumVariants::new(param_types_nested_vec.clone())?,
    //         generics: param_types_no_nested_vec.clone(),
    //     });
    //     is_nested(ParamType::Enum {
    //         variants: EnumVariants::new(param_types_no_nested_vec)?,
    //         generics: param_types_nested_vec,
    //     });
    //     Ok(())
    // }
    //
    // #[test]
    // fn test_complex_types_for_nested_heap_types_containing_bytes() -> Result<()> {
    //     let base_bytes = ParamType::Bytes;
    //     let param_types_no_nested_bytes = vec![ParamType::U64, ParamType::U32];
    //     let param_types_nested_bytes = vec![ParamType::Unit, ParamType::Bool, base_bytes.clone()];
    //
    //     let is_nested = |param_type: ParamType| assert!(param_type.contains_nested_heap_types());
    //     let not_nested = |param_type: ParamType| assert!(!param_type.contains_nested_heap_types());
    //
    //     not_nested(base_bytes.clone());
    //     is_nested(ParamType::Vector(Box::from(base_bytes.clone())));
    //
    //     not_nested(ParamType::Array(Box::from(ParamType::U8), 10));
    //     is_nested(ParamType::Array(Box::from(base_bytes), 10));
    //
    //     not_nested(ParamType::Tuple(param_types_no_nested_bytes.clone()));
    //     is_nested(ParamType::Tuple(param_types_nested_bytes.clone()));
    //
    //     let not_nested_struct = ParamType::Struct {
    //         generics: param_types_no_nested_bytes.clone(),
    //         fields: param_types_no_nested_bytes.clone(),
    //     };
    //     not_nested(not_nested_struct);
    //
    //     let nested_struct = ParamType::Struct {
    //         generics: param_types_nested_bytes.clone(),
    //         fields: param_types_no_nested_bytes.clone(),
    //     };
    //     is_nested(nested_struct);
    //
    //     let nested_struct = ParamType::Struct {
    //         generics: param_types_no_nested_bytes.clone(),
    //         fields: param_types_nested_bytes.clone(),
    //     };
    //     is_nested(nested_struct);
    //
    //     let not_nested_enum = ParamType::Enum {
    //         variants: EnumVariants::new(param_types_no_nested_bytes.clone())?,
    //         generics: param_types_no_nested_bytes.clone(),
    //     };
    //     not_nested(not_nested_enum);
    //
    //     let nested_enum = ParamType::Enum {
    //         variants: EnumVariants::new(param_types_nested_bytes.clone())?,
    //         generics: param_types_no_nested_bytes.clone(),
    //     };
    //     is_nested(nested_enum);
    //
    //     let nested_enum = ParamType::Enum {
    //         variants: EnumVariants::new(param_types_no_nested_bytes)?,
    //         generics: param_types_nested_bytes,
    //     };
    //     is_nested(nested_enum);
    //
    //     Ok(())
    // }
    //
    // #[test]
    // fn try_vector_is_type_path_backward_compatible() {
    //     // TODO: To be removed once https://github.com/FuelLabs/fuels-rs/issues/881 is unblocked.
    //     let the_type = given_vec_type_w_path("Vec");
    //
    //     let param_type = try_vector(&the_type).unwrap().unwrap();
    //
    //     assert_eq!(param_type, ParamType::Vector(Box::new(ParamType::U8)));
    // }
    //
    // #[test]
    // fn try_vector_correctly_resolves_param_type() {
    //     let the_type = given_vec_type_w_path("std::vec::Vec");
    //
    //     let param_type = try_vector(&the_type).unwrap().unwrap();
    //
    //     assert_eq!(param_type, ParamType::Vector(Box::new(ParamType::U8)));
    // }
    //
    // fn given_vec_type_w_path(path: &str) -> Type {
    //     Type {
    //         type_field: format!("struct {path}"),
    //         generic_params: vec![Type {
    //             type_field: "u8".to_string(),
    //             generic_params: vec![],
    //             components: vec![],
    //         }],
    //         components: vec![],
    //     }
    // }
}
