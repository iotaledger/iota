// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The leases of transactions.
//!
//! A transaction whose expiration is a valid window can carry a lease. The
//! first such transaction of a sender that post-consensus validation keeps
//! holds the lease until its last valid epoch: every other transaction of the
//! same sender with the same lease is dropped until then.
//!
//! Within an epoch the held leases are in the epoch tables, written with the
//! consensus commit that kept the transaction. For earlier epochs they are in
//! the historic ledger, written when the transaction's effects are.

use iota_sdk_types::{Address, EpochId, Transaction, TransactionDigest};
use iota_types::transaction::TransactionAPI;
use serde::{Deserialize, Serialize};

/// A sender and the lease of one of its transactions.
pub type LeaseKey = (Address, [u8; 32]);

/// The transaction that holds a lease, and the last epoch in which it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseHolder {
    pub transaction: TransactionDigest,
    pub last_valid: EpochId,
}

/// The stored form of a [`LeaseHolder`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaseHolderWrapper {
    V1(LeaseHolder),
}

impl LeaseHolderWrapper {
    pub fn into_inner(self) -> LeaseHolder {
        match self {
            Self::V1(holder) => holder,
        }
    }
}

impl From<LeaseHolder> for LeaseHolderWrapper {
    fn from(holder: LeaseHolder) -> Self {
        Self::V1(holder)
    }
}

/// The lease `transaction` carries with the last epoch in which it holds it,
/// or `None` if it carries no lease.
pub fn lease_of(transaction: &Transaction) -> Option<(LeaseKey, EpochId)> {
    let window = transaction.expiration().as_opt_valid_window()?;
    let lease = window.lease?;
    Some(((transaction.sender(), lease), window.last_valid))
}
