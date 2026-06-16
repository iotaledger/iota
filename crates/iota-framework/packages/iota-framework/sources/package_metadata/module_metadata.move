// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::module_metadata;

use iota::derived_object;
use iota::dynamic_field as field;
use std::ascii;

public struct ModuleMetadata has key, store {
    /// the ID of this module_metadata
    id: UID,
    /// the number of key-value pairs in the module_metadata
    size: u64,
}

public struct ModuleMetadataKey(ascii::String) has copy, drop, store;

public struct ViewFunctionMetadataV1FieldName has copy, drop, store {}

public fun borrow_view_functions_metadata_v1(self: &ModuleMetadata): &vector<ascii::String> {
    self.borrow(ViewFunctionMetadataV1FieldName {})
}

public fun contains_view_functions_metadata_v1(self: &ModuleMetadata): bool {
    self.contains(ViewFunctionMetadataV1FieldName {})
}

public fun is_view_function_v1(self: &ModuleMetadata, function_name: &ascii::String): bool {
    let view_functions_metadata = self.borrow_view_functions_metadata_v1();
    view_functions_metadata.find_index!(|s| s == *function_name).is_some()
}

/// Returns the size of the module_metadata, the number of key-value pairs
public fun length(module_metadata: &ModuleMetadata): u64 {
    module_metadata.size
}

/// Returns true iff the module_metadata is empty (if `length` returns `0`)
public fun is_empty(module_metadata: &ModuleMetadata): bool {
    module_metadata.size == 0
}

// === Public(package) functions ===

/// Creates a new, empty module_metadata
///
/// The object address is derived from the owning package id together with the
/// module name, so that each module in a package gets a distinct
/// `ModuleMetadata` (and therefore distinct dynamic field children), and that
/// the same module name in different packages does not collide.
public(package) fun new(package_storage_id: ID, module_name: ascii::String): ModuleMetadata {
    let id_address = derived_object::derive_address(
        package_storage_id,
        ModuleMetadataKey(module_name),
    );
    let id = object::new_uid_from_hash(id_address);
    ModuleMetadata {
        id,
        size: 0,
    }
}

public(package) fun add_view_function_metadata_v1(
    module_metadata: &mut ModuleMetadata,
    view_function_names: vector<ascii::String>,
) {
    add(
        module_metadata,
        ViewFunctionMetadataV1FieldName {},
        view_function_names,
    );
}

/// Adds a key-value pair to the module_metadata `module_metadata: &mut ModuleMetadata`
/// Aborts with `iota::dynamic_field::EFieldAlreadyExists` if the module_metadata already has an entry with
/// that key `k: K`.
public(package) fun add<K: copy + drop + store, V: store>(
    module_metadata: &mut ModuleMetadata,
    k: K,
    v: V,
) {
    field::add(&mut module_metadata.id, k, v);
    module_metadata.size = module_metadata.size + 1;
}

/// Mutably borrows the value associated with the key in the module_metadata `module_metadata: &mut ModuleMetadata`.
/// Aborts with `iota::dynamic_field::EFieldDoesNotExist` if the module_metadata does not have an entry with
/// that key `k: K`.
/// Aborts with `iota::dynamic_field::EFieldTypeMismatch` if the module_metadata has an entry for the key, but
/// the value does not have the specified type.
public(package) fun borrow_mut<K: copy + drop + store, V: store>(
    module_metadata: &mut ModuleMetadata,
    k: K,
): &mut V {
    field::borrow_mut(&mut module_metadata.id, k)
}

/// Mutably borrows the key-value pair in the module_metadata `module_metadata: &mut ModuleMetadata` and returns the value.
/// Aborts with `iota::dynamic_field::EFieldDoesNotExist` if the module_metadata does not have an entry with
/// that key `k: K`.
/// Aborts with `iota::dynamic_field::EFieldTypeMismatch` if the module_metadata has an entry for the key, but
/// the value does not have the specified type.
public(package) fun remove<K: copy + drop + store, V: store>(
    module_metadata: &mut ModuleMetadata,
    k: K,
): V {
    let v = field::remove(&mut module_metadata.id, k);
    module_metadata.size = module_metadata.size - 1;
    v
}

/// Immutable borrows the value associated with the key in the module_metadata `module_metadata: &ModuleMetadata`.
/// Aborts with `iota::dynamic_field::EFieldDoesNotExist` if the module_metadata does not have an entry with
/// that key `k: K`.
/// Aborts with `iota::dynamic_field::EFieldTypeMismatch` if the module_metadata has an entry for the key, but
/// the value does not have the specified type.
public(package) fun borrow<K: copy + drop + store, V: store>(
    module_metadata: &ModuleMetadata,
    k: K,
): &V {
    field::borrow(&module_metadata.id, k)
}

/// Returns true iff there is an value associated with the key `k: K` in the module_metadata `module_metadata: &ModuleMetadata`
public(package) fun contains<K: copy + drop + store>(module_metadata: &ModuleMetadata, k: K): bool {
    field::exists_<K>(&module_metadata.id, k)
}

/// Returns true iff there is an value associated with the key `k: K` in the module_metadata `module_metadata: &ModuleMetadata`
/// with an assigned value of type `V`
public(package) fun contains_with_type<K: copy + drop + store, V: store>(
    module_metadata: &ModuleMetadata,
    k: K,
): bool {
    field::exists_with_type<K, V>(&module_metadata.id, k)
}
