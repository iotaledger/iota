// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Defines constants and options that are used for module generation

#[derive(Clone, Debug)]
pub struct ModuleGeneratorOptions {
    /// The maximum number of locals that can be defined within a generated function definition.
    pub max_locals: usize,
    /// The maximum number of fields that will be generated for any struct.
    pub max_fields: usize,
    pub min_fields: usize,
    /// The maximum number of structs that can be generated for a module
    pub max_structs: usize,
    /// The maximum number of functions that can be generated for a module.
    pub max_functions: usize,
    /// The maximum number of type parameters functions and structs.
    pub max_ty_params: usize,
    /// The maximum size that generated byte arrays can be.
    pub byte_array_max_size: usize,
    /// The maximum size that a generated string can be.
    pub max_string_size: usize,
    /// The maximum number of arguments to generated function definitions.
    pub max_function_call_size: usize,
    /// The maximum number of return types of generated function definitions.
    pub max_ret_types_size: usize,
    /// Whether or not generate modules should only contain simple (non-reference, or nested
    /// struct) types.
    pub simple_types_only: bool,
    /// Whether references are allowed to be generated for e.g. function parameters, locals.
    pub references_allowed: bool,
    /// Whether the generated modules should have any resources declared.
    pub add_resources: bool,
    /// The minimum number of entries in any table
    pub min_table_size: usize,
    /// If set, all functions with type parameters will have arguments of those types as well.
    pub args_for_ty_params: bool,
}

impl Default for ModuleGeneratorOptions {
    fn default() -> Self {
        Self {
            min_fields: 1,
            max_locals: 10,
            max_fields: 20,
            max_structs: 100,
            max_functions: 100,
            max_ty_params: 5,
            byte_array_max_size: 64,
            max_string_size: 32,
            max_function_call_size: 23,
            max_ret_types_size: 4,
            simple_types_only: false,
            references_allowed: true,
            add_resources: true,
            min_table_size: 1,
            args_for_ty_params: false,
        }
    }
}
