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

use iota::ptb_command::Command;
use iota::table::{Self as tbl, Table};
use iota::vec_set::{Self, VecSet};

#[error(code = 1)]
const EFunctionKeyAlreadyAdded: vector<u8> = b"The function key has been added already";

#[error(code = 2)]
const EFunctionKeyDoesNotExist: vector<u8> = b"The function key does not exist";

#[error(code = 3)]
const EPublicKeyNotFound: vector<u8> = b"Public key entry not found";

#[error(code = 4)]
const EProgrammableMoveCallExpected: vector<u8> = b"The command is not a programmable Move call";

// =========================
// Types
// =========================

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
    function_keys: Table<vector<u8>, VecSet<FunctionKey>>,
}

// =========================
// Accessors / helpers
// =========================

public fun build_fn_keys_store(ctx: &mut TxContext): FunctionKeysStore {
    FunctionKeysStore { function_keys: tbl::new<vector<u8>, VecSet<FunctionKey>>(ctx) }
}

public fun make_func_key(
    package: address,
    module_name: vector<u8>,
    function_name: vector<u8>,
): FunctionKey {
    FunctionKey { package, module_name, function_name }
}

// =========================
// Per-pubkey allow-set ops
// =========================

/// Ensure a VecSet exists for `pub_key`; if absent, create an empty set.
/// Returns a &mut to the set.
fun ensure_key_entry(store: &mut FunctionKeysStore, pub_key: vector<u8>): &mut VecSet<FunctionKey> {
    if (!tbl::contains(&store.function_keys, pub_key)) {
        tbl::add(&mut store.function_keys, pub_key, vec_set::empty());
    };
    tbl::borrow_mut(&mut store.function_keys, pub_key)
}

/// **Allow** a function key for a specific public key.
public(package) fun allow(store: &mut FunctionKeysStore, pub_key: vector<u8>, fk: FunctionKey) {
    let entry = ensure_key_entry(store, pub_key);
    assert!(!entry.contains(&fk), EFunctionKeyAlreadyAdded);
    entry.insert(fk);
}

/// **Disallow** a function key for a specific public key.
public(package) fun disallow(store: &mut FunctionKeysStore, pub_key: vector<u8>, fk: &FunctionKey) {
    assert!(tbl::contains(&store.function_keys, pub_key), EPublicKeyNotFound);
    let entry = tbl::borrow_mut(&mut store.function_keys, pub_key);
    assert!(entry.contains(fk), EFunctionKeyDoesNotExist);
    entry.remove(fk);
}

/// Query: is `fk` allowed for `pub_key`?
public fun is_allowed(store: &FunctionKeysStore, pub_key: vector<u8>, fk: &FunctionKey): bool {
    if (!tbl::contains(&store.function_keys, pub_key)) return false;
    let entry = tbl::borrow(&store.function_keys, pub_key);
    entry.contains(fk)
}

// =========================
// PTB helper
// =========================

/// Extracts a canonical `FunctionKey` from a PTB `Command::MoveCall`.
public fun extract_func_key(cmd: &Command): FunctionKey {
    assert!(cmd.is_move_call(), EProgrammableMoveCallExpected);

    let mc = cmd.as_move_call().destroy_some();
    let package = mc.package().to_address();
    let module_name = mc.module_name().as_bytes();
    let function_name = mc.function().as_bytes();

    make_func_key(package, *module_name, *function_name)
}
