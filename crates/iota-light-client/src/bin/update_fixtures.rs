// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, ensure};
use iota_light_client::{
    checkpoint::{
        download_summaries_from_checkpoint_store, sync_checkpoint_list_to_latest_from_archive,
        write_checkpoint_list,
    },
    config::Config,
    object_store::CheckpointStore,
};
use iota_types::full_checkpoint_content::CheckpointData;
use tokio::fs::create_dir_all;
use tracing::info;

const FIXTURES_DIR: &str = "tests/fixtures";
// Determines which end-of-epoch checkpoints will be downloaded when running
// this binary. You should only ever add epochs to this list, but not remove.
const SELECTED_EPOCHS: &[usize] = &[0, 1];

#[tokio::main]
pub async fn main() -> Result<()> {
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_log_level("info")
        .with_env()
        .init();

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURES_DIR);
    if !path.exists() {
        create_dir_all(path.clone()).await?;
    }

    let mut config = Config::mainnet();
    config.checkpoints_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURES_DIR);
    config.validate()?;

    let mut checkpoint_list = sync_checkpoint_list_to_latest_from_archive(&config)
        .await
        .context("failed to sync checkpoints")?;

    // only keep necessary indexes
    let checkpoints = &mut checkpoint_list.checkpoints;
    let _ = checkpoints.split_off(*SELECTED_EPOCHS.last().unwrap() + 1);
    write_checkpoint_list(&config, &checkpoint_list)?;

    let checkpoints = SELECTED_EPOCHS
        .iter()
        .map(|i| *checkpoint_list.checkpoints.get(*i).unwrap())
        .collect::<Vec<_>>();

    ensure!(checkpoints.len() > 1, "not enough checkpoints");

    download_summaries_from_checkpoint_store(&config, checkpoints.clone()).await?;
    download_checkpoints_from_checkpoint_store(&config, checkpoints).await?;

    Ok(())
}

pub async fn download_checkpoints_from_checkpoint_store(
    config: &Config,
    checkpoints: Vec<u64>,
) -> Result<()> {
    info!("Downloading checkpoints from checkpoint store.");

    let checkpoint_store = CheckpointStore::new(config)?;
    for seq in checkpoints {
        info!("Downloading {seq}.chk");

        let checkpoint = checkpoint_store
            .fetch_full_checkpoint(seq)
            .await
            .context(format!(
                "Failed to download checkpoint '{seq}' from checkpoint store"
            ))?;
        write_full_checkpoint(config, &checkpoint)?;
    }

    Ok(())
}

pub fn write_full_checkpoint(config: &Config, checkpoint: &CheckpointData) -> Result<()> {
    let path = full_checkpoint_file_path(config, *checkpoint.checkpoint_summary.sequence_number());
    bcs::serialize_into(
        &mut std::fs::File::create(&path).context(format!(
            "error writing checkpoint file '{}'",
            path.display()
        ))?,
        &checkpoint,
    )
    .map_err(|_| anyhow!("error serializing to bcs"))?;
    Ok(())
}

pub fn full_checkpoint_file_path(config: &Config, seq: u64) -> PathBuf {
    Path::new(&config.checkpoints_dir).join(format!("{seq}.chk"))
}
