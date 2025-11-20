// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Package metadata management module
/// An IOTA package can have associated metadata that provides,
/// on-chain, additional information about the package.
module iota::package_metadata;

use iota::vec_map::VecMap;
use std::ascii;
use std::type_name::TypeName;

/// Key type for deriving the package metadata object address
public struct PackageMetadataKey has copy, drop, store {}

/// Represents the metadata of a Move package. This includes information
/// such as the storage ID, runtime ID, version, and metadata for the
/// functions contained within the package.
public struct PackageMetadataV1 has key {
    id: UID,
    /// Storage ID of the package represented by this metadata
    /// The object id of the runtime package metadata object is derived from
    /// this value.
    storage_id: ID,
    /// Runtime ID of the package represented by this metadata
    runtime_id: ID,
    /// Version of the package represented by this metadata
    package_version: u64,
    // Handles to internal package modules
    modules_metadata: VecMap<ascii::String, ModuleMetadataV1>,
}

/// Represents metadata associated with a module in the package.
/// V1 includes only the authenticator functions information.
public struct ModuleMetadataV1 has copy, drop, store {
    authenticator_metadata: vector<AuthenticatorMetadataV1>,
}

/// Represents metadata for an authenticator within the package.
/// It includes the name of the authenticate function and the TypeName
/// of the first parameter (i.e., the account object type).
public struct AuthenticatorMetadataV1 has copy, drop, store {
    function_name: ascii::String,
    account_type: TypeName,
}

/// Return the storage ID of the package represented by this metadata
public fun storage_id(metadata: &PackageMetadataV1): ID {
    metadata.storage_id
}

/// Return the runtime ID of the package represented by this metadata
public fun runtime_id(metadata: &PackageMetadataV1): ID {
    metadata.runtime_id
}

/// Return the version of the package represented by this metadata
public fun package_version(metadata: &PackageMetadataV1): u64 {
    metadata.package_version
}

/// Return the module metadata list of the package represented by this metadata
public fun modules_metadata_v1(
    self: &PackageMetadataV1,
    module_name: &ascii::String,
): Option<ModuleMetadataV1> {
    self.modules_metadata.try_get(module_name)
}

/// Returns the `AuthenticatorMetadataV1` associated with the specified
/// `module_name` and `function_name`, if any.
public fun authenticator_metadata_v1(
    self: &ModuleMetadataV1,
    function_name: ascii::String,
): Option<AuthenticatorMetadataV1> {
    self.authenticator_metadata.find_index!(|m| m.function_name == function_name).and!(|index| {
        option::some(self.authenticator_metadata[index])
    })
}

/// Return the account type of the authenticator represented by this metadata
public fun account_type(self: &AuthenticatorMetadataV1): TypeName {
    self.account_type
}

public fun try_get_authenticator_metadata_v1(
    self: &PackageMetadataV1,
    module_name: ascii::String,
    function_name: ascii::String,
): Option<AuthenticatorMetadataV1> {
    self.modules_metadata_v1(&module_name).and!(|modules_metadata| {
        modules_metadata.authenticator_metadata_v1(function_name)
    })
}

/// Creates a `PackageMetadataV1` instance for testing, skipping validation.
#[test_only]
public fun create_package_metadata_v1_for_testing(
    storage_id: ID,
    modules: vector<ascii::String>,
    functions: vector<ascii::String>,
    type_names: vector<TypeName>,
): PackageMetadataV1 {
    assert!(modules.length() == functions.length());
    assert!(modules.length() == type_names.length());
    let addr = iota::derived_object::derive_address_for_testing(
        storage_id,
        PackageMetadataKey {},
    );
    let id = object::new_uid_from_hash(addr);
    let mut modules_metadata = iota::vec_map::empty<ascii::String, ModuleMetadataV1>();
    let mut i = 0;
    while (i < modules.length()) {
        let module_name = modules[i];
        let function_name = functions[i];
        let account_type = type_names[i];
        let authenticator_metadata = vector[
            AuthenticatorMetadataV1 {
                function_name,
                account_type,
            },
        ];
        let module_meta = ModuleMetadataV1 { authenticator_metadata };
        modules_metadata.insert(module_name, module_meta);
        i = i + 1;
    };
    PackageMetadataV1 {
        id,
        storage_id,
        runtime_id: storage_id,
        package_version: 1,
        modules_metadata,
    }
}

/// Creates a `PackageMetadataV1` instance for testing with only one
/// authenticator, skipping validation.
#[test_only]
public fun create_package_metadata_v1_for_testing_one_authenticator(
    storage_id: ID,
    module_name: ascii::String,
    function_name: ascii::String,
    type_name: TypeName,
): PackageMetadataV1 {
    create_package_metadata_v1_for_testing(
        storage_id,
        vector[module_name],
        vector[function_name],
        vector[type_name],
    )
}
