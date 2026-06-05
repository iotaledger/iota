// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::module_metadata_dynamic;

use iota::derived_object;
use iota::dynamic_field as field;
use std::ascii;

// Attempted to destroy a non-empty module_metadata
const EModuleMetadataNotEmpty: u64 = 0;
const EViewFunctionMetadataNotFound: u64 = 1;

public struct ModuleMetadataDynamic has key, store {
    /// the ID of this module_metadata
    id: UID,
    /// the number of key-value pairs in the module_metadata
    size: u64,
}

public struct ModuleMetadataDynamickey has copy, drop, store {}

/// Represents view function metadata for a package.
public struct ViewFunctionMetadataV1 has copy, drop, store {
    function_name: ascii::String,
}

public struct ViewFunctionMetadaFieldName has copy, drop, store {}

/// Creates a new, empty module_metadata
public(package) fun new(module_id: ID): ModuleMetadataDynamic {
    let id_address = derived_object::derive_address(module_id, ModuleMetadataDynamickey {});
    let id = object::new_uid_from_hash(id_address);
    ModuleMetadataDynamic {
        id,
        size: 0,
    }
}

public fun add_view_function_metadata_v1(
    module_metadata: &mut ModuleMetadataDynamic,
    view_function_names: vector<ascii::String>,
) {
    let view_functions_metadata = view_function_names.map!(
        |view_function_name| ViewFunctionMetadataV1 { function_name: view_function_name },
    );
    add(
        module_metadata,
        ViewFunctionMetadaFieldName {},
        view_functions_metadata,
    );
}

public fun view_function_metadata_v1(
    module_metadata: &ModuleMetadataDynamic,
    function_name: &ascii::String,
): &ViewFunctionMetadataV1 {
    let view_functions_metadata = borrow<
        ViewFunctionMetadaFieldName,
        vector<ViewFunctionMetadataV1>,
    >(
        module_metadata,
        ViewFunctionMetadaFieldName {},
    );
    let mut index = view_functions_metadata.find_index!(|m| m.function_name == *function_name);
    assert!(index.is_some(), EViewFunctionMetadataNotFound);
    view_functions_metadata.borrow(index.extract())
}

public fun function_name(self: &ViewFunctionMetadataV1): &ascii::String {
    &self.function_name
}

/// Adds a key-value pair to the module_metadata `module_metadata: &mut ModuleMetadataDynamic`
/// Aborts with `iota::dynamic_field::EFieldAlreadyExists` if the module_metadata already has an entry with
/// that key `k: K`.
public fun add<K: copy + drop + store, V: store>(
    module_metadata: &mut ModuleMetadataDynamic,
    k: K,
    v: V,
) {
    field::add(&mut module_metadata.id, k, v);
    module_metadata.size = module_metadata.size + 1;
}

#[syntax(index)]
/// Immutable borrows the value associated with the key in the module_metadata `module_metadata: &ModuleMetadataDynamic`.
/// Aborts with `iota::dynamic_field::EFieldDoesNotExist` if the module_metadata does not have an entry with
/// that key `k: K`.
/// Aborts with `iota::dynamic_field::EFieldTypeMismatch` if the module_metadata has an entry for the key, but
/// the value does not have the specified type.
public fun borrow<K: copy + drop + store, V: store>(
    module_metadata: &ModuleMetadataDynamic,
    k: K,
): &V {
    field::borrow(&module_metadata.id, k)
}

#[syntax(index)]
/// Mutably borrows the value associated with the key in the module_metadata `module_metadata: &mut ModuleMetadataDynamic`.
/// Aborts with `iota::dynamic_field::EFieldDoesNotExist` if the module_metadata does not have an entry with
/// that key `k: K`.
/// Aborts with `iota::dynamic_field::EFieldTypeMismatch` if the module_metadata has an entry for the key, but
/// the value does not have the specified type.
public fun borrow_mut<K: copy + drop + store, V: store>(
    module_metadata: &mut ModuleMetadataDynamic,
    k: K,
): &mut V {
    field::borrow_mut(&mut module_metadata.id, k)
}

/// Mutably borrows the key-value pair in the module_metadata `module_metadata: &mut ModuleMetadataDynamic` and returns the value.
/// Aborts with `iota::dynamic_field::EFieldDoesNotExist` if the module_metadata does not have an entry with
/// that key `k: K`.
/// Aborts with `iota::dynamic_field::EFieldTypeMismatch` if the module_metadata has an entry for the key, but
/// the value does not have the specified type.
public fun remove<K: copy + drop + store, V: store>(
    module_metadata: &mut ModuleMetadataDynamic,
    k: K,
): V {
    let v = field::remove(&mut module_metadata.id, k);
    module_metadata.size = module_metadata.size - 1;
    v
}

/// Returns true iff there is an value associated with the key `k: K` in the module_metadata `module_metadata: &ModuleMetadataDynamic`
public fun contains<K: copy + drop + store>(module_metadata: &ModuleMetadataDynamic, k: K): bool {
    field::exists_<K>(&module_metadata.id, k)
}

/// Returns true iff there is an value associated with the key `k: K` in the module_metadata `module_metadata: &ModuleMetadataDynamic`
/// with an assigned value of type `V`
public fun contains_with_type<K: copy + drop + store, V: store>(
    module_metadata: &ModuleMetadataDynamic,
    k: K,
): bool {
    field::exists_with_type<K, V>(&module_metadata.id, k)
}

/// Returns the size of the module_metadata, the number of key-value pairs
public fun length(module_metadata: &ModuleMetadataDynamic): u64 {
    module_metadata.size
}

/// Returns true iff the module_metadata is empty (if `length` returns `0`)
public fun is_empty(module_metadata: &ModuleMetadataDynamic): bool {
    module_metadata.size == 0
}

/// Destroys an empty module_metadata
/// Aborts with `EModuleMetadataNotEmpty` if the module_metadata still contains values
public fun destroy_empty(module_metadata: ModuleMetadataDynamic) {
    let ModuleMetadataDynamic { id, size } = module_metadata;
    assert!(size == 0, EModuleMetadataNotEmpty);
    id.delete()
}
