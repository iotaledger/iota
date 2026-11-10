// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{IOTA_FRAMEWORK_ADDRESS, base_types::TxContextKind, error::ExecutionErrorKind};
use iota_verifier::private_generics::{
    ACCOUNT_MODULE, EVENT_MODULE, PRIVATE_ACCOUNT_FUNCTIONS, PRIVATE_TRANSFER_FUNCTIONS,
    TRANSFER_MODULE,
};
use move_binary_format::file_format::Visibility;
use move_core_types::{identifier::IdentStr, language_storage::ModuleId};

use crate::{
    compat::{ExecutionErrorTrait, execution_status::CommandArgumentError},
    execution_mode::ExecutionMode,
    sp,
    static_programmable_transactions::{env::Env, loading::ast::Type, typing::ast as T},
};

/// Checks the following
/// - valid visibility for move function calls
///   - Can be disabled under certain execution modes
/// - private generics rules for move function calls
/// - valid `TxContext` usage in the signature
pub fn verify<Mode: ExecutionMode>(
    env: &Env<Mode>,
    txn: &T::Transaction,
) -> Result<(), Mode::Error> {
    for c in &txn.commands {
        command::<Mode>(env, c).map_err(|e| e.with_command_index(c.idx as usize))?;
    }
    Ok(())
}

fn command<Mode: ExecutionMode>(
    env: &Env<Mode>,
    sp!(_, c): &T::Command,
) -> Result<(), Mode::Error> {
    let T::Command_ {
        command,
        result_type: _,
        drop_values: _,
        incurs_post_execution_checks: _,
    } = c;
    match command {
        T::Command__::MoveCall(call) => move_call::<Mode>(env, call)?,
        T::Command__::TransferObjects(_, _)
        | T::Command__::SplitCoins(_, _, _)
        | T::Command__::MergeCoins(_, _, _)
        | T::Command__::MakeMoveVec(_, _)
        | T::Command__::Publish(_, _, _)
        | T::Command__::Upgrade(_, _, _, _, _) => (),
    }
    Ok(())
}

/// Checks a move call for
/// - valid `TxContext` usage in the signature
/// - valid visibility
/// - private generics rules
fn move_call<Mode: ExecutionMode>(env: &Env<Mode>, call: &T::MoveCall) -> Result<(), Mode::Error> {
    let T::MoveCall {
        function,
        arguments: _,
    } = call;
    check_tx_context::<Mode>(env, function)?;
    check_private_generics(&function.original_mid, function.name.as_ident_str())?;
    check_visibility::<Mode>(env, function)?;
    Ok(())
}

/// Checks `TxContext` usage in the function's signature:
/// - In the parameters, `TxContext` can appear at most once as `&mut TxContext`, or any number of
///   times as `&TxContext`. It can never be taken by value.
/// - It can never appear in return position, meaning it can never become a result of a command.
///
/// These rules apply to the instantiated signature, so they cover generic parameters and return
/// types instantiated with `TxContext`. They are enforced under all execution modes.
fn check_tx_context<Mode: ExecutionMode>(
    _env: &Env<Mode>,
    function: &T::LoadedFunction,
) -> Result<(), Mode::Error> {
    check_no_tx_context_by_value::<Mode::Error>(&function.signature.parameters)?;
    check_tx_context_refs::<Mode::Error>(&function.signature.parameters)?;
    check_no_tx_context_return::<Mode::Error>(&function.signature.return_)?;
    Ok(())
}

/// `TxContext` can never be taken by value
fn check_no_tx_context_by_value<E: ExecutionErrorTrait>(parameters: &[Type]) -> Result<(), E> {
    let Some(idx) = parameters
        .iter()
        .position(|param| param.is_tx_context_by_value())
    else {
        return Ok(());
    };
    Err(E::new_with_source(
        ExecutionErrorKind::command_argument_error(
            CommandArgumentError::InvalidTxContext,
            checked_as!(idx, u16)?,
        ),
        "TxContext cannot be taken by value",
    ))
}

/// If `&mut TxContext` appears, it must be the only `TxContext` parameter: no other `TxContext`
/// reference, mutable or immutable, may appear alongside it
fn check_tx_context_refs<E: ExecutionErrorTrait>(parameters: &[Type]) -> Result<(), E> {
    let mut mut_idxs = parameters
        .iter()
        .enumerate()
        .filter(|(_, param)| param.is_tx_context() == TxContextKind::Mutable)
        .map(|(idx, _)| idx);
    let Some(first_mut_idx) = mut_idxs.next() else {
        return Ok(());
    };
    if let Some(second_mut_idx) = mut_idxs.next() {
        return Err(E::new_with_source(
            ExecutionErrorKind::command_argument_error(
                CommandArgumentError::InvalidTxContext,
                checked_as!(second_mut_idx, u16)?,
            ),
            "TxContext can be taken by mutable reference at most once",
        ));
    }
    if parameters
        .iter()
        .any(|param| param.is_tx_context() == TxContextKind::Immutable)
    {
        return Err(E::new_with_source(
            ExecutionErrorKind::command_argument_error(
                CommandArgumentError::InvalidTxContext,
                checked_as!(first_mut_idx, u16)?,
            ),
            "&mut TxContext cannot be used alongside other TxContext parameters",
        ));
    }
    Ok(())
}

/// `TxContext` can never appear in return position, by value or by reference
fn check_no_tx_context_return<E: ExecutionErrorTrait>(return_: &[Type]) -> Result<(), E> {
    let Some(idx) = return_.iter().position(|return_ty| {
        return_ty.is_tx_context() != TxContextKind::None || return_ty.is_tx_context_by_value()
    }) else {
        return Ok(());
    };
    Err(E::new_with_source(
        ExecutionErrorKind::command_argument_error(
            CommandArgumentError::InvalidTxContext,
            checked_as!(idx, u16)?,
        ),
        "TxContext cannot be returned from a Move call",
    ))
}

fn check_visibility<Mode: ExecutionMode>(
    _env: &Env<Mode>,
    function: &T::LoadedFunction,
) -> Result<(), Mode::Error> {
    let visibility = function.visibility;
    let is_entry = function.is_entry;
    match (visibility, is_entry) {
        // can call entry
        (Visibility::Private | Visibility::Friend, true) => (),
        // can call public entry
        (Visibility::Public, true) => (),
        // can call public
        (Visibility::Public, false) => (),
        // cannot call private or friend if not entry
        (Visibility::Private | Visibility::Friend, false) => {
            if !Mode::allow_arbitrary_function_calls() {
                return Err(Mode::Error::new_with_source(
                    ExecutionErrorKind::NonEntryFunctionInvoked,
                    "Can only call `entry` or `public` functions",
                ));
            }
        }
    };
    Ok(())
}

/// Verifies that certain private functions in the IOTA framework are not directly invoked:
/// `iota::event`, the private `iota::transfer` variants and the private `iota::account`
/// functions.
fn check_private_generics<E: ExecutionErrorTrait>(
    callee_package: &ModuleId,
    callee_function: &IdentStr,
) -> Result<(), E> {
    let module_addr = callee_package.address();
    let module_name = callee_package.name();
    if module_addr != &IOTA_FRAMEWORK_ADDRESS {
        return Ok(());
    }
    if module_name == EVENT_MODULE {
        return Err(E::new_with_source(
            ExecutionErrorKind::NonEntryFunctionInvoked,
            format!("Cannot directly call functions in iota::{EVENT_MODULE}"),
        ));
    }
    if module_name == TRANSFER_MODULE && PRIVATE_TRANSFER_FUNCTIONS.contains(&callee_function) {
        let msg = format!(
            "Cannot directly call iota::{TRANSFER_MODULE}::{callee_function}. \
             Use the public variant instead, iota::{TRANSFER_MODULE}::public_{callee_function}"
        );
        return Err(E::new_with_source(
            ExecutionErrorKind::NonEntryFunctionInvoked,
            msg,
        ));
    }
    if module_name == ACCOUNT_MODULE && PRIVATE_ACCOUNT_FUNCTIONS.contains(&callee_function) {
        let msg = format!("Cannot directly call iota::{ACCOUNT_MODULE}::{callee_function}.");
        return Err(E::new_with_source(
            ExecutionErrorKind::NonEntryFunctionInvoked,
            msg,
        ));
    }
    Ok(())
}
