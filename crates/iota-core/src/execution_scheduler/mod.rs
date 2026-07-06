// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use enum_dispatch::enum_dispatch;
use execution_scheduler_impl::ExecutionScheduler;
use iota_config::node::AuthorityOverloadConfig;
use iota_types::{
    digests::TransactionEffectsDigest,
    error::IotaResult,
    executable_transaction::VerifiedExecutableTransaction,
    storage::InputKey,
    transaction::{SenderSignedData, VerifiedCertificate},
};
use prometheus_filtered::IntGauge;
use tokio::{sync::mpsc::UnboundedSender, time::Instant};
use transaction_manager::TransactionManager;

use crate::{
    authority::{AuthorityMetrics, authority_per_epoch_store::AuthorityPerEpochStore},
    execution_cache::{ObjectCacheRead, TransactionCacheRead},
};

pub(crate) mod execution_scheduler_impl;
mod overload_tracker;
pub(crate) mod transaction_manager;

/// Timing statistics for a pending transaction in the execution scheduler.
///
/// It tracks when a transaction was enqueued and when it became ready for
/// execution and it is used for latency metrics.
///
/// The legacy name (in the certificate era) was `PendingCertificateStats`.
/// It is renamed to `PendingTransactionStats` because this type now (in the
/// certificate-less era) covers more consensus transaction kinds, not just
/// certificates. The renaming is safe and backward-compatible since this is
/// a fully internal type.
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
///
/// The legacy name (in the certificate era) was `PendingCertificate`.
/// It is renamed to `PendingTransaction` because this type now (in the
/// certificate-less era) covers more consensus transaction kinds, not just
/// certificates. The renaming is safe and backward-compatible since this is
/// a fully internal type.
#[derive(Debug)]
pub struct PendingTransaction {
    /// The transaction to be executed.
    pub transaction: VerifiedExecutableTransaction,
    /// When executing from checkpoint, the certified effects digest is
    /// provided, so that forks can be detected prior to committing the
    /// transaction.
    pub expected_effects_digest: Option<TransactionEffectsDigest>,
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
pub(crate) trait ExecutionSchedulerAPI {
    fn enqueue_impl(
        &self,
        certs: Vec<(
            VerifiedExecutableTransaction,
            Option<TransactionEffectsDigest>,
        )>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    );

    fn enqueue(
        &self,
        certs: Vec<VerifiedExecutableTransaction>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) {
        let certs = certs.into_iter().map(|cert| (cert, None)).collect();
        self.enqueue_impl(certs, epoch_store)
    }

    fn enqueue_with_expected_effects_digest(
        &self,
        certs: Vec<(VerifiedExecutableTransaction, TransactionEffectsDigest)>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) {
        let certs = certs
            .into_iter()
            .map(|(cert, fx)| (cert, Some(fx)))
            .collect();
        self.enqueue_impl(certs, epoch_store)
    }

    /// Enqueues certificates / verified transactions. Once all of the input
    /// objects are available locally for a certificate, the certified
    /// transaction will be sent to the execution driver.
    ///
    /// REQUIRED: Shared object locks must be taken before enqueueing
    /// transactions with shared objects!
    fn enqueue_certificates(
        &self,
        certs: Vec<VerifiedCertificate>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) {
        let executable_txns = certs
            .into_iter()
            .map(VerifiedExecutableTransaction::new_from_certificate)
            .collect();
        self.enqueue(executable_txns, epoch_store)
    }

    fn check_execution_overload(
        &self,
        overload_config: &AuthorityOverloadConfig,
        tx_data: &SenderSignedData,
    ) -> IotaResult;

    /// Returns the number of transactions pending or being executed right now.
    fn num_pending_certificates(&self) -> usize;
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
/// `ENABLE_TRANSACTION_MANAGER` is set — the single source of truth for the
/// default. Flip to `true` (and update the CI matrix) when promoting
/// `ExecutionScheduler` to the default; the selector below and any test that
/// asserts the default both follow it automatically.
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
