use lazy_static::lazy_static;
use regex::Regex;

/// Does `type_name` describe a Tuple type?
///
/// # Arguments
///
/// * `type_name`: `type_name` field from [`TypeDeclaration`]( `crate::program_abi::TypeDeclaration` )
pub fn has_tuple_format(type_name: &str) -> bool {
    type_name.starts_with('(') && type_name.ends_with(')')
}

/// If `type_name` contains a generic parameter, it will be returned.
///
/// # Arguments
///
/// * `type_name`: `type_name` field from [`TypeDeclaration`]( `crate::program_abi::TypeDeclaration` )
pub fn extract_generic_name(type_name: &str) -> Option<String> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^\s*generic\s+(\S+)\s*$").unwrap();
    }
    RE.captures(type_name)
        .map(|captures| String::from(&captures[1]))
}

/// If `type_name` represents an Array, its size will be returned;
///
/// # Arguments
///
/// * `type_name`: `type_name` field from [`TypeDeclaration`]( `crate::program_abi::TypeDeclaration` )
pub fn extract_array_len(type_name: &str) -> Option<usize> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^\s*\[.+;\s*(\d+)\s*\]\s*$").unwrap();
    }
    RE.captures(type_name)
        .map(|captures| captures[1].to_string())
        .map(|length: String| {
            length.parse::<usize>().unwrap_or_else(|_| {
                panic!("Could not extract array length from {length}! Original field {type_name}")
            })
        })
}

/// If `type_name` represents a string, its size will be returned;
///
/// # Arguments
///
/// * `type_name`: `type_name` field from [`TypeDeclaration`]( `crate::program_abi::TypeDeclaration` )
pub fn extract_str_len(type_name: &str) -> Option<usize> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^\s*str\s*\[\s*(\d+)\s*\]\s*$").unwrap();
    }
    RE.captures(type_name)
        .map(|captures| captures[1].to_string())
        .map(|length: String| {
            length.parse::<usize>().unwrap_or_else(|_| {
                panic!(
                    "Could not extract string length from {length}! Original field '{type_name}'"
                )
            })
        })
}

/// If `type_name` represents a custom type, its name will be returned.
///
/// # Arguments
///
/// * `type_name`: `type_name` field from [`TypeDeclaration`]( `crate::program_abi::TypeDeclaration` )
pub fn extract_custom_type_name(type_field: &str) -> Option<String> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"\s*(?:struct|enum)\s*(\S*)").unwrap();
    }

    RE.captures(type_field)
        .map(|captures| String::from(&captures[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuples_start_and_end_with_round_brackets() {
        assert!(has_tuple_format("(_, _)"));

        assert!(!has_tuple_format("(.."));

        assert!(!has_tuple_format("..)"));
    }

    #[test]
    fn generic_name_extracted() {
        let type_name = "    generic     T    ";

        let name = extract_generic_name(type_name).expect("Should have succeeded");

        assert_eq!(name, "T");
    }

    #[test]
    fn array_len_extracted() {
        let type_name = "  [  _  ;  8   ]  ";

        let size = extract_array_len(type_name).expect("Should have succeeded");

        assert_eq!(size, 8);
    }

    #[test]
    fn str_len_extracted() {
        let type_name = "  str [ 10  ] ";

        let str_len = extract_str_len(type_name).expect("Should have succeeded");

        assert_eq!(str_len, 10);
    }

    #[test]
    fn custom_struct_type_name_extracted() {
        let type_name = "  struct   SomeStruct ";

        let struct_name = extract_custom_type_name(type_name).expect("Should have succeeded");

        assert_eq!(struct_name, "SomeStruct");
    }

    #[test]
    fn custom_enum_type_name_extracted() {
        let type_name = "  enum   SomeEnum ";

        let enum_name = extract_custom_type_name(type_name).expect("Should have succeeded");

        assert_eq!(enum_name, "SomeEnum");
    }
}
