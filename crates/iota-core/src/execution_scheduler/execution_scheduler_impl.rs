// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashSet},
    sync::Arc,
};

use iota_config::node::AuthorityOverloadConfig;
use iota_metrics::spawn_monitored_task;
use iota_sdk_types::SenderSignedTransaction;
use iota_types::{
    effects::TransactionEffectsAPI,
    error::IotaResult,
    executable_transaction::VerifiedExecutableTransaction,
    storage::InputKey,
    transaction::{SenderSignedTransactionAPI, TransactionAPI},
};
use tokio::{sync::mpsc::UnboundedSender, time::Instant};
use tracing::{debug, warn};

use crate::{
    authority::{
        AuthorityMetrics, ExecutionEnv, authority_per_epoch_store::AuthorityPerEpochStore,
        shared_object_version_manager::Schedulable,
    },
    execution_cache::{ObjectCacheRead, TransactionCacheRead},
    execution_scheduler::{
        ExecutingGuard, ExecutionSchedulerAPI, PendingTransaction, PendingTransactionStats,
        overload_tracker::OverloadTracker,
    },
};

#[derive(Clone)]
pub struct ExecutionScheduler {
    object_cache_read: Arc<dyn ObjectCacheRead>,
    transaction_cache_read: Arc<dyn TransactionCacheRead>,
    overload_tracker: Arc<OverloadTracker>,
    tx_ready_transactions: UnboundedSender<PendingTransaction>,
    metrics: Arc<AuthorityMetrics>,
}

/// Increments the pending-certificate gauge and registers the transaction with
/// the overload tracker for the duration it is waiting for its input objects.
struct PendingGuard<'a> {
    scheduler: &'a ExecutionScheduler,
    tx: &'a VerifiedExecutableTransaction,
}

impl<'a> PendingGuard<'a> {
    fn new(scheduler: &'a ExecutionScheduler, tx: &'a VerifiedExecutableTransaction) -> Self {
        scheduler
            .metrics
            .transaction_manager_num_pending_certificates
            .inc();
        scheduler
            .overload_tracker
            .add_pending_transaction(tx.data());
        Self { scheduler, tx }
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
            .remove_pending_transaction(self.tx.data());
    }
}

impl ExecutionScheduler {
    pub(crate) fn new(
        object_cache_read: Arc<dyn ObjectCacheRead>,
        transaction_cache_read: Arc<dyn TransactionCacheRead>,
        tx_ready_transactions: UnboundedSender<PendingTransaction>,
        metrics: Arc<AuthorityMetrics>,
    ) -> Self {
        tracing::info!("Creating new ExecutionScheduler");
        Self {
            object_cache_read,
            transaction_cache_read,
            overload_tracker: Arc::new(OverloadTracker::new()),
            tx_ready_transactions,
            metrics,
        }
    }

    async fn schedule_transaction(
        self,
        tx: VerifiedExecutableTransaction,
        execution_env: ExecutionEnv,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) {
        let enqueue_time = Instant::now();
        let tx_data = tx.data().transaction();
        // Use the full input set (transaction inputs plus every MoveAuthenticator's
        // input objects), not just `input_objects()`. Execution reads the same
        // superset via `collect_all_input_object_kind_for_reading`; scheduling on
        // the narrower set would dispatch before an authenticator's shared input
        // (e.g. the Clock) is available and panic in the input loader.
        let input_object_kinds = tx
            .data()
            .collect_all_input_object_kind_for_reading()
            // The transaction is already verified by the time it is scheduled, so
            // its input objects are well-formed.
            .expect("collect_all_input_object_kind_for_reading() cannot fail");
        let input_object_keys: Vec<_> = epoch_store
            .get_input_object_keys(
                &tx.key(),
                &input_object_kinds,
                &execution_env.assigned_versions,
            )
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
        let digest = tx.digest();
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
            self.send_transaction_for_execution(&tx, execution_env, enqueue_time);
            return;
        }

        let _pending_guard = PendingGuard::new(&self, &tx);
        self.metrics
            .transaction_manager_num_enqueued_certificates
            .with_label_values(&["pending"])
            .inc();
        tokio::select! {
            _ = self.object_cache_read
                .notify_read_input_objects(&missing_input_keys, &receiving_object_keys, epoch) => {
                    self.metrics
                        .transaction_manager_transaction_queue_age_s
                        .observe(enqueue_time.elapsed().as_secs_f64());
                    debug!(?digest, "Input objects available");
                    self.send_transaction_for_execution(&tx, execution_env, enqueue_time);
                }
            _ = self.transaction_cache_read.notify_read_executed_effects_digests(
                "ExecutionScheduler::notify_read_executed_effects_digests",
                &digests,
            ) => {
                debug!(?digests, "Transaction already executed");
            }
        };
    }

    fn send_transaction_for_execution(
        &self,
        tx: &VerifiedExecutableTransaction,
        execution_env: ExecutionEnv,
        _enqueue_time: Instant,
    ) {
        let pending_tx = PendingTransaction {
            transaction: tx.clone(),
            execution_env,
            waiting_input_objects: BTreeSet::new(),
            stats: PendingTransactionStats {
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
        // Mirrors `TransactionManager::transaction_ready`: the ready rate feeds
        // the overload monitor's latency-based load shedding, and the execution
        // driver decrements `execution_driver_dispatch_queue` for every
        // transaction it receives regardless of which scheduler produced it.
        self.metrics.txn_ready_rate_tracker.lock().record();
        let _ = self.tx_ready_transactions.send(pending_tx);
        self.metrics.transaction_manager_num_ready.inc();
        self.metrics.execution_driver_dispatch_queue.inc();
    }

    /// When we schedule a transaction, it should be impossible for it to have
    /// been executed in a previous epoch.
    #[cfg(debug_assertions)]
    fn assert_not_executed_previous_epochs(&self, tx: &VerifiedExecutableTransaction) {
        let epoch = tx.epoch();
        let digest = *tx.digest();
        let digests = [digest];
        let executed = self
            .transaction_cache_read
            .multi_get_executed_effects(&digests)
            .pop()
            .unwrap();
        // Due to pruning, we may not always have executed effects for the
        // transaction even if it was executed. So this is a best-effort check.
        if let Some(executed) = executed {
            assert_eq!(
                executed.epoch(),
                epoch,
                "Transaction {} was executed in epoch {}, but scheduled again in epoch {}",
                digest,
                executed.epoch(),
                epoch
            );
        }
    }
}

impl ExecutionSchedulerAPI for ExecutionScheduler {
    fn enqueue(
        &self,
        transactions: Vec<(Schedulable, ExecutionEnv)>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) {
        // schedule all transactions immediately
        let mut txns = Vec::with_capacity(transactions.len());
        let mut rest = Vec::new();

        for (schedulable, env) in transactions {
            if let Schedulable::Transaction(tx) = schedulable {
                txns.push((tx, env));
            } else {
                rest.push((schedulable, env));
            }
        }

        self.enqueue_transactions(txns, epoch_store);

        if rest.is_empty() {
            return;
        }

        let rest_keys = rest
            .iter()
            .map(|(schedulable, _)| schedulable.key())
            .collect::<Vec<_>>();

        let scheduler = self.clone();
        let epoch_store = epoch_store.clone();
        spawn_monitored_task!(epoch_store.clone().within_alive_epoch(async move {
            let rest_digests = epoch_store
                .notify_read_tx_key_to_digest(&rest_keys)
                .await
                .expect("db error");
            // The keys resolve from a table that outlives pruning, so on a node far
            // enough behind in consensus a transaction one names can already be
            // executed and pruned away. Nothing is left to schedule then, and
            // demanding the block would crash the replay.
            let rest_transactions = scheduler
                .transaction_cache_read
                .multi_get_transaction_blocks(&rest_digests)
                .into_iter()
                .zip(rest.into_iter().map(|(_, env)| env))
                .zip(rest_digests.iter())
                .filter_map(|((tx, env), digest)| {
                    let Some(tx) = tx else {
                        // The pruner drops a transaction and its executed effects in
                        // one batch, so there is nothing left to tell this case from
                        // a transaction that never executed.
                        warn!(
                            "transaction {digest} named by a resolved key is missing, \
                             assuming it was executed and pruned"
                        );
                        return None;
                    };
                    let tx = tx.as_ref().clone();
                    Some((
                        VerifiedExecutableTransaction::new_system(tx, epoch_store.epoch()),
                        env,
                    ))
                })
                .collect::<Vec<_>>();
            scheduler.enqueue_transactions(rest_transactions, &epoch_store);
        }));
    }

    fn enqueue_transactions(
        &self,
        transactions: Vec<(VerifiedExecutableTransaction, ExecutionEnv)>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) {
        // Filter out transactions from the wrong epoch.
        let transactions: Vec<_> = transactions
            .into_iter()
            .filter_map(|txn| {
                if txn.0.epoch() == epoch_store.epoch() {
                    #[cfg(debug_assertions)]
                    self.assert_not_executed_previous_epochs(&txn.0);

                    Some(txn)
                } else {
                    warn!(
                        "Ignoring enqueued transaction from wrong epoch. Expected={} Transaction={:?}",
                        epoch_store.epoch(),
                        txn.0.epoch(),
                    );
                    None
                }
            })
            .collect();
        let digests: Vec<_> = transactions.iter().map(|(txn, _)| *txn.digest()).collect();
        let executed = self
            .transaction_cache_read
            .multi_get_executed_effects_digests(&digests);
        let mut already_executed_num = 0;
        let pending = transactions.into_iter().zip(executed).filter_map(
            |((txn, execution_env), executed)| {
                if executed.is_none() {
                    Some((txn, execution_env))
                } else {
                    already_executed_num += 1;
                    None
                }
            },
        );

        for (txn, execution_env) in pending {
            let scheduler = self.clone();
            let epoch_store = epoch_store.clone();
            spawn_monitored_task!(
                epoch_store.within_alive_epoch(scheduler.schedule_transaction(
                    txn,
                    execution_env,
                    &epoch_store,
                ))
            );
        }

        self.metrics
            .transaction_manager_num_enqueued_certificates
            .with_label_values(&["already_executed"])
            .inc_by(already_executed_num);
    }

    fn check_execution_overload(
        &self,
        overload_config: &AuthorityOverloadConfig,
        tx_data: &SenderSignedTransaction,
    ) -> IotaResult {
        let inflight_queue_len = self.num_pending_transactions();
        self.overload_tracker
            .check_execution_overload(overload_config, tx_data, inflight_queue_len)
    }

    fn num_pending_transactions(&self) -> usize {
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
