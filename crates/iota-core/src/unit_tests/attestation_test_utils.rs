// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Attestation extensions for [`AbstractAccountTestEnv`]: producing
//! attestations, honest or fabricated, and submitting transactions through
//! consensus as `UserTransactionV1`/`UserTransactionV2`. The environment itself
//! is attestation-agnostic; everything PCOOL-coupled lives here.

use std::sync::Arc;

use iota_sdk_types::ObjectReference;
use iota_types::{
    attestation::{Attestation, AttestationData, AttestedTransaction},
    effects::TransactionEffects,
    messages_consensus::ConsensusTransaction,
    transaction::Transaction,
};
use starfish_config::AuthorityIndex;

use crate::{
    authority::abstract_account_test_utils::AbstractAccountTestEnv,
    checkpoints::CheckpointServiceNoop, consensus_handler::SequencedConsensusTransaction,
};

/// Consensus accepts a validator attestation only from the block author, which
/// `SequencedConsensusTransaction::new_test` fixes at index 0.
fn attestor_index() -> AuthorityIndex {
    AuthorityIndex::new_for_test(0)
}

impl AbstractAccountTestEnv {
    /// Runs the attestor's own dry-run, which is how a genuine attestation is
    /// produced. Only succeeds while authentication still passes.
    pub fn attest(&self, tx: &Transaction) -> Attestation {
        let epoch_store = self.authority.epoch_store_for_testing();
        let verified = epoch_store.verify_transaction(tx.clone()).unwrap();
        let (payload, _) = self
            .authority
            .attest_transaction(&verified, &epoch_store)
            .expect("attesting must succeed while authentication passes");
        Attestation::Validator {
            payload,
            attestor_index: attestor_index(),
        }
    }

    /// An attestation vouching for the transaction at the given versions, as a
    /// dishonest attestor would produce. The computation estimate is the
    /// largest one consensus accepts, so the verdict on the recorded versions
    /// is never preempted by the attested-units cap on the re-run.
    pub fn attest_with_versions(&self, object_versions: Vec<ObjectReference>) -> Attestation {
        let computation_units = self.budget() / self.rgp();
        Attestation::Validator {
            payload: AttestationData::V1 {
                computation_units,
                object_versions,
            },
            attestor_index: attestor_index(),
        }
    }

    /// Submits the transaction to consensus, with an attestation when one is
    /// given, and executes what consensus schedules. Shared input versions are
    /// assigned here, so the transaction runs against the account's state at
    /// submission time rather than at the time it was built.
    pub async fn submit(
        &self,
        tx: Transaction,
        attestation: Option<Attestation>,
    ) -> TransactionEffects {
        let consensus_tx = match attestation {
            Some(attestation) => ConsensusTransaction::new_user_transaction_v2(
                AttestedTransaction::new(tx, attestation),
            ),
            None => ConsensusTransaction::new_user_transaction_v1(tx),
        };
        let epoch_store = self.authority.epoch_store_for_testing();
        let scheduled = epoch_store
            .process_consensus_transactions_for_tests(
                vec![SequencedConsensusTransaction::new_test(consensus_tx)],
                &Arc::new(CheckpointServiceNoop {}),
                self.authority.get_object_cache_reader().as_ref(),
                self.authority.get_transaction_cache_reader().as_ref(),
                &self.authority.metrics,
                true,
                &self.authority,
            )
            .await
            .unwrap();
        let executable = scheduled
            .into_iter()
            .next()
            .expect("consensus must schedule the submitted transaction");
        let (effects, _) = self
            .authority
            .try_execute_immediately(&executable, None, &epoch_store)
            .unwrap();
        effects
    }
}
