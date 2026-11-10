// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rate-limited reads of historical objects from a JSON-RPC full node.

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use iota_json_rpc_types::{
    IotaObjectDataOptions, IotaPastObjectResponse, IotaTransactionBlockResponseOptions,
};
use iota_sdk::{IotaClient, IotaClientBuilder};
use iota_sdk_types::{ObjectId, TransactionDigest, Version};
use iota_types::object::Object;
use tokio::{
    sync::Mutex,
    time::{Instant, sleep_until},
};

/// A JSON-RPC client that spaces its requests evenly to stay under a request rate.
pub struct Rpc {
    client: IotaClient,
    interval: Duration,
    next_slot: Mutex<Instant>,
}

impl Rpc {
    pub async fn new(url: &str, requests_per_second: u32) -> Result<Self> {
        let client = IotaClientBuilder::default()
            .build(url)
            .await
            .with_context(|| format!("connecting to {url}"))?;
        Ok(Self {
            client,
            interval: Duration::from_secs(1) / requests_per_second.max(1),
            next_slot: Mutex::new(Instant::now()),
        })
    }

    /// Waits until the next request may be sent.
    async fn wait_for_slot(&self) {
        let mut next_slot = self.next_slot.lock().await;
        let now = Instant::now();
        if *next_slot > now {
            sleep_until(*next_slot).await;
        }
        *next_slot = (*next_slot).max(now) + self.interval;
    }

    /// The object at exactly `version`, or `None` if the node does not have that version.
    pub async fn object_at_version(
        &self,
        id: ObjectId,
        version: Version,
    ) -> Result<Option<Object>> {
        self.wait_for_slot().await;
        let response = self
            .client
            .read_api()
            .try_get_parsed_past_object(id, version, IotaObjectDataOptions::bcs_lossless())
            .await
            .with_context(|| format!("reading object {id} at version {version}"))?;
        past_object(response)
    }

    /// The latest version of the object that is at most `version`.
    pub async fn object_at_or_before_version(
        &self,
        id: ObjectId,
        version: Version,
    ) -> Result<Option<Object>> {
        self.wait_for_slot().await;
        let response = self
            .client
            .read_api()
            .try_get_object_before_version(id, version)
            .await
            .with_context(|| format!("reading object {id} at or before version {version}"))?;
        past_object(response)
    }

    /// The latest version of the object.
    pub async fn latest_object(&self, id: ObjectId) -> Result<Option<Object>> {
        self.wait_for_slot().await;
        let response = self
            .client
            .read_api()
            .get_object_with_options(id, IotaObjectDataOptions::bcs_lossless())
            .await
            .with_context(|| format!("reading object {id}"))?;
        match response.data {
            Some(data) => Ok(Some(data.try_into().map_err(|e| anyhow!("{e}"))?)),
            None => Ok(None),
        }
    }

    /// The checkpoint that contains the transaction, if it is checkpointed.
    pub async fn transaction_checkpoint(&self, digest: TransactionDigest) -> Result<Option<u64>> {
        self.wait_for_slot().await;
        let response = self
            .client
            .read_api()
            .get_transaction_with_options(digest, IotaTransactionBlockResponseOptions::new())
            .await
            .with_context(|| format!("reading transaction {digest}"))?;
        Ok(response.checkpoint)
    }
}

fn past_object(response: IotaPastObjectResponse) -> Result<Option<Object>> {
    match response {
        IotaPastObjectResponse::VersionFound(data) => {
            Ok(Some(data.try_into().map_err(|e| anyhow!("{e}"))?))
        }
        IotaPastObjectResponse::ObjectDeleted(_)
        | IotaPastObjectResponse::ObjectNotExists(_)
        | IotaPastObjectResponse::VersionNotFound(..)
        | IotaPastObjectResponse::VersionTooHigh { .. } => Ok(None),
    }
}
