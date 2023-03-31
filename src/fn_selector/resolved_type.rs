use std::collections::HashMap;
use std::iter::zip;

use crate::error_codes::{Error, Result};
use crate::program_abi::{TypeApplication, TypeDeclaration};
use crate::utils::extract_generic_name;

#[derive(Debug, Clone)]
pub(crate) struct ResolvedType {
    pub(crate) type_field: String,
    pub(crate) generic_params: Vec<ResolvedType>,
    pub(crate) components: Vec<ResolvedType>,
}

impl ResolvedType {
    /// Will recursively drill down the given generic parameters until all types are
    /// resolved.
    ///
    /// # Arguments
    ///
    /// * `type_application`: the type we wish to resolve
    /// * `types`: all types used in the function call
    pub fn try_from(
        type_application: &TypeApplication,
        type_lookup: &HashMap<usize, TypeDeclaration>,
    ) -> Result<Self> {
        Self::resolve(type_application, type_lookup, &[])
    }

    fn resolve(
        type_application: &TypeApplication,
        type_lookup: &HashMap<usize, TypeDeclaration>,
        parent_generic_params: &[(usize, ResolvedType)],
    ) -> Result<Self> {
        let type_declaration = type_lookup.get(&type_application.type_id).ok_or_else(|| {
            //TODO: fix the message
            // "type id {} not found in type lookup", type_application.type_id
            Error::FnSelectorResolving
        })?;

        if extract_generic_name(&type_declaration.type_field).is_some() {
            let (_, generic_type) = parent_generic_params
                .iter()
                .find(|(id, _)| *id == type_application.type_id)
                .ok_or_else(|| {
                    // TODO: fix the error message
                    // "type id {} not found in parent's generic parameters",
                    Error::FnSelectorResolving
                })?;

            return Ok(generic_type.clone());
        }

        // Figure out what does the current type do with the inherited generic
        // parameters and reestablish the mapping since the current type might have
        // renamed the inherited generic parameters.
        let generic_params_lookup = Self::determine_generics_for_type(
            type_application,
            type_lookup,
            type_declaration,
            parent_generic_params,
        )?;

        // Resolve the enclosed components (if any) with the newly resolved generic
        // parameters.
        let components = type_declaration
            .components
            .iter()
            .flatten()
            .map(|component| Self::resolve(component, type_lookup, &generic_params_lookup))
            .collect::<Result<Vec<_>>>()?;

        Ok(ResolvedType {
            type_field: type_declaration.type_field.clone(),
            components,
            generic_params: generic_params_lookup
                .into_iter()
                .map(|(_, ty)| ty)
                .collect(),
        })
    }

    /// For the given type generates generic_type_id -> Type mapping describing to
    /// which types generic parameters should be resolved.
    ///
    /// # Arguments
    ///
    /// * `type_application`: The type on which the generic parameters are defined.
    /// * `types`: All types used.
    /// * `parent_generic_params`: The generic parameters as inherited from the
    ///                            enclosing type (a struct/enum/array etc.).
    fn determine_generics_for_type(
        type_application: &TypeApplication,
        type_lookup: &HashMap<usize, TypeDeclaration>,
        type_declaration: &TypeDeclaration,
        parent_generic_params: &[(usize, ResolvedType)],
    ) -> Result<Vec<(usize, Self)>> {
        match &type_declaration.type_parameters {
            // The presence of type_parameters indicates that the current type
            // (a struct or an enum) defines some generic parameters (i.e. SomeStruct<T, K>).
            Some(params) if !params.is_empty() => {
                // Determine what Types the generics will resolve to.
                let generic_params_from_current_type = type_application
                    .type_arguments
                    .iter()
                    .flatten()
                    .map(|ty| Self::resolve(ty, type_lookup, parent_generic_params))
                    .collect::<Result<Vec<_>>>()?;

                let generics_to_use = if !generic_params_from_current_type.is_empty() {
                    generic_params_from_current_type
                } else {
                    // Types such as arrays and enums inherit and forward their
                    // generic parameters, without declaring their own.
                    parent_generic_params
                        .iter()
                        .map(|(_, ty)| ty)
                        .cloned()
                        .collect()
                };

                // All inherited but unused generic types are dropped. The rest are
                // re-mapped to new type_ids since child types are free to rename
                // the generic parameters as they see fit -- i.e.
                // struct ParentStruct<T>{
                //     b: ChildStruct<T>
                // }
                // struct ChildStruct<K> {
                //     c: K
                // }

                Ok(zip(params.clone(), generics_to_use).collect())
            }
            _ => Ok(parent_generic_params.to_vec()),
        }
    }
}
