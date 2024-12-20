// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    base_types::ObjectID,
    crypto::deterministic_random_account_key,
    object::Object,
    transaction::{
        CallArg, CertifiedTransaction, ObjectArg, TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
        TransactionData,
    },
    utils::to_sender_signed_transaction,
};
use move_core_types::{account_address::AccountAddress, ident_str};

use super::*;
use crate::{
    authority::{AuthorityState, authority_tests::init_state_with_objects},
    checkpoints::CheckpointServiceNoop,
    consensus_handler::SequencedConsensusTransaction,
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
    execute: bool,
) -> Arc<ConsensusAdapter> {
    let metrics = ConsensusAdapterMetrics::new_test();

    #[derive(Clone)]
    struct SubmitDirectly(Arc<AuthorityState>, bool);

    #[async_trait::async_trait]
    impl SubmitToConsensus for SubmitDirectly {
        async fn submit_to_consensus(
            &self,
            transactions: &[ConsensusTransaction],
            epoch_store: &Arc<AuthorityPerEpochStore>,
        ) -> IotaResult {
            let sequenced_transactions = transactions
                .iter()
                .map(|txn| SequencedConsensusTransaction::new_test(txn.clone()))
                .collect();
            let transactions = epoch_store
                .process_consensus_transactions_for_tests(
                    sequenced_transactions,
                    &Arc::new(CheckpointServiceNoop {}),
                    self.0.get_object_cache_reader().as_ref(),
                    &self.0.metrics,
                    true,
                )
                .await?;
            if self.1 {
                self.0
                    .transaction_manager()
                    .enqueue(transactions, epoch_store);
            }
            Ok(())
        }
    }
    // Make a new consensus adapter instance.
    Arc::new(ConsensusAdapter::new(
        Arc::new(SubmitDirectly(state.clone(), execute)),
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
    let adapter = make_consensus_adapter_for_test(state.clone(), false);

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
