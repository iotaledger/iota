// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, path::PathBuf, str::FromStr};

use anyhow::Result;
use iota_replay::{execute_replay_command, ReplayToolCommand};

const LOCALNET_ADDR: &str = "http://127.0.0.1:9000";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/sandbox_snapshots");

    // Retrieve `tx_digest` from command-line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: <tx_digest>");
        return Ok(());
    }
    let tx_digest = &args[1]; // tx_digest passed as the first argument
    let localnet_addr = String::from_str(LOCALNET_ADDR).unwrap();

    let cmd = ReplayToolCommand::PersistSandbox {
        tx_digest: tx_digest.clone(),
        base_path: path,
    };

    execute_replay_command(Some(localnet_addr), true, true, None, cmd).await?;
    Ok(())
}
