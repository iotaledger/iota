// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};
use starfish_config::AuthorityIndex;

use crate::block_header::{BlockRef, Round, TransactionsCommitment};

/// BCS-serialized size of a [`TransactionRef`]: `round` (u32, 4) + `author`
/// (`AuthorityIndex`, 1) + `transactions_commitment` (32).
pub(crate) const SERIALIZED_TRANSACTION_REF_BYTES: usize = 37;

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

/// Helper function to convert BlockRefs to TransactionRefs, fetching each
/// block header for its transactions commitment.
#[cfg(test)]
pub(crate) fn convert_block_refs_to_transaction_refs(
    store: &dyn crate::storage::Store,
    block_refs: &[BlockRef],
) -> Vec<TransactionRef> {
    let headers = store.read_verified_block_headers(block_refs).unwrap();
    block_refs
        .iter()
        .enumerate()
        .map(|(idx, block_ref)| {
            let header = headers[idx].as_ref().unwrap();
            TransactionRef {
                round: block_ref.round,
                author: block_ref.author,
                transactions_commitment: header.transactions_commitment(),
            }
        })
        .collect()
}
