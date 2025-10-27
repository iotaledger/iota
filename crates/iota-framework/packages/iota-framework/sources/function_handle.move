module iota::function_handle;

use std::ascii;

// === Errors ===

// === Constants ===

// === Structs ===

public struct FunctionHandle has copy, drop, store {
    /// The module that defines the function.
    module_handle: u16,
    /// The name of the function.
    name: ascii::String,
    /// If set, it contains the authenticator version set for
    /// the function, otherwise None.
    authenticator_version: Option<u8>,
}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

// === View Functions ===

public fun module_handle(self: &FunctionHandle): u16 {
    self.module_handle
}

public fun name(self: &FunctionHandle): ascii::String {
    self.name
}

public fun authenticator_version(self: &FunctionHandle): Option<u8> {
    self.authenticator_version
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
