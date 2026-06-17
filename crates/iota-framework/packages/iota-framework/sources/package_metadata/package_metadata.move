// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Package metadata management module
/// An IOTA package can have associated metadata that provides,
/// on-chain, additional information about the package.
module iota::package_metadata;

use iota::derived_object;
use iota::dynamic_field;
use iota::module_metadata::{Self, ModuleMetadata};
use iota::vec_map::{Self, VecMap};
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
public struct ModuleMetadataV1FieldName has copy, drop, store {}
public struct ModulesMetadataFieldName has copy, drop, store {}

public struct ModuleName(ascii::String) has copy, drop, store;

/// Represents the metadata of a Move package. This includes information
/// such as the storage ID, runtime ID, version. The modules_metadata field
/// is deprecated in favor of a dynamic field attached to this object that
/// maps module names to iota::module_metadata::ModuleMetadata instances.
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

/// This is now deprecated in favor of iota::module_metadata::ModuleMetadata.
/// It represented the first version of the metadata associated with a module in the
/// package and included only the authenticator functions information.
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

/// Borrows the `ModuleMetadata` of the module named `module_name`.
/// Aborts with `EModuleMetadataNotFound` if the package has no metadata for that module.
public fun module_metadata(self: &PackageMetadataV1, module_name: &ascii::String): &ModuleMetadata {
    let modules_metadata = dynamic_field::borrow<
        ModulesMetadataFieldName,
        VecMap<ModuleName, ModuleMetadata>,
    >(
        &self.id,
        ModulesMetadataFieldName {},
    );
    let name = ModuleName(*module_name);
    assert!(modules_metadata.contains(&name), EModuleMetadataNotFound);
    let idx = modules_metadata.get_idx(&name);
    let (_, metadata) = modules_metadata.get_entry_by_idx(idx);
    metadata
}

/// Borrows the `AuthenticatorMetadataV1` of the function named `function_name`
/// within the module named `module_name`.
/// Aborts if the module or the authenticator metadata is not found.
#[allow(deprecated_usage)]
public fun module_authenticator_function_metadata_v1(
    self: &PackageMetadataV1,
    module_name: &ascii::String,
    function_name: &ascii::String,
): &AuthenticatorMetadataV1 {
    self.modules_metadata_v1(module_name).authenticator_metadata_v1(function_name)
}

/// Borrows the `AuthenticatorMetadataV1` of the function named `function_name`
/// from the given module metadata.
/// Aborts if the authenticator metadata is not found.
#[allow(deprecated_usage)]
public fun authenticator_function_metadata_v1(
    self: &ModuleMetadata,
    function_name: &ascii::String,
): &AuthenticatorMetadataV1 {
    let module_metadata_v1 = self.borrow<
        ModuleMetadataV1FieldName,
        ModuleMetadataV1,
    >(ModuleMetadataV1FieldName {});
    module_metadata_v1.authenticator_metadata_v1(function_name)
}

/// Safely gets the `AuthenticatorMetadataV1` of the function named `function_name`
/// from the given module metadata, returning `none` if it is not found.
#[allow(deprecated_usage)]
public fun try_get_authenticator_function_metadata_v1(
    self: &ModuleMetadata,
    function_name: &ascii::String,
): Option<AuthenticatorMetadataV1> {
    let module_metadata_v1 = self.borrow<
        ModuleMetadataV1FieldName,
        ModuleMetadataV1,
    >(ModuleMetadataV1FieldName {});
    module_metadata_v1.try_get_authenticator_metadata_v1(function_name)
}

/// Return the account type of the authenticator represented by this metadata
public fun account_type(self: &AuthenticatorMetadataV1): TypeName {
    self.account_type
}

/// Return the name of the authenticate function represented by this metadata
public fun function_name(self: &AuthenticatorMetadataV1): &ascii::String {
    &self.function_name
}

// ===  Private constructors ===

/// On-chain constructor: builds a `PackageMetadataV1` with the dynamic-field
/// layout and freezes it into an immutable object. Invoked by the system when a
/// package is published or upgraded.
#[allow(unused_function)]
fun create_package_metadata_v1_with_dynamic_metadata(
    storage_id: ID,
    runtime_id: ID,
    package_version: u64,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
    view_function_names: vector<vector<ascii::String>>,
) {
    let package_metadata = build_package_metadata_v1_with_dynamic_metadata(
        storage_id,
        runtime_id,
        package_version,
        modules,
        auth_functions,
        type_names,
        view_function_names,
    );
    transfer::freeze_object(package_metadata);
}

/// Builds a `PackageMetadataV1` with the dynamic-field layout and returns it
/// without freezing. The on-chain constructor freezes the result; tests keep
/// the owned value. Both paths share this builder so the recorded layout
/// cannot diverge.
fun build_package_metadata_v1_with_dynamic_metadata(
    storage_id: ID,
    runtime_id: ID,
    package_version: u64,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
    view_function_names: vector<vector<ascii::String>>,
): PackageMetadataV1 {
    let modules_metadata = create_modules_metadata(
        storage_id,
        modules,
        auth_functions,
        type_names,
        view_function_names,
    );

    let id_address = derived_object::derive_address(storage_id, PackageMetadataKey {});
    let id = object::new_uid_from_hash(id_address);

    let mut package_metadata = PackageMetadataV1 {
        id,
        storage_id,
        runtime_id,
        package_version,
        modules_metadata: vec_map::empty(),
    };

    dynamic_field::add(
        &mut package_metadata.id,
        PackageMetadataVersionFieldName {},
        2,
    );

    dynamic_field::add(
        &mut package_metadata.id,
        ModulesMetadataFieldName {},
        modules_metadata,
    );
    package_metadata
}

/// Builds the per-module metadata map for a package, deriving one
/// `ModuleMetadata` object per module and populating it with the module's
/// authenticator and view function metadata. The input vectors are parallel:
/// entry `i` describes the module named `modules[i]`.
fun create_modules_metadata(
    storage_id: ID,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
    view_function_names: vector<vector<ascii::String>>,
): VecMap<ModuleName, ModuleMetadata> {
    assert!(modules.length() == auth_functions.length());
    assert!(modules.length() == type_names.length());
    assert!(modules.length() == view_function_names.length());
    let mut modules_metadata = vec_map::empty<ModuleName, ModuleMetadata>();
    let mut i = 0;
    while (i < modules.length()) {
        let module_name = modules[i];
        let mut module_metadata = module_metadata::new(storage_id, module_name);
        let mut authenticator_metadata = vector[];
        let mut j = 0;
        while (j < auth_functions[i].length()) {
            let function_name = auth_functions[i][j];
            let account_type = type_names[i][j];
            authenticator_metadata.push_back(AuthenticatorMetadataV1 {
                function_name,
                account_type,
            });
            j = j + 1;
        };
        module_metadata.add(
            ModuleMetadataV1FieldName {},
            ModuleMetadataV1 { authenticator_metadata },
        );
        module_metadata.add_view_function_metadata_v1(view_function_names[i]);

        modules_metadata.insert(ModuleName(module_name), module_metadata);
        i = i + 1;
    };
    modules_metadata
}

// === Deprecated functions ===

/// Legacy function to safely get the module metadata list of the package represented by this metadata
#[deprecated]
public fun try_get_modules_metadata_v1(
    self: &PackageMetadataV1,
    module_name: &ascii::String,
): Option<ModuleMetadataV1> {
    if (
        dynamic_field::exists_<PackageMetadataVersionFieldName>(
            &self.id,
            PackageMetadataVersionFieldName {},
        )
    ) {
        let package_metadata_version = dynamic_field::borrow<PackageMetadataVersionFieldName, u64>(
            &self.id,
            PackageMetadataVersionFieldName {},
        );
        assert!(package_metadata_version == 2, EWrongPackageVersion);
        let modules_metadata = dynamic_field::borrow<
            ModulesMetadataFieldName,
            VecMap<ModuleName, ModuleMetadata>,
        >(&self.id, ModulesMetadataFieldName {});
        let name = ModuleName(*module_name);
        if (!modules_metadata.contains(&name)) {
            return option::none()
        };
        let idx = modules_metadata.get_idx(&name);
        let (_, module_metadata) = modules_metadata.get_entry_by_idx(idx);
        if (module_metadata.contains(ModuleMetadataV1FieldName {})) {
            option::some(*module_metadata.borrow(ModuleMetadataV1FieldName {}))
        } else {
            option::none()
        }
    } else {
        self.modules_metadata.try_get(module_name)
    }
}

/// Legacy function to borrow the module metadata list of the package represented by this metadata.
/// Aborts if the module is not found.
#[deprecated]
public fun modules_metadata_v1(
    self: &PackageMetadataV1,
    module_name: &ascii::String,
): &ModuleMetadataV1 {
    if (
        dynamic_field::exists_<PackageMetadataVersionFieldName>(
            &self.id,
            PackageMetadataVersionFieldName {},
        )
    ) {
        let package_metadata_version = dynamic_field::borrow<PackageMetadataVersionFieldName, u64>(
            &self.id,
            PackageMetadataVersionFieldName {},
        );
        assert!(package_metadata_version == 2, EWrongPackageVersion);
        let module_metadata = self.module_metadata(module_name);
        module_metadata.borrow(ModuleMetadataV1FieldName {})
    } else {
        assert!(self.modules_metadata.contains(module_name), EModuleMetadataNotFound);
        self.modules_metadata.get(module_name)
    }
}

/// Legacy function to safely get the `AuthenticatorMetadataV1` associated with the specified
/// `function_name` within the module metadata.
#[deprecated]
public fun try_get_authenticator_metadata_v1(
    self: &ModuleMetadataV1,
    function_name: &ascii::String,
): Option<AuthenticatorMetadataV1> {
    self.authenticator_metadata.find_index!(|m| m.function_name == *function_name).and!(|index| {
        option::some(self.authenticator_metadata[index])
    })
}

/// Legacy function to borrow the `AuthenticatorMetadataV1` associated with the specified
/// `function_name`.
/// Aborts if the authenticator metadata is not found for that function.
#[deprecated]
public fun authenticator_metadata_v1(
    self: &ModuleMetadataV1,
    function_name: &ascii::String,
): &AuthenticatorMetadataV1 {
    let mut index = self.authenticator_metadata.find_index!(|m| m.function_name == *function_name);
    assert!(index.is_some(), EAuthenticatorMetadataNotFound);
    &self.authenticator_metadata[index.extract()]
}

// === Test functions ===

/// Creates a legacy (pre-dynamic-field) `PackageMetadataV1` for testing.
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

    let addr = iota::derived_object::derive_address(
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

/// Creates a `PackageMetadataV1` with the dynamic-field layout for testing,
/// returning the owned object instead of freezing it.
#[test_only]
public fun create_package_metadata_v1_with_dynamic_metadata_for_testing(
    storage_id: ID,
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
    view_functions: vector<vector<ascii::String>>,
): PackageMetadataV1 {
    build_package_metadata_v1_with_dynamic_metadata(
        storage_id,
        // runtime_id and package_version default to a single-version package.
        storage_id,
        1,
        modules,
        auth_functions,
        type_names,
        view_functions,
    )
}

/// Builds the legacy `ModuleMetadataV1` map for testing. The input vectors are
/// parallel: entry `i` describes the module named `modules[i]`.
#[test_only]
public fun create_modules_metadata_v1(
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
): VecMap<ascii::String, ModuleMetadataV1> {
    assert!(modules.length() == auth_functions.length());
    assert!(modules.length() == type_names.length());
    let mut modules_metadata = vec_map::empty<ascii::String, ModuleMetadataV1>();
    let mut i = 0;
    while (i < modules.length()) {
        let module_name = modules[i];
        let mut authenticator_metadata = vector[];
        let mut j = 0;
        while (j < auth_functions[i].length()) {
            let function_name = auth_functions[i][j];
            let account_type = type_names[i][j];
            authenticator_metadata.push_back(AuthenticatorMetadataV1 {
                function_name,
                account_type,
            });
            j = j + 1;
        };
        modules_metadata.insert(
            module_name,
            ModuleMetadataV1 { authenticator_metadata },
        );
        i = i + 1;
    };
    modules_metadata
}

/// Creates a single `AuthenticatorMetadataV1` for testing.
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
