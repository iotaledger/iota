// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use iota_types::transaction::TransactionKind as NativeTransactionKind;

use self::{
    consensus_commit_prologue::ConsensusCommitPrologueTransaction, genesis::GenesisTransaction,
    randomness_state_update::RandomnessStateUpdateTransaction,
};
use crate::types::transaction_block_kind::{
    end_of_epoch::EndOfEpochTransaction, programmable::ProgrammableTransactionBlock,
};

pub(crate) mod consensus_commit_prologue;
pub(crate) mod end_of_epoch;
pub(crate) mod genesis;
pub(crate) mod programmable;
pub(crate) mod randomness_state_update;

/// The kind of transaction block, either a programmable transaction or a system
/// transaction.
#[derive(Union, PartialEq, Clone, Eq)]
pub(crate) enum TransactionBlockKind {
    ConsensusCommitPrologue(ConsensusCommitPrologueTransaction),
    Genesis(GenesisTransaction),
    Programmable(ProgrammableTransactionBlock),
    Randomness(RandomnessStateUpdateTransaction),
    EndOfEpoch(EndOfEpochTransaction),
}

impl TransactionBlockKind {
    pub(crate) fn from(kind: NativeTransactionKind, checkpoint_viewed_at: u64) -> Self {
        use NativeTransactionKind as K;
        use TransactionBlockKind as T;

        match kind {
            K::ProgrammableTransaction(pt) => T::Programmable(ProgrammableTransactionBlock {
                native: pt,
                checkpoint_viewed_at,
            }),
            K::Genesis(g) => T::Genesis(GenesisTransaction {
                native: g,
                checkpoint_viewed_at,
            }),
            K::ConsensusCommitPrologueV1(ccp) => {
                T::ConsensusCommitPrologue(ConsensusCommitPrologueTransaction {
                    native: ccp,
                    checkpoint_viewed_at,
                })
            }
            #[allow(deprecated)]
            K::AuthenticatorStateUpdateV1Deprecated => {
                // Deprecated: Authenticator state (JWK) is deprecated and
                // and was never enabled. These transaction kinds are retained
                // only for BCS enum variant compatibility.
                unreachable!(
                    "AuthenticatorState transactions are deprecated and were never created on IOTA"
                );
            }
            K::EndOfEpochTransaction(eoe) => T::EndOfEpoch(EndOfEpochTransaction {
                native: eoe,
                checkpoint_viewed_at,
            }),
            K::RandomnessStateUpdate(rsu) => T::Randomness(RandomnessStateUpdateTransaction {
                native: rsu,
                checkpoint_viewed_at,
            }),
        }
    }
}
