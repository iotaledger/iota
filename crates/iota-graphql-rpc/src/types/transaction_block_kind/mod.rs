// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use iota_types::transaction::TransactionKind as NativeTransactionKind;

use self::{
    consensus_commit_prologue::ConsensusCommitPrologueTransaction, genesis::GenesisTransaction,
    randomness_state_update::RandomnessStateUpdateTransaction,
};
use crate::{
    error::Error,
    types::transaction_block_kind::{
        end_of_epoch::EndOfEpochTransaction, programmable::ProgrammableTransactionBlock,
    },
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
    pub(crate) fn try_from(
        kind: NativeTransactionKind,
        checkpoint_viewed_at: u64,
    ) -> Result<Self, Error> {
        use NativeTransactionKind as K;
        use TransactionBlockKind as T;

        match kind {
            K::ProgrammableTransaction(pt) => Ok(T::Programmable(ProgrammableTransactionBlock {
                native: pt,
                checkpoint_viewed_at,
            })),
            K::Genesis(g) => Ok(T::Genesis(GenesisTransaction {
                native: g,
                checkpoint_viewed_at,
            })),
            K::ConsensusCommitPrologueV1(ccp) => Ok(T::ConsensusCommitPrologue(
                ConsensusCommitPrologueTransaction {
                    native: ccp,
                    checkpoint_viewed_at,
                },
            )),
            #[allow(deprecated)]
            K::AuthenticatorStateUpdateV1Deprecated => {
                // Deprecated: Authenticator state (JWK) is deprecated and
                // was never enabled. These transaction kinds are retained
                // only for BCS enum variant compatibility.
                Err(Error::UnsupportedFeature(
                    "AuthenticatorStateUpdateV1 transactions are deprecated and were never created on IOTA".to_string(),
                ))
            }
            K::EndOfEpochTransaction(eoe) => Ok(T::EndOfEpoch(EndOfEpochTransaction {
                native: eoe,
                checkpoint_viewed_at,
            })),
            K::RandomnessStateUpdate(rsu) => Ok(T::Randomness(RandomnessStateUpdateTransaction {
                native: rsu,
                checkpoint_viewed_at,
            })),
        }
    }
}
