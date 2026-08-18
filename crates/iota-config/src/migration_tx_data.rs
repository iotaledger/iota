// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    io::BufReader,
    path::Path,
};

use anyhow::{Context, Result};
use iota_sdk_ext::types::{
    TransactionDigest, TransactionEffects, TransactionEvents,
    checkpoint::{CheckpointContents, CheckpointSummary},
};
use iota_types::{
    effects::TransactionEffectsAPI, message_envelope::Message,
    messages_checkpoint::CheckpointContentsExt, transaction::TransactionEnvelope,
};
use serde::{Deserialize, Serialize};
use tracing::trace;

use crate::genesis::Genesis;

pub type TransactionsData =
    BTreeMap<TransactionDigest, (TransactionEnvelope, TransactionEffects, TransactionEvents)>;

// Migration data from the Stardust network is loaded separately after genesis
// to reduce the size of the genesis transaction.
#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Default)]
pub struct MigrationTxData {
    inner: TransactionsData,
}

impl MigrationTxData {
    pub fn txs_data(&self) -> &TransactionsData {
        &self.inner
    }

    fn validate_from_genesis_components(
        &self,
        checkpoint: &CheckpointSummary,
        contents: &CheckpointContents,
        genesis_tx_digest: TransactionDigest,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            checkpoint.contents_digest == contents.digest(),
            "checkpoint contents digest is corrupted"
        );
        let mut validation_digests_queue: HashSet<TransactionDigest> =
            self.inner.keys().copied().collect();
        for exec_digest in contents.iter() {
            // We skip the genesis transaction to process only migration transactions from
            // the migration.blob.
            if exec_digest.transaction == genesis_tx_digest {
                continue;
            }
            let valid_tx_digest = &exec_digest.transaction;
            let valid_effects_digest = &exec_digest.effects;
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
            } else if !events.is_empty() {
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
    /// validation is based on cryptographic links (i.e., hash digests) between
    /// transactions, transaction effects and events.
    pub fn validate_from_genesis(&self, genesis: &Genesis) -> anyhow::Result<()> {
        self.validate_from_genesis_components(
            &genesis.checkpoint(),
            genesis.checkpoint_contents(),
            *genesis.transaction().digest(),
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
}
