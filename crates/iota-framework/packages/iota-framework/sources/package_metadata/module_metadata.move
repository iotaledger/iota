// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// On-chain metadata associated with a single module of a package.
///
/// A `ModuleMetadata` is a key-value store backed by dynamic fields, where each
/// value holds a particular kind of module metadata (e.g. authenticator or view
/// function information). Instances are owned by the package's
/// `iota::package_metadata::PackageMetadataV1` object.
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

/// Key used to derive the address of a `ModuleMetadata` object from the owning
/// package id and the module name.
public struct ModuleMetadataKey(ascii::String) has copy, drop, store;

/// Dynamic field key for the list of view function names of the module.
public struct ViewFunctionMetadataV1FieldName has copy, drop, store {}

/// Borrows the list of view function names recorded for the module.
public fun borrow_view_functions_metadata_v1(self: &ModuleMetadata): &vector<ascii::String> {
    self.borrow(ViewFunctionMetadataV1FieldName {})
}

/// Returns true iff the module has recorded view function metadata.
public fun contains_view_functions_metadata_v1(self: &ModuleMetadata): bool {
    self.contains(ViewFunctionMetadataV1FieldName {})
}

/// Returns true iff `function_name` is one of the module's view functions.
public fun is_view_function_v1(self: &ModuleMetadata, function_name: &ascii::String): bool {
    let view_functions_metadata = self.borrow_view_functions_metadata_v1();
    view_functions_metadata.find_index!(|s| s == *function_name).is_some()
}

/// Returns the size of the module_metadata, the number of key-value pairs
public fun length(self: &ModuleMetadata): u64 {
    self.size
}

/// Returns true iff the module_metadata is empty (if `length` returns `0`)
public fun is_empty(self: &ModuleMetadata): bool {
    self.size == 0
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

/// Records the list of view function names for the module.
public(package) fun add_view_function_metadata_v1(
    self: &mut ModuleMetadata,
    view_function_names: vector<ascii::String>,
) {
    self.add(
        ViewFunctionMetadataV1FieldName {},
        view_function_names,
    );
}

/// Adds a key-value pair to the module metadata.
/// Aborts with `iota::dynamic_field::EFieldAlreadyExists` if an entry for `k` already exists.
public(package) fun add<K: copy + drop + store, V: store>(self: &mut ModuleMetadata, k: K, v: V) {
    field::add(&mut self.id, k, v);
    self.size = self.size + 1;
}

/// Mutably borrows the value associated with `k`.
/// Aborts with `iota::dynamic_field::EFieldDoesNotExist` if no entry for `k` exists.
/// Aborts with `iota::dynamic_field::EFieldTypeMismatch` if the value is not of type `V`.
public(package) fun borrow_mut<K: copy + drop + store, V: store>(
    self: &mut ModuleMetadata,
    k: K,
): &mut V {
    field::borrow_mut(&mut self.id, k)
}

/// Removes the entry for `k` and returns its value.
/// Aborts with `iota::dynamic_field::EFieldDoesNotExist` if no entry for `k` exists.
/// Aborts with `iota::dynamic_field::EFieldTypeMismatch` if the value is not of type `V`.
public(package) fun remove<K: copy + drop + store, V: store>(self: &mut ModuleMetadata, k: K): V {
    let v = field::remove(&mut self.id, k);
    self.size = self.size - 1;
    v
}

/// Immutably borrows the value associated with `k`.
/// Aborts with `iota::dynamic_field::EFieldDoesNotExist` if no entry for `k` exists.
/// Aborts with `iota::dynamic_field::EFieldTypeMismatch` if the value is not of type `V`.
public(package) fun borrow<K: copy + drop + store, V: store>(self: &ModuleMetadata, k: K): &V {
    field::borrow(&self.id, k)
}

/// Returns true iff there is a value associated with `k`.
public(package) fun contains<K: copy + drop + store>(self: &ModuleMetadata, k: K): bool {
    field::exists_<K>(&self.id, k)
}

/// Returns true iff there is a value of type `V` associated with `k`.
public(package) fun contains_with_type<K: copy + drop + store, V: store>(
    self: &ModuleMetadata,
    k: K,
): bool {
    field::exists_with_type<K, V>(&self.id, k)
}
