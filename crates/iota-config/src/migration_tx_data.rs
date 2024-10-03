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
    object::Object,
    transaction::Transaction,
};
use serde::{Deserialize, Serialize};
use tracing::trace;

pub type TransactionsData = BTreeMap<
    TransactionDigest,
    (
        Transaction,
        TransactionEffects,
        TransactionEvents,
        Vec<Object>,
    ),
>;
#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Default)]
pub struct MigrationTxData {
    inner: TransactionsData,
}

impl MigrationTxData {
    pub fn new(txs_data: TransactionsData) -> Self {
        Self { inner: txs_data }
    }

    pub fn extract_txs_data(self) -> TransactionsData {
        self.inner
    }

    pub fn txs_data(&self) -> &TransactionsData {
        &self.inner
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
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
