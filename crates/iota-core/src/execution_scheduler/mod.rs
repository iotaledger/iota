// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use enum_dispatch::enum_dispatch;
use execution_scheduler_impl::ExecutionScheduler;
use iota_config::node::AuthorityOverloadConfig;
use iota_sdk_types::SenderSignedTransaction;
use iota_types::{
    error::IotaResult, executable_transaction::VerifiedExecutableTransaction, storage::InputKey,
};
use prometheus_filtered::IntGauge;
use tokio::{sync::mpsc::UnboundedSender, time::Instant};
use transaction_manager::TransactionManager;

use crate::{
    authority::{
        AuthorityMetrics, ExecutionEnv, authority_per_epoch_store::AuthorityPerEpochStore,
        shared_object_version_manager::Schedulable,
    },
    execution_cache::{ObjectCacheRead, TransactionCacheRead},
};

pub(crate) mod execution_scheduler_impl;
mod overload_tracker;
pub(crate) mod transaction_manager;

/// Timing statistics for a pending transaction in the execution scheduler.
///
/// Tracks when a transaction was enqueued and when it became ready for
/// execution; used for latency metrics.
#[derive(Clone, Debug)]
pub struct PendingTransactionStats {
    /// The time this transaction enters the execution scheduler.
    #[cfg(test)]
    pub enqueue_time: Instant,
    /// The time this transaction becomes ready for execution.
    pub ready_time: Option<Instant>,
}

/// A transaction that is waiting in the execution scheduler for its input
/// objects to become available before it can be sent to the execution driver.
#[derive(Debug)]
pub struct PendingTransaction {
    /// The transaction to be executed.
    pub transaction: VerifiedExecutableTransaction,
    /// Environment in which the transaction will be executed.
    pub execution_env: ExecutionEnv,
    /// The input objects this transaction is waiting for to become available in
    /// order to be executed. Only used by `TransactionManager`.
    pub waiting_input_objects: BTreeSet<InputKey>,
    /// Stats about this transaction.
    pub stats: PendingTransactionStats,
    /// Held while the transaction is executing, to keep the
    /// executing-certificates gauge accurate. Only set by
    /// `ExecutionScheduler`.
    pub executing_guard: Option<ExecutingGuard>,
}

#[derive(Debug)]
pub struct ExecutingGuard {
    num_executing_certificates: IntGauge,
}

#[enum_dispatch]
pub trait ExecutionSchedulerAPI {
    fn enqueue(
        &self,
        transactions: Vec<(Schedulable, ExecutionEnv)>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    );

    fn enqueue_transactions(
        &self,
        transactions: Vec<(VerifiedExecutableTransaction, ExecutionEnv)>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    );

    fn check_execution_overload(
        &self,
        overload_config: &AuthorityOverloadConfig,
        tx_data: &SenderSignedTransaction,
    ) -> IotaResult;

    /// Returns the number of transactions pending or being executed right now.
    fn num_pending_transactions(&self) -> usize;
}

// The `TransactionManager` variant is much larger than `ExecutionScheduler`,
// but there is exactly one `ExecutionSchedulerWrapper` per node and it lives
// behind an `Arc`, so the variant size has no practical cost. Keep upstream's
// unboxed layout and silence the lint rather than boxing `Inner`.
#[allow(clippy::large_enum_variant)]
#[enum_dispatch(ExecutionSchedulerAPI)]
pub enum ExecutionSchedulerWrapper {
    ExecutionScheduler(ExecutionScheduler),
    TransactionManager(TransactionManager),
}

/// Scheduler selected when neither `ENABLE_EXECUTION_SCHEDULER` nor
/// `ENABLE_TRANSACTION_MANAGER` is set. The selector below and the tests that
/// assert the default both read it, so it is the only place to change.
pub(crate) const DEFAULT_USE_EXECUTION_SCHEDULER: bool = false;

impl ExecutionSchedulerWrapper {
    pub fn new(
        object_cache_read: Arc<dyn ObjectCacheRead>,
        transaction_cache_read: Arc<dyn TransactionCacheRead>,
        tx_ready_transactions: UnboundedSender<PendingTransaction>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        metrics: Arc<AuthorityMetrics>,
    ) -> Self {
        // Explicit env overrides win over the default, so the suite can be run
        // deterministically against either implementation (and pinned per test).
        // `ENABLE_TRANSACTION_MANAGER` (opt-out) takes precedence so it keeps
        // forcing TM even after the default is flipped. A proper node-config
        // selector is future work.
        let enable_execution_scheduler = if std::env::var("ENABLE_TRANSACTION_MANAGER").is_ok() {
            false
        } else if std::env::var("ENABLE_EXECUTION_SCHEDULER").is_ok() {
            true
        } else {
            DEFAULT_USE_EXECUTION_SCHEDULER
        };
        tracing::info!(
            "Using {} for transaction execution",
            if enable_execution_scheduler {
                "ExecutionScheduler"
            } else {
                "TransactionManager"
            }
        );
        if enable_execution_scheduler {
            Self::ExecutionScheduler(ExecutionScheduler::new(
                object_cache_read,
                transaction_cache_read,
                tx_ready_transactions,
                metrics,
            ))
        } else {
            Self::TransactionManager(TransactionManager::new(
                object_cache_read,
                transaction_cache_read,
                epoch_store,
                tx_ready_transactions,
                metrics,
            ))
        }
    }

    /// Whether the new `ExecutionScheduler` is in use (vs
    /// `TransactionManager`).
    pub fn uses_execution_scheduler(&self) -> bool {
        matches!(self, Self::ExecutionScheduler(_))
    }
}

impl ExecutingGuard {
    pub fn new(num_executing_certificates: IntGauge) -> Self {
        num_executing_certificates.inc();
        Self {
            num_executing_certificates,
        }
    }
}

impl Drop for ExecutingGuard {
    fn drop(&mut self) {
        self.num_executing_certificates.dec();
    }
}
