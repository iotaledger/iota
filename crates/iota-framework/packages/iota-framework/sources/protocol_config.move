// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This module provides access to protocol configuration feature flags.
/// Feature flags control the availability of various protocol features and
/// are enabled/disabled at specific protocol versions during epoch changes.

module iota::protocol_config;

/// Checks if a specific protocol feature flag is enabled.
///
/// Restricted to internal use within the iota-framework package only.
/// If we need to use it in iota-system, we can add friend declarations.
/// We should never need to expose this to user packages.
///
/// # Arguments
/// * `feature_flag_name` - The name of the feature flag as bytes (e.g., b"enable_vdf")
///   - It is expected to be a valid UTF-8 string
///   - The flag should exist in the protocol config
///
/// # Returns
/// * `true` if the feature is enabled in the current protocol version
/// * `false` if the feature is disabled
///
/// # Example (for framework use only)
/// ```move
/// use iota::protocol_config;
///
/// if (protocol_config::is_feature_enabled(b"enable_accumulators")) {
///     // Accumulators are available
/// };
/// ```
public(package) native fun is_feature_enabled(feature_flag_name: vector<u8>): bool;

/// Returns the value of a protocol config parameter.
///
/// Returns `none` if the parameter is not defined in the current protocol
/// version, or if `T` does not match the parameter's actual type.
///
/// Restricted to internal use within the iota-framework package.
///
/// # Arguments
/// * `param_name` - The name of the config parameter as bytes (e.g., b"max_arguments")
///
/// # Type parameter
/// * `T` - Must be one of `u16`, `u32`, `u64`, or `bool` — the concrete type of the parameter.
///
/// # Example (framework use only)
/// ```move
/// let max_args: Option<u32> = protocol_config::get_attr<u32>(b"max_arguments");
/// ```
public(package) native fun get_attr<T: copy + drop + store>(param_name: vector<u8>): Option<T>;
