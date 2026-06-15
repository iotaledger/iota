// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Account authenticator verifier
///
/// This module contains the verifier for the authenticator function used
/// by account objects to verify access to the account. The verifier checks
/// that the function signature matches the expected signature for an
/// authenticator function.
use iota_types::{
    auth_context::{AuthContext, AuthContextKind},
    base_types::{TxContext, TxContextKind},
    error::ExecutionError,
    is_object_struct, is_primitive_strict,
    transfer::Receiving,
};
use move_binary_format::{
    CompiledModule,
    file_format::{AbilitySet, SignatureToken, Visibility},
};
use move_bytecode_utils::format_signature_token;
use move_core_types::identifier::IdentStr;

use crate::verification_failure;

/// Verify if a given function can be used as an authenticator function
///
/// A function is an authenticator function if:
/// - only has read-only inputs (immutable owned/shared references or pure
///   types)
/// - has no return type
/// - must be a public non-entry function
/// - the first argument is a reference to the account object type (a Datatype
///   or a concrete DatatypeInstantiation, i.e., with no template type
///   parameters but concrete ones, both with `key` ability)
/// - the last two arguments in order are AuthContext and TxContext
/// - AuthContext has to be an immutable reference
/// - TxContext has to be an immutable reference
///
/// When `enable_mutable_shared` is set, object parameters may also be passed by
/// mutable reference (`&mut T`). The verifier only sees the Move type, so it
/// cannot distinguish shared from owned objects here; restricting mutable
/// references to *shared* objects is enforced at runtime when checking the
/// authenticator input objects.
pub fn verify_authenticate_func_v1(
    module: &CompiledModule,
    function_identifier: &IdentStr,
    enable_mutable_shared: bool,
) -> Result<(), ExecutionError> {
    let module_name = module.name();

    let Some((_, function_definition)) =
        module.find_function_def_by_name(function_identifier.as_str())
    else {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' not found in '{module_name}'"
        )));
    };

    if function_definition.visibility != Visibility::Public {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' must be public"
        )));
    }

    if function_definition.is_entry {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' cannot be marked as `entry`"
        )));
    }

    let function_handle = module.function_handle_at(function_definition.function);
    let function_signature = module.signature_at(function_handle.parameters);

    // at least three arguments
    if function_signature.0.len() < 3 {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' must require at least: a reference to an account object type, &AuthContext and &TxContext arguments."
        )));
    }

    // Check param 1st ///////////////////////////////////////

    // The first parameter must match the authenticated account type.
    // Additional restrictions on the first argument type are enforced in the
    // following check.
    let account_parameter = &function_signature.0[0];
    verify_authenticate_account_type(
        module,
        &function_handle.type_parameters,
        account_parameter,
        enable_mutable_shared,
    )
    .map_err(verification_failure)?;

    // Check params 2nd to N-2th /////////////////////////////

    // Apart from AuthContext and TxContext, object arguments must be passed by
    // immutable reference — unless `enable_mutable_shared` is set, in which case
    // they may also be passed by mutable reference. Pure values are always
    // allowed, as their mutability cannot affect outside state.
    for param in function_signature
        .0
        .iter()
        .take(function_signature.len() - 2)
    {
        verify_authenticate_param_type(
            module,
            &function_handle.type_parameters,
            param,
            enable_mutable_shared,
        )
        .map_err(verification_failure)?;
    }

    // Check params N-1th and Nth ////////////////////////////

    // Check type of AuthContext and TxContext, they both must be structs with the
    // appropriate names, addresses and access
    let auth_context = &function_signature.0[function_signature.len() - 2];
    let tx_context = &function_signature.0[function_signature.len() - 1];

    // AuthContext could potentially be passed as value, but that opens up the
    // possibility for the authenticator function to receive it as mutable
    // value, from which it could mutate before passing it to further `authenticate`
    // functions, so similarly to TxContext, it is simply not allowed.
    if !matches!(
        AuthContext::kind(module, auth_context),
        AuthContextKind::Immutable
    ) {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' can only receive 'AuthContext' as immutable reference"
        )));
    }

    // TxContext must be an immutable reference, unless `enable_mutable_shared` is
    // set. A `&mut TxContext` lets the authenticator create fresh objects via
    // `object::new`; this is safe because the authenticator shares the
    // transaction's `TxContext`, so created object ids stay unique and
    // deterministic across the authentication and transaction phases.
    let tx_context_kind = TxContext::kind(module, tx_context);
    let tx_context_allowed = matches!(tx_context_kind, TxContextKind::Immutable)
        || (enable_mutable_shared && matches!(tx_context_kind, TxContextKind::Mutable));
    if !tx_context_allowed {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' can only receive 'TxContext' as immutable reference"
        )));
    }

    // Check return type (empty) /////////////////////////////

    let return_signature = module.signature_at(function_handle.return_);
    if !return_signature.is_empty() {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' cannot have a return type"
        )));
    }

    Ok(())
}

/// Verify that the first parameter type of the authenticator function is a
/// reference to an Object type, i.e., a Datatype with `key` ability.
///
/// The reference must be immutable, unless `enable_mutable_shared` is set, in
/// which case a mutable reference (`&mut`) to the account is also allowed — the
/// account may then be mutated during authentication. As with the other object
/// parameters, the verifier cannot tell shared from owned objects here;
/// restricting `&mut` to *shared* accounts is enforced at runtime.
fn verify_authenticate_account_type(
    module: &CompiledModule,
    function_type_args: &[AbilitySet],
    param: &SignatureToken,
    enable_mutable_shared: bool,
) -> Result<(), String> {
    use SignatureToken::*;

    // The account must be passed by reference: always immutable, and mutable too
    // when the feature flag is set.
    let account_type = match param {
        Reference(ref_param) => Some(&**ref_param),
        MutableReference(ref_param) if enable_mutable_shared => Some(&**ref_param),
        _ => None,
    };

    if let Some(s) = account_type {
        // Check if a type is a concrete object type (i.e., a Datatype with
        // `key` ability or a DatatypeInstantiation with `key` ability
        // and all type arguments being concrete object types).
        match s {
            Datatype(_) => {
                let abilities = module
                    .abilities(s, function_type_args)
                    .map_err(|vm_err| vm_err.to_string())?;
                if abilities.has_key() {
                    return Ok(());
                }
            }
            DatatypeInstantiation(struct_inst) => {
                let (_, type_args) = &**struct_inst;
                let abilities = module
                    .abilities(s, function_type_args)
                    .map_err(|vm_err| vm_err.to_string())?;
                if abilities.has_key() && type_args.iter().all(is_not_type_parameter) {
                    return Ok(());
                }
            }
            _ => {}
        }
    }
    Err(format!(
        "Invalid authenticator function account type: {}. Valid types for the first parameter are references to an object type (with no generics).",
        format_signature_token(module, param),
    ))
}

/// Verify that the parameter type is a valid type for an authenticator
/// function. Check that:
/// - no Receiving objects are passed at all;
/// - objects are passed by immutable reference (and, when
///   `enable_mutable_shared` is set, also by mutable reference), never by
///   value;
/// - only primitive types are allowed by value.
fn verify_authenticate_param_type(
    module: &CompiledModule,
    function_type_args: &[AbilitySet],
    param: &SignatureToken,
    enable_mutable_shared: bool,
) -> Result<(), String> {
    use SignatureToken::*;

    // Reject receiving objects even if passed by immutable reference
    if Receiving::is_receiving(module, param) {
        return Err(format!(
            "Invalid authenticator function parameter type: {}. Receiving objects are invalid. Valid types are immutable references to objects or primitive types.",
            format_signature_token(module, param),
        ));
    }

    match param {
        Reference(inner) => {
            if is_object_struct(module, function_type_args, inner)? {
                Ok(())
            } else {
                Err(format!(
                    "Invalid parameter type for authenticator function: {}. Non object immutable references are invalid. Valid types are immutable references to objects or primitive types.",
                    format_signature_token(module, param)
                ))
            }
        }
        MutableReference(inner) if enable_mutable_shared => {
            if is_object_struct(module, function_type_args, inner)? {
                Ok(())
            } else {
                Err(format!(
                    "Invalid parameter type for authenticator function: {}. Non object mutable references are invalid. Valid types are references to objects or primitive types.",
                    format_signature_token(module, param)
                ))
            }
        }
        _ => {
            if is_primitive_strict(module, function_type_args, param) {
                Ok(())
            } else {
                Err(format!(
                    "Invalid parameter type for authenticator function: {}. Valid types are references to objects or primitive types.",
                    format_signature_token(module, param)
                ))
            }
        }
    }
}

/// Check that a type is not a type parameter, recursively
fn is_not_type_parameter(s: &SignatureToken) -> bool {
    use SignatureToken as S;
    match s {
        S::TypeParameter(_) => false,
        S::Bool
        | S::U8
        | S::U16
        | S::U32
        | S::U64
        | S::U128
        | S::U256
        | S::Address
        | S::Signer
        | S::Datatype(_) => true,
        S::DatatypeInstantiation(struct_inst) => {
            let (_, type_args) = &**struct_inst;
            type_args.iter().all(is_not_type_parameter)
        }
        S::Vector(inner) | S::Reference(inner) | S::MutableReference(inner) => {
            is_not_type_parameter(inner)
        }
    }
}
