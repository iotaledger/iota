// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
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
    pub fn as_str(&self) -> &'static str {
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
    /// Result of the operation: Ok(latency) if successful, Err(()) if failed.
    pub result: Result<Duration, ()>,
    /// The timestamp when the operation feedback was observed.
    pub timestamp: std::time::Instant,
}

impl OperationFeedback {
    pub fn builder(
        authority_name: AuthorityName,
        display_name: String,
        operation: OperationType,
    ) -> OperationFeedbackBuilder {
        OperationFeedbackBuilder {
            authority_name,
            display_name,
            operation,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OperationFeedbackBuilder {
    /// The unique authority name (public key)
    pub authority_name: AuthorityName,
    /// The human-readable display name for the validator
    pub display_name: String,
    /// The operation type
    pub operation: OperationType,
}

impl OperationFeedbackBuilder {
    pub fn result_at(
        self,
        result: Result<Duration, ()>,
        timestamp: std::time::Instant,
    ) -> OperationFeedback {
        OperationFeedback {
            authority_name: self.authority_name,
            display_name: self.display_name,
            operation: self.operation,
            result,
            timestamp,
        }
    }

    pub fn result_now(self, result: Result<Duration, ()>) -> OperationFeedback {
        self.result_at(result, std::time::Instant::now())
    }

    pub fn ok_at(self, latency: Duration, timestamp: std::time::Instant) -> OperationFeedback {
        self.result_at(Ok(latency), timestamp)
    }

    pub fn ok_now(self, latency: Duration) -> OperationFeedback {
        self.ok_at(latency, std::time::Instant::now())
    }

    pub fn err_at(self, timestamp: std::time::Instant) -> OperationFeedback {
        self.result_at(Err(()), timestamp)
    }

    pub fn err_now(self) -> OperationFeedback {
        self.err_at(std::time::Instant::now())
    }
}
