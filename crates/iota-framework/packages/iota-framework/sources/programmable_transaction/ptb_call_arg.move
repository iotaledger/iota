// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// This module provides the move-side replication of a
// Programmable Transaction Block (PTB). It is useful
// for reading the content of a transaction during the
// execution of an authentication.
module iota::ptb_call_arg;

use std::option::{some, none};

// === Structs and Enums ===

// Replicates CallArg enum used in PTBs.
// It represents either pure data or object data as input to a PTB.
public enum CallArg has copy, drop {
    PureData(vector<u8>),
    ObjectData(ObjectArg),
}

// Replicates ObjectArg enum used in PTBs.
// It represents different ways an object can be passed as input to a PTB.
public enum ObjectArg has copy, drop {
    ImmOrOwnedObject(ObjectRef),
    SharedObject {
        id: ID,
        initial_shared_version: u64,
        mutable: bool,
    },
    ReceivingObject(ObjectRef),
}

// Replicates ObjectRef.
// It represents a reference to an object in a PTB,
// i.e., its ID, sequence number, and digest.
public struct ObjectRef has copy, drop {
    object_id: ID,
    sequence_number: u64,
    object_digest: vector<u8>,
}

// === Public functions ===

// === CallArg ===

public fun is_pure_data(arg: &CallArg): bool {
    match (arg) {
        CallArg::PureData(_) => true,
        _ => false,
    }
}

public fun is_object_data(arg: &CallArg): bool {
    match (arg) {
        CallArg::ObjectData(_) => true,
        _ => false,
    }
}

public fun as_pure_data(arg: &CallArg): Option<vector<u8>> {
    match (arg) {
        CallArg::PureData(data) => some(*data),
        _ => none(),
    }
}

public fun as_object_data(arg: &CallArg): Option<ObjectArg> {
    match (arg) {
        CallArg::ObjectData(obj) => some(*obj),
        _ => none(),
    }
}

// == ObjectArg ===

public fun is_shared_object(obj_arg: &ObjectArg): bool {
    match (obj_arg) {
        ObjectArg::SharedObject { id: _, initial_shared_version: _, mutable: _ } => true,
        _ => false,
    }
}

public fun is_imm_or_owned_object(obj_arg: &ObjectArg): bool {
    match (obj_arg) {
        ObjectArg::ImmOrOwnedObject(_) => true,
        _ => false,
    }
}

public fun is_receiving_object(obj_arg: &ObjectArg): bool {
    match (obj_arg) {
        ObjectArg::ReceivingObject(_) => true,
        _ => false,
    }
}

public fun object_id(obj_arg: &ObjectArg): Option<ID> {
    match (obj_arg) {
        ObjectArg::ImmOrOwnedObject(obj_ref) => some(obj_ref.object_id),
        ObjectArg::ReceivingObject(obj_ref) => some(obj_ref.object_id),
        ObjectArg::SharedObject { id, initial_shared_version: _, mutable: _ } => some(*id),
        _ => none(),
    }
}

public fun object_version(obj_arg: &ObjectArg): Option<u64> {
    match (obj_arg) {
        ObjectArg::ImmOrOwnedObject(obj_ref) => some(obj_ref.sequence_number),
        ObjectArg::ReceivingObject(obj_ref) => some(obj_ref.sequence_number),
        ObjectArg::SharedObject { id: _, initial_shared_version, mutable: _ } => some(
            *initial_shared_version,
        ),
        _ => none(),
    }
}

public fun object_digest(obj_arg: &ObjectArg): Option<vector<u8>> {
    match (obj_arg) {
        ObjectArg::ImmOrOwnedObject(obj_ref) => some(obj_ref.object_digest),
        ObjectArg::ReceivingObject(obj_ref) => some(obj_ref.object_digest),
        _ => none(),
    }
}

public fun object_ref(obj_arg: &ObjectArg): Option<ObjectRef> {
    match (obj_arg) {
        ObjectArg::ImmOrOwnedObject(obj_ref) => some(*obj_ref),
        ObjectArg::ReceivingObject(obj_ref) => some(*obj_ref),
        _ => none(),
    }
}

public fun is_mutable_shared_object(obj_arg: &ObjectArg): Option<bool> {
    match (obj_arg) {
        ObjectArg::SharedObject { id: _, initial_shared_version: _, mutable } => some(*mutable),
        _ => none(),
    }
}

// == ObjectRef ===
public fun id(obj_ref: &ObjectRef): &ID {
    &obj_ref.object_id
}

public fun sequence_number(obj_ref: &ObjectRef): u64 {
    obj_ref.sequence_number
}

public fun digest(obj_ref: &ObjectRef): &vector<u8> {
    &obj_ref.object_digest
}

// === Test-only functions ===

#[test_only]
public fun new_call_arg_pure_for_testing(data: vector<u8>): CallArg {
    CallArg::PureData(data)
}

#[test_only]
public fun new_call_arg_object_for_testing(obj: ObjectArg): CallArg {
    CallArg::ObjectData(obj)
}

#[test_only]
public fun new_object_arg_imm_or_owned_for_testing(obj_ref: ObjectRef): ObjectArg {
    ObjectArg::ImmOrOwnedObject(obj_ref)
}

#[test_only]
public fun new_object_arg_shared_for_testing(
    id: ID,
    initial_shared_version: u64,
    mutable: bool,
): ObjectArg {
    ObjectArg::SharedObject { id, initial_shared_version, mutable }
}

#[test_only]
public fun new_object_arg_receiving_for_testing(obj_ref: ObjectRef): ObjectArg {
    ObjectArg::ReceivingObject(obj_ref)
}

#[test_only]
public fun new_object_ref_for_testing(
    object_id: ID,
    sequence_number: u64,
    object_digest: vector<u8>,
): ObjectRef {
    ObjectRef {
        object_id,
        sequence_number,
        object_digest,
    }
}
