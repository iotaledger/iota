// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Package metadata management module
/// An IOTA package can have associated metadata that provides,
/// on-chain, additional information about the package.
module iota::package_metadata;

use iota::vec_map::VecMap;
use std::ascii;
use std::type_name::TypeName;

// === Errors ===

#[error(code = 0)]
const EModuleMetadataNotFound: vector<u8> =
    b"The requested module metadata was not found in the package metadata.";
#[error(code = 1)]
const EAuthenticatorMetadataNotFound: vector<u8> =
    b"The requested authenticator metadata was not found in the module metadata.";

// === Structs ===

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
    /// Runtime ID of the package represented by this metadata. Runtime ID is
    /// the Storage ID of the first version of a package.
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

// === Public functions ===

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

/// Safely get the module metadata list of the package represented by this metadata
public fun try_get_modules_metadata_v1(
    self: &PackageMetadataV1,
    module_name: &ascii::String,
): Option<ModuleMetadataV1> {
    self.modules_metadata.try_get(module_name)
}

/// Borrow the module metadata list of the package represented by this metadata.
/// Aborts if the module is not found.
public fun modules_metadata_v1(
    self: &PackageMetadataV1,
    module_name: &ascii::String,
): &ModuleMetadataV1 {
    assert!(self.modules_metadata.contains(module_name), EModuleMetadataNotFound);
    self.modules_metadata.get(module_name)
}

/// Safely get the `AuthenticatorMetadataV1` associated with the specified
/// `function_name` within the module metadata.
public fun try_get_authenticator_metadata_v1(
    self: &ModuleMetadataV1,
    function_name: &ascii::String,
): Option<AuthenticatorMetadataV1> {
    self.authenticator_metadata.find_index!(|m| m.function_name == *function_name).and!(|index| {
        option::some(self.authenticator_metadata[index])
    })
}

/// Borrow the `AuthenticatorMetadataV1` associated with the specified
/// `function_name`.
/// Aborts if the authenticator metadata is not found for that function.
public fun authenticator_metadata_v1(
    self: &ModuleMetadataV1,
    function_name: &ascii::String,
): &AuthenticatorMetadataV1 {
    let mut index = self.authenticator_metadata.find_index!(|m| m.function_name == *function_name);
    assert!(index.is_some(), EAuthenticatorMetadataNotFound);
    &self.authenticator_metadata[index.extract()]
}

/// Return the account type of the authenticator represented by this metadata
public fun account_type(self: &AuthenticatorMetadataV1): TypeName {
    self.account_type
}

// === Test-only functions ===

/// Creates a `PackageMetadataV1` instance for testing, skipping validation.
/// From `storage_id` the package metadata object ID will be derived.
/// The `modules`, `functions`, and `type_names` vectors must have the same
/// length, each entry representing an authenticator in the package. This
/// means that the module name in the `modules` vector must be repeated for
/// each authenticator it contains.
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
        let authenticator = AuthenticatorMetadataV1 {
            function_name,
            account_type,
        };
        if (modules_metadata.contains(&module_name)) {
            modules_metadata.get_mut(&module_name).authenticator_metadata.push_back(authenticator);
        } else {
            modules_metadata.insert(
                module_name,
                ModuleMetadataV1 { authenticator_metadata: vector[authenticator] },
            );
        };
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
