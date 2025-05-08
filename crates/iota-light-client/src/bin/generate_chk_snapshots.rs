// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::PathBuf};

use iota_light_client::{checkpoint::sync_checkpoint_list_to_latest, config::Config};
use iota_rest_api::Client;
use tracing::info;

const FIXTURES_DIR: &str = "tests/fixtures";

#[tokio::main]
pub async fn main() {
    env_logger::init();

    let mut config = Config::get_mainnet_config();
    config.checkpoints_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURES_DIR);

    let checkpoint_list = sync_checkpoint_list_to_latest(&config)
        .await
        .expect("failed to sync checkpoint list");

    if checkpoint_list.len() < 2 {
        panic!("not enough checkpoints to sync")
    }

    let client = Client::new(format!("{}/rest", config.rpc_url));

    // We only need the first 2 end-of-epoch checkpoints for the tests
    for seq in checkpoint_list.checkpoints().iter().copied().take(2) {
        info!("Downloading full and summary checkpoint: {seq}");

        let summary = client
            .get_checkpoint_summary(seq)
            .await
            .expect("error downloading checkpoint summary");

        let full = client
            .get_full_checkpoint(seq)
            .await
            .expect("error downloading full checkpoint");

        bcs::serialize_into(
            &mut fs::File::create(format!("{}/{seq}.sum", config.checkpoints_dir.display()))
                .expect("error creating file"),
            &summary,
        )
        .expect("error serializing summary checkpoint to bcs");

        bcs::serialize_into(
            &mut fs::File::create(format!("{}/{seq}.chk", config.checkpoints_dir.display()))
                .expect("error creating file"),
            &full,
        )
        .expect("error serializing full checkpoint to bcs");
    }
}
