// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

use anyhow::{Context, Result};
use iota_types::{
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEvents},
    object::{Object, ObjectInner},
    transaction::{GenesisTransaction, Transaction, TransactionDataAPI},
};
use serde::{Deserialize, Serialize};
use tracing::trace;

pub type TransactionsData =
    BTreeMap<TransactionDigest, (Transaction, TransactionEffects, TransactionEvents)>;
#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Default)]
pub struct MigrationTxData {
    inner: TransactionsData,
}

impl MigrationTxData {
    pub fn new(txs_data: TransactionsData) -> Self {
        Self { inner: txs_data }
    }

    pub fn txs_data(&self) -> &TransactionsData {
        &self.inner
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn objects_by_tx_digest(
        &self,
        digest: TransactionDigest,
    ) -> Result<Vec<Object>, anyhow::Error> {
        let mut migration_objects = Vec::new();

        let (tx, _, _) = self
            .inner
            .get(&digest)
            .with_context(|| format!("No transaction found for digest: {:?}", digest))?;
        if let iota_types::transaction::TransactionKind::Genesis(GenesisTransaction {
            objects,
            ..
        }) = tx.transaction_data().kind()
        {
            for object in objects {
                match object {
                    iota_types::transaction::GenesisObject::RawObject { data, owner } => {
                        let object = ObjectInner {
                            data: data.to_owned(),
                            owner: owner.to_owned(),
                            previous_transaction: tx.digest().clone(),
                            storage_rebate: 0,
                        };
                        migration_objects.push(object.into());
                    }
                }
            }
        } else {
            panic!("Wrong transaction type of migration data");
        }
        Ok(migration_objects)
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        trace!("Reading Migration transaction data from {}", path.display());
        let read = File::open(path).with_context(|| {
            format!(
                "Unable to load Migration transaction data from {}",
                path.display()
            )
        })?;
        bcs::from_reader(BufReader::new(read)).with_context(|| {
            format!(
                "Unable to parse Migration transaction data from {}",
                path.display()
            )
        })
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        let path = path.as_ref();
        trace!("Writing Migration transaction data to {}", path.display());
        let mut write = BufWriter::new(File::create(path)?);
        bcs::serialize_into(&mut write, &self).with_context(|| {
            format!(
                "Unable to save Migration transaction data to {}",
                path.display()
            )
        })?;
        Ok(())
    }
}
