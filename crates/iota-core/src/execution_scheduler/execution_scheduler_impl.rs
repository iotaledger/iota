// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashSet},
    sync::Arc,
};

use iota_config::node::AuthorityOverloadConfig;
use iota_metrics::spawn_monitored_task;
use iota_types::{
    digests::TransactionEffectsDigest,
    error::IotaResult,
    executable_transaction::VerifiedExecutableTransaction,
    storage::InputKey,
    transaction::{SenderSignedData, TransactionDataAPI},
};
use tokio::{sync::mpsc::UnboundedSender, time::Instant};
use tracing::{debug, warn};

use super::{
    ExecutingGuard, ExecutionSchedulerAPI, PendingCertificate, PendingCertificateStats,
    overload_tracker::OverloadTracker,
};
use crate::{
    authority::{AuthorityMetrics, authority_per_epoch_store::AuthorityPerEpochStore},
    execution_cache::{ObjectCacheRead, TransactionCacheRead},
};

#[derive(Clone)]
pub struct ExecutionScheduler {
    object_cache_read: Arc<dyn ObjectCacheRead>,
    transaction_cache_read: Arc<dyn TransactionCacheRead>,
    overload_tracker: Arc<OverloadTracker>,
    tx_ready_certificates: UnboundedSender<PendingCertificate>,
    metrics: Arc<AuthorityMetrics>,
}

/// Increments the pending-certificate gauge and registers the transaction with
/// the overload tracker for the duration it is waiting for its input objects.
struct PendingGuard<'a> {
    scheduler: &'a ExecutionScheduler,
    cert: &'a VerifiedExecutableTransaction,
}

impl<'a> PendingGuard<'a> {
    fn new(scheduler: &'a ExecutionScheduler, cert: &'a VerifiedExecutableTransaction) -> Self {
        scheduler
            .metrics
            .transaction_manager_num_pending_certificates
            .inc();
        scheduler
            .overload_tracker
            .add_pending_certificate(cert.data());
        Self { scheduler, cert }
    }
}

impl Drop for PendingGuard<'_> {
    fn drop(&mut self) {
        self.scheduler
            .metrics
            .transaction_manager_num_pending_certificates
            .dec();
        self.scheduler
            .overload_tracker
            .remove_pending_certificate(self.cert.data());
    }
}

impl ExecutionScheduler {
    pub(crate) fn new(
        object_cache_read: Arc<dyn ObjectCacheRead>,
        transaction_cache_read: Arc<dyn TransactionCacheRead>,
        tx_ready_certificates: UnboundedSender<PendingCertificate>,
        metrics: Arc<AuthorityMetrics>,
    ) -> Self {
        tracing::info!("Creating new ExecutionScheduler");
        Self {
            object_cache_read,
            transaction_cache_read,
            overload_tracker: Arc::new(OverloadTracker::new()),
            tx_ready_certificates,
            metrics,
        }
    }

    async fn schedule_transaction(
        self,
        cert: VerifiedExecutableTransaction,
        expected_effects_digest: Option<TransactionEffectsDigest>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) {
        let enqueue_time = Instant::now();
        let tx_data = cert.transaction_data();
        let input_object_kinds = tx_data
            .input_objects()
            .expect("input_objects() cannot fail");
        let input_object_keys: Vec<_> =
            match epoch_store.get_input_object_keys(&cert.key(), &input_object_kinds) {
                Ok(keys) => keys,
                Err(_) => {
                    // This is possible if the transaction is already executed.
                    assert!(
                        self.transaction_cache_read
                            .is_tx_already_executed(cert.digest())
                    );
                    self.metrics
                        .transaction_manager_num_enqueued_certificates
                        .with_label_values(&["already_executed"])
                        .inc();
                    return;
                }
            }
            .into_iter()
            .collect();
        let receiving_object_keys: HashSet<_> = tx_data
            .receiving_objects()
            .into_iter()
            .map(|entry| InputKey::VersionedObject {
                id: entry.object_id,
                version: entry.version,
            })
            .collect();
        let input_and_receiving_keys = [
            input_object_keys,
            receiving_object_keys.iter().cloned().collect(),
        ]
        .concat();

        let epoch = epoch_store.epoch();
        let digest = cert.digest();
        let digests = [*digest];
        debug!(?digest, "Scheduled transaction in execution scheduler");

        let availability = self
            .object_cache_read
            .multi_input_objects_available_cache_only(&input_and_receiving_keys);
        // Most of the time the transaction's input objects are already available;
        // only wait on the missing ones.
        let missing_input_keys: Vec<_> = input_and_receiving_keys
            .into_iter()
            .zip(availability)
            .filter_map(|(key, available)| if !available { Some(key) } else { None })
            .collect();
        if missing_input_keys.is_empty() {
            self.metrics
                .transaction_manager_num_enqueued_certificates
                .with_label_values(&["ready"])
                .inc();
            debug!(?digest, "Input objects already available");
            self.send_transaction_for_execution(&cert, expected_effects_digest, enqueue_time);
            return;
        }

        let _pending_guard = PendingGuard::new(&self, &cert);
        self.metrics
            .transaction_manager_num_enqueued_certificates
            .with_label_values(&["pending"])
            .inc();
        tokio::select! {
            _ = self.object_cache_read
                .notify_read_input_objects(&missing_input_keys, &receiving_object_keys, &epoch) => {
                    self.metrics
                        .transaction_manager_transaction_queue_age_s
                        .observe(enqueue_time.elapsed().as_secs_f64());
                    debug!(?digest, "Input objects available");
                    self.send_transaction_for_execution(&cert, expected_effects_digest, enqueue_time);
                }
            _ = self.transaction_cache_read.notify_read_executed_effects_digests(&digests) => {
                debug!(?digests, "Transaction already executed");
            }
        };
    }

    fn send_transaction_for_execution(
        &self,
        cert: &VerifiedExecutableTransaction,
        expected_effects_digest: Option<TransactionEffectsDigest>,
        _enqueue_time: Instant,
    ) {
        let pending_cert = PendingCertificate {
            certificate: cert.clone(),
            expected_effects_digest,
            waiting_input_objects: BTreeSet::new(),
            stats: PendingCertificateStats {
                #[cfg(test)]
                enqueue_time: _enqueue_time,
                ready_time: Some(Instant::now()),
            },
            executing_guard: Some(ExecutingGuard::new(
                self.metrics
                    .transaction_manager_num_executing_certificates
                    .clone(),
            )),
        };
        let _ = self.tx_ready_certificates.send(pending_cert);
    }
}

impl ExecutionSchedulerAPI for ExecutionScheduler {
    fn enqueue_impl(
        &self,
        certs: Vec<(
            VerifiedExecutableTransaction,
            Option<TransactionEffectsDigest>,
        )>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) {
        // Filter out certificates from the wrong epoch.
        let certs: Vec<_> = certs
            .into_iter()
            .filter_map(|cert| {
                if cert.0.epoch() == epoch_store.epoch() {
                    Some(cert)
                } else {
                    warn!(
                        "Ignoring enqueued certificate from wrong epoch. Expected={} Certificate={:?}",
                        epoch_store.epoch(),
                        cert.0.epoch(),
                    );
                    None
                }
            })
            .collect();
        let digests: Vec<_> = certs.iter().map(|(cert, _)| *cert.digest()).collect();
        let executed = self
            .transaction_cache_read
            .multi_get_executed_effects_digests(&digests);
        let mut already_executed_certs_num = 0;
        let pending_certs = certs.into_iter().zip(executed).filter_map(
            |((cert, expected_effects_digest), executed)| {
                if executed.is_none() {
                    Some((cert, expected_effects_digest))
                } else {
                    already_executed_certs_num += 1;
                    None
                }
            },
        );

        for (cert, expected_effects_digest) in pending_certs {
            let scheduler = self.clone();
            let epoch_store = epoch_store.clone();
            spawn_monitored_task!(
                epoch_store.within_alive_epoch(scheduler.schedule_transaction(
                    cert,
                    expected_effects_digest,
                    &epoch_store,
                ))
            );
        }

        self.metrics
            .transaction_manager_num_enqueued_certificates
            .with_label_values(&["already_executed"])
            .inc_by(already_executed_certs_num);
    }

    fn check_execution_overload(
        &self,
        overload_config: &AuthorityOverloadConfig,
        tx_data: &SenderSignedData,
    ) -> IotaResult {
        let inflight_queue_len = self.num_pending_certificates();
        self.overload_tracker
            .check_execution_overload(overload_config, tx_data, inflight_queue_len)
    }

    fn num_pending_certificates(&self) -> usize {
        (self
            .metrics
            .transaction_manager_num_pending_certificates
            .get()
            + self
                .metrics
                .transaction_manager_num_executing_certificates
                .get()) as usize
    }
}

#[cfg(test)]
#[path = "../unit_tests/execution_scheduler_tests.rs"]
mod execution_scheduler_tests;
