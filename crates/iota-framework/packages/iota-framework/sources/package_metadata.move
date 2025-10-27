module iota::package_metadata;

use iota::function_handle::FunctionHandle;
use std::ascii;

// === Errors ===

#[error(code = 0)]
const EModuleNotFound: vector<u8> = b"The specified module is not part of the package.";
#[error(code = 1)]
const EFunctionNotFound: vector<u8> = b"The specified function is not part of the package.";

// === Constants ===

// === Structs ===

public struct PackageMetadataV1 has key {
    // PackageMetadata (derived) id
    id: UID,
    // Package id
    package_id: ID,
    // Package version
    package_version: u64,
    // Package name
    package_name: ascii::String,
    // Package modules
    module_handles: vector<ascii::String>,
    /// Handles to external and internal functions.
    function_handles: vector<FunctionHandle>,
}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

// === View Functions ===

public fun package_id(self: &PackageMetadataV1): ID {
    self.package_id
}

public fun version(self: &PackageMetadataV1): u64 {
    self.package_version
}

public fun name(self: &PackageMetadataV1): ascii::String {
    self.package_name
}

public fun package_modules(self: &PackageMetadataV1): &vector<ascii::String> {
    &self.module_handles
}

public fun function_handles(self: &PackageMetadataV1): &vector<FunctionHandle> {
    &self.function_handles
}

public fun find_module_handle(self: &PackageMetadataV1, module_name: ascii::String): u16 {
    let mut module_handle = self.module_handles.find_index!(|m| m == module_name);
    assert!(module_handle.is_some(), EModuleNotFound);

    module_handle.extract() as u16
}

public fun find_function_handle(
    self: &PackageMetadataV1,
    module_handle: u16,
    function_name: ascii::String,
): &FunctionHandle {
    let mut function_handle_index = self
        .function_handles
        .find_index!(|f| f.module_handle() == module_handle && f.name() == function_name);
    assert!(function_handle_index.is_some(), EFunctionNotFound);
    
    self.function_handles.borrow(function_handle_index.extract())
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
