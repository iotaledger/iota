// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Weak};

use iota_types::{
    error::{IotaError, IotaResult},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    transaction::VerifiedCertificate,
};
use prometheus::Registry;
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::debug;

use crate::{
    authority::{
        authority_per_epoch_store::AuthorityPerEpochStore, AuthorityMetrics, AuthorityState,
    },
    checkpoints::CheckpointServiceNoop,
    consensus_adapter::SubmitToConsensus,
    consensus_handler::SequencedConsensusTransaction,
};

pub struct MockConsensusClient {
    tx_sender: mpsc::Sender<ConsensusTransaction>,
    _consensus_handle: JoinHandle<()>,
}

pub enum ConsensusMode {
    // ConsensusClient does absolutely nothing when receiving a transaction
    Noop,
    // ConsensusClient directly sequences the transaction into the store.
    DirectSequencing,
}

impl MockConsensusClient {
    pub fn new(validator: Weak<AuthorityState>, consensus_mode: ConsensusMode) -> Self {
        let (tx_sender, tx_receiver) = mpsc::channel(1000000);
        let _consensus_handle = Self::run(validator, tx_receiver, consensus_mode);
        Self {
            tx_sender,
            _consensus_handle,
        }
    }

    pub fn run(
        validator: Weak<AuthorityState>,
        tx_receiver: mpsc::Receiver<ConsensusTransaction>,
        consensus_mode: ConsensusMode,
    ) -> JoinHandle<()> {
        tokio::spawn(async move { Self::run_impl(validator, tx_receiver, consensus_mode).await })
    }

    async fn run_impl(
        validator: Weak<AuthorityState>,
        mut tx_receiver: mpsc::Receiver<ConsensusTransaction>,
        consensus_mode: ConsensusMode,
    ) {
        let checkpoint_service = Arc::new(CheckpointServiceNoop {});
        let authority_metrics = Arc::new(AuthorityMetrics::new(&Registry::new()));
        while let Some(tx) = tx_receiver.recv().await {
            let Some(validator) = validator.upgrade() else {
                debug!("validator shut down; exiting MockConsensusClient");
                return;
            };
            let epoch_store = validator.epoch_store_for_testing();
            match consensus_mode {
                ConsensusMode::Noop => {}
                ConsensusMode::DirectSequencing => {
                    epoch_store
                        .process_consensus_transactions_for_tests(
                            vec![SequencedConsensusTransaction::new_test(tx.clone())],
                            &checkpoint_service,
                            validator.get_object_cache_reader().as_ref(),
                            &authority_metrics,
                            true,
                        )
                        .await
                        .unwrap();
                }
            }
            if let ConsensusTransactionKind::UserTransaction(tx) = tx.kind {
                if tx.contains_shared_object() {
                    validator.enqueue_certificates_for_execution(
                        vec![VerifiedCertificate::new_unchecked(*tx)],
                        &epoch_store,
                    );
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl SubmitToConsensus for MockConsensusClient {
    async fn submit_to_consensus(
        &self,
        transactions: &[ConsensusTransaction],
        _epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult {
        // TODO: maybe support multi-transactions and remove this check
        assert!(transactions.len() == 1);
        let transaction = &transactions[0];
        self.tx_sender
            .send(transaction.clone())
            .await
            .map_err(|e| IotaError::Unknown(e.to_string()))
    }
}
