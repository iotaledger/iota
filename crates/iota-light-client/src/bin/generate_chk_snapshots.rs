// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fs;

use iota_light_client::utils::{
    CheckpointsList, Config, read_checkpoint_list, sync_checkpoint_list_to_latest,
};
use iota_rest_api::Client;

#[tokio::main]
pub async fn main() {
    let config: Config = serde_yaml::from_reader(
        &fs::File::open(format!(
            "{}/example_config/light_client.yaml",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    sync_checkpoint_list_to_latest(&config).await.unwrap();
    let checkpoints_list: CheckpointsList = read_checkpoint_list(&config).unwrap();

    let client = Client::new(format!("{}/rest", config.full_node_url));
    for ckp in checkpoints_list.checkpoints {
        let summary = client.get_checkpoint_summary(ckp).await.unwrap();
        serde_json::to_writer_pretty(
            &mut fs::File::create(format!(
                "{}/example_config/{ckp}.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
            &summary,
        )
        .unwrap();
        let full = client.get_full_checkpoint(ckp).await.unwrap();
        serde_json::to_writer_pretty(
            &mut fs::File::create(format!(
                "{}/example_config/{ckp}_full.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
            &full,
        )
        .unwrap();
        bcs::serialize_into(
            &mut fs::File::create(format!(
                "{}/example_config/{ckp}.chk",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
            &full,
        )
        .unwrap();
    }
}
