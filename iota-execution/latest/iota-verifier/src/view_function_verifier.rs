// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{error::ExecutionError, transfer::RESOLVED_RECEIVING_STRUCT};
use move_binary_format::{
    CompiledModule,
    file_format::{AbilitySet, SignatureToken, Visibility},
};
use move_bytecode_utils::{format_signature_token, resolve_struct};
use move_core_types::identifier::IdentStr;

use crate::verification_failure;

/// Verify that a function marked with the IOTA `view` attribute satisfies the
/// same bytecode-visible constraints enforced by the source typing pass.
pub fn verify_view_func(
    module: &CompiledModule,
    function_identifier: &IdentStr,
) -> Result<(), ExecutionError> {
    let module_name = module.name();

    let Some((_, function_definition)) =
        module.find_function_def_by_name(function_identifier.as_str())
    else {
        return Err(verification_failure(format!(
            "View function '{function_identifier}' not found in '{module_name}'"
        )));
    };

    if function_definition.visibility != Visibility::Public {
        return Err(verification_failure(format!(
            "View function '{function_identifier}' must be public"
        )));
    }

    let function_handle = module.function_handle_at(function_definition.function);
    for parameter in &module.signature_at(function_handle.parameters).0 {
        verify_view_parameter_type(module, &function_handle.type_parameters, parameter)
            .map_err(verification_failure)?;
    }

    let return_signature = module.signature_at(function_handle.return_);
    if return_signature.is_empty() {
        return Err(verification_failure(format!(
            "View function '{function_identifier}' must return at least one value"
        )));
    }

    for return_type in &return_signature.0 {
        verify_view_return_type(module, &function_handle.type_parameters, return_type)
            .map_err(verification_failure)?;
    }

    Ok(())
}

/// A valid view parameter type must:
/// - contain no mutable references, including nested mutable references
/// - be safe to pass by value when the parameter is not behind a reference
///
/// Immutable references are allowed because the referenced value is not moved
/// into the view function. Source-only rules, such as `mut value: T` parameter
/// bindings, are not represented in bytecode and are enforced by the compiler.
fn verify_view_parameter_type(
    module: &CompiledModule,
    function_type_args: &[AbilitySet],
    parameter: &SignatureToken,
) -> Result<(), String> {
    if contains_mutable_reference_type(parameter) {
        return Err(format!(
            "Invalid view function parameter type: {}. View functions cannot accept mutable references.",
            format_signature_token(module, parameter),
        ));
    }

    if contains_view_unsafe_by_value_type(module, function_type_args, parameter)? {
        return Err(format!(
            "Invalid view function parameter type: {}. View functions cannot accept objects or values that could contain objects by value.",
            format_signature_token(module, parameter),
        ));
    }

    Ok(())
}

/// A valid view return type must:
/// - contain no mutable references, including nested mutable references
/// - be safe to return by value, which means it cannot be an object or a value
///   that could contain an object
fn verify_view_return_type(
    module: &CompiledModule,
    function_type_args: &[AbilitySet],
    return_type: &SignatureToken,
) -> Result<(), String> {
    if contains_mutable_reference_type(return_type) {
        return Err(format!(
            "Invalid view function return type: {}. View functions cannot return mutable references.",
            format_signature_token(module, return_type),
        ));
    }

    if contains_view_unsafe_by_value_type(module, function_type_args, return_type)? {
        return Err(format!(
            "Invalid view function return type: {}. View functions cannot return objects or values that could contain objects.",
            format_signature_token(module, return_type),
        ));
    }

    Ok(())
}

/// Returns true if `signature_token` contains a mutable reference, including a
/// mutable reference nested in an immutable reference, vector, or datatype
/// instantiation.
fn contains_mutable_reference_type(signature_token: &SignatureToken) -> bool {
    use SignatureToken as S;
    match signature_token {
        S::MutableReference(_) => true,
        S::Reference(inner) | S::Vector(inner) => contains_mutable_reference_type(inner),
        S::DatatypeInstantiation(instantiation) => {
            let (_, type_arguments) = &**instantiation;
            type_arguments.iter().any(contains_mutable_reference_type)
        }
        S::Bool
        | S::U8
        | S::U16
        | S::U32
        | S::U64
        | S::U128
        | S::U256
        | S::Address
        | S::Signer
        | S::TypeParameter(_)
        | S::Datatype(_) => false,
    }
}

/// Returns true if `signature_token` is unsafe to pass or return by value from
/// a view function.
///
/// A type is safe by value when it is:
/// - a primitive bytecode type
/// - a type parameter constrained with `copy` or `drop`
/// - a datatype with no `key` ability and with `copy` or `drop`, other than
///   `iota::transfer::Receiving`
/// - a vector whose element type is by-value-safe
///
/// A datatype's instantiated abilities already account for its non-phantom type
/// arguments, so an object contained through a type argument leaves the
/// datatype without `copy`/`drop` and is caught by the ability check.
/// `Receiving<T>` is the exception: it has `drop` and only a phantom object
/// type argument, so it is matched by name.
///
/// References are never considered by-value here. Mutable references are
/// rejected before this check.
fn contains_view_unsafe_by_value_type(
    module: &CompiledModule,
    function_type_args: &[AbilitySet],
    signature_token: &SignatureToken,
) -> Result<bool, String> {
    use SignatureToken as S;
    match signature_token {
        // References are not by-value. Mutable references are rejected separately.
        S::Reference(_) | S::MutableReference(_) => Ok(false),
        S::TypeParameter(idx) => {
            let abilities = function_type_args.get(*idx as usize).ok_or_else(|| {
                format!("Unexpected CompiledModule error: type parameter index {idx} out of bounds")
            })?;
            Ok(!(abilities.has_copy() || abilities.has_drop()))
        }
        S::Datatype(_) | S::DatatypeInstantiation(_) => {
            if is_receiving_type(module, signature_token) {
                return Ok(true);
            }
            let abilities = module
                .abilities(signature_token, function_type_args)
                .map_err(|err| format!("Unexpected CompiledModule error: {err}"))?;
            Ok(abilities.has_key() || !(abilities.has_copy() || abilities.has_drop()))
        }
        S::Vector(inner) => contains_view_unsafe_by_value_type(module, function_type_args, inner),
        S::Bool | S::U8 | S::U16 | S::U32 | S::U64 | S::U128 | S::U256 | S::Address | S::Signer => {
            Ok(false)
        }
    }
}

/// Returns true if `signature_token` is `iota::transfer::Receiving`, matching
/// the source typing pass, which rejects it by name for the same reason.
fn is_receiving_type(module: &CompiledModule, signature_token: &SignatureToken) -> bool {
    use SignatureToken as S;
    let handle = match signature_token {
        S::Datatype(idx) => *idx,
        S::DatatypeInstantiation(instantiation) => instantiation.0,
        _ => return false,
    };
    resolve_struct(module, handle) == RESOLVED_RECEIVING_STRUCT
}

#[cfg(test)]
mod tests {
    use move_binary_format::file_format::{
        Ability, DatatypeHandle, DatatypeHandleIndex, FunctionDefinition, FunctionHandle,
        IdentifierIndex, ModuleHandleIndex, Signature, SignatureIndex, empty_module,
    };
    use move_core_types::identifier::Identifier;

    use super::*;

    fn module_with_view_signature(
        visibility: Visibility,
        type_parameters: Vec<AbilitySet>,
        parameters: Vec<SignatureToken>,
        returns: Vec<SignatureToken>,
    ) -> CompiledModule {
        let mut module = empty_module();
        module
            .identifiers
            .push(Identifier::new("view".to_owned()).unwrap());
        let name = IdentifierIndex((module.identifiers.len() - 1) as u16);

        let parameters = push_signature(&mut module, parameters);
        let return_ = push_signature(&mut module, returns);
        module.function_handles.push(FunctionHandle {
            module: ModuleHandleIndex(0),
            name,
            parameters,
            return_,
            type_parameters,
        });
        module.function_defs.push(FunctionDefinition {
            visibility,
            ..Default::default()
        });
        module
    }

    fn push_signature(module: &mut CompiledModule, tokens: Vec<SignatureToken>) -> SignatureIndex {
        module.signatures.push(Signature(tokens));
        SignatureIndex((module.signatures.len() - 1) as u16)
    }

    fn push_datatype(module: &mut CompiledModule, abilities: AbilitySet) -> DatatypeHandleIndex {
        module
            .identifiers
            .push(Identifier::new("Object".to_owned()).unwrap());
        let name = IdentifierIndex((module.identifiers.len() - 1) as u16);
        module.datatype_handles.push(DatatypeHandle {
            module: ModuleHandleIndex(0),
            name,
            abilities,
            type_parameters: vec![],
        });
        DatatypeHandleIndex((module.datatype_handles.len() - 1) as u16)
    }

    fn verify(module: &CompiledModule) -> Result<(), ExecutionError> {
        verify_view_func(module, &Identifier::new("view".to_owned()).unwrap())
    }

    fn assert_error_contains(module: &CompiledModule, expected: &str) {
        let err = verify(module).unwrap_err();
        let source = err.source().as_ref().unwrap().to_string();
        assert!(
            source.contains(expected),
            "expected error to contain {expected:?}, got {source:?}"
        );
    }

    #[test]
    fn accepts_valid_view_signature() {
        let module = module_with_view_signature(
            Visibility::Public,
            vec![AbilitySet::EMPTY | Ability::Drop],
            vec![
                SignatureToken::U64,
                SignatureToken::Reference(Box::new(SignatureToken::TypeParameter(0))),
            ],
            vec![SignatureToken::Vector(Box::new(
                SignatureToken::TypeParameter(0),
            ))],
        );

        verify(&module).unwrap();
    }

    #[test]
    fn rejects_non_public_view_function() {
        let module = module_with_view_signature(
            Visibility::Private,
            vec![],
            vec![],
            vec![SignatureToken::Bool],
        );

        assert_error_contains(&module, "must be public");
    }

    #[test]
    fn accepts_unused_type_parameter_without_copy_or_drop() {
        let module = module_with_view_signature(
            Visibility::Public,
            vec![AbilitySet::EMPTY],
            vec![],
            vec![SignatureToken::Bool],
        );

        verify(&module).unwrap();
    }

    #[test]
    fn accepts_immutable_reference_to_type_parameter_without_copy_or_drop() {
        let module = module_with_view_signature(
            Visibility::Public,
            vec![AbilitySet::EMPTY],
            vec![SignatureToken::Reference(Box::new(
                SignatureToken::TypeParameter(0),
            ))],
            vec![SignatureToken::Bool],
        );

        verify(&module).unwrap();
    }

    #[test]
    fn accepts_immutable_reference_to_vector_type_parameter_without_copy_or_drop() {
        let module = module_with_view_signature(
            Visibility::Public,
            vec![AbilitySet::EMPTY],
            vec![SignatureToken::Reference(Box::new(SignatureToken::Vector(
                Box::new(SignatureToken::TypeParameter(0)),
            )))],
            vec![SignatureToken::Bool],
        );

        verify(&module).unwrap();
    }

    #[test]
    fn rejects_by_value_type_parameter_without_copy_or_drop() {
        let module = module_with_view_signature(
            Visibility::Public,
            vec![AbilitySet::EMPTY | Ability::Store],
            vec![SignatureToken::TypeParameter(0)],
            vec![SignatureToken::Bool],
        );

        assert_error_contains(
            &module,
            "cannot accept objects or values that could contain objects by value",
        );
    }

    #[test]
    fn rejects_by_value_vector_type_parameter_without_copy_or_drop() {
        let module = module_with_view_signature(
            Visibility::Public,
            vec![AbilitySet::EMPTY],
            vec![SignatureToken::Vector(Box::new(
                SignatureToken::TypeParameter(0),
            ))],
            vec![SignatureToken::Bool],
        );

        assert_error_contains(
            &module,
            "cannot accept objects or values that could contain objects by value",
        );
    }

    #[test]
    fn rejects_empty_return_signature() {
        let module = module_with_view_signature(Visibility::Public, vec![], vec![], vec![]);

        assert_error_contains(&module, "must return at least one value");
    }

    #[test]
    fn rejects_mutable_reference_parameter() {
        let module = module_with_view_signature(
            Visibility::Public,
            vec![],
            vec![SignatureToken::Vector(Box::new(
                SignatureToken::MutableReference(Box::new(SignatureToken::U64)),
            ))],
            vec![SignatureToken::Bool],
        );

        assert_error_contains(&module, "cannot accept mutable references");
    }

    #[test]
    fn rejects_by_value_object_parameter() {
        let mut module = module_with_view_signature(
            Visibility::Public,
            vec![],
            vec![],
            vec![SignatureToken::Bool],
        );
        let object = push_datatype(
            &mut module,
            AbilitySet::EMPTY | Ability::Key | Ability::Store,
        );
        let parameters = push_signature(&mut module, vec![SignatureToken::Datatype(object)]);
        module.function_handles[0].parameters = parameters;

        assert_error_contains(
            &module,
            "cannot accept objects or values that could contain objects by value",
        );
    }

    #[test]
    fn accepts_immutable_object_reference_parameter() {
        let mut module = module_with_view_signature(
            Visibility::Public,
            vec![],
            vec![],
            vec![SignatureToken::Bool],
        );
        let object = push_datatype(
            &mut module,
            AbilitySet::EMPTY | Ability::Key | Ability::Store,
        );
        let parameters = push_signature(
            &mut module,
            vec![SignatureToken::Reference(Box::new(
                SignatureToken::Datatype(object),
            ))],
        );
        module.function_handles[0].parameters = parameters;

        verify(&module).unwrap();
    }

    #[test]
    fn rejects_nested_by_value_object_parameter() {
        let mut module = module_with_view_signature(
            Visibility::Public,
            vec![],
            vec![],
            vec![SignatureToken::Bool],
        );
        let object = push_datatype(
            &mut module,
            AbilitySet::EMPTY | Ability::Key | Ability::Store,
        );
        let parameters = push_signature(
            &mut module,
            vec![SignatureToken::Vector(Box::new(SignatureToken::Datatype(
                object,
            )))],
        );
        module.function_handles[0].parameters = parameters;

        assert_error_contains(
            &module,
            "cannot accept objects or values that could contain objects by value",
        );
    }

    #[test]
    fn rejects_store_only_return_type() {
        let mut module = module_with_view_signature(Visibility::Public, vec![], vec![], vec![]);
        let store_only = push_datatype(&mut module, AbilitySet::EMPTY | Ability::Store);
        let return_ = push_signature(&mut module, vec![SignatureToken::Datatype(store_only)]);
        module.function_handles[0].return_ = return_;

        assert_error_contains(
            &module,
            "cannot return objects or values that could contain objects",
        );
    }

    #[test]
    fn rejects_object_return_type() {
        let mut module = module_with_view_signature(Visibility::Public, vec![], vec![], vec![]);
        let object = push_datatype(
            &mut module,
            AbilitySet::EMPTY | Ability::Key | Ability::Store,
        );
        let return_ = push_signature(&mut module, vec![SignatureToken::Datatype(object)]);
        module.function_handles[0].return_ = return_;

        assert_error_contains(
            &module,
            "cannot return objects or values that could contain objects",
        );
    }

    #[test]
    fn rejects_type_parameter_return_without_copy_or_drop() {
        let module = module_with_view_signature(
            Visibility::Public,
            vec![AbilitySet::EMPTY | Ability::Store],
            vec![],
            vec![SignatureToken::TypeParameter(0)],
        );

        assert_error_contains(
            &module,
            "cannot return objects or values that could contain objects",
        );
    }

    #[test]
    fn accepts_immutable_reference_return_type() {
        let module = module_with_view_signature(
            Visibility::Public,
            vec![],
            vec![SignatureToken::Reference(Box::new(SignatureToken::U64))],
            vec![SignatureToken::Reference(Box::new(SignatureToken::U64))],
        );

        verify(&module).unwrap();
    }

    #[test]
    fn accepts_nested_immutable_reference_return_type() {
        let module = module_with_view_signature(
            Visibility::Public,
            vec![],
            vec![SignatureToken::Reference(Box::new(SignatureToken::U64))],
            vec![SignatureToken::Vector(Box::new(SignatureToken::Reference(
                Box::new(SignatureToken::U64),
            )))],
        );

        verify(&module).unwrap();
    }

    #[test]
    fn rejects_mutable_reference_return_type() {
        let module = module_with_view_signature(
            Visibility::Public,
            vec![],
            vec![SignatureToken::Reference(Box::new(SignatureToken::U64))],
            vec![SignatureToken::MutableReference(Box::new(
                SignatureToken::U64,
            ))],
        );

        assert_error_contains(&module, "cannot return mutable references");
    }
}
