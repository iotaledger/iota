// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use iota_config::genesis::Genesis;
use iota_light_client::{
    checkpoint::{CheckpointList, read_checkpoint_list, sync_checkpoint_list_to_latest},
    config::Config,
};
use iota_rest_api::Client;
use tracing::info;

const TEST_FILES_DIR: &str = "tests/fixtures";

#[tokio::main]
pub async fn main() {
    let mut config = Config::default();
    config.cache_dir = PathBuf::from(format!("{}/{TEST_FILES_DIR}", env!("CARGO_MANIFEST_DIR")));

    let checkpoint_list = sync_checkpoint_list_to_latest(&config)
        .await
        .expect("failed to sync checkpoint list");

    if checkpoint_list.len() < 2 {
        panic!("not enough checkpoints to sync")
    }

    let client = Client::new(format!("{}/rest", config.full_node_url));

    for (i, ckp) in checkpoint_list.checkpoints().iter().enumerate() {
        if i >= 2 {
            // We only need the first 2 end-of-epoch checkpoints for the tests
            break;
        }
        println!("Downloading full and summary checkpoint: {ckp}");

        let summary = client
            .get_checkpoint_summary(*ckp)
            .await
            .expect("error downloading checkpoint summary");

        let full = client
            .get_full_checkpoint(*ckp)
            .await
            .expect("error downloading full checkpoint");

        bcs::serialize_into(
            &mut fs::File::create(format!("{}/{ckp}.sum", config.cache_dir.display()))
                .expect("error creating file"),
            &summary,
        )
        .expect("error serializing summary checkpoint to bcs");

        bcs::serialize_into(
            &mut fs::File::create(format!("{}/{ckp}.chk", config.cache_dir.display()))
                .expect("error creating file"),
            &full,
        )
        .expect("error serializing full checkpoint to bcs");
    }
}
