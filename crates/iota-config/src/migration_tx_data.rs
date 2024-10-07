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
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    message_envelope::Message,
    messages_checkpoint::{CheckpointContents, CheckpointSummary},
    object::{Object, ObjectInner},
    transaction::{GenesisTransaction, Transaction, TransactionDataAPI},
};
use serde::{Deserialize, Serialize};
use tracing::trace;

use crate::genesis::{Genesis, UnsignedGenesis};

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

    fn validate_from_genesis_components(
        &self,
        checkpoint: &CheckpointSummary,
        contents: &CheckpointContents,
        genesis_tx_digest: TransactionDigest,
    ) -> anyhow::Result<()> {
        assert_eq!(checkpoint.content_digest, *contents.digest());

        for (valid_tx_digest, valid_effects_digest) in contents.iter().filter_map(|exec_digest| {
            if exec_digest.transaction != genesis_tx_digest {
                Some((&exec_digest.transaction, &exec_digest.effects))
            } else {
                None
            }
        }) {
            let (tx, effects, events) = self
                .inner
                .get(&valid_tx_digest)
                .ok_or(anyhow::anyhow!("Missing transaction digest"))?;

            if &effects.digest() != valid_effects_digest
                || effects.transaction_digest() != valid_tx_digest
                || &tx.data().digest() != valid_tx_digest
            {
                return Err(anyhow::anyhow!("Invalid transaction or effects data"));
            }

            if let Some(valid_events) = effects.events_digest() {
                if &events.digest() != valid_events {
                    return Err(anyhow::anyhow!("Invalid events data"));
                }
            } else if !events.data.is_empty() {
                return Err(anyhow::anyhow!("Invalid events data"));
            }
        }
        Ok(())
    }

    pub fn validate_from_genesis(&self, genesis: &Genesis) -> anyhow::Result<()> {
        self.validate_from_genesis_components(
            &genesis.checkpoint(),
            genesis.checkpoint_contents(),
            *genesis.transaction().digest(),
        )
    }

    pub fn validate_from_unsigned_genesis(
        &self,
        unsigned_genesis: &UnsignedGenesis,
    ) -> anyhow::Result<()> {
        self.validate_from_genesis_components(
            unsigned_genesis.checkpoint(),
            unsigned_genesis.checkpoint_contents(),
            *unsigned_genesis.transaction().digest(),
        )
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
