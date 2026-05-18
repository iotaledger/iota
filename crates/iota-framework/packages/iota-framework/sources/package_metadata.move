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
#[error(code = 2)]
const EViewFunctionMetadataNotFound: vector<u8> =
    b"The requested view function metadata was not found in the module metadata.";
#[error(code = 3)]
const EViewFunctionMetadataNotEmpty: vector<u8> =
    b"Cannot convert PackageMetadataV2 to PackageMetadataV1 while view function metadata is present.";

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

/// Represents the metadata of a Move package. V2 adds view function metadata
/// while preserving the V1 layout for existing metadata objects.
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
    // Handles to internal package modules
    modules_metadata: VecMap<ascii::String, ModuleMetadataV2>,
}

/// V2 metadata associated with a module in the package.
public struct ModuleMetadataV2 has copy, drop, store {
    authenticator_metadata: vector<AuthenticatorMetadataV1>,
    view_function_metadata: vector<ViewFunctionMetadataV1>,
}

/// Represents metadata for a view function within the package.
public struct ViewFunctionMetadataV1 has copy, drop, store {
    function_name: ascii::String,
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

/// Return the storage ID of the package represented by this V2 metadata.
public fun storage_id_v2(metadata: &PackageMetadataV2): ID {
    metadata.storage_id
}

/// Return the runtime ID of the package represented by this V2 metadata.
public fun runtime_id_v2(metadata: &PackageMetadataV2): ID {
    metadata.runtime_id
}

/// Return the version of the package represented by this V2 metadata.
public fun package_version_v2(metadata: &PackageMetadataV2): u64 {
    metadata.package_version
}

/// Safely get the module metadata list of the package represented by this V2
/// metadata.
public fun try_get_modules_metadata_v2(
    self: &PackageMetadataV2,
    module_name: &ascii::String,
): Option<ModuleMetadataV2> {
    self.modules_metadata.try_get(module_name)
}

/// Borrow the module metadata list of the package represented by this V2
/// metadata. Aborts if the module is not found.
public fun modules_metadata_v2(
    self: &PackageMetadataV2,
    module_name: &ascii::String,
): &ModuleMetadataV2 {
    assert!(self.modules_metadata.contains(module_name), EModuleMetadataNotFound);
    self.modules_metadata.get(module_name)
}

/// Safely get the `AuthenticatorMetadataV1` associated with the specified
/// `function_name` within the V2 module metadata.
public fun try_get_authenticator_metadata_v2(
    self: &ModuleMetadataV2,
    function_name: &ascii::String,
): Option<AuthenticatorMetadataV1> {
    self.authenticator_metadata.find_index!(|m| m.function_name == *function_name).and!(|index| {
        option::some(self.authenticator_metadata[index])
    })
}

/// Borrow the `AuthenticatorMetadataV1` associated with the specified
/// `function_name` from V2 module metadata.
/// Aborts if the authenticator metadata is not found for that function.
public fun authenticator_metadata_v2(
    self: &ModuleMetadataV2,
    function_name: &ascii::String,
): &AuthenticatorMetadataV1 {
    let mut index = self.authenticator_metadata.find_index!(|m| m.function_name == *function_name);
    assert!(index.is_some(), EAuthenticatorMetadataNotFound);
    &self.authenticator_metadata[index.extract()]
}

/// Safely get the `ViewFunctionMetadataV1` associated with the specified
/// `function_name` within the module metadata.
public fun try_get_view_function_metadata_v1(
    self: &ModuleMetadataV2,
    function_name: &ascii::String,
): Option<ViewFunctionMetadataV1> {
    self.view_function_metadata.find_index!(|m| m.function_name == *function_name).and!(|index| {
        option::some(self.view_function_metadata[index])
    })
}

/// Borrow the `ViewFunctionMetadataV1` associated with the specified
/// `function_name`.
/// Aborts if the view function metadata is not found for that function.
public fun view_function_metadata_v1(
    self: &ModuleMetadataV2,
    function_name: &ascii::String,
): &ViewFunctionMetadataV1 {
    let mut index = self.view_function_metadata.find_index!(|m| m.function_name == *function_name);
    assert!(index.is_some(), EViewFunctionMetadataNotFound);
    &self.view_function_metadata[index.extract()]
}

/// Return true if the function is a view function.
public fun is_view_function_v1(self: &ModuleMetadataV2, function_name: &ascii::String): bool {
    self.try_get_view_function_metadata_v1(function_name).is_some()
}

/// Return the name of the view function represented by this metadata.
public fun view_function_name_v1(self: &ViewFunctionMetadataV1): ascii::String {
    self.function_name
}

/// Convert `PackageMetadataV1` into `PackageMetadataV2`, preserving the object
/// id and all package fields. The resulting V2 module metadata has empty view
/// function metadata.
public fun package_metadata_v1_to_v2(self: &PackageMetadataV1): PackageMetadataV2 {
    PackageMetadataV2 {
        id: object::new_uid_from_hash(self.id.to_address()),
        storage_id: self.storage_id,
        runtime_id: self.runtime_id,
        package_version: self.package_version,
        modules_metadata: modules_metadata_v1_to_v2(self.modules_metadata),
    }
}

/// Convert `PackageMetadataV2` into `PackageMetadataV1`, preserving the object
/// id and all package fields. Aborts if any module contains view function
/// metadata, because V1 cannot represent it.
public fun package_metadata_v2_to_v1(self: &PackageMetadataV2): PackageMetadataV1 {
    PackageMetadataV1 {
        id: object::new_uid_from_hash(self.id.to_address()),
        storage_id: self.storage_id,
        runtime_id: self.runtime_id,
        package_version: self.package_version,
        modules_metadata: modules_metadata_v2_to_v1(self.modules_metadata),
    }
}

/// Destroys `PackageMetadataV1` object. This is only intended to be used after calling
/// `package_metadata_v2_to_v1`.
public fun destroy_package_metadata_v1(metadata: PackageMetadataV1) {
    let PackageMetadataV1 {
        id,
        storage_id: _,
        runtime_id: _,
        package_version: _,
        modules_metadata: _,
    } = metadata;
    object::delete(id);
}

/// Destroys `PackageMetadataV2` object. This is only intended to be used after calling
/// `package_metadata_v1_to_v2`.
public fun destroy_package_metadata_v2(metadata: PackageMetadataV2) {
    let PackageMetadataV2 {
        id,
        storage_id: _,
        runtime_id: _,
        package_version: _,
        modules_metadata: _,
    } = metadata;
    object::delete(id);
}

fun modules_metadata_v1_to_v2(
    modules_metadata: VecMap<ascii::String, ModuleMetadataV1>,
): VecMap<ascii::String, ModuleMetadataV2> {
    let (module_names, mut module_metadata_v1) = modules_metadata.into_keys_values();
    let mut module_metadata_v2 = vector[];
    module_metadata_v1.reverse();
    while (!module_metadata_v1.is_empty()) {
        module_metadata_v2.push_back(module_metadata_v1_to_v2(module_metadata_v1.pop_back()));
    };
    module_metadata_v1.destroy_empty();
    iota::vec_map::from_keys_values(module_names, module_metadata_v2)
}

fun modules_metadata_v2_to_v1(
    modules_metadata: VecMap<ascii::String, ModuleMetadataV2>,
): VecMap<ascii::String, ModuleMetadataV1> {
    let (module_names, mut module_metadata_v2) = modules_metadata.into_keys_values();
    let mut module_metadata_v1 = vector[];
    module_metadata_v2.reverse();
    while (!module_metadata_v2.is_empty()) {
        module_metadata_v1.push_back(module_metadata_v2_to_v1(module_metadata_v2.pop_back()));
    };
    module_metadata_v2.destroy_empty();
    iota::vec_map::from_keys_values(module_names, module_metadata_v1)
}

fun module_metadata_v1_to_v2(module_metadata: ModuleMetadataV1): ModuleMetadataV2 {
    let ModuleMetadataV1 {
        authenticator_metadata,
    } = module_metadata;
    ModuleMetadataV2 {
        authenticator_metadata,
        view_function_metadata: vector[],
    }
}

fun module_metadata_v2_to_v1(module_metadata: ModuleMetadataV2): ModuleMetadataV1 {
    let ModuleMetadataV2 {
        authenticator_metadata,
        view_function_metadata,
    } = module_metadata;
    assert!(view_function_metadata.is_empty(), EViewFunctionMetadataNotEmpty);
    view_function_metadata.destroy_empty();
    ModuleMetadataV1 {
        authenticator_metadata,
    }
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
                ModuleMetadataV1 {
                    authenticator_metadata: vector[authenticator],
                },
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

/// Creates a `PackageMetadataV2` instance for testing, including
/// authenticator and view functions and skipping validation.
#[test_only]
public fun create_package_metadata_v2_for_testing(
    storage_id: ID,
    modules: vector<ascii::String>,
    functions: vector<ascii::String>,
    type_names: vector<TypeName>,
    view_modules: vector<ascii::String>,
    view_functions: vector<ascii::String>,
): PackageMetadataV2 {
    assert!(modules.length() == functions.length());
    assert!(modules.length() == type_names.length());
    assert!(view_modules.length() == view_functions.length());
    let addr = iota::derived_object::derive_address_for_testing(
        storage_id,
        PackageMetadataKey {},
    );
    let id = object::new_uid_from_hash(addr);
    let mut modules_metadata = iota::vec_map::empty<ascii::String, ModuleMetadataV2>();
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
                ModuleMetadataV2 {
                    authenticator_metadata: vector[authenticator],
                    view_function_metadata: vector[],
                },
            );
        };
        i = i + 1;
    };
    i = 0;
    while (i < view_modules.length()) {
        let module_name = view_modules[i];
        let function_name = view_functions[i];
        let view_function = ViewFunctionMetadataV1 {
            function_name,
        };
        if (modules_metadata.contains(&module_name)) {
            modules_metadata.get_mut(&module_name).view_function_metadata.push_back(view_function);
        } else {
            modules_metadata.insert(
                module_name,
                ModuleMetadataV2 {
                    authenticator_metadata: vector[],
                    view_function_metadata: vector[view_function],
                },
            );
        };
        i = i + 1;
    };
    PackageMetadataV2 {
        id,
        storage_id,
        runtime_id: storage_id,
        package_version: 1,
        modules_metadata,
    }
}

/// Creates a `PackageMetadataV2` instance for testing with only one view
/// function, skipping validation.
#[test_only]
public fun create_package_metadata_v2_for_testing_one_view_function(
    storage_id: ID,
    module_name: ascii::String,
    function_name: ascii::String,
): PackageMetadataV2 {
    create_package_metadata_v2_for_testing(
        storage_id,
        vector[],
        vector[],
        vector[],
        vector[module_name],
        vector[function_name],
    )
}
