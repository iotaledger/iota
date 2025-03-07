// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use consensus_core::{BlockRef, BlockStatus};
use std::collections::HashSet;

use fastcrypto::traits::KeyPair;
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    base_types::{ExecutionDigests, ObjectID},
    crypto::deterministic_random_account_key,
    gas::GasCostSummary,
    messages_checkpoint::{
        CheckpointContents, CheckpointSignatureMessage, CheckpointSummary, SignedCheckpointSummary,
    },
    object::Object,
    transaction::{
        CallArg, CertifiedTransaction, ObjectArg, TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
        TransactionData,
    },
    utils::{make_committee_key, to_sender_signed_transaction},
};
use move_core_types::{account_address::AccountAddress, ident_str};
use parking_lot::Mutex;
use rand::{SeedableRng, rngs::StdRng};

use super::*;
use crate::{
    authority::{AuthorityState, authority_tests::init_state_with_objects},
    checkpoints::CheckpointServiceNoop,
    consensus_handler::SequencedConsensusTransaction,
    mock_consensus::with_block_status,
};

/// Fixture: a few test gas objects.
pub fn test_gas_objects() -> Vec<Object> {
    thread_local! {
        static GAS_OBJECTS: Vec<Object> = (0..4)
            .map(|_| {
                let gas_object_id = ObjectID::random();
                let (owner, _) = deterministic_random_account_key();
                Object::with_id_owner_for_testing(gas_object_id, owner)
            })
            .collect();
    }

    GAS_OBJECTS.with(|v| v.clone())
}

/// Fixture: a few test certificates containing a shared object.
pub async fn test_certificates(
    authority: &AuthorityState,
    shared_object: Object,
) -> Vec<CertifiedTransaction> {
    let epoch_store = authority.load_epoch_store_one_call_per_task();
    let (sender, keypair) = deterministic_random_account_key();
    let rgp = epoch_store.reference_gas_price();

    let mut certificates = Vec::new();
    let shared_object_arg = ObjectArg::SharedObject {
        id: shared_object.id(),
        initial_shared_version: shared_object.version(),
        mutable: true,
    };
    for gas_object in test_gas_objects() {
        // Object digest may be different in genesis than originally generated.
        let gas_object = authority
            .get_object(&gas_object.id())
            .await
            .unwrap()
            .unwrap();
        // Make a sample transaction.
        let module = "object_basics";
        let function = "create";

        let data = TransactionData::new_move_call(
            sender,
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!(module).to_owned(),
            ident_str!(function).to_owned(),
            // type_args
            vec![],
            gas_object.compute_object_reference(),
            // args
            vec![
                CallArg::Object(shared_object_arg),
                CallArg::Pure(16u64.to_le_bytes().to_vec()),
                CallArg::Pure(bcs::to_bytes(&AccountAddress::from(sender)).unwrap()),
            ],
            rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
            rgp,
        )
        .unwrap();

        let transaction = epoch_store
            .verify_transaction(to_sender_signed_transaction(data, &keypair))
            .unwrap();

        // Submit the transaction and assemble a certificate.
        let response = authority
            .handle_transaction(&epoch_store, transaction.clone())
            .await
            .unwrap();
        let vote = response.status.into_signed_for_testing();
        let certificate = CertifiedTransaction::new(
            transaction.into_message(),
            vec![vote.clone()],
            &authority.clone_committee_for_testing(),
        )
        .unwrap();
        certificates.push(certificate);
    }
    certificates
}

pub fn make_consensus_adapter_for_test(
    state: Arc<AuthorityState>,
    process_via_checkpoint: HashSet<TransactionDigest>,
    execute: bool,
    mock_block_status_receivers: Vec<BlockStatusReceiver>,
) -> Arc<ConsensusAdapter> {
    let metrics = ConsensusAdapterMetrics::new_test();

    #[derive(Clone)]
    struct SubmitDirectly {
        state: Arc<AuthorityState>,
        process_via_checkpoint: HashSet<TransactionDigest>,
        execute: bool,
        mock_block_status_receivers: Arc<Mutex<Vec<BlockStatusReceiver>>>,
    }

    #[async_trait::async_trait]
    impl ConsensusClient for SubmitDirectly {
        async fn submit(
            &self,
            transactions: &[ConsensusTransaction],
            epoch_store: &Arc<AuthorityPerEpochStore>,
        ) -> IotaResult<BlockStatusReceiver> {
            let sequenced_transactions = transactions
                .iter()
                .map(|txn| SequencedConsensusTransaction::new_test(txn.clone()))
                .collect();

            let checkpoint_service = Arc::new(CheckpointServiceNoop {});
            let mut transactions = Vec::new();
            let mut executed_via_checkpoint = 0;

            for tx in sequenced_transactions {
                if let Some(transaction_digest) = tx.transaction.executable_transaction_digest() {
                    if self.process_via_checkpoint.contains(&transaction_digest) {
                        epoch_store
                            .insert_finalized_transactions(vec![transaction_digest].as_slice(), 10)
                            .expect("Should not fail");
                        executed_via_checkpoint += 1;
                    } else {
                        transactions.extend(
                            epoch_store
                                .process_consensus_transactions_for_tests(
                                    vec![tx],
                                    &checkpoint_service,
                                    self.state.get_object_cache_reader().as_ref(),
                                    &self.state.metrics,
                                    true,
                                )
                                .await?,
                        );
                    }
                } else if let SequencedConsensusTransactionKey::External(
                    ConsensusTransactionKey::CheckpointSignature(_, checkpoint_sequence_number),
                ) = tx.transaction.key()
                {
                    epoch_store.notify_synced_checkpoint(checkpoint_sequence_number);
                } else {
                    transactions.extend(
                        epoch_store
                            .process_consensus_transactions_for_tests(
                                vec![tx],
                                &checkpoint_service,
                                self.state.get_object_cache_reader().as_ref(),
                                &self.state.metrics,
                                true,
                            )
                            .await?,
                    );
                }
            }

            assert_eq!(
                executed_via_checkpoint,
                self.process_via_checkpoint.len(),
                "Some transactions were not executed via checkpoint"
            );

            if self.execute {
                self.state
                    .transaction_manager()
                    .enqueue(transactions, epoch_store);
            }

            assert!(
                !self.mock_block_status_receivers.lock().is_empty(),
                "No mock submit responses left"
            );
            Ok(self.mock_block_status_receivers.lock().remove(0))
        }
    }
    // Make a new consensus adapter instance.
    Arc::new(ConsensusAdapter::new(
        Arc::new(SubmitDirectly {
            state: state.clone(),
            process_via_checkpoint,
            execute,
            mock_block_status_receivers: Arc::new(Mutex::new(mock_block_status_receivers)),
        }),
        state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        metrics,
    ))
}

#[tokio::test]
async fn submit_transaction_to_consensus_adapter() {
    telemetry_subscribers::init_for_testing();

    // Initialize an authority with a (owned) gas object and a shared object; then
    // make a test certificate.
    let mut objects = test_gas_objects();
    let shared_object = Object::shared_for_testing();
    objects.push(shared_object.clone());
    let state = init_state_with_objects(objects).await;
    let certificate = test_certificates(&state, shared_object)
        .await
        .pop()
        .unwrap();
    let epoch_store = state.epoch_store_for_testing();

    // Make a new consensus adapter instance.
    let block_status_receivers = vec![
        with_block_status(BlockStatus::GarbageCollected(BlockRef::MIN)),
        with_block_status(BlockStatus::GarbageCollected(BlockRef::MIN)),
        with_block_status(BlockStatus::GarbageCollected(BlockRef::MIN)),
        with_block_status(BlockStatus::Sequenced(BlockRef::MIN)),
    ];
    let adapter = make_consensus_adapter_for_test(state.clone(), HashSet::new(), false, block_status_receivers);

    // Submit the transaction and ensure the adapter reports success to the caller.
    // Note that consensus may drop some transactions (so we may need to
    // resubmit them).
    let transaction = ConsensusTransaction::new_certificate_message(&state.name, certificate);
    let waiter = adapter
        .submit(
            transaction.clone(),
            Some(&epoch_store.get_reconfig_state_read_lock_guard()),
            &epoch_store,
        )
        .unwrap();
    waiter.await.unwrap();
}

#[tokio::test]
async fn submit_multiple_transactions_to_consensus_adapter() {
    telemetry_subscribers::init_for_testing();

    // Initialize an authority with a (owned) gas object and a shared object; then
    // make a test certificate.
    let mut objects = test_gas_objects();
    let shared_object = Object::shared_for_testing();
    objects.push(shared_object.clone());
    let state = init_state_with_objects(objects).await;
    let certificates = test_certificates(&state, shared_object).await;
    let epoch_store = state.epoch_store_for_testing();

    // Mark the first two transactions to be "executed via checkpoint" and the other
    // two to appear via consensus output.
    assert_eq!(certificates.len(), 4);

    let mut process_via_checkpoint = HashSet::new();
    process_via_checkpoint.insert(*certificates[0].digest());
    process_via_checkpoint.insert(*certificates[1].digest());

    // Make a new consensus adapter instance.
    let adapter = make_consensus_adapter_for_test(state.clone(), process_via_checkpoint, false);

    // Submit the transaction and ensure the adapter reports success to the caller.
    // Note that consensus may drop some transactions (so we may need to
    // resubmit them).
    let transactions = certificates
        .into_iter()
        .map(|certificate| ConsensusTransaction::new_certificate_message(&state.name, certificate))
        .collect::<Vec<_>>();

    let waiter = adapter
        .submit_batch(
            &transactions,
            Some(&epoch_store.get_reconfig_state_read_lock_guard()),
            &epoch_store,
        )
        .unwrap();
    waiter.await.unwrap();
}

#[tokio::test]
async fn submit_checkpoint_signature_to_consensus_adapter() {
    telemetry_subscribers::init_for_testing();

    let mut rng = StdRng::seed_from_u64(1_100);
    let (keys, committee) = make_committee_key(&mut rng);

    // Initialize an authority
    let state = init_state_with_objects(vec![]).await;
    let epoch_store = state.epoch_store_for_testing();

    // Make a new consensus adapter instance.
    let adapter = make_consensus_adapter_for_test(state, HashSet::new(), false);

    let checkpoint_summary = CheckpointSummary::new(
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        1,
        2,
        10,
        &CheckpointContents::new_with_digests_only_for_tests([ExecutionDigests::random()]),
        None,
        GasCostSummary::default(),
        None,
        100,
        Vec::new(),
    );

    let authority_key = &keys[0];
    let authority = authority_key.public().into();
    let signed_checkpoint_summary = SignedCheckpointSummary::new(
        committee.epoch,
        checkpoint_summary,
        authority_key,
        authority,
    );

    let transactions = vec![ConsensusTransaction::new_checkpoint_signature_message(
        CheckpointSignatureMessage {
            summary: signed_checkpoint_summary,
        },
    )];
    let waiter = adapter
        .submit_batch(
            &transactions,
            Some(&epoch_store.get_reconfig_state_read_lock_guard()),
            &epoch_store,
        )
        .unwrap();
    waiter.await.unwrap();
}
