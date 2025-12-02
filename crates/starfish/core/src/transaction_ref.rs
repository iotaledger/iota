// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
use std::sync::Arc;
use std::{
    fmt,
    hash::{Hash, Hasher},
};

use fastcrypto::hash::Digest;
use serde::{Deserialize, Serialize};
use starfish_config::{AuthorityIndex, DIGEST_LENGTH};

use crate::block_header::{BlockHeaderDigest, BlockRef, Round, TransactionsCommitment};
#[cfg(test)]
use crate::context::Context;

#[derive(Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionRef {
    pub round: Round,
    pub author: AuthorityIndex,
    pub transactions_commitment: TransactionsCommitment,
    pub block_digest: BlockHeaderDigest,
}

impl fmt::Display for TransactionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "Tr{}({},{})",
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

impl From<TransactionRef> for BlockRef {
    fn from(tr: TransactionRef) -> Self {
        BlockRef {
            round: tr.round,
            author: tr.author,
            digest: tr.block_digest,
        }
    }
}

/// A generic reference to either a block or a transaction.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum GenericTransactionRef {
    BlockRef(BlockRef),
    TransactionRef(TransactionRef),
}

// Converts BlockRef to GenericTransactionsRef
impl From<BlockRef> for GenericTransactionRef {
    fn from(b: BlockRef) -> Self {
        GenericTransactionRef::BlockRef(b)
    }
}

// Converts TransactionRef to GenericTransactionsRef
impl From<TransactionRef> for GenericTransactionRef {
    fn from(t: TransactionRef) -> Self {
        GenericTransactionRef::TransactionRef(t)
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

impl GenericTransactionRef {
    pub(crate) fn author(&self) -> AuthorityIndex {
        match self {
            GenericTransactionRef::BlockRef(b) => b.author,
            GenericTransactionRef::TransactionRef(t) => t.author,
        }
    }

    pub(crate) fn round(&self) -> Round {
        match self {
            GenericTransactionRef::BlockRef(b) => b.round,
            GenericTransactionRef::TransactionRef(t) => t.round,
        }
    }

    pub(crate) fn digest(&self) -> Digest<DIGEST_LENGTH> {
        match self {
            GenericTransactionRef::BlockRef(b) => b.digest.into(),
            GenericTransactionRef::TransactionRef(t) => t.transactions_commitment.into(),
        }
    }

    /// Returns the variant name as a static string.
    pub(crate) fn variant_name(&self) -> &'static str {
        match self {
            GenericTransactionRef::BlockRef(_) => "BlockRef",
            GenericTransactionRef::TransactionRef(_) => "TransactionRef",
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
    if context.protocol_config.consensus_transaction_ref() {
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
                    block_digest: block_ref.digest,
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
