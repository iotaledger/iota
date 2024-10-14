// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

use anyhow::{Context, Result};
use iota_genesis_common::prepare_and_execute_genesis_transaction;
use iota_types::{
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    message_envelope::Message,
    messages_checkpoint::{CheckpointContents, CheckpointSummary},
    object::Object,
    transaction::{Transaction, TransactionDataAPI, TransactionKind},
};
use serde::{Deserialize, Serialize};
use tracing::trace;

use crate::genesis::{Genesis, GenesisCeremonyParameters, UnsignedGenesis};

pub type TransactionsData =
    BTreeMap<TransactionDigest, (Transaction, TransactionEffects, TransactionEvents)>;

// Migration data from the Stardust network is loaded separately after genesis
// to reduce the size of the genesis transaction.
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

    /// Executes all the migration transactions for this migration data and
    /// returns the vector of objects created by these executions.
    pub fn get_objects(&self) -> impl Iterator<Item = Object> + '_ {
        self.inner.values().flat_map(|(tx, _, _)| {
            self.objects_by_tx_digest(*tx.digest())
                .expect("the migration data is corrupted")
                .into_iter()
        })
    }

    /// Executes the migration transaction identified by `digest` and returns
    /// the vector of objects created by the execution.
    pub fn objects_by_tx_digest(&self, digest: TransactionDigest) -> Option<Vec<Object>> {
        let (tx, _, _) = self.inner.get(&digest)?;
        assert!(
            matches!(tx.transaction_data().kind(), TransactionKind::Genesis(_)),
            "wrong transaction type of migration data"
        );

        // We use default ceremony parameters, not the real ones. This should not affect
        // the execution of a genesis transaction.
        let default_ceremony_parameters = GenesisCeremonyParameters::default();

        let (_, _, objects) = prepare_and_execute_genesis_transaction(
            default_ceremony_parameters.chain_start_timestamp_ms,
            default_ceremony_parameters.protocol_version,
            tx,
        );

        Some(objects)
    }

    fn validate_from_genesis_components(
        &self,
        checkpoint: &CheckpointSummary,
        contents: &CheckpointContents,
        genesis_tx_digest: TransactionDigest,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            checkpoint.content_digest == *contents.digest(),
            "checkpoint's content digest is corrupted"
        );
        let mut validation_digests_queue: HashSet<TransactionDigest> =
            self.inner.keys().copied().collect();
        // We skip the genesis transaction to process only migration transactions from
        // the migration.blob.
        for (valid_tx_digest, valid_effects_digest) in contents.iter().filter_map(|exec_digest| {
            (exec_digest.transaction != genesis_tx_digest)
                .then_some((&exec_digest.transaction, &exec_digest.effects))
        }) {
            let (tx, effects, events) = self
                .inner
                .get(valid_tx_digest)
                .ok_or(anyhow::anyhow!("missing transaction digest"))?;

            if &effects.digest() != valid_effects_digest
                || effects.transaction_digest() != valid_tx_digest
                || &tx.data().digest() != valid_tx_digest
            {
                anyhow::bail!("invalid transaction or effects data");
            }

            if let Some(valid_events_digest) = effects.events_digest() {
                if &events.digest() != valid_events_digest {
                    anyhow::bail!("invalid events data");
                }
            } else if !events.data.is_empty() {
                anyhow::bail!("invalid events data");
            }
            validation_digests_queue.remove(valid_tx_digest);
        }
        anyhow::ensure!(
            validation_digests_queue.is_empty(),
            "the migration data is corrupted"
        );
        Ok(())
    }

    /// Validates the content of the migration data through a `Genesis`. The
    /// vaidation is based on cryptographic links (i.e., hash digests) between
    /// transactions, transaction effects and events.
    pub fn validate_from_genesis(&self, genesis: &Genesis) -> anyhow::Result<()> {
        self.validate_from_genesis_components(
            &genesis.checkpoint(),
            genesis.checkpoint_contents(),
            *genesis.transaction().digest(),
        )
    }

    /// Validates the content of the migration data through an
    /// `UnsignedGenesis`. The vaidation is based on cryptographic links
    /// (i.e., hash digests) between transactions, transaction effects and
    /// events.
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

    /// Loads a `MigrationTxData` in memory from a file found in `path`.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        trace!("reading Migration transaction data from {}", path.display());
        let read = File::open(path).with_context(|| {
            format!(
                "unable to load Migration transaction data from {}",
                path.display()
            )
        })?;
        bcs::from_reader(BufReader::new(read)).with_context(|| {
            format!(
                "unable to parse Migration transaction data from {}",
                path.display()
            )
        })
    }

    /// Saves a `MigrationTxData` from memory into a file in `path`.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        let path = path.as_ref();
        trace!("writing Migration transaction data to {}", path.display());
        let mut write = BufWriter::new(File::create(path)?);
        bcs::serialize_into(&mut write, &self).with_context(|| {
            format!(
                "unable to save Migration transaction data to {}",
                path.display()
            )
        })?;
        Ok(())
    }
}
