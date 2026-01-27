// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Enables the creation of objects with deterministic addresses derived from a parent object's UID.
module iota::derived_object;

/// An internal key to protect from generating the same UID twice (e.g. collide with DFs)
#[allow(unused_field)]
public struct DerivedObjectKey<K: copy + drop + store>(K) has copy, drop, store;

/// Given an ID and a Key, it calculates the derived address.
#[test_only]
public fun derive_address_for_testing<K: copy + drop + store>(parent: ID, key: K): address {
    iota::dynamic_field::hash_type_and_key(parent.to_address(), DerivedObjectKey(key))
}
