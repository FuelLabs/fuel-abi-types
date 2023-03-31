use std::collections::HashMap;

use itertools::Itertools;
use sha2::{Digest, Sha256};

use crate::{
    error_codes,
    fn_selector::resolved_type::ResolvedType,
    program_abi::{TypeApplication, TypeDeclaration},
    utils::extract_array_len,
};

/// Used to hash an encoded function selector using SHA256 and returns the first 4 bytes.
/// The function selector has to have been already encoded following the ABI specs defined
/// [here](https://github.com/FuelLabs/fuel-specs/blob/1be31f70c757d8390f74b9e1b3beb096620553eb/specs/protocol/abi.md)
pub fn first_four_bytes_of_sha256_hash(string: &str) -> [u8; 4] {
    let mut hasher = Sha256::default();
    hasher.update(string.as_bytes());

    let result = hasher.finalize();

    result[..4].try_into().unwrap()
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
        .map_ok(|t| fn_signature_format(&t))
        .collect::<error_codes::Result<Vec<_>>>()?
        .join(",");

    Ok(format!("{name}({types})"))
}

fn fn_signature_format(ttype: &ResolvedType) -> String {
    match ttype.type_field.as_ref() {
        "raw untyped slice" => "rawslice".into(),
        "raw untyped ptr" => "rawptr".into(),
        struct_typef if struct_typef.starts_with("struct ") => {
            let strings = ttype.components.iter().map(fn_signature_format).join(",");
            let generics = ttype
                .generic_params
                .iter()
                .map(fn_signature_format)
                .join(",");

            let generics_str = if generics.is_empty() {
                "".into()
            } else {
                format!("<{generics}>")
            };

            format!("s{generics_str}({strings})")
        }
        enum_typef if enum_typef.starts_with("enum ") => {
            let strings = ttype.components.iter().map(fn_signature_format).join(",");
            let generics = ttype
                .generic_params
                .iter()
                .map(fn_signature_format)
                .join(",");

            let generics_str = if generics.is_empty() {
                "".into()
            } else {
                format!("<{generics}>")
            };

            format!("e{generics_str}({strings})")
        }
        tuple_typef if tuple_typef.starts_with('(') && tuple_typef.ends_with(')') => {
            let strings = ttype.components.iter().map(fn_signature_format).join(",");

            format!("({strings})")
        }
        array_typef if array_typef.starts_with('[') && array_typef.ends_with(']') => {
            let inner_array_type = ttype
                .components
                .iter()
                .map(fn_signature_format)
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

    use crate::{
        error_codes::Result,
        fn_selector::resolver::resolve_fn_signature,
        program_abi::{TypeApplication, TypeDeclaration},
    };

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
    fn handles_raw_ptr() {
        let raw_ptr = AbiStubs::new_raw_ptr();

        let signature =
            resolve_fn_signature("some_fn", &raw_ptr.applications, &raw_ptr.lookup()).unwrap();

        assert_eq!(signature, "some_fn(rawptr)");
    }

    #[test]
    fn handles_raw_slice() {
        let raw_slice = AbiStubs::new_raw_slice();

        let signature =
            resolve_fn_signature("some_fn", &raw_slice.applications, &raw_slice.lookup()).unwrap();

        assert_eq!(signature, "some_fn(rawslice)");
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

    #[test]
    fn handles_vectors() {
        let vector = AbiStubs::new_vector();

        let selector =
            resolve_fn_signature("some_fun", &vector.applications, &vector.lookup()).unwrap();

        assert_eq!(selector, "some_fun(s<u8>(s<u8>(rawptr,u64),u64))");
    }

    #[test]
    fn handles_structs() {
        let custom_type = AbiStubs::new_struct(false);

        let signature =
            resolve_fn_signature("some_fn", &custom_type.applications, &custom_type.lookup())
                .unwrap();

        assert_eq!(signature, "some_fn(s(u8))");
    }

    #[test]
    fn handles_generic_structs() -> Result<()> {
        let custom_struct = AbiStubs::new_struct(true);

        // when
        let signature = resolve_fn_signature(
            "some_fn",
            &custom_struct.applications,
            &custom_struct.lookup(),
        )?;

        // then
        assert_eq!(signature, "some_fn(s<u8>(u8))");

        Ok(())
    }

    #[test]
    fn handles_enums() -> Result<()> {
        // given
        let enum_custom_type = AbiStubs::new_enum(false);

        // when
        let signature = resolve_fn_signature(
            "some_fn",
            &enum_custom_type.applications,
            &enum_custom_type.lookup(),
        )?;

        // then
        assert_eq!(signature, "some_fn(e(u8))");

        Ok(())
    }

    #[test]
    fn handles_generic_enums() -> Result<()> {
        // given
        let custom_enum = AbiStubs::new_enum(true);

        // when
        let signature =
            resolve_fn_signature("some_fn", &custom_enum.applications, &custom_enum.lookup())?;

        // then
        assert_eq!(signature, "some_fn(e<u8>(u8))");

        Ok(())
    }

    #[test]
    fn handles_tuples() {
        let tuple = AbiStubs::new_tuple();

        let signature =
            resolve_fn_signature("some_fn", &tuple.applications, &tuple.lookup()).unwrap();

        assert_eq!(signature, "some_fn((u8,str[15]))")
    }

    #[test]
    fn handles_mega_case() {
        let mega_case = AbiStubs::new_mega_case();

        let signature =
            resolve_fn_signature("complex_test", &mega_case.applications, &mega_case.lookup())
                .unwrap();

        let expected_signature = "complex_test(s<str[2],b256>((a[b256;2],str[2]),s<(a[e<s<s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2])>((s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2]),s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2])))>(u64,s<s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2])>((s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2]),s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2]))));1],u32)>(s<(a[e<s<s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2])>((s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2]),s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2])))>(u64,s<s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2])>((s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2]),s<s<str[2]>(s<str[2]>(str[2]))>(a[s<str[2]>(s<str[2]>(str[2]));2]))));1],u32)>(rawptr,u64),u64)))";

        assert_eq!(signature, expected_signature);
    }

    struct AbiStubs {
        declarations: Vec<TypeDeclaration>,
        applications: Vec<TypeApplication>,
    }

    impl AbiStubs {
        fn new_struct(use_generics: bool) -> Self {
            let (declarations, custom_type_id) =
                Self::declarations_for_custom_type("struct SomeStruct".to_string(), use_generics);

            let type_application = Self::arg_for_type_id(custom_type_id, use_generics);

            Self {
                declarations,
                applications: vec![type_application],
            }
        }

        fn new_enum(use_generics: bool) -> Self {
            let (declarations, custom_type_id) =
                Self::declarations_for_custom_type("enum SomeEnum".to_string(), use_generics);

            let type_application = Self::arg_for_type_id(custom_type_id, use_generics);

            Self {
                declarations,
                applications: vec![type_application],
            }
        }

        fn new_raw_ptr() -> Self {
            let declarations = vec![TypeDeclaration {
                type_id: 1,
                type_field: "raw untyped ptr".to_string(),
                components: None,
                type_parameters: None,
            }];

            let application = TypeApplication {
                name: "arg".to_string(),
                type_id: 1,
                type_arguments: None,
            };

            Self {
                declarations,
                applications: vec![application],
            }
        }

        fn new_raw_slice() -> Self {
            let declarations = vec![TypeDeclaration {
                type_id: 1,
                type_field: "raw untyped slice".to_string(),
                components: None,
                type_parameters: None,
            }];

            let application = TypeApplication {
                name: "arg".to_string(),
                type_id: 1,
                type_arguments: None,
            };

            Self {
                declarations,
                applications: vec![application],
            }
        }

        fn new_vector() -> Self {
            // given
            let declarations = vec![
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

            let application = TypeApplication {
                name: "arg".to_string(),
                type_id: 4,
                type_arguments: Some(vec![TypeApplication {
                    name: "".to_string(),
                    type_id: 6,
                    type_arguments: None,
                }]),
            };

            Self {
                declarations,
                applications: vec![application],
            }
        }

        fn new_tuple() -> Self {
            let declarations = vec![
                TypeDeclaration {
                    type_id: 1,
                    type_field: "(_, _)".to_string(),
                    components: Some(vec![
                        TypeApplication {
                            name: "__tuple_element".to_string(),
                            type_id: 3,
                            type_arguments: None,
                        },
                        TypeApplication {
                            name: "__tuple_element".to_string(),
                            type_id: 2,
                            type_arguments: None,
                        },
                    ]),
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 2,
                    type_field: "str[15]".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 3,
                    type_field: "u8".to_string(),
                    components: None,
                    type_parameters: None,
                },
            ];

            let application = TypeApplication {
                name: "arg".to_string(),
                type_id: 1,
                type_arguments: None,
            };

            Self {
                declarations,
                applications: vec![application],
            }
        }

        fn new_mega_case() -> Self {
            let declarations = vec![
                TypeDeclaration {
                    type_id: 0,
                    type_field: "()".to_string(),
                    components: Some(vec![]),
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 1,
                    type_field: "(_, _)".to_string(),
                    components: Some(vec![
                        TypeApplication {
                            name: "__tuple_element".to_string(),
                            type_id: 11,
                            type_arguments: None,
                        },
                        TypeApplication {
                            name: "__tuple_element".to_string(),
                            type_id: 11,
                            type_arguments: None,
                        },
                    ]),
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 2,
                    type_field: "(_, _)".to_string(),
                    components: Some(vec![
                        TypeApplication {
                            name: "__tuple_element".to_string(),
                            type_id: 4,
                            type_arguments: None,
                        },
                        TypeApplication {
                            name: "__tuple_element".to_string(),
                            type_id: 24,
                            type_arguments: None,
                        },
                    ]),
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 3,
                    type_field: "(_, _)".to_string(),
                    components: Some(vec![
                        TypeApplication {
                            name: "__tuple_element".to_string(),
                            type_id: 5,
                            type_arguments: None,
                        },
                        TypeApplication {
                            name: "__tuple_element".to_string(),
                            type_id: 13,
                            type_arguments: None,
                        },
                    ]),
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 4,
                    type_field: "[_; 1]".to_string(),
                    components: Some(vec![TypeApplication {
                        name: "__array_element".to_string(),
                        type_id: 8,
                        type_arguments: Some(vec![TypeApplication {
                            name: "".to_string(),
                            type_id: 22,
                            type_arguments: Some(vec![TypeApplication {
                                name: "".to_string(),
                                type_id: 21,
                                type_arguments: Some(vec![TypeApplication {
                                    name: "".to_string(),
                                    type_id: 18,
                                    type_arguments: Some(vec![TypeApplication {
                                        name: "".to_string(),
                                        type_id: 13,
                                        type_arguments: None,
                                    }]),
                                }]),
                            }]),
                        }]),
                    }]),
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 5,
                    type_field: "[_; 2]".to_string(),
                    components: Some(vec![TypeApplication {
                        name: "__array_element".to_string(),
                        type_id: 14,
                        type_arguments: None,
                    }]),
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 6,
                    type_field: "[_; 2]".to_string(),
                    components: Some(vec![TypeApplication {
                        name: "__array_element".to_string(),
                        type_id: 10,
                        type_arguments: None,
                    }]),
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 7,
                    type_field: "b256".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 8,
                    type_field: "enum EnumWGeneric".to_string(),
                    components: Some(vec![
                        TypeApplication {
                            name: "a".to_string(),
                            type_id: 25,
                            type_arguments: None,
                        },
                        TypeApplication {
                            name: "b".to_string(),
                            type_id: 12,
                            type_arguments: None,
                        },
                    ]),
                    type_parameters: Some(vec![12]),
                },
                TypeDeclaration {
                    type_id: 9,
                    type_field: "generic K".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 10,
                    type_field: "generic L".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 11,
                    type_field: "generic M".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 12,
                    type_field: "generic N".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 13,
                    type_field: "generic T".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 14,
                    type_field: "generic U".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 15,
                    type_field: "raw untyped ptr".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 16,
                    type_field: "str[2]".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 17,
                    type_field: "struct MegaExample".to_string(),
                    components: Some(vec![
                        TypeApplication {
                            name: "a".to_string(),
                            type_id: 3,
                            type_arguments: None,
                        },
                        TypeApplication {
                            name: "b".to_string(),
                            type_id: 23,
                            type_arguments: Some(vec![TypeApplication {
                                name: "".to_string(),
                                type_id: 2,
                                type_arguments: None,
                            }]),
                        },
                    ]),
                    type_parameters: Some(vec![13, 14]),
                },
                TypeDeclaration {
                    type_id: 18,
                    type_field: "struct PassTheGenericOn".to_string(),
                    components: Some(vec![TypeApplication {
                        name: "one".to_string(),
                        type_id: 20,
                        type_arguments: Some(vec![TypeApplication {
                            name: "".to_string(),
                            type_id: 9,
                            type_arguments: None,
                        }]),
                    }]),
                    type_parameters: Some(vec![9]),
                },
                TypeDeclaration {
                    type_id: 19,
                    type_field: "struct RawVec".to_string(),
                    components: Some(vec![
                        TypeApplication {
                            name: "ptr".to_string(),
                            type_id: 15,
                            type_arguments: None,
                        },
                        TypeApplication {
                            name: "cap".to_string(),
                            type_id: 25,
                            type_arguments: None,
                        },
                    ]),
                    type_parameters: Some(vec![13]),
                },
                TypeDeclaration {
                    type_id: 20,
                    type_field: "struct SimpleGeneric".to_string(),
                    components: Some(vec![TypeApplication {
                        name: "single_generic_param".to_string(),
                        type_id: 13,
                        type_arguments: None,
                    }]),
                    type_parameters: Some(vec![13]),
                },
                TypeDeclaration {
                    type_id: 21,
                    type_field: "struct StructWArrayGeneric".to_string(),
                    components: Some(vec![TypeApplication {
                        name: "a".to_string(),
                        type_id: 6,
                        type_arguments: None,
                    }]),
                    type_parameters: Some(vec![10]),
                },
                TypeDeclaration {
                    type_id: 22,
                    type_field: "struct StructWTupleGeneric".to_string(),
                    components: Some(vec![TypeApplication {
                        name: "a".to_string(),
                        type_id: 1,
                        type_arguments: None,
                    }]),
                    type_parameters: Some(vec![11]),
                },
                TypeDeclaration {
                    type_id: 23,
                    type_field: "struct Vec".to_string(),
                    components: Some(vec![
                        TypeApplication {
                            name: "buf".to_string(),
                            type_id: 19,
                            type_arguments: Some(vec![TypeApplication {
                                name: "".to_string(),
                                type_id: 13,
                                type_arguments: None,
                            }]),
                        },
                        TypeApplication {
                            name: "len".to_string(),
                            type_id: 25,
                            type_arguments: None,
                        },
                    ]),
                    type_parameters: Some(vec![13]),
                },
                TypeDeclaration {
                    type_id: 24,
                    type_field: "u32".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: 25,
                    type_field: "u64".to_string(),
                    components: None,
                    type_parameters: None,
                },
            ];

            let applications = vec![TypeApplication {
                name: "arg1".to_string(),
                type_id: 17,
                type_arguments: Some(vec![
                    TypeApplication {
                        name: "".to_string(),
                        type_id: 16,
                        type_arguments: None,
                    },
                    TypeApplication {
                        name: "".to_string(),
                        type_id: 7,
                        type_arguments: None,
                    },
                ]),
            }];

            Self {
                declarations,
                applications,
            }
        }

        fn lookup(&self) -> HashMap<usize, TypeDeclaration> {
            self.declarations
                .iter()
                .map(|decl| (decl.type_id, decl.clone()))
                .collect()
        }

        fn declarations_for_custom_type(
            type_field: String,
            use_generics: bool,
        ) -> (Vec<TypeDeclaration>, usize) {
            let custom_type_id = 2;
            let inner_type_id = if use_generics { 3 } else { 1 };

            let mut declarations = vec![
                TypeDeclaration {
                    type_id: 1,
                    type_field: "u8".to_string(),
                    components: None,
                    type_parameters: None,
                },
                TypeDeclaration {
                    type_id: custom_type_id,
                    type_field,
                    components: Some(vec![TypeApplication {
                        name: "_0".to_string(),
                        type_id: inner_type_id,
                        type_arguments: None,
                    }]),
                    type_parameters: use_generics.then_some(vec![inner_type_id]),
                },
            ];

            if use_generics {
                declarations.push(TypeDeclaration {
                    type_id: 3,
                    type_field: "generic T".to_string(),
                    components: None,
                    type_parameters: None,
                })
            }

            (declarations, custom_type_id)
        }

        fn arg_for_type_id(custom_type_id: usize, use_generics: bool) -> TypeApplication {
            TypeApplication {
                name: "arg".to_string(),
                type_id: custom_type_id,
                type_arguments: use_generics.then_some(vec![TypeApplication {
                    name: "".to_string(),
                    type_id: 1,
                    type_arguments: None,
                }]),
            }
        }
    }
}
