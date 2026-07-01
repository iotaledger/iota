// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod metrics;
mod monitor;
mod stats;

#[cfg(test)]
mod tests;

use std::time::Duration;

use iota_types::base_types::AuthorityName;
pub use metrics::ValidatorClientMetrics;
pub use monitor::ValidatorClientMonitor;
use strum::EnumIter;

/// Operation types for validator performance tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, EnumIter)]
pub enum OperationType {
    Submit,
    Effects,
    HealthCheck,
    Consensus,
}

impl OperationType {
    pub fn as_str(&self) -> &str {
        match self {
            OperationType::Submit => "submit",
            OperationType::Effects => "effects",
            OperationType::HealthCheck => "health_check",
            OperationType::Consensus => "consensus",
        }
    }
}

/// Feedback from TransactionDriver operations
#[derive(Debug, Clone)]
pub struct OperationFeedback {
    /// The unique authority name (public key)
    pub authority_name: AuthorityName,
    /// The human-readable display name for the validator
    pub display_name: String,
    /// The operation type
    pub operation: OperationType,
    /// The ping type. If it's not a ping request, then this is None.
    pub ping: bool,
    /// Result of the operation: Ok(latency) if successful, Err(()) if failed.
    pub result: Result<Duration, ()>,
}
