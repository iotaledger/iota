// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_vm_config::verifier::MeterConfig;
use serde::{Deserialize, Serialize};

// Default values for verifier signing config.
pub const DEFAULT_MAX_PER_FUN_METER_UNITS: usize = 2_200_000;
pub const DEFAULT_MAX_PER_MOD_METER_UNITS: usize = 2_200_000;
pub const DEFAULT_MAX_PER_PKG_METER_UNITS: usize = 2_200_000;

pub const DEFAULT_MAX_BACK_EDGES_PER_FUNCTION: usize = 10_000;
pub const DEFAULT_MAX_BACK_EDGES_PER_MODULE: usize = 10_000;

pub const DEFAULT_SANITY_CHECK_WITH_REGEX_REFERENCE_SAFETY_UNITS: usize = 2_200_000;

/// This holds limits that are only set and used by the verifier during signing
/// _only_. There are additional limits in the `MeterConfig` and
/// `VerifierConfig` that are used during both signing and execution, however
/// those limits cannot be set here and must be protocol versioned.
///
/// Post-consensus validation has to reach the same verdict on every validator,
/// so it meters published packages with the protocol config's limits instead;
/// see `ProtocolConfig::meter_config` and
/// `ProtocolConfig::verifier_signing_limits`. The defaults here equal those
/// protocol values.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct VerifierSigningConfig {
    #[serde(default)]
    max_per_fun_meter_units: Option<usize>,
    #[serde(default)]
    max_per_mod_meter_units: Option<usize>,
    #[serde(default)]
    max_per_pkg_meter_units: Option<usize>,

    #[serde(default)]
    max_back_edges_per_function: Option<usize>,
    #[serde(default)]
    max_back_edges_per_module: Option<usize>,

    #[serde(default)]
    pub sanity_check_with_regex_reference_safety: Option<usize>,
}

impl VerifierSigningConfig {
    pub fn max_per_fun_meter_units(&self) -> usize {
        self.max_per_fun_meter_units
            .unwrap_or(DEFAULT_MAX_PER_FUN_METER_UNITS)
    }

    pub fn max_per_mod_meter_units(&self) -> usize {
        self.max_per_mod_meter_units
            .unwrap_or(DEFAULT_MAX_PER_MOD_METER_UNITS)
    }

    pub fn max_per_pkg_meter_units(&self) -> usize {
        self.max_per_pkg_meter_units
            .unwrap_or(DEFAULT_MAX_PER_PKG_METER_UNITS)
    }

    pub fn max_back_edges_per_function(&self) -> usize {
        self.max_back_edges_per_function
            .unwrap_or(DEFAULT_MAX_BACK_EDGES_PER_FUNCTION)
    }

    pub fn max_back_edges_per_module(&self) -> usize {
        self.max_back_edges_per_module
            .unwrap_or(DEFAULT_MAX_BACK_EDGES_PER_MODULE)
    }

    pub fn sanity_check_with_regex_reference_safety(&self) -> usize {
        self.sanity_check_with_regex_reference_safety
            .unwrap_or(DEFAULT_SANITY_CHECK_WITH_REGEX_REFERENCE_SAFETY_UNITS)
    }

    /// Return sign-time only limit for back edges for the verifier.
    pub fn limits_for_signing(&self) -> (usize, usize, usize) {
        (
            self.max_back_edges_per_function(),
            self.max_back_edges_per_module(),
            self.sanity_check_with_regex_reference_safety(),
        )
    }

    /// MeterConfig for metering packages during signing. It is NOT stable
    /// between binaries and cannot used during execution.
    pub fn meter_config_for_signing(&self) -> MeterConfig {
        MeterConfig {
            max_per_fun_meter_units: Some(self.max_per_fun_meter_units() as u128),
            max_per_mod_meter_units: Some(self.max_per_mod_meter_units() as u128),
            max_per_pkg_meter_units: Some(self.max_per_pkg_meter_units() as u128),
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_protocol_config::ProtocolConfig;

    use super::*;

    /// A validator that leaves its `VerifierSigningConfig` at the defaults must
    /// reach the same verdict at admission as post-consensus validation
    /// does with the protocol config's limits, so the two sets of defaults
    /// have to agree.
    #[test]
    fn defaults_match_protocol_config() {
        let protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
        let signing_config = VerifierSigningConfig::default();

        assert_eq!(
            signing_config.limits_for_signing(),
            protocol_config.verifier_signing_limits()
        );

        let from_node = signing_config.meter_config_for_signing();
        let from_protocol = protocol_config.meter_config();
        assert_eq!(
            from_node.max_per_fun_meter_units,
            from_protocol.max_per_fun_meter_units
        );
        assert_eq!(
            from_node.max_per_mod_meter_units,
            from_protocol.max_per_mod_meter_units
        );
        assert_eq!(
            from_node.max_per_pkg_meter_units,
            from_protocol.max_per_pkg_meter_units
        );
    }
}
