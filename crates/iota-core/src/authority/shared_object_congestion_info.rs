use im::HashMap;
use iota_types::base_types::ObjectID;

use super::shared_object_congestion_tracker::ExecutionTime;

/// Holds shared object congestion data for a single shared object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerObjectCongestionInfo {
    /// List of gas prices of scheduled transactions operating on a shared
    /// object.
    scheduled_txs_gas_prices: Vec<u64>,

    /// List of execution times (duration) of transactions operating on a
    /// shared object.
    execution_times: Vec<ExecutionTime>,
}

impl PerObjectCongestionInfo {
    /// Create/initialize a new `PerObjectCongestionInfo` with empty shared
    /// object congestion info.
    pub fn new() -> Self {
        Self {
            scheduled_txs_gas_prices: Vec::new(),
            execution_times: Vec::new(),
        }
    }
}

impl Default for PerObjectCongestionInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Holds shared object congestion data for a single consensus commit round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerCommitCongestionInfo {
    /// Shared object congestion data for multiple shared objects appearing
    /// in a single consensus commit round.
    object_data: HashMap<ObjectID, PerObjectCongestionInfo>,
}

impl PerCommitCongestionInfo {
    /// Create/initialize a new `PerCommitCongestionInfo` with empty shared
    /// object congestion info.
    pub fn new() -> Self {
        Self {
            object_data: HashMap::new(),
        }
    }
}

impl Default for PerCommitCongestionInfo {
    fn default() -> Self {
        Self::new()
    }
}
