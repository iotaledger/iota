// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
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

#[derive(Clone, Debug)]
pub struct PendingTransactionStats {
    // The time this certificate enters execution scheduler.
    #[allow(unused)]
    #[cfg(test)]
    pub enqueue_time: Instant,
    // The time this certificate becomes ready for execution.
    pub ready_time: Option<Instant>,
}

#[derive(Debug)]
pub struct PendingTransaction {
    // Certified transaction to be executed.
    pub transaction: VerifiedExecutableTransaction,
    // When executing from checkpoint, the certified effects digest is provided, so that forks can
    // be detected prior to committing the transaction.
    pub expected_effects_digest: Option<TransactionEffectsDigest>,
    // The input object this certificate is waiting for to become available in order to be
    // executed. This is only used by TransactionManager.
    pub waiting_input_objects: BTreeSet<InputKey>,
    // Stores stats about this transaction.
    pub stats: PendingTransactionStats,
    pub executing_guard: Option<ExecutingGuard>,
}

#[derive(Debug)]
pub(crate) struct ExecutingGuard {
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

    /// Enqueues certificates / verified transactions into TransactionManager.
    /// Once all of the input objects are available locally for a
    /// certificate, the certified transaction will be sent to execution driver.
    ///
    /// REQUIRED: Shared object locks must be taken before calling enqueueing
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

    // Returns the number of transactions pending or being executed right now.
    fn num_pending_certificates(&self) -> usize;

    // Verify TM has no pending item for tests.
    #[cfg(test)]
    fn check_empty_for_testing(&self);
}

#[allow(clippy::large_enum_variant)]
#[enum_dispatch(ExecutionSchedulerAPI)]
pub(crate) enum ExecutionSchedulerWrapper {
    ExecutionScheduler(ExecutionScheduler),
    TransactionManager(TransactionManager),
}

pub(crate) const DEFAULT_USE_EXECUTION_SCHEDULER: bool = false;

impl ExecutionSchedulerWrapper {
    pub fn new(
        object_cache_read: Arc<dyn ObjectCacheRead>,
        transaction_cache_read: Arc<dyn TransactionCacheRead>,
        tx_ready_transactions: UnboundedSender<PendingTransaction>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        metrics: Arc<AuthorityMetrics>,
    ) -> Self {
        let enable_execution_scheduler = if std::env::var("ENABLE_EXECUTION_SCHEDULER").is_ok() {
            true
        } else if std::env::var("ENABLE_TRANSACTION_MANAGER").is_ok() {
            false
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

    /// Whether the new `ExecutionScheduler` is in use (vs the legacy
    /// `TransactionManager`). Used by tests to assert which implementation a
    /// parameterized run actually selected, guarding against a silent fallback.
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
