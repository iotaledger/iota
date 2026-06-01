// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Package metadata management module
/// An IOTA package can have associated metadata that provides,
/// on-chain, additional information about the package.
module iota::package_metadata;

use iota::dynamic_field;
use iota::package_metadata_inner::{
    PackageMetadataV2Inner,
    ModuleMetadataV2,
    create_package_metadata_v2_inner
};
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
#[error(code = 2)]
const EWrongInnerVersion: vector<u8> =
    b"The provided package metadata has an unsupported inner version.";

// === Dynamic field keys ===
public struct PackageMetadataV2Key has copy, drop, store {}

// === Structs ===

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

/// Represents the metadata of a Move package. This includes information
/// such as the storage ID, runtime ID, version, and metadata for the
/// functions contained within the package.
public struct PackageMetadataV2 has key {
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
// ===  Constructors ===

public(package) fun create_package_metadata_v1(
    metadata_id: address,
    storage_id: ID,
    runtime_id: ID,
    package_version: u64,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
) {
    let modules_metadata = create_modules_metadata_v1(
        modules,
        auth_functions,
        type_names,
    );
    let id = object::new_uid_from_hash(metadata_id);
    let package_metadata = PackageMetadataV1 {
        id,
        storage_id,
        runtime_id,
        package_version,
        modules_metadata,
    };
    transfer::freeze_object(package_metadata);
}

public(package) fun create_package_metadata_v2(
    metadata_id: address,
    storage_id: ID,
    runtime_id: ID,
    package_version: u64,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
    view_function_names: vector<vector<ascii::String>>,
) {
    let id = object::new_uid_from_hash(metadata_id);
    let mut package_metadata = PackageMetadataV2 {
        id,
        storage_id,
        runtime_id,
        package_version,
    };
    let package_metadata_inner = create_package_metadata_v2_inner(
        modules,
        auth_functions,
        type_names,
        view_function_names,
    );
    dynamic_field::add(
        &mut package_metadata.id,
        PackageMetadataV2Key {},
        package_metadata_inner,
    );
    transfer::freeze_object(package_metadata);
}

public(package) fun create_modules_metadata_v1(
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
): VecMap<ascii::String, ModuleMetadataV1> {
    assert!(modules.length() == auth_functions.length());
    assert!(modules.length() == type_names.length());
    let mut modules_metadata = iota::vec_map::empty<ascii::String, ModuleMetadataV1>();
    let mut i = 0;
    while (i < modules.length()) {
        let module_name = modules[i];
        let mut authenticator_metadata = vector[];
        let mut j = 0;
        while (j < auth_functions[i].length()) {
            let function_name = auth_functions[i][j];
            let account_type = type_names[i][j];
            authenticator_metadata.push_back(
                create_authenticator_metadata_v1(function_name, account_type),
            );
            j = j + 1;
        };
        modules_metadata.insert(
            module_name,
            create_module_metadata_v1(authenticator_metadata),
        );
        i = i + 1;
    };
    modules_metadata
}

public(package) fun create_module_metadata_v1(
    authenticator_metadata: vector<AuthenticatorMetadataV1>,
): ModuleMetadataV1 {
    ModuleMetadataV1 {
        authenticator_metadata,
    }
}

public(package) fun create_authenticator_metadata_v1(
    function_name: ascii::String,
    account_type: TypeName,
): AuthenticatorMetadataV1 {
    AuthenticatorMetadataV1 {
        function_name,
        account_type,
    }
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

/// Return the storage ID of the package represented by this metadata
public fun storage_id_v2(metadata: &PackageMetadataV2): ID {
    metadata.storage_id
}

/// Return the runtime ID of the package represented by this metadata
public fun runtime_id_v2(metadata: &PackageMetadataV2): ID {
    metadata.runtime_id
}

/// Return the version of the package represented by this metadata
public fun package_version_v2(metadata: &PackageMetadataV2): u64 {
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

public fun modules_metadata_v2(
    self: &PackageMetadataV2,
    module_name: &ascii::String,
): ModuleMetadataV2 {
    let package_metadata_inner = load_inner_package_metadata(self);
    let mut module_metadata = package_metadata_inner.try_get_module_metadata_v2(module_name);
    assert!(module_metadata.is_some(), EModuleMetadataNotFound);
    module_metadata.extract()
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

// === Private functions ===

fun load_inner_package_metadata(self: &PackageMetadataV2): &PackageMetadataV2Inner {
    if (self.package_version == 1 || self.package_version == 2) {
        dynamic_field::borrow<PackageMetadataV2Key, PackageMetadataV2Inner>(
            &self.id,
            PackageMetadataV2Key {},
        )
    } else {
        abort (EWrongInnerVersion)
    }
}

// === Test functions ===

#[test_only]
public fun create_package_metadata_v1_for_testing(
    storage_id: ID,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
): PackageMetadataV1 {
    let addr = iota::derived_object::derive_address_for_testing(
        storage_id,
        PackageMetadataV2Key {},
    );
    let id = object::new_uid_from_hash(addr);

    assert!(modules.length() == auth_functions.length());
    assert!(modules.length() == type_names.length());

    let mut modules_metadata = iota::vec_map::empty<ascii::String, ModuleMetadataV1>();
    let mut i = 0;
    while (i < modules.length()) {
        let module_name = modules[i];
        let mut authenticator_metadata = vector[];
        let mut j = 0;
        while (j < auth_functions[i].length()) {
            let function_name = auth_functions[i][j];
            let account_type = type_names[i][j];
            authenticator_metadata.push_back(
                create_authenticator_metadata_v1_for_testing(function_name, account_type),
            );
            j = j + 1;
        };
        modules_metadata.insert(
            module_name,
            ModuleMetadataV1 { authenticator_metadata },
        );
        i = i + 1;
    };

    let package_metadata = PackageMetadataV1 {
        id,
        storage_id,
        runtime_id: storage_id,
        package_version: 1,
        modules_metadata,
    };
    package_metadata
}

#[test_only]
public fun create_authenticator_metadata_v1_for_testing(
    function_name: ascii::String,
    account_type: TypeName,
): AuthenticatorMetadataV1 {
    AuthenticatorMetadataV1 {
        function_name,
        account_type,
    }
}

#[test_only]
public fun create_package_metadata_v2_for_testing(
    storage_id: ID,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
    view_functions: vector<vector<ascii::String>>,
): PackageMetadataV2 {
    let addr = iota::derived_object::derive_address_for_testing(
        storage_id,
        PackageMetadataV2Key {},
    );
    let id = object::new_uid_from_hash(addr);

    let mut package_metadata = PackageMetadataV2 {
        id,
        storage_id,
        runtime_id: storage_id,
        package_version: 2,
    };
    let pkg_metadata_v2 = create_package_metadata_v2_inner(
        modules,
        auth_functions,
        type_names,
        view_functions,
    );
    dynamic_field::add(
        &mut package_metadata.id,
        PackageMetadataV2Key {},
        pkg_metadata_v2,
    );
    package_metadata
}

/// Creates a `PackageMetadata` instance for testing with only one
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
        vector[vector[function_name]],
        vector[vector[type_name]],
    )
}
