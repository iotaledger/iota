// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Storage & helpers for Function Keys allow-set.
///
/// This module owns:
/// - The **dynamic field key** under which the store lives on an `IOTAccount`.
/// - The canonical `FunctionKey` (package, module, function) identifier.
/// - A small store type backed by `VecSet<FunctionKey>` to model an **allow-set**.
/// - Idempotent operations to **allow / disallow / query** a function key.
/// - A helper to **extract** a `FunctionKey` from a `Command::MoveCall`
module function_keys::fk_store;

use iota::programmable_transaction::{Command, move_call_data};
use iota::vec_set::{Self, VecSet};
use iotaccount::iotaccount::IOTAccount;

#[error(code = 0)]
const EFunctionKeyAlreadyAdded: vector<u8> = b"The function key has been added already";

#[error(code = 1)]
const EFunctionKeyDoesNotExist: vector<u8> = b"The function key does not exist";

// =========================
// Types
// =========================

/// Dynamic-field name for the Function Keys store inside the `IOTAccount`.
public struct FunctionKeysName has copy, drop, store {}

/// An **exact** function identity (no wildcards, no type args in v1).
/// - `package`: on-chain address of the package containing the module
/// - `module_name`: ASCII bytes of the module name
/// - `function_name`: ASCII bytes of the function name
///
/// Doc: We keep these as raw bytes to match PTB.
public struct FunctionKey has copy, drop, store {
    package: address,
    module_name: vector<u8>,
    function_name: vector<u8>,
}

/// Value stored under the `FunctionKeysName` dynamic field of an account.
/// A **set** of allowed function keys modeled with `VecSet<FunctionKey>`.
public struct FunctionKeysStore has store {
    function_keys: VecSet<FunctionKey>,
}

// =========================
// Accessors / helpers
// =========================

/// Returns the **dynamic field key** used to access the Function Keys store.
public fun fk_store_key(): FunctionKeysName { FunctionKeysName {} }

/// Creates an **empty** Function Keys store.
public fun new_store(): FunctionKeysStore {
    FunctionKeysStore { function_keys: vec_set::empty() }
}

/// Checks whether the account has an initialized Function Keys store.
/// Useful for gating admin/auth flows.
public fun store_exists(account: &IOTAccount): bool {
    account.has_field(fk_store_key())
}

/// Borrows the store **immutably** (aborts if missing).
public fun borrow_store(account: &IOTAccount): &FunctionKeysStore {
    account.borrow_field(fk_store_key())
}

/// Borrows the store **mutably** (aborts if missing).
/// Passing `ctx` aligns with the account’s DF mutation rules.
public fun borrow_store_mut(account: &mut IOTAccount, ctx: &TxContext): &mut FunctionKeysStore {
    account.borrow_field_mut(fk_store_key(), ctx)
}

/// Constructs a canonical `FunctionKey` for `(package, module, function)`.
/// The inputs should be the **same canonical bytes** used by the PTB builder.
public fun make_func_key(
    package: address,
    module_name: vector<u8>,
    function_name: vector<u8>,
): FunctionKey {
    FunctionKey { package, module_name, function_name }
}

// =========================
/* Allow-set operations */
// =========================

/// **Allow** a function key.
///
/// Behavior:
/// - **Aborts** with `EFunctionKeyAlreadyAdded` if `fk` is already present.
/// - Otherwise inserts it into the set.
public fun allow(self: &mut FunctionKeysStore, fk: FunctionKey) {
    assert!(!self.function_keys.contains(&fk), EFunctionKeyAlreadyAdded);
    self.function_keys.insert(fk);
}

/// **Disallow** a function key.
///
/// Behavior:
/// - **Aborts** with `EFunctionKeyDoesNotExist` if `fk` is not in the set.
/// - Otherwise removes it.
///
/// Note: `VecSet::remove` consumes a value equal to the element to be removed.
/// We pass `&fk` for the existence check, then the set consumes an owned copy.
public fun disallow(self: &mut FunctionKeysStore, fk: &FunctionKey) {
    assert!(self.function_keys.contains(fk), EFunctionKeyDoesNotExist);
    self.function_keys.remove(fk);
}

/// Returns `true` iff the function key is currently **allowed**.
public fun is_allowed(self: &FunctionKeysStore, fk: &FunctionKey): bool {
    self.function_keys.contains(fk)
}

/// Extracts a `FunctionKey` from a PTB `Command`.
///
/// Precondition: `cmd` **must** be a `MoveCall`. The authenticator guarantees
/// PTB command shape; this helper focuses on decoding the Command.
///
/// Implementation detail:
/// - `move_call_data(cmd)` exposes `(package_id, module_name, function_name)`
/// - We convert `package_id` → `address`, and `.as_bytes()` for the names.
///
public fun extract_func_key(cmd: &Command): FunctionKey {
    let prog_move_call = move_call_data(cmd);
    let package = prog_move_call.package_id().to_address();
    let module_name = prog_move_call.module_name().as_bytes();
    let function_name = prog_move_call.function_name().as_bytes();

    make_func_key(package, *module_name, *function_name)
}
