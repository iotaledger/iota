// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, time::Duration};

use anyhow::{Result, anyhow, bail};
use iota_sdk::{
    IotaClient,
    rpc_types::IotaObjectDataOptions,
    types::base_types::{IotaAddress, ObjectID},
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct FaucetResponse {
    task: String,
    error: Option<String>,
}

pub async fn request_tokens(client: &IotaClient, address: IotaAddress) -> Result<()> {
    let address_str = address.to_string();
    let reqwest_client = Client::new();
    let body = json!({ "FixedAmountRequest": { "recipient": &address_str } });

    let response = reqwest_client
        .post("http://127.0.0.1:9123/v1/gas")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        bail!("Faucet request failed with status {}", response.status());
    }

    let FaucetResponse { task, error } = response.json().await?;
    if let Some(err) = error {
        bail!("Faucet request error: {}", err);
    }

    wait_for_faucet_completion(client, &reqwest_client, &task, &address).await
}

async fn wait_for_faucet_completion(
    client: &IotaClient,
    reqwest_client: &Client,
    task_id: &str,
    expected_owner: &IotaAddress,
) -> Result<()> {
    let coin_id = loop {
        let response = reqwest_client
            .get(format!("http://127.0.0.1:9123/v1/status/{task_id}"))
            .send()
            .await?
            .text()
            .await?;

        if response.contains("SUCCEEDED") {
            let json: serde_json::Value = serde_json::from_str(&response)?;
            let id = json
                .pointer("/status/transferred_gas_objects/sent/0/id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Failed to parse coin ID from faucet response"))?;
            break id.to_string();
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    };

    let object_id = IotaObjectDataOptions::new().with_owner();
    loop {
        let object = client
            .read_api()
            .get_object_with_options(ObjectID::from_str(&coin_id)?, object_id.clone())
            .await?;

        if let Some(owner) = object.owner() {
            if owner.get_owner_address()? == *expected_owner {
                break;
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
