// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::PathBuf};

use iota_config::genesis::Genesis;
use iota_light_client::{
    checkpoint::{CheckpointsList, read_checkpoint_list, sync_checkpoint_list_to_latest},
    config::Config,
};
use iota_rest_api::Client;
use tracing::info;

const TEST_FILES_DIR: &str = "test_files";

#[tokio::main]
pub async fn main() {
    env_logger::init();

    let mut config = Config::default();

    config.checkpoint_summary_dir =
        PathBuf::from(format!("{}/{TEST_FILES_DIR}", env!("CARGO_MANIFEST_DIR")));
    config.full_node_url = "http://localhost:9000".to_string();
    config.graphql_url = Some("http://localhost:8000".to_string());
    config.genesis_filename = "/home/me/vps/.iota/iota_config/genesis.blob".to_string();

    // println!("{config:#?}");

    sync_checkpoint_list_to_latest(&config).await.unwrap();

    // let mut genesis_path = config.checkpoint_summary_dir.clone();
    // genesis_path.push(&config.genesis_filename);
    // let genesis_committee =
    // Genesis::load(&genesis_path).unwrap().committee().unwrap();
    // println!("{genesis_committee:#?}");

    let checkpoints_list: CheckpointsList = read_checkpoint_list(&config).unwrap();

    let client = Client::new(format!("{}/rest", config.full_node_url));
    for ckp in checkpoints_list.checkpoints() {
        println!("{ckp}");
        let summary = client.get_checkpoint_summary(*ckp).await.unwrap();

        serde_json::to_writer_pretty(
            &mut fs::File::create(format!(
                "{}/test_files/{ckp}.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
            &summary,
        )
        .unwrap();

        // let full = client.get_full_checkpoint(*ckp).await.unwrap();
        // serde_json::to_writer_pretty(
        //     &mut fs::File::create(format!(
        //         "{}/test_files/{ckp}_full.json",
        //         env!("CARGO_MANIFEST_DIR")
        //     ))
        //     .unwrap(),
        //     &full,
        // )
        // .unwrap();

        // bcs::serialize_into(
        //     &mut fs::File::create(format!(
        //         "{}/test_files/{ckp}.chk",
        //         env!("CARGO_MANIFEST_DIR")
        //     ))
        //     .unwrap(),
        //     &full,
        // )
        // .unwrap();
    }
}
