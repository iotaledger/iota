// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Simple example that connects to a local IOTA node via gRPC and prints the
//! current epoch number.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p iota-grpc-client --example get_epoch
//! ```

use iota_grpc_client::{Client, ResponseExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::connect("http://localhost:50051").await?;

    let response = client.get_epoch(None, None).await?;
    let epoch = response.body();

    if let Some(number) = epoch.epoch {
        println!("Current epoch: {number}");
    }
    if let Some(gas_price) = epoch.reference_gas_price {
        println!("Reference gas price: {gas_price} NANOS");
    }
    if let Some(chain) = response.chain() {
        println!("Chain: {chain}");
    }
    if let Some(checkpoint) = response.checkpoint_height() {
        println!("Checkpoint height: {checkpoint}");
    }

    Ok(())
}
