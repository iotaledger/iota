use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExpensiveSafetyCheckConfig {
    /// If enabled, at epoch boundary, we will check that the storage
    /// fund balance is always identical to the sum of the storage
    /// rebate of all live objects, and that the total SUI in the network remains
    /// the same.
    #[serde(default)]
    enable_epoch_sui_conservation_check: bool,

    /// If enabled, we will check that the total SUI in all input objects of a tx
    /// (both the Move part and the storage rebate) matches the total SUI in all
    /// output objects of the tx + gas fees
    #[serde(default)]
    enable_deep_per_tx_sui_conservation_check: bool,

    /// Disable epoch SUI conservation check even when we are running in debug mode.
    #[serde(default)]
    force_disable_epoch_sui_conservation_check: bool,

    /// If enabled, at epoch boundary, we will check that the accumulated
    /// live object state matches the end of epoch root state digest.
    #[serde(default)]
    enable_state_consistency_check: bool,

    /// Disable state consistency check even when we are running in debug mode.
    #[serde(default)]
    force_disable_state_consistency_check: bool,

    #[serde(default)]
    enable_secondary_index_checks: bool,
    // TODO: Add more expensive checks here
}

impl ExpensiveSafetyCheckConfig {
    pub fn new_enable_all() -> Self {
        Self {
            enable_epoch_sui_conservation_check: true,
            enable_deep_per_tx_sui_conservation_check: true,
            force_disable_epoch_sui_conservation_check: false,
            enable_state_consistency_check: true,
            force_disable_state_consistency_check: false,
            enable_secondary_index_checks: false, // Disable by default for now
        }
    }

    pub fn new_disable_all() -> Self {
        Self {
            enable_epoch_sui_conservation_check: false,
            enable_deep_per_tx_sui_conservation_check: false,
            force_disable_epoch_sui_conservation_check: true,
            enable_state_consistency_check: false,
            force_disable_state_consistency_check: true,
            enable_secondary_index_checks: false,
        }
    }

    pub fn force_disable_epoch_sui_conservation_check(&mut self) {
        self.force_disable_epoch_sui_conservation_check = true;
    }

    pub fn enable_epoch_sui_conservation_check(&self) -> bool {
        (self.enable_epoch_sui_conservation_check || cfg!(debug_assertions))
            && !self.force_disable_epoch_sui_conservation_check
    }

    pub fn force_disable_state_consistency_check(&mut self) {
        self.force_disable_state_consistency_check = true;
    }

    pub fn enable_state_consistency_check(&self) -> bool {
        (self.enable_state_consistency_check || cfg!(debug_assertions))
            && !self.force_disable_state_consistency_check
    }

    pub fn enable_deep_per_tx_sui_conservation_check(&self) -> bool {
        self.enable_deep_per_tx_sui_conservation_check || cfg!(debug_assertions)
    }

    pub fn enable_secondary_index_checks(&self) -> bool {
        self.enable_secondary_index_checks
    }
}
