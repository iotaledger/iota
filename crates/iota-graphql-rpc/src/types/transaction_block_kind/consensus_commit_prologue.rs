// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use fastcrypto::encoding::{Base58, Encoding};
use iota_types::{
    digests::{AdditionalConsensusStatesDigest, ConsensusCommitDigest},
    messages_checkpoint::CheckpointTimestamp,
    messages_consensus::{
        ConsensusCommitPrologueV1 as NativeConsensusCommitPrologueTransactionV1,
        ConsensusCommitPrologueV2 as NativeConsensusCommitPrologueTransactionV2,
        ConsensusDeterminedVersionAssignments,
    },
};

use crate::types::{date_time::DateTime, epoch::Epoch, uint53::UInt53};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ConsensusCommitPrologueTransaction {
    epoch: u64,
    round: u64,
    /// The sub DAG index of the consensus commit. This field will be populated
    /// if there are multiple consensus commits per round.
    sub_dag_index: Option<u64>,
    commit_timestamp_ms: CheckpointTimestamp,
    consensus_commit_digest: ConsensusCommitDigest,
    /// The checkpoint sequence number this was viewed at.
    checkpoint_viewed_at: u64,
    /// Stores consensus handler determined shared object version assignments
    /// for transactions within the consensus commit.
    /// Note that currently it only stores cancelled transactions' version
    /// assignments.
    consensus_determined_version_assignments: ConsensusDeterminedVersionAssignments,
    /// Digest of any additional state computed by the consensus handler.
    /// Used to detect forking bugs as early as possible.
    additional_states_digests: Option<Vec<AdditionalConsensusStatesDigest>>,
}

impl ConsensusCommitPrologueTransaction {
    pub fn from_v1(
        native: NativeConsensusCommitPrologueTransactionV1,
        checkpoint_viewed_at: u64,
    ) -> Self {
        Self {
            epoch: native.epoch,
            round: native.round,
            sub_dag_index: native.sub_dag_index,
            commit_timestamp_ms: native.commit_timestamp_ms,
            consensus_commit_digest: native.consensus_commit_digest,
            checkpoint_viewed_at,
            consensus_determined_version_assignments: native
                .consensus_determined_version_assignments,
            additional_states_digests: None,
        }
    }

    pub fn from_v2(
        native: NativeConsensusCommitPrologueTransactionV2,
        checkpoint_viewed_at: u64,
    ) -> Self {
        Self {
            epoch: native.epoch,
            round: native.round,
            sub_dag_index: native.sub_dag_index,
            commit_timestamp_ms: native.commit_timestamp_ms,
            consensus_commit_digest: native.consensus_commit_digest,
            checkpoint_viewed_at,
            consensus_determined_version_assignments: native
                .consensus_determined_version_assignments,
            additional_states_digests: Some(native.additional_states_digests),
        }
    }
}

/// System transaction that runs at the beginning of a checkpoint, and is
/// responsible for setting the current value of the clock, based on the
/// timestamp from consensus.
#[Object]
impl ConsensusCommitPrologueTransaction {
    /// Epoch of the commit prologue transaction.
    async fn epoch(&self, ctx: &Context<'_>) -> Result<Option<Epoch>> {
        Epoch::query(ctx, Some(self.epoch), self.checkpoint_viewed_at)
            .await
            .extend()
    }

    /// Consensus round of the commit.
    async fn round(&self) -> UInt53 {
        self.round.into()
    }

    /// Unix timestamp from consensus.
    async fn commit_timestamp(&self) -> Result<DateTime, Error> {
        Ok(DateTime::from_ms(self.commit_timestamp_ms as i64)?)
    }

    /// Digest of consensus output, encoded as a Base58 string.
    async fn consensus_commit_digest(&self) -> String {
        Base58::encode(self.consensus_commit_digest.inner())
    }
}
