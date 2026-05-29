// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Package metadata management module
/// An IOTA package can have associated metadata that provides,
/// on-chain, additional information about the package.
module iota::package_metadata_inner;

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
const EViewFunctionMetadataNotCompatible: vector<u8> =
    b"The requested view function metadata is not supported in this version.";

// === Structs ===

/// Represents the metadata of a Move package V2.
public struct PackageMetadataV2Inner has copy, drop, store {
    modules_metadata: VecMap<ascii::String, ModuleMetadataV2>,
}

/// Represents metadata associated with a module in the package.
/// V2 includes both the authenticator functions information and
///  view function information.
public struct ModuleMetadataV2 has copy, drop, store {
    authenticator_metadata: vector<AuthenticatorMetadataV2>,
    view_functions_metadata: vector<ViewFunctionMetadataV1>,
}

/// Represents metadata for an authenticator within the package.
/// It includes the name of the authenticate function and the TypeName
/// of the first parameter (i.e., the account object type).
public struct AuthenticatorMetadataV2 has copy, drop, store {
    function_name: ascii::String,
    account_type: TypeName,
}

/// Represents view function metadata for a package.
public struct ViewFunctionMetadataV1 has copy, drop, store {
    function_name: ascii::String,
}

// === Public constructors ===
public(package) fun create_package_metadata_v2_inner(
    modules: vector<ascii::String>,
    auth_functions: vector<vector<ascii::String>>,
    type_names: vector<vector<TypeName>>,
    view_functions: vector<vector<ascii::String>>,
): PackageMetadataV2Inner {
    assert!(modules.length() == auth_functions.length());
    assert!(modules.length() == type_names.length());
    assert!(modules.length() == view_functions.length());
    let mut modules_metadata = iota::vec_map::empty<ascii::String, ModuleMetadataV2>();
    let mut i = 0;
    while (i < modules.length()) {
        let module_name = modules[i];
        let mut authenticator_metadata = vector[];
        let mut view_functions_metadata = vector[];
        let mut j = 0;
        while (j < auth_functions[i].length()) {
            let function_name = auth_functions[i][j];
            let account_type = type_names[i][j];
            authenticator_metadata.push_back(
                create_authenticator_metadata_v2(function_name, account_type),
            );
            j = j + 1;
        };
        let mut k = 0;
        while (k < view_functions[i].length()) {
            let view_function_name = view_functions[i][k];
            view_functions_metadata.push_back(
                create_view_function_metadata_v1(view_function_name),
            );
            k = k + 1;
        };
        modules_metadata.insert(
            module_name,
            create_module_metadata_v2(authenticator_metadata, view_functions_metadata),
        );
        i = i + 1;
    };
    PackageMetadataV2Inner {
        modules_metadata,
    }
}

public(package) fun create_module_metadata_v2(
    authenticator_metadata: vector<AuthenticatorMetadataV2>,
    view_functions_metadata: vector<ViewFunctionMetadataV1>,
): ModuleMetadataV2 {
    ModuleMetadataV2 {
        authenticator_metadata,
        view_functions_metadata,
    }
}

public(package) fun create_authenticator_metadata_v2(
    function_name: ascii::String,
    account_type: TypeName,
): AuthenticatorMetadataV2 {
    AuthenticatorMetadataV2 {
        function_name,
        account_type,
    }
}

public(package) fun create_view_function_metadata_v1(
    function_name: ascii::String,
): ViewFunctionMetadataV1 {
    ViewFunctionMetadataV1 {
        function_name,
    }
}

// === Public functions ===

public fun try_get_module_metadata_v2(
    self: &PackageMetadataV2Inner,
    module_name: &ascii::String,
): Option<ModuleMetadataV2> {
    self.modules_metadata.try_get(module_name).and!(|module_metadata_v2| {
        option::some(module_metadata_v2)
    })
}

public fun module_metadata(
    self: &PackageMetadataV2Inner,
    module_name: &ascii::String,
): ModuleMetadataV2 {
    let modules_metadata = self.modules_metadata;
    assert!(modules_metadata.contains(module_name), EModuleMetadataNotFound);
    let module_metadata = modules_metadata.get(module_name);
    *module_metadata
}

public fun try_get_authenticator_metadata(
    self: &ModuleMetadataV2,
    function_name: &ascii::String,
): Option<AuthenticatorMetadataV2> {
    self.authenticator_metadata.find_index!(|m| m.function_name == *function_name).and!(|index| {
        option::some(self.authenticator_metadata[index])
    })
}

public fun authenticator_metadata(
    self: &ModuleMetadataV2,
    function_name: &ascii::String,
): AuthenticatorMetadataV2 {
    let mut index = self.authenticator_metadata.find_index!(|m| m.function_name == *function_name);
    assert!(index.is_some(), EAuthenticatorMetadataNotFound);
    self.authenticator_metadata[index.extract()]
}

public fun auth_function_name(self: &AuthenticatorMetadataV2): &ascii::String {
    &self.function_name
}

/// Return the account type of the authenticator represented by this metadata
public fun account_type(self: &AuthenticatorMetadataV2): TypeName {
    self.account_type
}

public fun view_functions_metadata(self: &ModuleMetadataV2): vector<ViewFunctionMetadataV1> {
    self.view_functions_metadata
}

public fun view_function_metadata(
    self: &ModuleMetadataV2,
    function_name: &ascii::String,
): ViewFunctionMetadataV1 {
    let view_functions_metadata = self.view_functions_metadata();
    let mut index = view_functions_metadata.find_index!(|m| m.function_name == *function_name);
    assert!(index.is_some(), EAuthenticatorMetadataNotFound);
    let view_function_metadata = view_functions_metadata[index.extract()];
    view_function_metadata
}

public fun try_get_view_function_metadata(
    self: &ModuleMetadataV2,
    function_name: &ascii::String,
): Option<ViewFunctionMetadataV1> {
    let view_functions_metadata = self.view_functions_metadata();
    view_functions_metadata.find_index!(|m| m.function_name == *function_name).and!(|index| {
        option::some(view_functions_metadata[index])
    })
}

public fun view_function_name(self: &ViewFunctionMetadataV1): &ascii::String {
    &self.function_name
}

/// Creates a `PackageMetadataV1` instance for testing with only one
/// authenticator, skipping validation.
#[test_only]
public fun create_package_metadata_v2_for_testing_one_authenticator(
    module_name: ascii::String,
    function_name: ascii::String,
    type_name: TypeName,
): PackageMetadataV2Inner {
    create_package_metadata_v2_inner(
        vector[module_name],
        vector[vector[function_name]],
        vector[vector[type_name]],
        vector[vector[]],
    )
}

/// Creates a `PackageMetadataV2` instance for testing with only one
/// authenticator and package view function metadata, skipping validation.
#[test_only]
public fun create_package_metadata_v2_for_testing_with_one_view_function(
    module_name: ascii::String,
    view_function: ascii::String,
): PackageMetadataV2Inner {
    create_package_metadata_v2_inner(
        vector[module_name],
        vector[vector[]],
        vector[vector[]],
        vector[vector[view_function]],
    )
}
