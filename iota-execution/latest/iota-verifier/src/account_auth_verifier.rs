// Copyright (c) 2025 IOTA Stiftung
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
    base_types::{
        RESOLVED_ASCII_STR, RESOLVED_STD_OPTION, RESOLVED_UTF8_STR, TxContext, TxContextKind,
    },
    error::ExecutionError,
    id::RESOLVED_IOTA_ID,
};
use move_binary_format::{
    CompiledModule,
    file_format::{AbilitySet, SignatureToken, Visibility},
};
use move_bytecode_utils::resolve_struct;

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
    for param in function_signature
        .0
        .iter()
        .take(function_signature.len() - 2)
    {
        verify_authenticate_param_type(module, &function_handle.type_parameters, param)
            .map_err(verification_failure)?;
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

/// Evaluate that signature type is of [pure input](https://docs.iota.org/developer/iota-101/transactions/ptb/programmable-transaction-blocks#inputs)
///
/// ATTENTION!///
/// This check implements a very loose definition of a pure type, because it is
/// based on the assumption that the authenticate function is executed
/// equivalently to a PTB with a single command.
/// 1. This means that potentially, a parameter of type `T`, with `T` being a
///    generic, would be accepted by the check of this verify function even if
///    the instance of `T` is not pure by definition. An example is passing an
///    instance of the `Simple` as concrete type of T; in this case, `Simple` is
///    not considered pure. This verify function works as this because it is
///    executed in a moment in which the concrete types of a generic are not
///    known. However, since the authenticate function is executed equivalently
///    to a PTB with a single command, this assures that only pure types and
///    objects can actually be passed by design. So the case of having ´Simple´
///    as concrete type of `T` cannot exist.
/// 2. Moreover, this check assures that no object can be passed as concrete
///    type of a generic `T` because in the constraints of every generic it
///    checks that the `key` ability is not set. This is not enough because a
///    case like this could happen `fn authenticate()<T>(...)` where the key
///    constraint is not set. In this case the compiler helps us by forcing the
///    `T` concrete type to have a `drop` ability. To calm the compiler down the
///    function `authenticate` must either:
///    1. not use the `<T: drop>` constraint and return the parameter with type
///       `T` -> this is not allowed by design, as an authenticate function has
///       no returns;
///    2. not use the `<T: drop>` constraint but the `<T: key>` constraint ->
///       this is not allowed by this verify function;
///    3. use the `<T: drop>` constraint -> this means no object type can be
///       used as concrete type because an object with `drop` ability cannot
///       exist.
///
///
/// A parameter is considered `pure input` if that can't be used to modify
/// ledger state in any way, i.e., not an object, and that can be constructed
/// before calling the function itself.
///
/// A general struct, with no unresolved template arguments:
/// ```
/// public struct Simple has store {
///   a: u8,
///   some_vec: vector<ascii::String>
/// }
/// ```
/// is not considered a `pure input` either as it is not a built-in type so it
/// can't be constructed before the (single) PTB move call itself. On
/// the contrary std::ascii::String and std::string::String are okay.
/// On a similar notion a simple `vector<T>` and an `Option<T>` are both also
/// acceptable as they are built-in move types with rust side counterpart as
/// long as `T` is recursively `pure` as well.
fn verify_pure_input_type(
    module: &CompiledModule,
    function_type_args: &[AbilitySet],
    param: &SignatureToken,
) -> Result<(), String> {
    use SignatureToken::*;

    match param {
        U8 | U16 | U32 | U64 | U128 | U256 | Bool | Address => Ok(()),
        Vector(inner) => verify_pure_input_type(module, function_type_args, inner),
        Datatype(handle_index) => {
            let resolved_struct = resolve_struct(module, *handle_index);
            if resolved_struct == RESOLVED_ASCII_STR
                || resolved_struct == RESOLVED_UTF8_STR
                || resolved_struct == RESOLVED_IOTA_ID
            {
                Ok(())
            } else {
                Err(format!(
                    "Invalid pure type. A datatype must be a string or an ID, offending argument: {:?}",
                    param
                ))
            }
        }
        DatatypeInstantiation(datatype_instance) => {
            let (idx, type_args) = &**datatype_instance;
            let resolved_struct = resolve_struct(module, *idx);
            if resolved_struct == RESOLVED_STD_OPTION && type_args.len() == 1 {
                verify_pure_input_type(module, function_type_args, &type_args[0])
            } else {
                Err(format!(
                    "Invalid pure type. A datatype instantiation must be an option of pure types, offending argument: {:?}",
                    param
                ))
            }
        }
        TypeParameter(idx) => {
            if function_type_args[*idx as usize].has_key() {
                Err(format!(
                    "Invalid pure type. A type parameter cannot have the 'key' ability, offending argument: {:?}",
                    param
                ))
            } else {
                Ok(())
            }
        }
        Signer => Err(format!(
            "Invalid pure type. Signer is not a pure type, offending argument: {:?}",
            param
        )),
        Reference(_) => Err(format!(
            "Invalid pure type. Reference is not a pure type, offending argument: {:?}",
            param
        )),
        MutableReference(_) => Err(format!(
            "Invalid pure type. MutableReference is not a pure type, offending argument: {:?}",
            param
        )),
    }
}

/// Verify that the parameter type is a valid type for an `authenticate`
/// function The parameter type can be:
/// - an immutable reference
/// - a pure input type (see [verify_pure_input_type])
fn verify_authenticate_param_type(
    module: &CompiledModule,
    function_type_args: &[AbilitySet],
    param: &SignatureToken,
) -> Result<(), String> {
    use SignatureToken::*;

    match param {
        Reference(_) => Ok(()),
        // This mutable reference could be allowed to enable a smother authenticate() function
        // composition, but its usage must be checked before being enabled:
        // MutableReference(inner) => verify_pure_input_type(module, function_type_args, inner),
        _ => verify_pure_input_type(module, function_type_args, param),
    }
}
