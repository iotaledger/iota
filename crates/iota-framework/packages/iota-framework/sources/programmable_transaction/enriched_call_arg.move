// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Enriched variants of PTB call-argument types exposed via `auth_context`.
//
// BCS layout MUST match the Rust counterparts in
// `iota_types::auth_context::enriched_types` field-for-field and
// variant-for-variant.
module iota::enriched_call_arg;

use std::type_name::TypeName;

// === Structs ===

/// Enriched representation of an immutable-or-owned object argument.
///
/// Fields must stay in this exact order to preserve BCS compatibility with the
/// Rust `ImmOrOwnedObjectArg` struct.
public struct ImmOrOwnedObjectArg has copy, drop {
    id: ID,
    version: u64,
    digest: vector<u8>,
    mutable: bool,
    type_name: TypeName,
}

/// Enriched representation of a shared object argument.
///
/// Fields must stay in this exact order to preserve BCS compatibility with the
/// Rust `SharedObjectArg` struct.
public struct SharedObjectArg has copy, drop {
    id: ID,
    initial_shared_version: u64,
    mutable: bool,
    digest: vector<u8>,
    type_name: TypeName,
}

// === Enums ===

/// Flat (non-nested) enriched counterpart of `ptb_call_arg::CallArg`.
///
/// Variant order MUST match the Rust `EnrichedCallArg` enum:
///   0 – Pure
///   1 – ImmOrOwnedObject
///   2 – SharedObject
///   3 – Receiving
public enum EnrichedCallArg has copy, drop {
    Pure { value: vector<u8>, type_name: TypeName },
    ImmOrOwnedObject(ImmOrOwnedObjectArg),
    SharedObject(SharedObjectArg),
    Receiving(ImmOrOwnedObjectArg),
}

// === Public functions ===

// -- EnrichedCallArg --

public fun is_pure(arg: &EnrichedCallArg): bool {
    match (arg) {
        EnrichedCallArg::Pure { .. } => true,
        _ => false,
    }
}

public fun is_imm_or_owned_object(arg: &EnrichedCallArg): bool {
    match (arg) {
        EnrichedCallArg::ImmOrOwnedObject(_) => true,
        _ => false,
    }
}

public fun is_shared_object(arg: &EnrichedCallArg): bool {
    match (arg) {
        EnrichedCallArg::SharedObject(_) => true,
        _ => false,
    }
}

public fun is_receiving(arg: &EnrichedCallArg): bool {
    match (arg) {
        EnrichedCallArg::Receiving(_) => true,
        _ => false,
    }
}

// -- ImmOrOwnedObjectArg accessors --

public fun imm_or_owned_id(arg: &ImmOrOwnedObjectArg): &ID {
    &arg.id
}

public fun imm_or_owned_version(arg: &ImmOrOwnedObjectArg): u64 {
    arg.version
}

public fun imm_or_owned_digest(arg: &ImmOrOwnedObjectArg): &vector<u8> {
    &arg.digest
}

public fun imm_or_owned_mutable(arg: &ImmOrOwnedObjectArg): bool {
    arg.mutable
}

public fun imm_or_owned_type_name(arg: &ImmOrOwnedObjectArg): &TypeName {
    &arg.type_name
}

// -- SharedObjectArg accessors --

public fun shared_id(arg: &SharedObjectArg): &ID {
    &arg.id
}

public fun shared_initial_shared_version(arg: &SharedObjectArg): u64 {
    arg.initial_shared_version
}

public fun shared_mutable(arg: &SharedObjectArg): bool {
    arg.mutable
}

public fun shared_digest(arg: &SharedObjectArg): &vector<u8> {
    &arg.digest
}

public fun shared_type_name(arg: &SharedObjectArg): &TypeName {
    &arg.type_name
}

/// Returns the `ImmOrOwnedObjectArg` when the input is an
/// `ImmOrOwnedObject` variant; `option::none()` otherwise.
public fun as_imm_or_owned_object(arg: &EnrichedCallArg): Option<ImmOrOwnedObjectArg> {
    match (arg) {
        EnrichedCallArg::ImmOrOwnedObject(obj) => option::some(*obj),
        _ => option::none(),
    }
}

/// Short accessor for the `mutable` field — callable as `arg.mutable()`.
public fun mutable(arg: &ImmOrOwnedObjectArg): bool {
    arg.mutable
}

/// Returns the `type_name` when the input is a `Pure` variant;
/// `option::none()` for object inputs.
public fun pure_type_name(arg: &EnrichedCallArg): Option<TypeName> {
    match (arg) {
        EnrichedCallArg::Pure { type_name, .. } => option::some(*type_name),
        _ => option::none(),
    }
}

// === Test-only functions ===

#[test_only]
public fun new_pure_for_testing(value: vector<u8>, type_name: TypeName): EnrichedCallArg {
    EnrichedCallArg::Pure { value, type_name }
}

#[test_only]
public fun new_imm_or_owned_for_testing(
    id: ID,
    version: u64,
    digest: vector<u8>,
    mutable: bool,
    type_name: TypeName,
): EnrichedCallArg {
    EnrichedCallArg::ImmOrOwnedObject(ImmOrOwnedObjectArg {
        id,
        version,
        digest,
        mutable,
        type_name,
    })
}

#[test_only]
public fun new_shared_for_testing(
    id: ID,
    initial_shared_version: u64,
    mutable: bool,
    digest: vector<u8>,
    type_name: TypeName,
): EnrichedCallArg {
    EnrichedCallArg::SharedObject(SharedObjectArg {
        id,
        initial_shared_version,
        mutable,
        digest,
        type_name,
    })
}

#[test_only]
public fun new_receiving_for_testing(
    id: ID,
    version: u64,
    digest: vector<u8>,
    mutable: bool,
    type_name: TypeName,
): EnrichedCallArg {
    EnrichedCallArg::Receiving(ImmOrOwnedObjectArg {
        id,
        version,
        digest,
        mutable,
        type_name,
    })
}
