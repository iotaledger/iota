// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
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

use super::{ExecutionSchedulerAPI, PendingTransaction, overload_tracker::OverloadTracker};
use crate::{
    authority::{AuthorityMetrics, authority_per_epoch_store::AuthorityPerEpochStore},
    execution_cache::{ObjectCacheRead, TransactionCacheRead},
    execution_scheduler::{ExecutingGuard, PendingTransactionStats},
};

#[derive(Clone)]
pub(crate) struct ExecutionScheduler {
    object_cache_read: Arc<dyn ObjectCacheRead>,
    transaction_cache_read: Arc<dyn TransactionCacheRead>,
    overload_tracker: Arc<OverloadTracker>,
    tx_ready_transactions: UnboundedSender<PendingTransaction>,
    metrics: Arc<AuthorityMetrics>,
}

struct PendingGuard<'a> {
    scheduler: &'a ExecutionScheduler,
    cert: &'a VerifiedExecutableTransaction,
}

impl<'a> PendingGuard<'a> {
    pub fn new(scheduler: &'a ExecutionScheduler, cert: &'a VerifiedExecutableTransaction) -> Self {
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
    pub fn new(
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
                    // TODO: Eventually we could pass assigned shared object versions
                    // to the scheduler so that this call cannot return Err.
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
            .map(|entry| {
                InputKey::VersionedObject {
                    // TODO: Add support for receiving ConsensusV2 objects. For now this assumes
                    // fastpath.
                    id: *entry.object_id(),
                    version: entry.version(),
                }
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
        tracing::trace!(
            ?digests,
            "Waiting for input objects: {:?}",
            input_and_receiving_keys
        );

        let availability = self
            .object_cache_read
            .multi_input_objects_available_cache_only(&input_and_receiving_keys);
        // Most of the times, the transaction's input objects are already available.
        // We can check the availability of the input objects first, and only wait for
        // the missing input objects if necessary.
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
            self.send_transaction_for_execution(
                &cert,
                expected_effects_digest,
                #[cfg(test)]
                enqueue_time,
            );
            return;
        }

        let _pending_guard = PendingGuard::new(&self, &cert);
        self.metrics
            .transaction_manager_num_enqueued_certificates
            .with_label_values(&["pending"])
            .inc();
        tokio::select! {
            _ = self.object_cache_read
                .notify_read_input_objects(&missing_input_keys, &receiving_object_keys, &epoch)
                => {
                    self.metrics
                        .transaction_manager_transaction_queue_age_s
                        .observe(enqueue_time.elapsed().as_secs_f64());
                    debug!(?digest, "Input objects available");
                    // TODO: Eventually we could fold execution_driver into the scheduler.
                    self.send_transaction_for_execution(&cert, expected_effects_digest, #[cfg(test)] enqueue_time);
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
        #[cfg(test)] enqueue_time: Instant,
    ) {
        let pending_cert = PendingTransaction {
            transaction: cert.clone(),
            expected_effects_digest,
            waiting_input_objects: BTreeSet::new(),
            stats: PendingTransactionStats {
                #[cfg(test)]
                enqueue_time,
                ready_time: Some(Instant::now()),
            },
            executing_guard: Some(ExecutingGuard::new(
                self.metrics
                    .transaction_manager_num_executing_certificates
                    .clone(),
            )),
        };
        let _ = self.tx_ready_transactions.send(pending_cert);
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
        // Filter out certificates from wrong epoch.
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

    #[cfg(test)]
    fn check_empty_for_testing(&self) {
        assert_eq!(self.num_pending_certificates(), 0);
    }
}

#[cfg(test)]
mod test {
    use std::{time::Duration, vec};

    use iota_sdk_types::{ObjectId, Owner, VersionAssignment};
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        IOTA_FRAMEWORK_PACKAGE_ID,
        base_types::SequenceNumber,
        crypto::deterministic_random_account_key,
        executable_transaction::VerifiedExecutableTransaction,
        object::Object,
        transaction::{CallArg, SharedObjectRef, VerifiedTransaction},
    };
    use tokio::{
        sync::mpsc::{UnboundedReceiver, error::TryRecvError, unbounded_channel},
        time::{Instant, sleep},
    };

    use super::{ExecutionScheduler, PendingTransaction};
    use crate::{
        authority::{AuthorityState, authority_tests::init_state_with_objects},
        execution_scheduler::{ExecutionSchedulerAPI, ExecutionSchedulerWrapper},
    };

    #[allow(clippy::disallowed_methods)] // allow unbounded_channel()
    fn make_execution_scheduler(
        state: &AuthorityState,
    ) -> (
        ExecutionSchedulerWrapper,
        UnboundedReceiver<PendingTransaction>,
    ) {
        // Create a new execution scheduler instead of reusing the authority's, to
        // examine execution_scheduler output from rx_ready_transactions.
        let (tx_ready_transactions, rx_ready_transactions) = unbounded_channel();
        // Do not call ExecutionSchedulerWrapper::new() here, because we want to always
        // construct an ExecutionScheduler in the tests here, not TransactionManager.
        let execution_scheduler =
            ExecutionSchedulerWrapper::ExecutionScheduler(ExecutionScheduler::new(
                state.get_object_cache_reader().clone(),
                state.get_transaction_cache_reader().clone(),
                tx_ready_transactions,
                state.metrics.clone(),
            ));

        (execution_scheduler, rx_ready_transactions)
    }

    fn make_transaction(gas_object: Object, input: Vec<CallArg>) -> VerifiedExecutableTransaction {
        // Use fake module, function, package and gas prices since they are irrelevant
        // for testing execution scheduler.
        let rgp = 100;
        let (sender, keypair) = deterministic_random_account_key();
        let transaction = TestTransactionBuilder::new(sender, gas_object.object_ref(), rgp)
            .move_call(IOTA_FRAMEWORK_PACKAGE_ID, "counter", "assert_value", input)
            .build_and_sign(&keypair);
        VerifiedExecutableTransaction::new_system(
            VerifiedTransaction::new_unchecked(transaction),
            0,
        )
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn execution_scheduler_basics() {
        // Initialize an authority state.
        let (owner, _keypair) = deterministic_random_account_key();
        let gas_objects: Vec<Object> = (0..10)
            .map(|_| {
                let gas_object_id = ObjectId::random();
                Object::with_id_owner_for_testing(gas_object_id, owner)
            })
            .collect();
        let state = init_state_with_objects(gas_objects.clone()).await;

        // Create a new execution scheduler instead of reusing the authority's, to
        // examine execution_scheduler output from rx_ready_transactions.
        let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);
        // scheduler should output no transaction.
        assert!(
            rx_ready_transactions
                .try_recv()
                .is_err_and(|err| err == TryRecvError::Empty)
        );
        // scheduler should be empty at the beginning.
        assert_eq!(execution_scheduler.num_pending_certificates(), 0);

        // Enqueue empty vec should not crash.
        execution_scheduler.enqueue(vec![], &state.epoch_store_for_testing());
        // scheduler should output no transaction.
        assert!(
            rx_ready_transactions
                .try_recv()
                .is_err_and(|err| err == TryRecvError::Empty)
        );

        // Enqueue a transaction with existing gas object, empty input.
        let transaction = make_transaction(gas_objects[0].clone(), vec![]);
        let tx_start_time = Instant::now();
        execution_scheduler.enqueue(vec![transaction.clone()], &state.epoch_store_for_testing());
        // scheduler should output the transaction eventually.
        let pending_certificate = rx_ready_transactions.recv().await.unwrap();

        // Tests that pending certificate stats are recorded properly.
        assert!(pending_certificate.stats.enqueue_time >= tx_start_time);
        assert!(
            pending_certificate.stats.ready_time.unwrap() >= pending_certificate.stats.enqueue_time
        );

        assert_eq!(execution_scheduler.num_pending_certificates(), 1);

        // Predent we have just executed the transaction.
        drop(pending_certificate);

        // scheduler should be empty.
        execution_scheduler.check_empty_for_testing();

        // Enqueue a transaction with a new gas object, empty input.
        let gas_object_new = Object::with_id_owner_version_for_testing(
            ObjectId::random(),
            0.into(),
            Owner::Address(owner),
        );
        let transaction = make_transaction(gas_object_new.clone(), vec![]);
        let tx_start_time = Instant::now();
        execution_scheduler.enqueue(vec![transaction.clone()], &state.epoch_store_for_testing());
        // scheduler should output no transaction yet.
        sleep(Duration::from_secs(1)).await;
        assert!(
            rx_ready_transactions
                .try_recv()
                .is_err_and(|err| err == TryRecvError::Empty)
        );

        assert_eq!(execution_scheduler.num_pending_certificates(), 1);

        // Duplicated enqueue is allowed.
        execution_scheduler.enqueue(vec![transaction.clone()], &state.epoch_store_for_testing());
        sleep(Duration::from_secs(1)).await;
        assert!(
            rx_ready_transactions
                .try_recv()
                .is_err_and(|err| err == TryRecvError::Empty)
        );

        assert_eq!(execution_scheduler.num_pending_certificates(), 2);

        // Notify scheduler about availability of the gas object.
        state
            .get_cache_writer()
            .write_object_entry_for_test(gas_object_new);
        // scheduler should output the transaction eventually.
        // We will see both the original and the duplicated transaction.
        let pending_certificate = rx_ready_transactions.recv().await.unwrap();
        let pending_certificate2 = rx_ready_transactions.recv().await.unwrap();
        assert_eq!(
            pending_certificate.transaction.digest(),
            pending_certificate2.transaction.digest()
        );

        // Tests that pending certificate stats are recorded properly. The ready time
        // should be 2 seconds apart from the enqueue time.
        assert!(pending_certificate.stats.enqueue_time >= tx_start_time);
        assert!(
            pending_certificate.stats.ready_time.unwrap() - pending_certificate.stats.enqueue_time
                >= Duration::from_secs(2)
        );

        // Predent we have just executed the transaction.
        drop(pending_certificate);
        drop(pending_certificate2);

        // scheduler should be empty at the end.
        execution_scheduler.check_empty_for_testing();
    }

    fn shared_start_version(obj: &Object) -> SequenceNumber {
        match obj.owner() {
            Owner::Shared(v) => *v,
            _ => panic!("expected a shared object"),
        }
    }

    // Tests when objects become available, correct set of transactions can be sent
    // to execute. Specifically, we have following setup,
    //         shared_object     shared_object_2
    //       /    |    \     \    /
    //    tx_0  tx_1  tx_2    tx_3
    //     r      r     w      r
    // And when shared_object is available, tx_0, tx_1, and tx_2 can be executed.
    // And when shared_object_2 becomes available, tx_3 can be executed.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn execution_scheduler_object_dependency() {
        telemetry_subscribers::init_for_testing();
        // Initialize an authority state, with gas objects and a shared object.
        let (owner, _keypair) = deterministic_random_account_key();
        let gas_objects: Vec<Object> = (0..10)
            .map(|_| {
                let gas_object_id = ObjectId::random();
                Object::with_id_owner_for_testing(gas_object_id, owner)
            })
            .collect();
        let shared_object = Object::shared_for_testing();
        let initial_shared_version = shared_start_version(&shared_object);
        let shared_object_2 = Object::shared_for_testing();
        let initial_shared_version_2 = shared_start_version(&shared_object_2);

        let state = init_state_with_objects(
            [
                gas_objects.clone(),
                vec![shared_object.clone(), shared_object_2.clone()],
            ]
            .concat(),
        )
        .await;

        // Create a new execution scheduler instead of reusing the authority's, to
        // examine execution_scheduler output from rx_ready_transactions.
        let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);
        // scheduler should output no transaction.
        assert!(rx_ready_transactions.try_recv().is_err());

        // Enqueue two transactions with the same shared object input in read-only mode.
        let shared_version = 1000.into();
        let shared_object_arg_read =
            SharedObjectRef::new(shared_object.id(), initial_shared_version, false);
        let transaction_read_0 = make_transaction(
            gas_objects[0].clone(),
            vec![CallArg::Shared(shared_object_arg_read)],
        );
        let transaction_read_1 = make_transaction(
            gas_objects[1].clone(),
            vec![CallArg::Shared(shared_object_arg_read)],
        );
        state
            .epoch_store_for_testing()
            .set_shared_object_versions_for_testing(
                transaction_read_0.digest(),
                &[VersionAssignment::new(shared_object.id(), shared_version)],
            )
            .unwrap();
        state
            .epoch_store_for_testing()
            .set_shared_object_versions_for_testing(
                transaction_read_1.digest(),
                &[VersionAssignment::new(shared_object.id(), shared_version)],
            )
            .unwrap();

        // Enqueue one transaction with the same shared object in mutable mode.
        let shared_object_arg_default =
            SharedObjectRef::new(shared_object.id(), initial_shared_version, true);
        let transaction_default = make_transaction(
            gas_objects[2].clone(),
            vec![CallArg::Shared(shared_object_arg_default)],
        );
        state
            .epoch_store_for_testing()
            .set_shared_object_versions_for_testing(
                transaction_default.digest(),
                &[VersionAssignment::new(shared_object.id(), shared_version)],
            )
            .unwrap();

        // Enqueue one transaction with two readonly shared object inputs,
        // `shared_object` and `shared_object_2`.
        let shared_version_2 = 1000.into();
        let shared_object_arg_read_2 =
            SharedObjectRef::new(shared_object_2.id(), initial_shared_version_2, false);
        let transaction_read_2 = make_transaction(
            gas_objects[3].clone(),
            vec![
                CallArg::Shared(shared_object_arg_default),
                CallArg::Shared(shared_object_arg_read_2),
            ],
        );
        state
            .epoch_store_for_testing()
            .set_shared_object_versions_for_testing(
                transaction_read_2.digest(),
                &[
                    VersionAssignment::new(shared_object.id(), shared_version),
                    VersionAssignment::new(shared_object_2.id(), shared_version_2),
                ],
            )
            .unwrap();

        execution_scheduler.enqueue(
            vec![
                transaction_read_0.clone(),
                transaction_read_1.clone(),
                transaction_default.clone(),
                transaction_read_2.clone(),
            ],
            &state.epoch_store_for_testing(),
        );

        // scheduler should output no transaction yet.
        sleep(Duration::from_secs(1)).await;
        assert!(rx_ready_transactions.try_recv().is_err());

        assert_eq!(execution_scheduler.num_pending_certificates(), 4);

        // Notify scheduler about availability of the first shared object.
        let new_shared_object = Object::with_id_owner_version_for_testing(
            shared_object.id(),
            shared_version_2,
            *shared_object.owner(),
        );
        state
            .get_cache_writer()
            .write_object_entry_for_test(new_shared_object);

        // scheduler should output the 3 transactions that are only waiting for this
        // object.
        let tx_0 = rx_ready_transactions.recv().await.unwrap().transaction;
        let tx_1 = rx_ready_transactions.recv().await.unwrap().transaction;
        let tx_2 = rx_ready_transactions.recv().await.unwrap().transaction;
        {
            let mut want_digests = vec![
                transaction_read_0.digest(),
                transaction_read_1.digest(),
                transaction_default.digest(),
            ];
            want_digests.sort();
            let mut got_digests = vec![tx_0.digest(), tx_1.digest(), tx_2.digest()];
            got_digests.sort();
            assert_eq!(want_digests, got_digests);
        }

        sleep(Duration::from_secs(1)).await;
        assert!(rx_ready_transactions.try_recv().is_err());

        assert_eq!(execution_scheduler.num_pending_certificates(), 1);

        let new_shared_object_2 = Object::with_id_owner_version_for_testing(
            shared_object_2.id(),
            shared_version_2,
            *shared_object.owner(),
        );
        state
            .get_cache_writer()
            .write_object_entry_for_test(new_shared_object_2);

        // Now, the transaction waiting for both shared objects can be executed.
        let tx_3 = rx_ready_transactions.recv().await.unwrap().transaction;
        assert_eq!(transaction_read_2.digest(), tx_3.digest());

        sleep(Duration::from_secs(1)).await;
        assert!(rx_ready_transactions.try_recv().is_err());

        execution_scheduler.check_empty_for_testing();
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn execution_scheduler_receiving_notify_commit() {
        telemetry_subscribers::init_for_testing();
        // Initialize an authority state.
        let (owner, _keypair) = deterministic_random_account_key();
        let gas_objects: Vec<Object> = (0..10)
            .map(|_| {
                let gas_object_id = ObjectId::random();
                Object::with_id_owner_for_testing(gas_object_id, owner)
            })
            .collect();
        let state = init_state_with_objects(gas_objects.clone()).await;

        // Create a new execution scheduler instead of reusing the authority's, to
        // examine execution_scheduler output from rx_ready_transactions.
        let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);
        // scheduler should output no transaction.
        assert!(rx_ready_transactions.try_recv().is_err());
        // scheduler should be empty at the beginning.
        execution_scheduler.check_empty_for_testing();

        let obj_id = ObjectId::random();
        let object_arguments: Vec<_> = (0..10)
            .map(|i| {
                let object = Object::with_id_owner_version_for_testing(
                    obj_id,
                    i.into(),
                    Owner::Address(owner),
                );
                // Every other transaction receives the object, and we create a run of multiple
                // receives in a row at the beginning to test that the scheduler
                // doesn't get stuck in either configuration of:
                // ImmOrOwnedObject => Receiving,
                // Receiving => Receiving
                // Receiving => ImmOrOwnedObject
                // ImmOrOwnedObject => ImmOrOwnedObject is already tested as the default case on
                // mainnet.
                let object_arg = if i % 2 == 0 || i == 3 {
                    CallArg::Receiving(object.object_ref())
                } else {
                    CallArg::ImmutableOrOwned(object.object_ref())
                };
                let txn = make_transaction(gas_objects[0].clone(), vec![object_arg]);
                (object, txn)
            })
            .collect();

        for (i, (_, txn)) in object_arguments.iter().enumerate() {
            // scheduler should output no transaction yet since waiting on receiving object
            // or ImmOrOwnedObject input.
            execution_scheduler.enqueue(vec![txn.clone()], &state.epoch_store_for_testing());
            sleep(Duration::from_secs(1)).await;
            assert!(rx_ready_transactions.try_recv().is_err());
            assert_eq!(execution_scheduler.num_pending_certificates(), i + 1);
        }

        // Now start to unravel the transactions by notifying that each subsequent
        // transaction has been processed.
        let len = object_arguments.len();
        for (i, (object, txn)) in object_arguments.into_iter().enumerate() {
            // Mark the object as available.
            // We should now eventually see the transaction as ready.
            state
                .get_cache_writer()
                .write_object_entry_for_test(object.clone());

            // scheduler should output the transaction eventually now that the receiving
            // object has become available.
            rx_ready_transactions.recv().await.unwrap();

            // Only one transaction at a time should become available though. So if we try
            // to get another one it should fail.
            sleep(Duration::from_secs(1)).await;
            assert!(rx_ready_transactions.try_recv().is_err());

            // Notify the scheduler that the transaction has been processed.
            drop(txn);

            // scheduler should now output another transaction to run since it the next
            // version of that object has become available.
            assert_eq!(execution_scheduler.num_pending_certificates(), len - i - 1);
        }

        // After everything scheduler should be empty.
        execution_scheduler.check_empty_for_testing();
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn execution_scheduler_receiving_object_ready_notifications() {
        telemetry_subscribers::init_for_testing();
        // Initialize an authority state.
        let (owner, _keypair) = deterministic_random_account_key();
        let gas_objects: Vec<Object> = (0..10)
            .map(|_| {
                let gas_object_id = ObjectId::random();
                Object::with_id_owner_for_testing(gas_object_id, owner)
            })
            .collect();
        let state = init_state_with_objects(gas_objects.clone()).await;

        // Create a new execution scheduler instead of reusing the authority's, to
        // examine execution_scheduler output from rx_ready_transactions.
        let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);
        // scheduler should output no transaction.
        assert!(rx_ready_transactions.try_recv().is_err());
        // scheduler should be empty at the beginning.
        execution_scheduler.check_empty_for_testing();

        let obj_id = ObjectId::random();
        let receiving_object_new0 =
            Object::with_id_owner_version_for_testing(obj_id, 0.into(), Owner::Address(owner));
        let receiving_object_new1 =
            Object::with_id_owner_version_for_testing(obj_id, 1.into(), Owner::Address(owner));
        let receiving_object_arg0 = CallArg::Receiving(receiving_object_new0.object_ref());
        let receive_object_transaction0 =
            make_transaction(gas_objects[0].clone(), vec![receiving_object_arg0]);

        let receiving_object_arg1 = CallArg::Receiving(receiving_object_new1.object_ref());
        let receive_object_transaction1 =
            make_transaction(gas_objects[0].clone(), vec![receiving_object_arg1]);

        // scheduler should output no transaction yet since waiting on receiving object.
        execution_scheduler.enqueue(
            vec![receive_object_transaction0.clone()],
            &state.epoch_store_for_testing(),
        );
        sleep(Duration::from_secs(1)).await;
        assert!(rx_ready_transactions.try_recv().is_err());
        assert_eq!(execution_scheduler.num_pending_certificates(), 1);

        // scheduler should output no transaction yet since waiting on receiving object.
        execution_scheduler.enqueue(
            vec![receive_object_transaction1.clone()],
            &state.epoch_store_for_testing(),
        );
        sleep(Duration::from_secs(1)).await;
        assert!(rx_ready_transactions.try_recv().is_err());
        assert_eq!(execution_scheduler.num_pending_certificates(), 2);

        // Duplicate enqueue of receiving object is allowed.
        execution_scheduler.enqueue(
            vec![receive_object_transaction0.clone()],
            &state.epoch_store_for_testing(),
        );
        sleep(Duration::from_secs(1)).await;
        assert!(rx_ready_transactions.try_recv().is_err());
        assert_eq!(execution_scheduler.num_pending_certificates(), 3);

        // Notify scheduler that the receiving object 0 is available.
        state
            .get_cache_writer()
            .write_object_entry_for_test(receiving_object_new0.clone());

        // scheduler should output the transaction eventually now that the receiving
        // object has become available.
        rx_ready_transactions.recv().await.unwrap();

        // Notify scheduler that the receiving object 0 is available.
        state
            .get_cache_writer()
            .write_object_entry_for_test(receiving_object_new1.clone());

        // scheduler should output the transaction eventually now that the receiving
        // object has become available.
        rx_ready_transactions.recv().await.unwrap();
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn execution_scheduler_receiving_object_ready_notifications_multiple_of_same_receiving() {
        telemetry_subscribers::init_for_testing();
        // Initialize an authority state.
        let (owner, _keypair) = deterministic_random_account_key();
        let gas_objects: Vec<Object> = (0..10)
            .map(|_| {
                let gas_object_id = ObjectId::random();
                Object::with_id_owner_for_testing(gas_object_id, owner)
            })
            .collect();
        let state = init_state_with_objects(gas_objects.clone()).await;

        // Create a new execution scheduler instead of reusing the authority's, to
        // examine execution_scheduler output from rx_ready_transactions.
        let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);
        // scheduler should output no transaction.
        assert!(rx_ready_transactions.try_recv().is_err());
        // scheduler should be empty at the beginning.
        execution_scheduler.check_empty_for_testing();

        let obj_id = ObjectId::random();
        let receiving_object_new0 =
            Object::with_id_owner_version_for_testing(obj_id, 0.into(), Owner::Address(owner));
        let receiving_object_new1 =
            Object::with_id_owner_version_for_testing(obj_id, 1.into(), Owner::Address(owner));
        let receiving_object_arg0 = receiving_object_new0.object_ref();
        let receive_object_transaction0 = make_transaction(
            gas_objects[0].clone(),
            vec![CallArg::Receiving(receiving_object_arg0)],
        );

        let receive_object_transaction01 = make_transaction(
            gas_objects[1].clone(),
            vec![CallArg::Receiving(receiving_object_arg0)],
        );

        let receiving_object_arg1 = CallArg::Receiving(receiving_object_new1.object_ref());
        let receive_object_transaction1 =
            make_transaction(gas_objects[0].clone(), vec![receiving_object_arg1]);

        // Enqueuing a transaction with a receiving object that is available at the time
        // it is enqueued should become immediately available.
        let gas_receiving_arg = CallArg::Receiving(gas_objects[3].object_ref());
        let tx1 = make_transaction(gas_objects[0].clone(), vec![gas_receiving_arg]);

        // scheduler should output no transaction yet since waiting on receiving object.
        execution_scheduler.enqueue(
            vec![receive_object_transaction0.clone()],
            &state.epoch_store_for_testing(),
        );
        sleep(Duration::from_secs(1)).await;
        assert!(rx_ready_transactions.try_recv().is_err());
        assert_eq!(execution_scheduler.num_pending_certificates(), 1);

        // scheduler should output no transaction yet since waiting on receiving object.
        execution_scheduler.enqueue(
            vec![receive_object_transaction1.clone()],
            &state.epoch_store_for_testing(),
        );
        sleep(Duration::from_secs(1)).await;
        assert!(rx_ready_transactions.try_recv().is_err());
        assert_eq!(execution_scheduler.num_pending_certificates(), 2);

        // Different transaction with a duplicate receiving object reference is allowed.
        // Both transaction's will be outputted once the receiving object is available.
        execution_scheduler.enqueue(
            vec![receive_object_transaction01.clone()],
            &state.epoch_store_for_testing(),
        );
        sleep(Duration::from_secs(1)).await;
        assert!(rx_ready_transactions.try_recv().is_err());
        assert_eq!(execution_scheduler.num_pending_certificates(), 3);

        // Notify scheduler that the receiving object 0 is available.
        state
            .get_cache_writer()
            .write_object_entry_for_test(receiving_object_new0.clone());

        // scheduler should output both transactions depending on the receiving object
        // now that the transaction's receiving object has become available.
        rx_ready_transactions.recv().await.unwrap();

        rx_ready_transactions.recv().await.unwrap();

        // Only two transactions that were dependent on the receiving object should be
        // output.
        assert!(rx_ready_transactions.try_recv().is_err());

        // Enqueue a transaction with a receiving object that is available at the time
        // it is enqueued. This should be immediately available.
        execution_scheduler.enqueue(vec![tx1.clone()], &state.epoch_store_for_testing());
        sleep(Duration::from_secs(1)).await;
        rx_ready_transactions.recv().await.unwrap();

        // Notify scheduler that the receiving object 0 is available.
        state
            .get_cache_writer()
            .write_object_entry_for_test(receiving_object_new1.clone());

        // scheduler should output the transaction eventually now that the receiving
        // object has become available.
        rx_ready_transactions.recv().await.unwrap();
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn execution_scheduler_receiving_object_ready_if_current_version_greater() {
        telemetry_subscribers::init_for_testing();
        // Initialize an authority state.
        let (owner, _keypair) = deterministic_random_account_key();
        let mut gas_objects: Vec<Object> = (0..10)
            .map(|_| {
                let gas_object_id = ObjectId::random();
                Object::with_id_owner_for_testing(gas_object_id, owner)
            })
            .collect();
        let receiving_object = Object::with_id_owner_version_for_testing(
            ObjectId::random(),
            10.into(),
            Owner::Address(owner),
        );
        gas_objects.push(receiving_object.clone());
        let state = init_state_with_objects(gas_objects.clone()).await;

        // Create a new execution scheduler instead of reusing the authority's, to
        // examine execution_scheduler output from rx_ready_transactions.
        let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);
        // scheduler should output no transaction.
        assert!(rx_ready_transactions.try_recv().is_err());
        // scheduler should be empty at the beginning.
        execution_scheduler.check_empty_for_testing();

        let receiving_object_new0 = Object::with_id_owner_version_for_testing(
            receiving_object.id(),
            0.into(),
            Owner::Address(owner),
        );
        let receiving_object_new1 = Object::with_id_owner_version_for_testing(
            receiving_object.id(),
            1.into(),
            Owner::Address(owner),
        );
        let receiving_object_arg0 = receiving_object_new0.object_ref();
        let receive_object_transaction0 = make_transaction(
            gas_objects[0].clone(),
            vec![CallArg::Receiving(receiving_object_arg0)],
        );

        let receive_object_transaction01 = make_transaction(
            gas_objects[1].clone(),
            vec![CallArg::Receiving(receiving_object_arg0)],
        );

        let receiving_object_arg1 = receiving_object_new1.object_ref();
        let receive_object_transaction1 = make_transaction(
            gas_objects[0].clone(),
            vec![CallArg::Receiving(receiving_object_arg1)],
        );

        // scheduler should output no transaction yet since waiting on receiving object.
        execution_scheduler.enqueue(
            vec![receive_object_transaction0.clone()],
            &state.epoch_store_for_testing(),
        );
        execution_scheduler.enqueue(
            vec![receive_object_transaction01.clone()],
            &state.epoch_store_for_testing(),
        );
        execution_scheduler.enqueue(
            vec![receive_object_transaction1.clone()],
            &state.epoch_store_for_testing(),
        );
        sleep(Duration::from_secs(1)).await;
        rx_ready_transactions.recv().await.unwrap();
        rx_ready_transactions.recv().await.unwrap();
        rx_ready_transactions.recv().await.unwrap();
        assert!(rx_ready_transactions.try_recv().is_err());
    }

    // Tests transaction cancellation logic in execution scheduler. Mainly tests
    // that for cancelled transaction, execution scheduler only waits for all
    // non-shared objects to be available before outputting the transaction.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn execution_scheduler_with_cancelled_transactions() {
        // Initialize an authority state, with gas objects and 3 shared objects.
        let (owner, _keypair) = deterministic_random_account_key();
        let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
        let shared_object_1 = Object::shared_for_testing();
        let initial_shared_version_1 = shared_start_version(&shared_object_1);
        let shared_object_2 = Object::shared_for_testing();
        let initial_shared_version_2 = shared_start_version(&shared_object_2);
        let owned_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);

        let state = init_state_with_objects(vec![
            gas_object.clone(),
            shared_object_1.clone(),
            shared_object_2.clone(),
            owned_object.clone(),
        ])
        .await;

        // Create a new execution scheduler instead of reusing the authority's, to
        // examine execution_scheduler output from rx_ready_transactions.
        let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);
        // scheduler should output no transaction.
        assert!(rx_ready_transactions.try_recv().is_err());

        // Enqueue one transaction with 2 shared object inputs and 1 owned input.
        let shared_object_arg_1 =
            SharedObjectRef::new(shared_object_1.id(), initial_shared_version_1, true);
        let shared_object_arg_2 =
            SharedObjectRef::new(shared_object_2.id(), initial_shared_version_2, true);

        // Changes the desired owned object version to a higher version. We will make it
        // available later.
        let owned_version = 2000.into();
        let mut owned_ref = owned_object.object_ref();
        owned_ref.version = owned_version;

        let cancelled_transaction = make_transaction(
            gas_object.clone(),
            vec![
                CallArg::Shared(shared_object_arg_1),
                CallArg::Shared(shared_object_arg_2),
                CallArg::ImmutableOrOwned(owned_ref),
            ],
        );
        state
            .epoch_store_for_testing()
            .set_shared_object_versions_for_testing(
                cancelled_transaction.digest(),
                &[
                    VersionAssignment::new(shared_object_1.id(), SequenceNumber::CANCELLED_READ),
                    VersionAssignment::new(
                        shared_object_2.id(),
                        SequenceNumber::CONGESTED_PRIOR_TO_GAS_PRICE_FEEDBACK,
                    ),
                ],
            )
            .unwrap();

        execution_scheduler.enqueue(
            vec![cancelled_transaction.clone()],
            &state.epoch_store_for_testing(),
        );

        // scheduler should output no transaction yet.
        sleep(Duration::from_secs(1)).await;
        assert!(rx_ready_transactions.try_recv().is_err());

        assert_eq!(execution_scheduler.num_pending_certificates(), 1);

        // Notify scheduler about availability of the owned object.
        let new_owned_object = Object::with_id_owner_version_for_testing(
            owned_object.id(),
            owned_version,
            *owned_object.owner(),
        );
        state
            .get_cache_writer()
            .write_object_entry_for_test(new_owned_object);

        // scheduler should output the transaction as soon as the owned object is
        // available.
        let available_txn = rx_ready_transactions.recv().await.unwrap().transaction;
        assert_eq!(available_txn.digest(), cancelled_transaction.digest());

        sleep(Duration::from_secs(1)).await;
        assert!(rx_ready_transactions.try_recv().is_err());

        execution_scheduler.check_empty_for_testing();
    }
}
