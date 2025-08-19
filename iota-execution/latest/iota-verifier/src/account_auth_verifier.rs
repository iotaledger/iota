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
    base_types::{RESOLVED_ASCII_STR, RESOLVED_UTF8_STR, TxContext, TxContextKind},
    error::ExecutionError,
    id::RESOLVED_IOTA_ID,
};
use move_binary_format::{
    CompiledModule,
    file_format::{SignatureToken, Visibility},
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

/// ************ FOR DEMONSTRATION PURPOSES ONLY ***********************

// Below is the "implementation" of the checks that the above [verify_authenticate_func]
// should perform.
// This does not seem to be feasible at the moment, because the appropriate
// type_args/[TypeTag](move-core-types::TypeTag) are not available to this scope.
// The native function [create_auth_info_v1_impl] calling [verify_authenticate_func], does have
// a set of type_args but those are for its context and not for the passed in `authenticate`
// function.
//
// On the callsite:
// ```
// account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"minimally_viable_auth_function"))
// ```
//
// There is nothing we know about the exact types of the referred by the authenticate function.
// Let us assume we can load the code containing this function for this verification step and
// further assume that it is some templated type.
//
// ```
// pub fn authenticate<T>(v: T, auth_ctx: &AuthContext, ctx: &TxContext)
// ```
/// The verifier can check the signature of this function as it appears in
/// CompiledModule, by the templated type, in this case `T`, denoted as
/// `TypeParameter`. It doesn't actually know what type is there, because this
/// is not known during compilation, thus it isn't part of CompiledModule.
/// Only during execution when the transaction's call-chain is initiated do we
/// receive this additional context through
/// the [TypeTag](move-core-types::TypeTag) arguments.
///
/// Another higher level example for potentially more clarity:
/// Context one: Tx -> somewhere inside call create_auth_info_v1 and attach the
/// function to the object, here verifier has only access to the CompiledModule
/// for the referred function Context two: Tx -> call authenticate (as part of
/// the process), here we have all the additional type information
///
/// For an example how all this comes together take a look at [resolve_call_arg](https://github.com/iotaledger/iota/blob/2f7e29c18b6b986cc795e068d135f4b46732f992/crates/iota-json/src/lib.rs#L698) which is called when a function is move function called.

/// Returns `true` if [SignatureToken] is a primitive type
#[warn(dead_code)]
fn primitive(st: &SignatureToken) -> bool {
    use SignatureToken::*;

    matches!(st, U8 | U16 | U32 | U64 | U128 | U256 | Bool | Address)
}

/// Evaluate that signature type is of [pure input](https://docs.iota.org/developer/iota-101/transactions/ptb/programmable-transaction-blocks#inputs)
///
/// A `pure input` is seems to be any type that can't be used to modify ledger
/// state in any way and can be constructed before calling the function itself.
/// The ledger state can be modified:
///     - through `&mut TxContext`
///     - through `&mut T`
///     - by publishing (publish_shared) of objects (thus object even as a
///       value, cannot be used)
///
/// A general struct, with no unresolved template arguments:
/// ```
/// public struct Simple has store {
///   a: u8,
///   some_vec: vector<ascii::String>
/// }
/// ```
/// should be also acceptable, but isn't considered a pure_type either as it
/// isn't a built-in type so it can't be constructed before the call itself. On
/// the contrary std::ascii::String and std::string::String are okay.
/// On a similar notion a simple `vector<T>` and an `Option<T>` are both also
/// acceptable as they are built-in move types with rust side counterpart as
/// long as `T` is recursively `pure` as well.
#[warn(dead_code)]
fn pure_type(module: &CompiledModule, st: &SignatureToken) -> bool {
    use SignatureToken::*;

    match st {
        st if primitive(st) => true,
        Datatype(handle_index) => {
            let resolved_struct = resolve_struct(module, *handle_index);
            resolved_struct == RESOLVED_ASCII_STR
                || resolved_struct == RESOLVED_UTF8_STR
                || resolved_struct == RESOLVED_IOTA_ID
        }
        // DatatypeInstantiation(datatype_instance) => {
        //     let (idx, type_tokens) = &**datatype_instance;
        //     let resolved_struct = resolve_struct(module, *idx);
        //     // is option of a primitive
        //     resolved_struct == RESOLVED_STD_OPTION && type_tokens.len() == 1 // && check if type
        // argument is of pure input }
        // TypeParameter(idx) =>
        //     // check if type argument is of pure type
        // ,
        // Vector(type_token) => {
        //     // check if type argument is of pure input
        // }
        _ => false,
    }
}
