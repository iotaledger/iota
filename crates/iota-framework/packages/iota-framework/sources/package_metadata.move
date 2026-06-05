// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Package metadata management module
/// An IOTA package can have associated metadata that provides,
/// on-chain, additional information about the package.
module iota::package_metadata;

use iota::derived_object;
use iota::dynamic_field;
use iota::module_metadata_dynamic::{Self, ModuleMetadataDynamic, ViewFunctionMetadataV1};
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
const EWrongPackageVersion: vector<u8> =
    b"The provided package metadata has an unsupported package version.";

// === Structs ===

/// Key type for deriving the package metadata object address
public struct PackageMetadataKey has copy, drop, store {}
/// Key types for dynamic field keys
public struct PackageMetadataVersionFieldName has copy, drop, store {}
public struct ModulesMetadataDynamicFieldName has copy, drop, store {}
public struct AuthenticatorMetadataFieldName has copy, drop, store {}

public struct ModuleName(ascii::String) has copy, drop, store;

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
    // package_view_function: VecMap<ascii::String, vector<ViewFunctionMetadataV1>>,
}

public struct ModuleMetadataKey has copy, drop, store {
    module_name: ascii::String,
    version: u8,
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
    package_id: ID,
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
    let id_address = derived_object::derive_address(package_id, PackageMetadataKey {});
    let id = object::new_uid_from_hash(id_address);

    let package_metadata = PackageMetadataV1 {
        id,
        storage_id,
        runtime_id,
        package_version,
        modules_metadata,
    };
    transfer::freeze_object(package_metadata);
}

public(package) fun create_package_metadata_v1_with_dynamic_metadata(
    package_id: ID,
    storage_id: ID,
    runtime_id: ID,
    package_version: u64,
    module_id: ID,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
    view_function_names: vector<vector<ascii::String>>,
) {
    let modules_metadata = create_modules_metadata_dynamic(
        module_id,
        modules,
        auth_functions,
        type_names,
        view_function_names,
    );

    let id_address = derived_object::derive_address(package_id, PackageMetadataKey {});
    let id = object::new_uid_from_hash(id_address);

    let mut package_metadata = PackageMetadataV1 {
        id,
        storage_id,
        runtime_id,
        package_version,
        modules_metadata: iota::vec_map::empty<ascii::String, ModuleMetadataV1>(),
    };

    dynamic_field::add(
        &mut package_metadata.id,
        PackageMetadataVersionFieldName {},
        2,
    );

    dynamic_field::add(
        &mut package_metadata.id,
        ModulesMetadataDynamicFieldName {},
        modules_metadata,
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

public(package) fun create_modules_metadata_dynamic(
    module_id: ID,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
    view_function_names: vector<vector<ascii::String>>,
): VecMap<ModuleName, ModuleMetadataDynamic> {
    assert!(modules.length() == auth_functions.length());
    assert!(modules.length() == type_names.length());
    assert!(modules.length() == view_function_names.length());
    let mut modules_metadata = iota::vec_map::empty<ModuleName, ModuleMetadataDynamic>();
    let mut i = 0;
    while (i < modules.length()) {
        let module_name = modules[i];
        let mut module_metadata = module_metadata_dynamic::new(module_id);
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
        module_metadata.add(AuthenticatorMetadataFieldName {}, authenticator_metadata);
        let view_function_names = view_function_names[i];
        module_metadata.add_view_function_metadata_v1(view_function_names);

        modules_metadata.insert(ModuleName(module_name), module_metadata);
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

/// Safely get the module metadata list of the package represented by this metadata
/// TO BE DEPRECATED
public fun try_get_modules_metadata_v1(
    self: &PackageMetadataV1,
    module_name: &ascii::String,
): Option<ModuleMetadataV1> {
    self.modules_metadata.try_get(module_name)
}

/// Borrow the module metadata list of the package represented by this metadata.
/// Aborts if the module is not found.
/// TO BE DEPRECATED
public fun modules_metadata_v1(
    self: &PackageMetadataV1,
    module_name: &ascii::String,
): &ModuleMetadataV1 {
    if (
        dynamic_field::exists_<ModulesMetadataDynamicFieldName>(
            &self.id,
            ModulesMetadataDynamicFieldName {},
        )
    ) {
        abort (EWrongPackageVersion)
    } else {
        self.modules_metadata.get(module_name)
    }
}

public fun modules_metadata(
    self: &PackageMetadataV1,
    module_name: &ascii::String,
): &ModuleMetadataDynamic {
    dynamic_field::borrow<
        ModulesMetadataDynamicFieldName,
        VecMap<ModuleName, ModuleMetadataDynamic>,
    >(
        &self.id,
        ModulesMetadataDynamicFieldName {},
    ).get(&ModuleName(*module_name))
}

public fun view_function_metadata_v1(
    self: &ModuleMetadataDynamic,
    function_name: &ascii::String,
): &ViewFunctionMetadataV1 {
    self.view_function_metadata_v1(function_name)
}

public fun authenticator_function_metadata_v1(
    self: &ModuleMetadataDynamic,
    function_name: &ascii::String,
): &AuthenticatorMetadataV1 {
    let authenticator_metadata = self.borrow<
        AuthenticatorMetadataFieldName,
        vector<AuthenticatorMetadataV1>,
    >(AuthenticatorMetadataFieldName {});
    let mut index = authenticator_metadata.find_index!(|m| m.function_name() == *function_name);
    assert!(index.is_some(), EAuthenticatorMetadataNotFound);
    authenticator_metadata.borrow(index.extract())
}

// public fun try_get_modules_metadata_v2(
//     self: &PackageMetadataV1,
//     module_name: &ascii::String,
// ): Option<ModuleMetadataV2> {
//     assert!(self.package_version == 2, EWrongPackageVersion);
//     load_inner_package_metadata(self).try_get_module_metadata_v2(module_name)
// }

// public fun modules_metadata_v2(
//     self: &PackageMetadataV1,
//     module_name: &ascii::String,
// ): &ModuleMetadataV2 {
//     assert!(self.package_version == 2, EWrongPackageVersion);
//     load_inner_package_metadata(self).modules_metadata_v2(module_name)
// }

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

public fun function_name(self: &AuthenticatorMetadataV1): &ascii::String {
    &self.function_name
}

// === Test functions ===

#[test_only]
public fun create_package_metadata_v1_for_testing(
    storage_id: ID,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
): PackageMetadataV1 {
    let modules_metadata = create_modules_metadata_v1(
        modules,
        auth_functions,
        type_names,
    );

    let addr = iota::derived_object::derive_address_for_testing(
        storage_id,
        PackageMetadataKey {},
    );
    let id = object::new_uid_from_hash(addr);

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
public fun create_package_metadata_v1_with_dynamic_metadata_for_testing(
    storage_id: ID,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
    view_functions: vector<vector<ascii::String>>,
): PackageMetadataV1 {
    let modules_metadata = create_modules_metadata_dynamic(
        storage_id,
        modules,
        auth_functions,
        type_names,
        view_functions,
    );

    let addr = iota::derived_object::derive_address_for_testing(
        storage_id,
        PackageMetadataKey {},
    );
    let id = object::new_uid_from_hash(addr);

    let mut package_metadata = PackageMetadataV1 {
        id,
        storage_id,
        runtime_id: storage_id,
        package_version: 2,
        modules_metadata: iota::vec_map::empty<ascii::String, ModuleMetadataV1>(),
    };
    dynamic_field::add(
        &mut package_metadata.id,
        PackageMetadataVersionFieldName {},
        2,
    );
    dynamic_field::add(
        &mut package_metadata.id,
        ModulesMetadataDynamicFieldName {},
        modules_metadata,
    );
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
