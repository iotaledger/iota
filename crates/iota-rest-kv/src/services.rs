// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes services for interacting with the kv store

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use aws_config::{BehaviorVersion, Region, timeout::TimeoutConfig};
use aws_sdk_dynamodb::{Client, config::Credentials, primitives::Blob, types::AttributeValue};
use bytes::Bytes;
use iota_config::object_storage_config::ObjectStoreConfig;
use iota_storage::http_key_value_store::{Key, TaggedKey};
use iota_types::storage::ObjectKey;
use object_store::{DynObjectStore, path::Path};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::types::ItemType;

const OPERATION_TIMEOUT_SECS: Duration = Duration::from_secs(3);
const OPERATION_ATTEMPT_TIMEOUT_SECS: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT_SECS: Duration = Duration::from_secs(3);
const AWS_STATUS_CACHE_TTL: Duration = Duration::from_secs(5);

/// Configuration for the storage API service that reads data from both S3
/// compatible bucket and DynamoDB.
///
/// This configuration combines settings for both object storage (S3) and
/// DynamoDB access, matching the storage locations used by the `KVStoreWorker`
/// in the `iota-data-ingestion` crate.
///
/// The service uses:
/// - S3 for checkpoint-related data (contents)
/// - DynamoDB
///     - transaction: transactions, effects, events, objects
///     - checkpoint: summaries
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct KvStoreConfig {
    pub object_store_config: ObjectStoreConfig,
    pub dynamo_db_config: DynamoDbConfig,
}

/// Configuration for DynamoDB connection used to access data written by the
/// ingestion pipeline's KVStore.
///
/// This configuration matches the AWS resources used by the `KVStoreWorker` in
/// the `iota-data-ingestion` crate, allowing the API service to read the stored
/// data.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct DynamoDbConfig {
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    /// Useful for local testing eg. (localstack)
    pub aws_endpoint: Option<String>,
    pub aws_region: String,
    pub table_name: String,
}

/// Represents the status of AWS components the [KvStoreService] relies on
#[derive(Debug, Serialize, Clone)]
pub struct AwsStatus {
    pub dynamodb: ServiceStatus,
    pub s3: ServiceStatus,
}

/// A simple representation of service status, (e.g. the status of AWS
/// components)
#[derive(Debug, Serialize, Clone)]
pub struct ServiceStatus {
    status: String,
    latency_ms: u64,
}

/// A cached status representation to avoid overwhelming the AWS components with
/// requests
#[derive(Debug)]
pub struct CachedAwsStatus {
    status: AwsStatus,
    cached_at: Instant,
}

/// A read-only service for retrieving data from DynamoDB and S3 compatible
/// remote store backends used by the ingestion pipeline.
///
/// This service provides read access to data written by the `KVStoreWorker` in
/// the `iota-data-ingestion` crate:
/// - S3 for checkpoint-related data (contents)
/// - DynamoDB
///     - transaction: transactions, effects, events, objects
///     - checkpoint: summaries
///
/// The service acts as a read-only interface, supporting the HTTP fallback
/// mechanism implemented by
/// [HttpKVStore](iota_storage::http_key_value_store::HttpKVStore).
#[derive(Debug, Clone)]
pub struct KvStoreService {
    /// DynamoDb client
    dynamo_db_client: Client,
    /// S3 compatible bucket client
    remote_store: Arc<DynObjectStore>,
    /// DynamoDb table name
    table_name: String,
    /// The representation of the uptime of the service
    start_time: Instant,
    /// Cached AWS components sttaus
    cached_status: Arc<RwLock<Option<CachedAwsStatus>>>,
    /// The TTL of the [CachedAwsStatus]
    cache_duration: Duration,
}

impl KvStoreService {
    /// Create a new instance of the service
    ///
    /// Internally it instantiates a DynamoDb Client and an S3 compatible bucket
    /// Client
    pub async fn new(config: KvStoreConfig) -> Result<Self> {
        let dynamodb_config = config.dynamo_db_config;

        let credentials = Credentials::new(
            &dynamodb_config.aws_access_key_id,
            &dynamodb_config.aws_secret_access_key,
            None,
            None,
            "dynamodb",
        );
        let timeout_config = TimeoutConfig::builder()
            .operation_timeout(OPERATION_TIMEOUT_SECS)
            .operation_attempt_timeout(OPERATION_ATTEMPT_TIMEOUT_SECS)
            .connect_timeout(CONNECT_TIMEOUT_SECS)
            .build();
        let mut aws_config_loader = aws_config::defaults(BehaviorVersion::latest())
            .credentials_provider(credentials)
            .region(Region::new(dynamodb_config.aws_region))
            .timeout_config(timeout_config);

        if let Some(url) = dynamodb_config.aws_endpoint {
            aws_config_loader = aws_config_loader.endpoint_url(url);
        }
        let aws_config = aws_config_loader.load().await;
        let dynamo_db_client = Client::new(&aws_config);
        let remote_store = config.object_store_config.make()?;

        Ok(Self {
            dynamo_db_client,
            remote_store,
            table_name: dynamodb_config.table_name,
            start_time: Instant::now(),
            cache_duration: AWS_STATUS_CACHE_TTL,
            cached_status: Arc::new(RwLock::new(None)),
        })
    }

    /// Get the elapsed time from which the service was instantiated
    pub fn get_uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    async fn check_dynamodb_health(&self) -> ServiceStatus {
        let start = Instant::now();

        let status = match self
            .dynamo_db_client
            .describe_table()
            .table_name(&self.table_name)
            .send()
            .await
        {
            Ok(_) => "ok".into(),
            Err(err) => {
                tracing::error!("failed describing dynamodb table: {err}");
                "not ok".into()
            }
        };

        ServiceStatus {
            status,
            latency_ms: start.elapsed().as_millis() as u64,
        }
    }

    async fn check_s3_health(&self) -> ServiceStatus {
        let start = Instant::now();

        // Just check if we can access the bucket by trying to get a non-existent key
        let test_path = Path::from("health-check-test");
        let status = match self.remote_store.head(&test_path).await {
            Ok(_) => "ok".into(),
            Err(object_store::Error::NotFound { .. }) => "ok".to_string(), // Not found is OK
            Err(err) => {
                tracing::error!("failed checking file metadata on S3: {err}");
                "not ok".into()
            }
        };

        ServiceStatus {
            status,
            latency_ms: start.elapsed().as_millis() as u64,
        }
    }

    async fn check_aws_health(&self) -> AwsStatus {
        AwsStatus {
            dynamodb: self.check_dynamodb_health().await,
            s3: self.check_s3_health().await,
        }
    }

    /// Get AWS service status
    pub async fn get_aws_health(&self) -> AwsStatus {
        // Read lock for checking cache status
        let should_refresh = {
            let cached = self.cached_status.read().await;
            cached.is_none()
                || cached
                    .as_ref()
                    .map(|a| a.cached_at.elapsed() > self.cache_duration)
                    .unwrap_or(true)
        };

        if should_refresh {
            let new_status = self.check_aws_health().await;
            // Write lock only when updating
            let mut cached = self.cached_status.write().await;
            *cached = Some(CachedAwsStatus {
                status: new_status.clone(),
                cached_at: Instant::now(),
            });

            return new_status;
        }

        // Read lock for getting cached value
        if let Some(cached) = self.cached_status.read().await.as_ref() {
            cached.status.clone()
        } else {
            // Cache was cleared between our check and here, get fresh status
            self.check_aws_health().await
        }
    }

    /// Get value as [Bytes] from DynamoDb
    async fn get_from_dynamodb<T: AsRef<[u8]>>(
        &self,
        digest: T,
        item_type: ItemType,
    ) -> Result<Option<Bytes>> {
        let result = self
            .dynamo_db_client
            .get_item()
            .table_name(&self.table_name)
            .key("digest", AttributeValue::B(Blob::new(digest.as_ref())))
            .key("type", AttributeValue::S(item_type.to_string()))
            .send()
            .await?;

        if let Some(item) = result.item {
            if let Some(AttributeValue::B(blob)) = item.get("bcs") {
                return Ok(Some(Bytes::copy_from_slice(blob.as_ref())));
            }
        }

        Ok(None)
    }

    /// Get value as [Bytes] from the S3 compatible bucket
    async fn get_from_remote_store<T: AsRef<[u8]>>(&self, digest: &T) -> Result<Option<Bytes>> {
        let path = Path::from(base64_url::encode(digest));

        // Get the object
        match self.remote_store.get(&path).await {
            Ok(response) => {
                // Get bytes from the response
                let data = response.bytes().await.map_err(|err| {
                    anyhow::anyhow!("Failed to read data from remote store: {err}")
                })?;

                Ok(Some(data))
            }
            Err(err) => {
                match err {
                    // Handle specific object_store errors
                    object_store::Error::NotFound { .. } => Ok(None),
                    _ => Err(anyhow::anyhow!("remote store error: {err}")),
                }
            }
        }
    }

    /// Get value as [Bytes] from the kv store
    ///
    /// Based on the provided `Key` it will fetch the data from DynamoDb or S3
    /// compatible buckets
    pub async fn get(&self, key: Key) -> Result<Option<Bytes>> {
        let item_type = key.into();

        match key {
            Key::Tx(transaction_digest) => {
                self.get_from_dynamodb(transaction_digest, item_type).await
            }
            Key::Fx(transaction_digest) => {
                self.get_from_dynamodb(transaction_digest, item_type).await
            }
            Key::CheckpointContents(chk_seq_num) => {
                let serialized_checkpoint_number =
                    bcs::to_bytes(&TaggedKey::CheckpointSequenceNumber(chk_seq_num))?;

                self.get_from_remote_store(&serialized_checkpoint_number)
                    .await
            }
            Key::CheckpointSummary(chk_seq_num) => {
                let serialized_checkpoint_number =
                    bcs::to_bytes(&TaggedKey::CheckpointSequenceNumber(chk_seq_num))?;

                self.get_from_dynamodb(serialized_checkpoint_number, item_type)
                    .await
            }
            Key::CheckpointSummaryByDigest(checkpoint_digest) => {
                self.get_from_dynamodb(checkpoint_digest, item_type).await
            }
            Key::TxToCheckpoint(transaction_digest) => {
                self.get_from_dynamodb(transaction_digest, item_type).await
            }
            Key::ObjectKey(object_id, sequence_number) => {
                let object_key = ObjectKey(object_id, sequence_number);
                let serialized_object_key = bcs::to_bytes(&object_key)?;
                self.get_from_dynamodb(serialized_object_key, item_type)
                    .await
            }
            Key::EventsByTxDigest(transaction_digest) => {
                self.get_from_dynamodb(transaction_digest, item_type).await
            }
        }
    }
}
