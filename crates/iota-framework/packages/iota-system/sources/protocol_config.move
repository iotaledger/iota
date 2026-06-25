// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This module mirrors `iota::protocol_config` for use within the iota-system
/// package. The native implementations are identical; they are registered under
/// the iota-system address so that `public(package)` visibility keeps them
/// inaccessible to user packages.
module iota_system::protocol_config;

/// Checks if a specific protocol feature flag is enabled.
///
/// # Arguments
/// * `feature_flag_name` - The name of the feature flag as bytes (e.g., b"enable_vdf")
///
/// # Returns
/// * `true` if the feature is enabled in the current protocol version
/// * `false` if the feature is disabled or unknown
public(package) native fun is_feature_enabled(feature_flag_name: vector<u8>): bool;

/// Returns the value of a protocol config parameter.
///
/// Aborts if the parameter is absent in the current protocol version,
/// if `T` does not match the parameter's actual type, or if `param_name` is
/// not valid UTF-8.
///
/// # Type parameter
/// * `T` - Must be one of `u16`, `u32`, `u64`, or `bool`.
public(package) native fun get_attr<T: copy + drop + store>(param_name: vector<u8>): T;
