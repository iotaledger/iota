// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::btree_map::BTreeMap, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use iota_storage::blob::Blob;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    crypto::get_account_key_pair, effects::TransactionEffectsAPI,
    full_checkpoint_content::CheckpointData, gas_coin::NANOS_PER_IOTA,
    utils::to_sender_signed_transaction,
};
use simulacrum::Simulacrum;
use tokio::fs;
use tracing::info;

/// Configuration for generating synthetic checkpoint data for for benchmarking
/// or testing ingestion processes. Defines the behavior of synthetic data
/// generation, such as checkpoint size, number of checkpoints, and the
/// directory for storing generated checkpoint data.
#[derive(Parser, Debug, Clone)]
pub struct Config {
    /// Directory to write the ingestion data to.
    #[arg(long)]
    pub ingestion_dir: PathBuf,
    /// Starting checkpoint sequence number for workload generation.
    /// Useful for benchmarking or testing against a non-empty database.
    #[arg(long, default_value_t = Self::DEFAULT_STARTING_CHECKPOINT)]
    pub starting_checkpoint: u64,
    /// Number of checkpoints to generate.
    /// If `starting_checkpoint` is 0 (default), two additional initial
    /// checkpoints are generated:
    /// - `0.chk`: genesis state
    /// - `1.chk`: initial gas provisioning
    ///
    /// Thus, the total number of generated checkpoint files will be
    /// `num_checkpoints + 2`. Otherwise, exactly `num_checkpoints`
    /// checkpoints are generated.
    #[arg(long, default_value_t = Self::DEFAULT_NUM_CHECKPOINTS)]
    pub num_checkpoints: u64,
    /// Number of transactions in a checkpoint.
    #[arg(long, default_value_t = Self::DEFAULT_CHECKPOINT_SIZE)]
    pub checkpoint_size: u64,
}

impl Config {
    const DEFAULT_STARTING_CHECKPOINT: u64 = 0;
    const DEFAULT_NUM_CHECKPOINTS: u64 = 2000;
    const DEFAULT_CHECKPOINT_SIZE: u64 = 200;
}

/// Generates synthetic checkpoint data based on the provided configuration.
// TODO: Simulacrum does serial execution which could be slow if
// we need to generate a large number of transactions.
// We may want to make Simulacrum support parallel execution.
pub async fn generate_ingestion(config: Config) -> Result<()> {
    info!("Generating synthetic ingestion data. config: {:?}", config);
    let timer = std::time::Instant::now();

    let Config {
        ingestion_dir,
        checkpoint_size,
        num_checkpoints,
        starting_checkpoint,
    } = config;

    // Simulacrum will generate `0.chk` as the genesis checkpoint.
    let sim = Simulacrum::new();
    sim.set_data_ingestion_path(ingestion_dir.clone());

    let gas_price = sim.reference_gas_price();
    let (sender, keypair) = get_account_key_pair();
    let mut gas_object = {
        let effects = sim.request_gas(sender, NANOS_PER_IOTA * 1000000)?;
        // Generate `1.chk` and includes the gas request transaction.
        sim.create_checkpoint();
        effects.created()[0].0
    };

    // When generating a workload that includes the genesis state, retain the
    // initial checkpoints `0.chk` (genesis state) and `1.chk` (initial gas
    // provisioning) to accurately represent the state history.
    // For starting_checkpoint > 0, remove existing checkpoints (0 and 1)
    // and generate a consistent workload.
    if starting_checkpoint > 0 {
        fs::remove_file(ingestion_dir.join("0.chk")).await?;
        fs::remove_file(ingestion_dir.join("1.chk")).await?;
        sim.override_next_checkpoint_number(starting_checkpoint);
    }

    let mut tx_count = 0;
    for i in 0..num_checkpoints {
        for _ in 0..checkpoint_size {
            let tx_data = TestTransactionBuilder::new(sender, gas_object, gas_price)
                .transfer_iota(Some(1), sender)
                .build();
            let tx = to_sender_signed_transaction(tx_data, &keypair);
            let (effects, _) = sim.execute_transaction(tx)?;
            gas_object = effects.gas_object().0;
            tx_count += 1;
        }

        let checkpoint = sim.create_checkpoint();

        let expected_checkpoint_number = if starting_checkpoint == 0 {
            i + 2 // offset by 2 because of the auto-generated `0.chk` and `1.chk` files
        } else {
            i + starting_checkpoint
        };

        assert_eq!(checkpoint.sequence_number, expected_checkpoint_number);

        if (i + 1) % 100 == 0 {
            info!("Generated {} checkpoints, {tx_count} transactions", i + 1);
        }
    }

    info!(
        "Synthetic ingestion generation completed: {num_checkpoints} checkpoints generated (excluding genesis and gas funding), {tx_count} transactions in {:.2?}.",
        timer.elapsed()
    );

    Ok(())
}

/// Reads serialized, synthetic checkpoint data from disk into memory.
pub async fn read_ingestion_data(path: &PathBuf) -> anyhow::Result<BTreeMap<u64, CheckpointData>> {
    let mut data = BTreeMap::new();
    let mut dir = fs::read_dir(path).await?;
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        let bytes = fs::read(path).await?;
        let checkpoint_data: CheckpointData = Blob::from_bytes(&bytes)?;
        data.insert(
            checkpoint_data.checkpoint_summary.sequence_number,
            checkpoint_data,
        );
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use iota_storage::blob::Blob;
    use iota_types::full_checkpoint_content::CheckpointData;

    use crate::synthetic_ingestion::generate_ingestion;

    #[tokio::test]
    async fn test_ingestion_from_zero() {
        let ingestion_tempdir = tempfile::tempdir().unwrap();
        let ingestion_dir = ingestion_tempdir.path().to_path_buf();

        let config = super::Config {
            ingestion_dir: ingestion_dir.clone(),
            starting_checkpoint: 0,
            num_checkpoints: 10,
            checkpoint_size: 2,
        };

        generate_ingestion(config).await.unwrap();

        // Check for the genesis checkpoint (0.chk)
        check_checkpoint_data(&ingestion_dir, 0, 1, 1).await;
        // Check for the gas funding checkpoint (1.chk)
        check_checkpoint_data(&ingestion_dir, 1, 1, 1).await;
        // Rest of the checkpoints
        check_checkpoint_data(&ingestion_dir, 2, 10, 2).await;
    }

    #[tokio::test]
    async fn test_ingestion_from_non_zero() {
        let ingestion_tempdir = tempfile::tempdir().unwrap();
        let ingestion_dir = ingestion_tempdir.path().to_path_buf();

        let config = super::Config {
            ingestion_dir: ingestion_dir.clone(),
            starting_checkpoint: 10,
            num_checkpoints: 10,
            checkpoint_size: 2,
        };

        generate_ingestion(config).await.unwrap();
        check_checkpoint_data(&ingestion_dir, 10, 10, 2).await;
    }

    async fn check_checkpoint_data(
        ingestion_dir: &Path,
        first_checkpoint: u64,
        num_checkpoints: u64,
        checkpoint_size: u64,
    ) {
        for checkpoint in first_checkpoint..first_checkpoint + num_checkpoints {
            let path = ingestion_dir.join(format!("{checkpoint}.chk"));
            let bytes = tokio::fs::read(&path).await.unwrap();
            let checkpoint_data: CheckpointData = Blob::from_bytes(&bytes).unwrap();

            assert_eq!(
                checkpoint_data.checkpoint_summary.sequence_number,
                checkpoint
            );
            assert_eq!(checkpoint_data.transactions.len(), checkpoint_size as usize);
        }
    }
}
