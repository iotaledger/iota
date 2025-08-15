// Copyright (c) IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Account `authenticate` function verifier
///
/// The account `authenticate` verifier module is special in the sense
/// that it isn't called on a module during publication/execution, but on a
/// specific `authenticate` function only during execution.
/// This is because an `authenticate` function only exists as a concept. There
/// is no compiler support for identifying/validating them. Furthermore they are
/// resolved dynamically during execution.
use iota_types::{
    Identifier,
    auth_context::{AuthContext, AuthContextKind},
    base_types::{TxContext, TxContextKind},
    error::ExecutionError,
};
use move_binary_format::{
    CompiledModule,
    file_format::{SignatureToken, Visibility},
};

use crate::verification_failure;

/// Verify if a given function can be used as an `authenticate` function
///
/// A function is an authenticate function if:
/// - only has read-only inputs (immutable owned/shared references or pure
///   types)
/// - has no return type
/// - the last two arguments in order are AuthContext and TxContext
/// - AuthContext has to be an immutable reference
/// - TxContext hat to be an immutable reference
pub fn verify_authenticate_func(
    module: &CompiledModule,
    function_identifier: Identifier,
) -> Result<(), ExecutionError> {
    let module_name = module.name();

    let Some((_, function_definition)) =
        module.find_function_def_by_name(function_identifier.as_str())
    else {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' not found in '{module_name}'"
        )));
    };

    if function_definition.is_entry {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' cannot be marked as `entry`"
        )));
    }

    // Consider alleviating these restrictions in the future by considering:
    // - we can execute private functions from the rust side
    // - a dev, setting this function as private, means that this is declared as not
    //   taking part to composability with other authenticate() (same logic as using
    //   just entry for normal functions)
    if function_definition.visibility != Visibility::Public {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' must be public"
        )));
    }

    let function_handle = module.function_handle_at(function_definition.function);
    let function_signature = module.signature_at(function_handle.parameters);

    // at least two arguments
    if function_signature.0.len() < 2 {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' must require at least &AuthContext and &TxContext arguments."
        )));
    }

    // Apart from AuthContext and TxContext we only require that the arguments are
    // not mutable references. They can be mutable values, as their mutability
    // cannot affect outside state.
    for token in function_signature
        .0
        .iter()
        .take(function_signature.len() - 2)
    {
        match token {
            SignatureToken::Signer | SignatureToken::MutableReference(_) => {
                return Err(verification_failure(format!(
                    "Authenticator function '{function_identifier}' cannot use mutable references or signers, offending argument: {:?}",
                    token
                )));
            }
            _ => (),
        }
    }

    // Check type of AuthContext and TxContext, they both must be structs with the
    // appropriate names, addresses and access
    let auth_context = &function_signature.0[function_signature.len() - 2];
    let tx_context = &function_signature.0[function_signature.len() - 1];

    // AuthContext could potentially passed as value, but that opens up the
    // possibility for the `authenticate` function to receive it as mutable
    // value, from which it could mutate before passing it to further `authenticate`
    // functions, so similarly to TxContext, it is simply not allowed.
    if !matches!(
        AuthContext::kind(module, auth_context),
        AuthContextKind::Immutable
    ) {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' can only receive
            'AuthContext' as immutable reference"
        )));
    }

    // TxContext can only be an immutable reference. Passing it as mutable would
    // allow `authenticate` functions to create objects, which would be
    // problematic.
    if !matches!(
        TxContext::kind(module, tx_context),
        TxContextKind::Immutable
    ) {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' can only receive 'TxContext' as immutable reference"
        )));
    }

    let return_signature = module.signature_at(function_handle.return_);
    if !return_signature.is_empty() {
        return Err(verification_failure(format!(
            "Authenticator function '{function_identifier}' cannot have a return type"
        )));
    }

    Ok(())
}
