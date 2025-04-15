// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet, VecDeque},
    iter::repeat,
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use async_trait::async_trait;
use aws_config::{BehaviorVersion, timeout::TimeoutConfig};
use aws_sdk_dynamodb::{
    Client,
    config::{Credentials, Region},
    primitives::Blob,
    types::{AttributeValue, PutRequest, WriteRequest},
};
use backoff::{ExponentialBackoff, backoff::Backoff};
use bytes::Bytes;
use iota_config::object_storage_config::ObjectStoreConfig;
use iota_data_ingestion_core::Worker;
use iota_storage::http_key_value_store::{ItemType, TaggedKey};
use iota_types::{full_checkpoint_content::CheckpointData, storage::ObjectKey};
use object_store::{DynObjectStore, path::Path};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct KVStoreTaskConfig {
    pub object_store_config: ObjectStoreConfig,
    pub dynamodb_config: DynamoDBConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct DynamoDBConfig {
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub aws_region: String,
    pub aws_endpoint: Option<String>,
    pub table_name: String,
}

#[derive(Clone)]
pub struct KVStoreWorker {
    dynamo_client: Client,
    remote_store: Arc<DynObjectStore>,
    table_name: String,
}

impl KVStoreWorker {
    pub async fn new(config: KVStoreTaskConfig) -> anyhow::Result<Self> {
        let credentials = Credentials::new(
            &config.dynamodb_config.aws_access_key_id,
            &config.dynamodb_config.aws_secret_access_key,
            None,
            None,
            "dynamodb",
        );
        let timeout_config = TimeoutConfig::builder()
            .operation_timeout(Duration::from_secs(3))
            .operation_attempt_timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(3))
            .build();

        let mut aws_config_loader = aws_config::defaults(BehaviorVersion::latest())
            .credentials_provider(credentials)
            .region(Region::new(config.dynamodb_config.aws_region))
            .timeout_config(timeout_config);

        if let Some(url) = config.dynamodb_config.aws_endpoint {
            aws_config_loader = aws_config_loader.endpoint_url(url);
        }
        let aws_config = aws_config_loader.load().await;
        Ok(Self {
            dynamo_client: Client::new(&aws_config),
            remote_store: config.object_store_config.make()?,
            table_name: config.dynamodb_config.table_name,
        })
    }

    async fn multi_set<V: Serialize>(
        &self,
        item_type: ItemType,
        values: impl IntoIterator<Item = (Vec<u8>, V)> + std::marker::Send,
    ) -> anyhow::Result<()> {
        let mut items = vec![];
        let mut seen = HashSet::new();
        for (digest, value) in values {
            if seen.contains(&digest) {
                continue;
            }
            seen.insert(digest.clone());
            let item = WriteRequest::builder()
                .set_put_request(Some(
                    PutRequest::builder()
                        .item("digest", AttributeValue::B(Blob::new(digest)))
                        .item("type", AttributeValue::S(item_type.to_string()))
                        .item(
                            "bcs",
                            AttributeValue::B(Blob::new(bcs::to_bytes(value.borrow())?)),
                        )
                        .build()?,
                ))
                .build();
            items.push(item);
        }
        if items.is_empty() {
            return Ok(());
        }
        let mut backoff = ExponentialBackoff::default();
        let mut queue: VecDeque<Vec<_>> = items.chunks(25).map(|ck| ck.to_vec()).collect();
        while let Some(chunk) = queue.pop_front() {
            let response = self
                .dynamo_client
                .batch_write_item()
                .set_request_items(Some(HashMap::from([(
                    self.table_name.clone(),
                    chunk.to_vec(),
                )])))
                .send()
                .await
                .inspect_err(|sdk_err| {
                    error!(
                        "{:?}",
                        sdk_err.as_service_error().map(|e| e.meta().to_string())
                    )
                })?;
            if let Some(response) = response.unprocessed_items {
                if let Some(unprocessed) = response.into_iter().next() {
                    if !unprocessed.1.is_empty() {
                        if queue.is_empty() {
                            if let Some(duration) = backoff.next_backoff() {
                                tokio::time::sleep(duration).await;
                            }
                        }
                        queue.push_back(unprocessed.1);
                    }
                }
            }
        }
        Ok(())
    }

    /// Uploads checkpoint contents to storage, with automatic fallback from
    /// DynamoDB to S3.
    ///
    /// This function attempts to store checkpoint contents in DynamoDB first.
    /// If that fails (typically due to size constraints), it automatically
    /// falls back to uploading the contents to S3 instead.
    async fn upload_checkpoint_contents<V: Serialize + std::marker::Send>(
        &self,
        key: Vec<u8>,
        value: V,
    ) -> anyhow::Result<()> {
        let bcs_bytes = bcs::to_bytes(value.borrow())?;

        let attributes = HashMap::from([
            (
                "digest".to_string(),
                AttributeValue::B(Blob::new(key.clone())),
            ),
            (
                "type".to_string(),
                AttributeValue::S(ItemType::CheckpointContents.to_string()),
            ),
            (
                "bcs".to_string(),
                AttributeValue::B(Blob::new(bcs_bytes.clone())),
            ),
        ]);

        let res = self
            .dynamo_client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(attributes))
            .send()
            .await
            .inspect_err(|err| warn!("dynamodb error: {err}"));

        if res.is_err() {
            info!("attempt to store chekpoint contents on S3");
            let location = Path::from(base64_url::encode(&key));
            self.remote_store
                .put(&location, Bytes::from(bcs_bytes).into())
                .await?;
        }

        Ok(())
    }
}

#[async_trait]
impl Worker for KVStoreWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error> {
        let mut transactions = vec![];
        let mut effects = vec![];
        let mut events = vec![];
        let mut objects = vec![];
        let mut transactions_to_checkpoint = vec![];
        let checkpoint_number = checkpoint.checkpoint_summary.sequence_number;

        for transaction in &checkpoint.transactions {
            let transaction_digest = transaction.transaction.digest().into_inner().to_vec();
            effects.push((transaction_digest.clone(), transaction.effects.clone()));
            transactions_to_checkpoint.push((transaction_digest.clone(), checkpoint_number));
            transactions.push((transaction_digest.clone(), transaction.transaction.clone()));

            if let Some(tx_events) = &transaction.events {
                events.push((tx_events.digest().into_inner().to_vec(), tx_events));
            }
            for object in &transaction.output_objects {
                let object_key = ObjectKey(object.id(), object.version());
                objects.push((bcs::to_bytes(&object_key)?, object));
            }
        }
        self.multi_set(ItemType::Transaction, transactions).await?;
        self.multi_set(ItemType::TransactionEffects, effects)
            .await?;
        self.multi_set(ItemType::EventTransactionDigest, events)
            .await?;
        self.multi_set(ItemType::Object, objects).await?;
        self.multi_set(
            ItemType::TransactionToCheckpoint,
            transactions_to_checkpoint,
        )
        .await?;

        let serialized_checkpoint_number =
            bcs::to_bytes(&TaggedKey::CheckpointSequenceNumber(checkpoint_number))?;
        let checkpoint_summary = &checkpoint.checkpoint_summary;

        self.upload_checkpoint_contents(
            serialized_checkpoint_number.clone(),
            checkpoint.checkpoint_contents.clone(),
        )
        .await?;

        self.multi_set(
            ItemType::CheckpointSummary,
            [
                serialized_checkpoint_number,
                checkpoint_summary.digest().into_inner().to_vec(),
            ]
            .into_iter()
            .zip(repeat(checkpoint_summary)),
        )
        .await?;
        Ok(())
    }
}
