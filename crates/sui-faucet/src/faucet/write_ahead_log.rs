// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use serde::{Deserialize, Serialize};
use sui_types::base_types::SuiAddress;
use sui_types::{base_types::ObjectID, transaction::TransactionData};
use uuid::Uuid;

/// Persistent log of transactions paying out sui from the faucet, keyed by the coin serving the
/// request.  Transactions are expected to be written to the log before they are sent to full-node,
/// and removed after receiving a response back, before the coin becomes available for subsequent
/// writes.
///
/// This allows the faucet to go down and back up, and not forget which requests were in-flight that
/// it needs to confirm succeeded or failed.
#[derive(Clone)]
pub struct WriteAheadLog {
    // TODO: implement key-value database that can be persisted to disk
    //pub log: DBMap<ObjectID, Entry>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum TypedStoreError {
    SerializationError(String),
}

impl std::fmt::Display for TypedStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypedStoreError::SerializationError(s) => write!(f, "Serialization error: {}", s),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Entry {
    pub uuid: uuid::Bytes,
    // TODO (jian): remove recipient
    pub recipient: SuiAddress,
    pub tx: TransactionData,
    pub retry_count: u64,
    pub in_flight: bool,
}

impl WriteAheadLog {
    pub(crate) fn open(_path: &Path) -> Self {
        unimplemented!()
    }

    /// Mark `coin` as reserved for transaction `tx` sending coin to `recipient`. Fails if `coin` is
    /// already in the WAL pointing to an existing transaction.
    pub(crate) fn reserve(
        &mut self,
        _uuid: Uuid,
        _coin: ObjectID,
        _recipient: SuiAddress,
        _tx: TransactionData,
    ) -> Result<(), TypedStoreError> {
        unimplemented!()
    }

    /// Check whether `coin` has a pending transaction in the WAL.  Returns `Ok(Some(entry))` if a
    /// pending transaction exists, `Ok(None)` if not, and `Err(_)` if there was an internal error
    /// accessing the WAL.
    pub(crate) fn reclaim(&self, _coin: ObjectID) -> Result<Option<Entry>, TypedStoreError> {
        unimplemented!()
    }

    /// Indicate that the transaction in flight for `coin` has landed, and the entry in the WAL can
    /// be removed.
    pub(crate) fn commit(&mut self, _coin: ObjectID) -> Result<(), TypedStoreError> {
        unimplemented!()
    }

    pub(crate) fn set_in_flight(
        &mut self,
        _coin: ObjectID,
        _bool: bool,
    ) -> Result<(), TypedStoreError> {
        unimplemented!()
    }


    pub(crate) fn increment_retry_count(&mut self, coin: ObjectID) -> Result<(), TypedStoreError> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use sui_types::{
        base_types::{random_object_ref, ObjectRef},
    };

    use super::*;

    #[ignore]
    #[tokio::test]
    async fn reserve_reclaim_reclaim() {
        let tmp = tempfile::tempdir().unwrap();
        let mut wal = WriteAheadLog::open(&tmp.path().join("wal"));

        let uuid = Uuid::new_v4();
        let coin = random_object_ref();
        let (recv, tx) = random_request(coin);

        assert!(wal.reserve(uuid, coin.0, recv, tx.clone()).is_ok());

        // Reclaim once
        let Some(entry) = wal.reclaim(coin.0).unwrap() else {
            panic!("Entry not found for {}", coin.0);
        };

        assert_eq!(uuid, Uuid::from_bytes(entry.uuid));
        assert_eq!(recv, entry.recipient);
        assert_eq!(tx, entry.tx);

        // Reclaim again, should still be there.
        let Some(entry) = wal.reclaim(coin.0).unwrap() else {
            panic!("Entry not found for {}", coin.0);
        };

        assert_eq!(uuid, Uuid::from_bytes(entry.uuid));
        assert_eq!(recv, entry.recipient);
        assert_eq!(tx, entry.tx);
    }

    #[ignore]
    #[tokio::test]
    async fn test_increment_wal() {
        let tmp = tempfile::tempdir().unwrap();
        let mut wal = WriteAheadLog::open(&tmp.path().join("wal"));
        let uuid = Uuid::new_v4();
        let coin = random_object_ref();
        let (recv0, tx0) = random_request(coin);

        // First write goes through
        wal.reserve(uuid, coin.0, recv0, tx0).unwrap();
        wal.increment_retry_count(coin.0).unwrap();

        let entry = wal.reclaim(coin.0).unwrap().unwrap();
        assert_eq!(entry.retry_count, 1);
    }

    #[ignore]
    #[tokio::test]
    async fn reserve_reserve() {
        let tmp = tempfile::tempdir().unwrap();
        let mut wal = WriteAheadLog::open(&tmp.path().join("wal"));

        let uuid = Uuid::new_v4();
        let coin = random_object_ref();
        let (recv0, tx0) = random_request(coin);
        let (recv1, tx1) = random_request(coin);

        // First write goes through
        wal.reserve(uuid, coin.0, recv0, tx0).unwrap();

        // Second write fails because it tries to write to the same coin
        assert!(matches!(
            wal.reserve(uuid, coin.0, recv1, tx1),
            Err(TypedStoreError::SerializationError(_)),
        ));
    }

    #[ignore]
    #[tokio::test]
    async fn reserve_reclaim_commit_reclaim() {
        let tmp = tempfile::tempdir().unwrap();
        let mut wal = WriteAheadLog::open(&tmp.path().join("wal"));

        let uuid = Uuid::new_v4();
        let coin = random_object_ref();
        let (recv, tx) = random_request(coin);

        wal.reserve(uuid, coin.0, recv, tx.clone()).unwrap();

        // Reclaim to show that the entry is there
        let Some(entry) = wal.reclaim(coin.0).unwrap() else {
            panic!("Entry not found for {}", coin.0);
        };

        assert_eq!(uuid, Uuid::from_bytes(entry.uuid));
        assert_eq!(recv, entry.recipient);
        assert_eq!(tx, entry.tx);

        // Commit the transaction, which removes it from the log.
        wal.commit(coin.0).unwrap();

        // Expect it to now be gone
        assert_eq!(Ok(None), wal.reclaim(coin.0));
    }

    #[ignore]
    #[tokio::test]
    async fn reserve_commit_reserve() {
        let tmp = tempfile::tempdir().unwrap();
        let mut wal = WriteAheadLog::open(&tmp.path().join("wal"));

        let uuid = Uuid::new_v4();
        let coin = random_object_ref();
        let (recv0, tx0) = random_request(coin);
        let (recv1, tx1) = random_request(coin);

        // Write the transaction
        wal.reserve(uuid, coin.0, recv0, tx0).unwrap();

        // Commit the transaction, which removes it from the log.
        wal.commit(coin.0).unwrap();

        // Write a fresh transaction, which should now pass
        wal.reserve(uuid, coin.0, recv1, tx1).unwrap();
    }

    fn random_request(coin: ObjectRef) -> (SuiAddress, TransactionData) {
        unimplemented!()
    }
}
