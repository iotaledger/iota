// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
use std::sync::Arc;
use std::{
    fmt,
    hash::{Hash, Hasher},
};

use enum_dispatch::enum_dispatch;
use fastcrypto::hash::Digest;
use serde::{Deserialize, Serialize};
use starfish_config::{AuthorityIndex, DIGEST_LENGTH};

#[cfg(test)]
use crate::context::Context;
use crate::{
    block_header::{BlockRef, Round, TransactionsCommitment},
    error::{ConsensusError, ConsensusResult},
};

#[derive(Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionRef {
    pub round: Round,
    pub author: AuthorityIndex,
    pub transactions_commitment: TransactionsCommitment,
}

impl TransactionRef {
    pub fn new(block_ref: BlockRef, transactions_commitment: TransactionsCommitment) -> Self {
        Self {
            round: block_ref.round,
            author: block_ref.author,
            transactions_commitment,
        }
    }
}

impl fmt::Display for TransactionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "Tx{}({},{})",
            self.round, self.author, self.transactions_commitment
        )
    }
}

impl fmt::Debug for TransactionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        fmt::Display::fmt(self, f)
    }
}

impl Hash for TransactionRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.transactions_commitment.0[..8]);
    }
}

/// Accessors to transaction reference info.
#[enum_dispatch]
pub(crate) trait GenericTransactionRefAPI {
    fn author(&self) -> AuthorityIndex;
    fn round(&self) -> Round;
    fn digest(&self) -> Digest<DIGEST_LENGTH>;
    fn variant_name(&self) -> &'static str;
}

/// A generic reference to either a block or a transaction.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[enum_dispatch(GenericTransactionRefAPI)]
pub enum GenericTransactionRef {
    BlockRef(BlockRef),
    TransactionRef(TransactionRef),
}

impl GenericTransactionRefAPI for BlockRef {
    fn author(&self) -> AuthorityIndex {
        self.author
    }

    fn round(&self) -> Round {
        self.round
    }

    fn digest(&self) -> Digest<DIGEST_LENGTH> {
        self.digest.into()
    }

    fn variant_name(&self) -> &'static str {
        "BlockRef"
    }
}

impl GenericTransactionRefAPI for TransactionRef {
    fn author(&self) -> AuthorityIndex {
        self.author
    }

    fn round(&self) -> Round {
        self.round
    }

    fn digest(&self) -> Digest<DIGEST_LENGTH> {
        self.transactions_commitment.into()
    }

    fn variant_name(&self) -> &'static str {
        "TransactionRef"
    }
}

impl GenericTransactionRef {
    /// Extract TransactionRef, returning error if this is a BlockRef variant.
    /// This should only be called when consensus_fast_commit_sync flag is true.
    pub(crate) fn expect_transaction_ref(self) -> ConsensusResult<TransactionRef> {
        match self {
            GenericTransactionRef::TransactionRef(tr) => Ok(tr),
            GenericTransactionRef::BlockRef(_) => {
                Err(ConsensusError::TransactionRefVariantMismatch {
                    protocol_flag_enabled: true,
                    expected_variant: "TransactionRef",
                    received_variant: self.variant_name(),
                })
            }
        }
    }

    /// Extract BlockRef, returning error if this is a TransactionRef variant.
    /// This should only be called when consensus_fast_commit_sync flag is
    /// false.
    #[allow(dead_code)]
    pub(crate) fn expect_block_ref(self) -> ConsensusResult<BlockRef> {
        match self {
            GenericTransactionRef::BlockRef(br) => Ok(br),
            GenericTransactionRef::TransactionRef(_) => {
                Err(ConsensusError::TransactionRefVariantMismatch {
                    protocol_flag_enabled: false,
                    expected_variant: "BlockRef",
                    received_variant: self.variant_name(),
                })
            }
        }
    }
}

impl fmt::Display for GenericTransactionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenericTransactionRef::BlockRef(b) => write!(f, "{}", b),
            GenericTransactionRef::TransactionRef(t) => write!(f, "{}", t),
        }
    }
}

impl Hash for GenericTransactionRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            GenericTransactionRef::BlockRef(b) => b.hash(state),
            GenericTransactionRef::TransactionRef(t) => t.hash(state),
        }
    }
}

/// Helper function to convert BlockRefs to GenericTransactionRefs based on
/// protocol flag.
#[cfg(test)]
pub(crate) fn convert_block_refs_to_generic_transaction_refs(
    context: &Arc<Context>,
    store: &dyn crate::storage::Store,
    block_refs: &[BlockRef],
) -> Vec<GenericTransactionRef> {
    if context.protocol_config.consensus_fast_commit_sync() {
        // Fetch headers to get transactions_commitment for TransactionRef
        let headers = store.read_verified_block_headers(block_refs).unwrap();
        block_refs
            .iter()
            .enumerate()
            .map(|(idx, block_ref)| {
                let header = headers[idx].as_ref().unwrap();
                GenericTransactionRef::TransactionRef(TransactionRef {
                    round: block_ref.round,
                    author: block_ref.author,
                    transactions_commitment: header.transactions_commitment(),
                })
            })
            .collect()
    } else {
        block_refs
            .iter()
            .map(|br| GenericTransactionRef::BlockRef(*br))
            .collect()
    }
}
