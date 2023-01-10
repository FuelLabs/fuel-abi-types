#[cfg(test)]
mod tests {
    use fuel_abi_types::program_abi::TypeDeclaration;

    #[test]
    fn detects_enum_and_struct_types() {
        let enum_decl = type_decl_w_type_field("enum Something");
        assert!(enum_decl.is_enum_type());
        assert!(!enum_decl.is_struct_type());

        let struct_decl = type_decl_w_type_field("struct Something");
        assert!(struct_decl.is_struct_type());
        assert!(!struct_decl.is_enum_type());
    }

    fn type_decl_w_type_field(type_field: &str) -> TypeDeclaration {
        TypeDeclaration {
            type_id: 0,
            type_field: type_field.to_string(),
            components: None,
            type_parameters: None,
        }
    }
}
