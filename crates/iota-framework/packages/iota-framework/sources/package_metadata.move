// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Package metadata management module
/// An IOTA package can have associated metadata that provides,
/// on-chain, additional information about the package.
module iota::package_metadata;

use iota::account::AuthenticatorInfoMetadataV1;
use std::ascii;

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
    module_handles: vector<ascii::String>,
    /// Handles to internal modules functions, with (module_handle,
    /// function_name).
    function_handles: vector<FunctionHandle>,
    /// Metadata for functions in the package, indexed by function handle.
    function_metadata: vector<FunctionMetadataV1>,
}

/// Represents a handle to a function within a module in the package.
public struct FunctionHandle has copy, drop, store {
    module_handle: u16,
    function_name: ascii::String,
}

/// Represents metadata associated with a function in the package. This includes
/// the authenticator information.
public struct FunctionMetadataV1 has copy, drop, store {
    authenticator_info: AuthenticatorInfoMetadataV1,
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

/// Return the function metadata list of the package represented by this metadata
public fun function_metadata_v1(self: &PackageMetadataV1): &vector<FunctionMetadataV1> {
    &self.function_metadata
}

/// Returns the `AuthenticatorInfoMetadataV1` associated with the specified
/// `module_name` and `function_name`, if any.
public fun authenticator_info_metadata_v1(
    self: &PackageMetadataV1,
    module_name: ascii::String,
    function_name: ascii::String,
): Option<AuthenticatorInfoMetadataV1> {
    self.module_handles.find_index!(|m| m == module_name).and!(|module_handle| {
        self
            .function_handles
            .find_index!(
                |f| f.module_handle == module_handle as u16 && f.function_name == function_name,
            )
            .and!(|fm| option::some(self.function_metadata[fm].authenticator_info))
    })
}
