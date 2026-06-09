// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Storage & helpers for Function Call Keys allow-set.
///
/// This module owns:
/// - The **dynamic field key** under which the store lives on an `IOTAccount`.
/// - The canonical `FunctionRef` (package, module, function) identifier.
/// - A small store type backed by `VecSet<FunctionRef>` to model an **allow-set**.
/// - Idempotent operations to **allow / disallow / query** a function call key.
/// - A helper to **extract** a `FunctionRef` from a `Command::MoveCall`
module function_call_keys::function_call_keys_store;

use iota::ptb_command::Command;
use iota::table::{Self as tbl, Table};
use iota::vec_set::{Self, VecSet};
use std::ascii;

// === Errors ===

#[error(code = 1)]
const EFunctionCallKeyAlreadyAdded: vector<u8> = b"The function call key has been added already";

#[error(code = 2)]
const EFunctionCallKeyDoesNotExist: vector<u8> = b"The function call key does not exist";

#[error(code = 3)]
const EPublicKeyNotFound: vector<u8> = b"Public key entry not found";

#[error(code = 4)]
const EProgrammableMoveCallExpected: vector<u8> = b"The command is not a programmable Move call";

// === Structs ===

/// An **exact** function identity (no wildcards, no type args in v1).
/// - `package`: on-chain address of the package containing the module
/// - `module_name`: ASCII bytes of the module name
/// - `function_name`: ASCII bytes of the function name
///
/// Doc: We keep these as raw bytes to match PTB.
public struct FunctionRef has copy, drop, store {
    package: address,
    module_name: ascii::String,
    function_name: ascii::String,
}

/// Value stored under the `FunctionCallKeysName` dynamic field of an account.
/// A **set** of allowed function call keys modeled with `VecSet<FunctionRef>`.
public struct FunctionCallKeysStore has store {
    function_keys: Table<vector<u8>, VecSet<FunctionRef>>,
}

/// Dynamic-field name for the Function Call Keys Store.
public struct FunctionCallKeysStoreFieldName has copy, drop, store {}

// === Helpers ===

public fun build(ctx: &mut TxContext): FunctionCallKeysStore {
    FunctionCallKeysStore { function_keys: tbl::new<vector<u8>, VecSet<FunctionRef>>(ctx) }
}

public fun make_function_ref(
    package: address,
    module_name: ascii::String,
    function_name: ascii::String,
): FunctionRef {
    FunctionRef { package, module_name, function_name }
}

// === Per-pubkey allow-set ops ===

/// Ensure a VecSet exists for `pub_key`; if absent, create an empty set.
/// Returns a &mut to the set.
fun ensure_key_entry(
    store: &mut FunctionCallKeysStore,
    pub_key: vector<u8>,
): &mut VecSet<FunctionRef> {
    if (!tbl::contains(&store.function_keys, pub_key)) {
        tbl::add(&mut store.function_keys, pub_key, vec_set::empty());
    };
    tbl::borrow_mut(&mut store.function_keys, pub_key)
}

/// **Allow** a function call key for a specific public key.
public(package) fun allow(store: &mut FunctionCallKeysStore, pub_key: vector<u8>, fk: FunctionRef) {
    let entry = ensure_key_entry(store, pub_key);
    assert!(!entry.contains(&fk), EFunctionCallKeyAlreadyAdded);
    entry.insert(fk);
}

/// **Disallow** a function call key for a specific public key.
public(package) fun disallow(
    store: &mut FunctionCallKeysStore,
    pub_key: vector<u8>,
    fk: &FunctionRef,
) {
    assert!(tbl::contains(&store.function_keys, pub_key), EPublicKeyNotFound);
    let entry = tbl::borrow_mut(&mut store.function_keys, pub_key);
    assert!(entry.contains(fk), EFunctionCallKeyDoesNotExist);
    entry.remove(fk);
}

/// Query: is `fk` allowed for `pub_key`?
public fun is_allowed(store: &FunctionCallKeysStore, pub_key: vector<u8>, fk: &FunctionRef): bool {
    if (!tbl::contains(&store.function_keys, pub_key)) return false;
    let entry = tbl::borrow(&store.function_keys, pub_key);
    entry.contains(fk)
}

/// Extracts a canonical `FunctionRef` from a PTB `Command::MoveCall`.
public fun extract_function_ref(cmd: &Command): FunctionRef {
    assert!(cmd.is_move_call(), EProgrammableMoveCallExpected);

    let mc = cmd.as_move_call().destroy_some();
    let package = mc.package().to_address();
    let module_name = mc.module_name();
    let function_name = mc.function();

    make_function_ref(package, *module_name, *function_name)
}

// === Public Package ===

/// An utility function to construct the dynamic field key for the Function Call Keys Store.
public(package) fun function_call_keys_store_field_name(): FunctionCallKeysStoreFieldName {
    FunctionCallKeysStoreFieldName {}
}
