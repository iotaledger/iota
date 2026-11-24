// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Attestation extensions for [`AbstractAccountTestEnv`]: producing
//! attestations, honest or fabricated, and submitting transactions through
//! consensus, attested or not. The environment itself is attestation-agnostic;
//! everything PCOOL-coupled lives here.

use std::{collections::HashMap, sync::Arc};

use iota_sdk_types::{ObjectReference, TransactionDigest, TransactionEffects, Version};
use iota_types::{
    attestation::{
        Attestation, AttestationData, AttestationRecord, AttestedTransaction, SharedInputIndex,
    },
    error::IotaResult,
    messages_consensus::ConsensusTransaction,
    transaction::TransactionEnvelope,
};

use crate::{
    authority::{
        ExecutionEnv, abstract_account_test_utils::AbstractAccountTestEnv,
        shared_object_version_manager::Schedulable,
    },
    checkpoints::CheckpointServiceNoop,
    consensus_handler::SequencedConsensusTransaction,
    execution_scheduler::transaction_manager::AttestationSource,
};

/// Consensus accepts a validator attestation only from the block author, which
/// `SequencedConsensusTransaction::new_test` fixes at index 0.
fn authority_index() -> u8 {
    0
}

impl AbstractAccountTestEnv {
    /// Runs the attestor's own dry-run, which is how a genuine attestation is
    /// produced. Only succeeds while authentication still passes.
    pub fn attest(&self, tx: &TransactionEnvelope) -> Attestation {
        let (payload, _) = self
            .try_attest(tx)
            .expect("attesting must succeed while authentication passes");
        Attestation::Authority {
            payload,
            authority_index: authority_index(),
        }
    }

    /// Runs the attestor's own dry run and returns its result, a refusal
    /// included.
    pub fn try_attest(
        &self,
        tx: &TransactionEnvelope,
    ) -> IotaResult<(AttestationData, Vec<ObjectReference>)> {
        let epoch_store = self.authority.epoch_store_for_testing();
        let verified = epoch_store.verify_transaction(tx.clone()).unwrap();

        self.authority.attest_transaction(
            &verified,
            &epoch_store,
            &self.authority.config.transaction_deny_config,
            |_| Ok(()),
        )
    }

    /// Returns an attestation vouching for an account transaction at
    /// `account_version` of the account, as a dishonest attestor would
    /// produce: its one entry is for the account of the Move authenticator at
    /// signature 0. The computation estimate is the largest one consensus
    /// accepts, so the attested-units cap does not cut the judge's re-run
    /// short.
    pub fn attest_with_account_version(&self, account_version: Version) -> Attestation {
        let computation_units = self.budget() / self.rgp();
        Attestation::Authority {
            payload: AttestationData::V1 {
                computation_units,
                shared_object_versions: vec![(
                    SharedInputIndex::AuthenticatorAccount { authenticator: 0 },
                    account_version,
                )],
            },
            authority_index: authority_index(),
        }
    }

    /// Submits the transaction to consensus, with an attestation when one is
    /// given, and executes what consensus schedules. Shared input versions are
    /// assigned here, so the transaction runs against the account's state at
    /// submission time rather than at the time it was built.
    pub async fn submit(
        &self,
        tx: TransactionEnvelope,
        attestation: Option<Attestation>,
    ) -> TransactionEffects {
        self.submit_with_source(tx, attestation, None).await
    }

    /// Like [`Self::submit()`], but executes with `source`, when given, in place
    /// of what consensus delivered, as a node executing from a checkpoint does
    /// with the certified record.
    pub async fn submit_with_source(
        &self,
        tx: TransactionEnvelope,
        attestation: Option<Attestation>,
        source: Option<AttestationSource>,
    ) -> TransactionEffects {
        self.submit_in_one_commit(vec![(tx, attestation, source)])
            .await
            .pop()
            .expect("the effects of the one submitted transaction")
    }

    /// Submits the transactions to consensus in one commit, in the given order,
    /// and executes what consensus schedules, in the same order. A
    /// transaction's [`AttestationSource`], when given, replaces what consensus
    /// delivered for it, as a node executing from a checkpoint does with the
    /// certified record.
    pub async fn submit_in_one_commit(
        &self,
        txs: Vec<(
            TransactionEnvelope,
            Option<Attestation>,
            Option<AttestationSource>,
        )>,
    ) -> Vec<TransactionEffects> {
        let order: Vec<TransactionDigest> = txs.iter().map(|(tx, ..)| *tx.digest()).collect();
        let mut sources: HashMap<TransactionDigest, AttestationSource> = HashMap::new();
        let consensus_txs = txs
            .into_iter()
            .map(|(tx, attestation, source)| {
                if let Some(source) = source {
                    sources.insert(*tx.digest(), source);
                }
                let consensus_tx = match attestation {
                    Some(attestation) => ConsensusTransaction::new_attested_user_transaction(
                        AttestedTransaction::new(tx, Some(attestation)),
                    ),
                    None => ConsensusTransaction::new_user_transaction(tx),
                };

                SequencedConsensusTransaction::new_test(consensus_tx)
            })
            .collect();
        let epoch_store = self.authority.epoch_store_for_testing();
        let (scheduled, assigned_versions) = epoch_store
            .process_consensus_transactions_for_tests(
                consensus_txs,
                &Arc::new(CheckpointServiceNoop {}),
                self.authority.get_object_cache_reader().as_ref(),
                &self.authority.metrics,
                true,
                &self.authority,
            )
            .await
            .unwrap();
        let assigned_versions = assigned_versions.into_map();
        let mut scheduled: HashMap<_, _> = scheduled
            .into_iter()
            .map(|scheduled| {
                let key = scheduled.key();
                let Schedulable::Transaction(attested) = scheduled else {
                    panic!("consensus must schedule only the submitted transactions, got {key:?}");
                };
                let (executable, attestation) = attested.into_parts();

                (*executable.digest(), (key, executable, attestation))
            })
            .collect();

        let mut effects = Vec::with_capacity(order.len());
        for digest in order {
            let Some((key, executable, attestation)) = scheduled.remove(&digest) else {
                let error = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    epoch_store.notify_read_dropped_digests(digest),
                )
                .await
                .expect("a dropped transaction has its error recorded");
                panic!("consensus must schedule the submitted transaction, got {error:?}");
            };
            let execution_env = ExecutionEnv::new()
                .with_assigned_versions(assigned_versions.get(&key).cloned().unwrap_or_default())
                .with_attestation(sources.remove(&digest).unwrap_or(attestation));
            let (executed, _) = self
                .authority
                .try_execute_immediately(&executable, execution_env, &epoch_store)
                .unwrap();
            effects.push(executed);
        }

        effects
    }

    /// The verdict the authority recorded for an executed transaction.
    pub fn attestation_record(&self, digest: &TransactionDigest) -> Option<AttestationRecord> {
        self.authority
            .get_transaction_cache_reader()
            .multi_get_attestation_records_in_epoch(
                self.authority.epoch_store_for_testing().epoch(),
                &[*digest],
            )
            .pop()
            .expect("one result per digest")
    }
}
