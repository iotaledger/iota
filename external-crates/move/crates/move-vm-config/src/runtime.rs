// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_binary_format::{binary_config::BinaryConfig, file_format_common::VERSION_MAX};
#[cfg(feature = "tracing")]
use once_cell::sync::Lazy;

use crate::verifier::{DEFAULT_MAX_CONSTANT_VECTOR_LEN, VerifierConfig};

pub const DEFAULT_MAX_VALUE_NEST_DEPTH: u64 = 128;

/// Dynamic config options for the Move VM.
#[derive(Clone, Debug)]
pub struct VMConfig {
    pub verifier: VerifierConfig,
    pub max_binary_format_version: u32,
    pub runtime_limits_config: VMRuntimeLimitsConfig,
    // When this flag is set to true, MoveVM will check invariant violation in swap_loc
    pub enable_invariant_violation_check_in_swap_loc: bool,
    // When this flag is set to true, MoveVM will check that there are no trailing bytes after
    // deserializing and check for no metadata bytes
    pub check_no_extraneous_bytes_during_deserialization: bool,
    // When this flag is set to true, errors from the VM will be augmented with execution state
    // (stacktrace etc.)
    pub error_execution_state: bool,
    // configuration for binary deserialization (modules)
    pub binary_config: BinaryConfig,
    // Configs for profiling VM
    pub profiler_config: Option<VMProfilerConfig>,
    // Whether value serialization errors when generating type layouts should be rethrown or
    // converted to a different error.
    pub rethrow_serialization_type_layout_errors: bool,
    /// Maximal nodes which are allowed when converting to layout. This includes
    /// the types of fields for struct types.
    pub max_type_to_layout_nodes: Option<u64>,
    /// Count variants as nodes.
    pub variant_nodes: bool,
    /// Check for deprecated global storage operations during deserialization.
    pub deprecate_global_storage_ops_during_deserialization: bool,
    /// Normalize all formula to have a consistent structure.
    pub normalize_depth_formula: bool,
    /// When true, `LdConst` charges for the abstract value size of the constant instead of its
    /// serialized byte length.
    pub charge_ld_const_abstract_size: bool,
}

impl VMConfig {
    /// Create a config for test execution:
    /// - `allow_unpublishable_code_execution` allows loading unpublishable modules
    /// - `package_arena_size` overrides the arena package size
    pub fn new_for_test(
        allow_unpublishable_code_execution: bool,
        package_arena_size: Option<u64>,
    ) -> Self {
        let binary_config = if allow_unpublishable_code_execution {
            BinaryConfig::new_unpublishable()
        } else {
            BinaryConfig::legacy_with_flags(
                // check_no_extraneous_bytes
                true, // deprecate_global_storage_ops
                false,
            )
        };
        Self {
            verifier: VerifierConfig::default(),
            max_binary_format_version: VERSION_MAX,
            runtime_limits_config: VMRuntimeLimitsConfig {
                vector_len_max: DEFAULT_MAX_CONSTANT_VECTOR_LEN,
                max_value_nest_depth: Some(DEFAULT_MAX_VALUE_NEST_DEPTH),
                hardened_otw_check: true,
                package_arena_size,
            },
            enable_invariant_violation_check_in_swap_loc: true,
            profiler_config: None,
            check_no_extraneous_bytes_during_deserialization: true,
            error_execution_state: true,
            binary_config,
            rethrow_serialization_type_layout_errors: false,
            max_type_to_layout_nodes: Some(512),
            variant_nodes: true,
            deprecate_global_storage_ops_during_deserialization: false,
            normalize_depth_formula: true,
            charge_ld_const_abstract_size: true,
        }
    }
}

impl Default for VMConfig {
    fn default() -> Self {
        Self {
            check_no_extraneous_bytes_during_deserialization: false,
            binary_config: BinaryConfig::legacy_with_flags(false, false),
            ..Self::new_for_test(false, None)
        }
    }
}

#[derive(Clone, Debug)]
pub struct VMRuntimeLimitsConfig {
    /// Maximum number of items that can be pushed into a vec
    pub vector_len_max: u64,
    /// Maximum value nest depth for structs
    pub max_value_nest_depth: Option<u64>,
    // include hardened OTW checks
    pub hardened_otw_check: bool,
    /// Maximum size (in bytes) for an individual package's arena size.
    /// NOTE: YOU SHOULD PROBABLY NOT BE SETTING THIS FLAG FOR ANY REASON.
    pub package_arena_size: Option<u64>,
}

#[cfg(feature = "tracing")]
const MOVE_VM_PROFILER_ENV_VAR_NAME: &str = "MOVE_VM_PROFILE";

#[cfg(feature = "tracing")]
static PROFILER_ENABLED: Lazy<bool> =
    Lazy::new(|| std::env::var(MOVE_VM_PROFILER_ENV_VAR_NAME).is_ok());


#[derive(Clone, Debug)]
pub struct VMProfilerConfig {
    /// User configured full path override
    pub full_path: std::path::PathBuf,
    /// Whether or not to track bytecode instructions
    pub track_bytecode_instructions: bool,
    /// Whether or not to use the long name for functions
    pub use_long_function_name: bool,
}

#[cfg(feature = "tracing")]
impl std::default::Default for VMProfilerConfig {
    fn default() -> Self {
        Self {
            full_path: get_default_output_filepath(),
            track_bytecode_instructions: false,
            use_long_function_name: false,
        }
    }
}

#[cfg(feature = "tracing")]
impl VMProfilerConfig {
    pub fn get_default_config_if_enabled() -> Option<VMProfilerConfig> {
        if *PROFILER_ENABLED {
            Some(VMProfilerConfig::default())
        } else {
            None
        }
    }
}

pub fn get_default_output_filepath() -> std::path::PathBuf {
    let mut default_name = std::path::PathBuf::from(".");
    default_name.push("gas_profile.json");
    default_name
}
