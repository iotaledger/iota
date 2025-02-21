// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use fastcrypto::encoding::{Base58, Encoding};
use iota_types::messages_consensus::ConsensusCommitPrologueV1 as NativeConsensusCommitPrologueTransactionV1;

use crate::types::{date_time::DateTime, epoch::Epoch, uint53::UInt53};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ConsensusCommitPrologueTransaction {
    pub native: NativeConsensusCommitPrologueTransactionV1,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
}

/// System transaction that runs at the beginning of a checkpoint, and is
/// responsible for setting the current value of the clock, based on the
/// timestamp from consensus.
#[Object]
impl ConsensusCommitPrologueTransaction {
    /// Epoch of the commit prologue transaction.
    async fn epoch(&self, ctx: &Context<'_>) -> Result<Option<Epoch>> {
        Epoch::query(ctx, Some(self.native.epoch), self.checkpoint_viewed_at)
            .await
            .extend()
    }

    /// Consensus round of the commit.
    async fn round(&self) -> UInt53 {
        self.native.round.into()
    }

    /// Unix timestamp from consensus.
    async fn commit_timestamp(&self) -> Result<DateTime, Error> {
        Ok(DateTime::from_ms(self.native.commit_timestamp_ms as i64)?)
    }

    /// Digest of consensus output, encoded as a Base58 string.
    async fn consensus_commit_digest(&self) -> String {
        Base58::encode(self.native.consensus_commit_digest.inner())
    }
}
