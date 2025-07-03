// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use gcp_auth::{Token, TokenProvider};
use http::{HeaderValue, Request, Response};
use iota_types::{
    base_types::{ObjectID, TransactionDigest},
    digests::CheckpointDigest,
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{CheckpointSequenceNumber, CheckpointSummary},
    object::Object,
    storage::ObjectKey,
};
use prometheus::Registry;
use prost::Message;
use tonic::{
    Streaming,
    body::Body,
    codegen::Service,
    transport::{Certificate, Channel, ClientTlsConfig},
};
use tracing::error;

use crate::{
    Checkpoint, KeyValueStoreReader, KeyValueStoreWriter, TransactionData,
    bigtable::{
        metrics::KvMetrics,
        proto::bigtable::v2::{
            MutateRowsRequest, MutateRowsResponse, Mutation, ReadRowsRequest, RowFilter, RowRange,
            RowSet,
            bigtable_client::BigtableClient as BigtableInternalClient,
            mutate_rows_request::Entry,
            mutation::{self, SetCell},
            read_rows_response::cell_chunk::RowStatus,
            row_filter::Filter,
            row_range::EndKey,
        },
    },
};

/// BigTable GRPC Server max request size in bytes.
const GRPC_MAX_REQUEST_SIZE: usize = 250 * 1024 * 1024; // 250 MB

const OBJECTS_TABLE: &str = "objects";
const TRANSACTIONS_TABLE: &str = "transactions";
const CHECKPOINTS_TABLE: &str = "checkpoints";
const CHECKPOINTS_BY_DIGEST_TABLE: &str = "checkpoints_by_digest";
const WATERMARK_TABLE: &str = "watermark";

const COLUMN_FAMILY_NAME: &str = "iota";
const DEFAULT_COLUMN_QUALIFIER: &str = "";
const CHECKPOINT_SUMMARY_COLUMN_QUALIFIER: &str = "cs";
const CHECKPOINT_CONTENTS_COLUMN_QUALIFIER: &str = "cc";
const TRANSACTION_COLUMN_QUALIFIER: &str = "tx";
const EFFECTS_COLUMN_QUALIFIER: &str = "fx";
const EVENTS_COLUMN_QUALIFIER: &str = "evtx";
const TRANSACTION_TO_CHECKPOINT: &str = "tx2c";

type Bytes = Vec<u8>;

/// A high-level client for interacting with Google Bigtable using authenticated
/// requests.
///
/// `BigTableClient` provides convenient methods for reading and writing
/// objects, transactions, and checkpoints, while handling authentication,
/// metrics, and table name prefixing internally.
#[derive(Clone)]
pub struct BigTableClient {
    table_prefix: String,
    client: BigtableInternalClient<AuthChannel>,
    client_name: String,
    metrics: Option<Arc<KvMetrics>>,
}

#[async_trait]
impl KeyValueStoreWriter for BigTableClient {
    async fn save_objects(&mut self, objects: &[&Object]) -> Result<()> {
        let mut items = Vec::with_capacity(objects.len());
        for object in objects {
            let object_key = ObjectKey(object.id(), object.version());
            items.push((
                Self::raw_object_key(&object_key)?,
                vec![(DEFAULT_COLUMN_QUALIFIER, bcs::to_bytes(object)?)],
            ));
        }
        self.multi_set(OBJECTS_TABLE, items).await
    }

    async fn save_transactions(&mut self, transactions: &[TransactionData]) -> Result<()> {
        let mut items = Vec::with_capacity(transactions.len());
        for TransactionData {
            transaction,
            effects,
            events,
            checkpoint_number,
        } in transactions
        {
            let cells = vec![
                (TRANSACTION_COLUMN_QUALIFIER, bcs::to_bytes(transaction)?),
                (EFFECTS_COLUMN_QUALIFIER, bcs::to_bytes(effects)?),
                (EVENTS_COLUMN_QUALIFIER, bcs::to_bytes(events)?),
                (TRANSACTION_TO_CHECKPOINT, bcs::to_bytes(checkpoint_number)?),
            ];
            items.push((transaction.digest().inner().to_vec(), cells));
        }
        self.multi_set(TRANSACTIONS_TABLE, items).await
    }

    async fn save_checkpoint(&mut self, checkpoint: &CheckpointData) -> Result<()> {
        let summary = &checkpoint.checkpoint_summary;
        let contents = &checkpoint.checkpoint_contents;
        let key = summary.sequence_number.to_be_bytes().to_vec();
        let cells = vec![
            (CHECKPOINT_SUMMARY_COLUMN_QUALIFIER, bcs::to_bytes(summary)?),
            (
                CHECKPOINT_CONTENTS_COLUMN_QUALIFIER,
                bcs::to_bytes(contents)?,
            ),
        ];
        self.multi_set(CHECKPOINTS_TABLE, [(key.clone(), cells)])
            .await?;
        self.multi_set(
            CHECKPOINTS_BY_DIGEST_TABLE,
            [(
                checkpoint.checkpoint_summary.digest().inner().to_vec(),
                vec![(DEFAULT_COLUMN_QUALIFIER, key)],
            )],
        )
        .await
    }

    async fn save_watermark(&mut self, watermark: CheckpointSequenceNumber) -> Result<()> {
        let key = watermark.to_be_bytes().to_vec();
        self.multi_set(
            WATERMARK_TABLE,
            [(key, vec![(DEFAULT_COLUMN_QUALIFIER, vec![])])],
        )
        .await
    }
}

#[async_trait]
impl KeyValueStoreReader for BigTableClient {
    async fn get_objects(&mut self, object_keys: &[ObjectKey]) -> Result<Vec<Object>> {
        let keys: Result<_, _> = object_keys.iter().map(Self::raw_object_key).collect();
        let mut objects = vec![];
        for row in self.multi_get(OBJECTS_TABLE, keys?, None).await? {
            for (_, value) in row {
                objects.push(bcs::from_bytes(&value)?);
            }
        }
        Ok(objects)
    }

    async fn get_transactions(
        &mut self,
        transactions: &[TransactionDigest],
    ) -> Result<Vec<TransactionData>> {
        let keys = transactions.iter().map(|tx| tx.inner().to_vec()).collect();
        let mut result = vec![];
        for row in self.multi_get(TRANSACTIONS_TABLE, keys, None).await? {
            let mut transaction = None;
            let mut effects = None;
            let mut events = None;
            let mut checkpoint_number = 0;

            for (column, value) in row {
                match std::str::from_utf8(&column)? {
                    TRANSACTION_COLUMN_QUALIFIER => transaction = Some(bcs::from_bytes(&value)?),
                    EFFECTS_COLUMN_QUALIFIER => effects = Some(bcs::from_bytes(&value)?),
                    EVENTS_COLUMN_QUALIFIER => events = Some(bcs::from_bytes(&value)?),
                    TRANSACTION_TO_CHECKPOINT => checkpoint_number = bcs::from_bytes(&value)?,
                    _ => error!("unexpected column {column:?} in transactions table"),
                }
            }
            result.push(TransactionData {
                transaction: transaction.ok_or_else(|| anyhow!("transaction field is missing"))?,
                effects: effects.ok_or_else(|| anyhow!("effects field is missing"))?,
                events: events.ok_or_else(|| anyhow!("events field is missing"))?,
                checkpoint_number,
            })
        }
        Ok(result)
    }

    async fn get_checkpoints(
        &mut self,
        sequence_numbers: &[CheckpointSequenceNumber],
    ) -> Result<Vec<Checkpoint>> {
        let keys = sequence_numbers
            .iter()
            .map(|sq| sq.to_be_bytes().to_vec())
            .collect();
        let mut checkpoints = vec![];
        for row in self.multi_get(CHECKPOINTS_TABLE, keys, None).await? {
            let mut summary = None;
            let mut contents = None;
            for (column, value) in row {
                match std::str::from_utf8(&column)? {
                    CHECKPOINT_SUMMARY_COLUMN_QUALIFIER => summary = Some(bcs::from_bytes(&value)?),
                    CHECKPOINT_CONTENTS_COLUMN_QUALIFIER => {
                        contents = Some(bcs::from_bytes(&value)?)
                    }
                    _ => error!("unexpected column {column:?} in checkpoints table"),
                }
            }
            let checkpoint = Checkpoint {
                summary: summary.ok_or_else(|| anyhow!("summary field is missing"))?,
                contents: contents.ok_or_else(|| anyhow!("contents field is missing"))?,
            };
            checkpoints.push(checkpoint);
        }
        Ok(checkpoints)
    }

    async fn get_checkpoint_by_digest(
        &mut self,
        digest: CheckpointDigest,
    ) -> Result<Option<Checkpoint>> {
        let key = digest.inner().to_vec();
        let mut response = self
            .multi_get(CHECKPOINTS_BY_DIGEST_TABLE, vec![key], None)
            .await?;
        if let Some(row) = response.pop() {
            if let Some((_, value)) = row.into_iter().next() {
                let sequence_number = u64::from_be_bytes(value.as_slice().try_into()?);
                if let Some(chk) = self.get_checkpoints(&[sequence_number]).await?.pop() {
                    return Ok(Some(chk));
                }
            }
        }
        Ok(None)
    }

    async fn get_latest_checkpoint(&mut self) -> Result<CheckpointSequenceNumber> {
        let upper_limit = u64::MAX.to_be_bytes().to_vec();
        match self
            .reversed_scan(WATERMARK_TABLE, upper_limit)
            .await?
            .pop()
        {
            Some((key_bytes, _)) => Ok(u64::from_be_bytes(key_bytes.as_slice().try_into()?)),
            None => Ok(0),
        }
    }

    async fn get_latest_checkpoint_summary(&mut self) -> Result<Option<CheckpointSummary>> {
        let sequence_number = self.get_latest_checkpoint().await?;
        if sequence_number == 0 {
            return Ok(None);
        }

        // Fetch just the summary for the latest checkpoint sequence number.
        let mut response = self
            .multi_get(
                CHECKPOINTS_TABLE,
                vec![(sequence_number - 1).to_be_bytes().to_vec()],
                Some(RowFilter {
                    filter: Some(Filter::ColumnQualifierRegexFilter(
                        CHECKPOINT_SUMMARY_COLUMN_QUALIFIER.into(),
                    )),
                }),
            )
            .await?;

        let Some(row) = response.pop() else {
            return Ok(None);
        };

        let mut summary: Option<CheckpointSummary> = None;
        for (column, value) in row {
            match std::str::from_utf8(&column)? {
                CHECKPOINT_SUMMARY_COLUMN_QUALIFIER => summary = Some(bcs::from_bytes(&value)?),
                _ => error!("unexpected column {:?} in checkpoints table", column),
            }
        }

        Ok(summary)
    }

    async fn get_latest_object(&mut self, object_id: &ObjectID) -> Result<Option<Object>> {
        let upper_limit = Self::raw_object_key(&ObjectKey::max_for_id(object_id))?;
        if let Some((_, row)) = self.reversed_scan(OBJECTS_TABLE, upper_limit).await?.pop() {
            if let Some((_, value)) = row.into_iter().next() {
                return Ok(Some(bcs::from_bytes(&value)?));
            }
        }
        Ok(None)
    }
}

impl BigTableClient {
    /// Creates a new BigTableClient instance for a local instance using the
    /// emulator feature. It reads the emulator host from the
    /// `BIGTABLE_EMULATOR_HOST` environment variable.
    pub async fn new_local(instance_id: impl AsRef<str>) -> Result<Self> {
        let emulator_host = std::env::var("BIGTABLE_EMULATOR_HOST")?;
        let channel = Channel::from_shared(format!("http://{emulator_host}"))?.connect_lazy();
        let policy = "https://www.googleapis.com/auth/bigtable.data";
        let auth_channel = AuthChannel::new_localhost(channel, policy);
        Ok(Self {
            table_prefix: format!(
                "projects/emulator/instances/{}/tables/",
                instance_id.as_ref()
            ),
            client: BigtableInternalClient::new(auth_channel),
            client_name: "local".to_string(),
            metrics: None,
        })
    }

    /// Creates a new BigTableClient instance for a remote instance. It checks
    /// for the `GOOGLE_APPLICATION_CREDENTIALS` environment variable.
    pub async fn new_remote(
        instance_id: impl AsRef<str>,
        is_read_only: bool,
        timeout: Option<Duration>,
        client_name: impl Into<String>,
        registry: Option<&Registry>,
    ) -> Result<Self> {
        let policy = if is_read_only {
            "https://www.googleapis.com/auth/bigtable.data.readonly"
        } else {
            "https://www.googleapis.com/auth/bigtable.data"
        };
        let token_provider = gcp_auth::provider().await?;
        let tls_config = ClientTlsConfig::new()
            .ca_certificate(Certificate::from_pem(include_bytes!("./certs/google.pem")))
            .domain_name("bigtable.googleapis.com");
        let mut endpoint = Channel::from_static("https://bigtable.googleapis.com")
            .http2_keep_alive_interval(Duration::from_secs(60))
            .keep_alive_while_idle(true)
            .tls_config(tls_config)?;
        if let Some(timeout) = timeout {
            endpoint = endpoint.timeout(timeout);
        }
        let table_prefix = format!(
            "projects/{}/instances/{}/tables/",
            token_provider.project_id().await?,
            instance_id.as_ref()
        );
        let auth_channel = AuthChannel::new_remote(endpoint.connect_lazy(), policy, token_provider);
        Ok(Self {
            table_prefix,
            client: BigtableInternalClient::new(auth_channel),
            client_name: client_name.into(),
            metrics: registry.map(KvMetrics::new),
        })
    }

    fn table_name(&self, table_name: &str) -> String {
        format!("{}{table_name}", self.table_prefix)
    }

    pub async fn mutate_rows(
        &mut self,
        request: MutateRowsRequest,
    ) -> Result<Streaming<MutateRowsResponse>> {
        Ok(self.client.mutate_rows(request).await?.into_inner())
    }

    pub async fn read_rows(
        &mut self,
        request: ReadRowsRequest,
    ) -> Result<Vec<(Vec<u8>, Vec<(Vec<u8>, Vec<u8>)>)>> {
        let mut result = vec![];
        let mut response = self.client.read_rows(request).await?.into_inner();

        let mut row_key = None;
        let mut row = vec![];
        let mut cell_value = vec![];
        let mut cell_name = None;
        let mut timestamp = 0;

        while let Some(message) = response.message().await? {
            for mut chunk in message.chunks.into_iter() {
                // new row check
                if !chunk.row_key.is_empty() {
                    row_key = Some(chunk.row_key);
                }
                match chunk.qualifier {
                    // new cell started
                    Some(qualifier) => {
                        if let Some(cell_name) = cell_name {
                            row.push((cell_name, cell_value));
                            cell_value = vec![];
                        }
                        cell_name = Some(qualifier.value);
                        timestamp = chunk.timestamp_micros;
                        cell_value.append(&mut chunk.value);
                    }
                    None => {
                        if chunk.timestamp_micros == 0 {
                            cell_value.append(&mut chunk.value);
                        } else if chunk.timestamp_micros >= timestamp {
                            // newer version of cell is available
                            timestamp = chunk.timestamp_micros;
                            cell_value = chunk.value;
                        }
                    }
                }
                if chunk.row_status.is_some() {
                    if let Some(RowStatus::CommitRow(_)) = chunk.row_status {
                        if let Some(cell_name) = cell_name {
                            row.push((cell_name, cell_value));
                        }
                        if let Some(row_key) = row_key {
                            result.push((row_key, row));
                        }
                    }
                    row_key = None;
                    row = vec![];
                    cell_value = vec![];
                    cell_name = None;
                }
            }
        }
        Ok(result)
    }

    async fn multi_set(
        &mut self,
        table_name: &str,
        values: impl IntoIterator<Item = (Bytes, Vec<(&str, Bytes)>)> + std::marker::Send,
    ) -> Result<()> {
        for chunk in values.into_iter().collect::<Vec<_>>().chunks(50_000) {
            self.multi_set_internal(table_name, chunk.iter().cloned())
                .await?;
        }
        Ok(())
    }

    async fn multi_set_internal(
        &mut self,
        table_name: &str,
        values: impl IntoIterator<Item = (Bytes, Vec<(&str, Bytes)>)> + std::marker::Send,
    ) -> Result<()> {
        let entries = values
            .into_iter()
            .map(|(row_key, cells)| {
                let mutations = cells
                    .into_iter()
                    .map(|(column_name, value)| Mutation {
                        mutation: Some(mutation::Mutation::SetCell(SetCell {
                            family_name: COLUMN_FAMILY_NAME.to_string(),
                            column_qualifier: column_name.to_owned().into_bytes(),
                            // The timestamp of the cell into which new data should be written.
                            // Use -1 for current Bigtable server time.
                            timestamp_micros: -1,
                            value,
                        })),
                    })
                    .collect();
                Entry { row_key, mutations }
            })
            .collect::<Vec<Entry>>();

        for entries in Self::batch_entries_by_size(entries) {
            let request = MutateRowsRequest {
                table_name: self.table_name(table_name),
                entries,
                ..MutateRowsRequest::default()
            };

            let mut response = self.mutate_rows(request).await?;
            while let Some(part) = response.message().await? {
                for entry in part.entries {
                    if let Some(status) = entry.status {
                        if status.code != 0 {
                            return Err(anyhow!(
                                "bigtable write failed {} {}",
                                status.code,
                                status.message
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn multi_get(
        &mut self,
        table_name: &str,
        keys: Vec<Vec<u8>>,
        filter: Option<RowFilter>,
    ) -> Result<Vec<Vec<(Bytes, Bytes)>>> {
        let start_time = Instant::now();
        let num_keys_requested = keys.len();
        let result = self.multi_get_internal(table_name, keys, filter).await;
        let elapsed_ms = start_time.elapsed().as_millis() as f64;

        let Some(metrics) = &self.metrics else {
            return result;
        };

        let labels = [&self.client_name, table_name];
        let Ok(rows) = &result else {
            metrics.kv_get_errors.with_label_values(&labels).inc();
            return result;
        };

        metrics
            .kv_get_batch_size
            .with_label_values(&labels)
            .observe(num_keys_requested as f64);

        if num_keys_requested > rows.len() {
            metrics
                .kv_get_not_found
                .with_label_values(&labels)
                .inc_by((num_keys_requested - rows.len()) as u64);
        }

        metrics
            .kv_get_success
            .with_label_values(&labels)
            .inc_by(rows.len() as u64);

        metrics
            .kv_get_latency_ms
            .with_label_values(&labels)
            .observe(elapsed_ms);

        if num_keys_requested > 0 {
            metrics
                .kv_get_latency_ms_per_key
                .with_label_values(&labels)
                .observe(elapsed_ms / num_keys_requested as f64);
        }

        result
    }

    pub async fn multi_get_internal(
        &mut self,
        table_name: &str,
        keys: Vec<Vec<u8>>,
        filter: Option<RowFilter>,
    ) -> Result<Vec<Vec<(Bytes, Bytes)>>> {
        let request = ReadRowsRequest {
            table_name: self.table_name(table_name),
            rows_limit: keys.len() as i64,
            rows: Some(RowSet {
                row_keys: keys,
                row_ranges: vec![],
            }),
            filter,
            ..ReadRowsRequest::default()
        };
        let mut result = vec![];
        for (_, cells) in self.read_rows(request).await? {
            result.push(cells);
        }
        Ok(result)
    }

    async fn reversed_scan(
        &mut self,
        table_name: &str,
        upper_limit: Bytes,
    ) -> Result<Vec<(Bytes, Vec<(Bytes, Bytes)>)>> {
        let start_time = Instant::now();
        let result = self.reversed_scan_internal(table_name, upper_limit).await;
        let elapsed_ms = start_time.elapsed().as_millis() as f64;
        let labels = [&self.client_name, table_name];
        match &self.metrics {
            Some(metrics) => match result {
                Ok(result) => {
                    metrics.kv_scan_success.with_label_values(&labels).inc();
                    if result.is_empty() {
                        metrics.kv_scan_not_found.with_label_values(&labels).inc();
                    }
                    metrics
                        .kv_scan_latency_ms
                        .with_label_values(&labels)
                        .observe(elapsed_ms);
                    Ok(result)
                }
                Err(e) => {
                    metrics.kv_scan_error.with_label_values(&labels).inc();
                    Err(e)
                }
            },
            None => result,
        }
    }

    async fn reversed_scan_internal(
        &mut self,
        table_name: &str,
        upper_limit: Bytes,
    ) -> Result<Vec<(Bytes, Vec<(Bytes, Bytes)>)>> {
        let range = RowRange {
            start_key: None,
            end_key: Some(EndKey::EndKeyClosed(upper_limit)),
        };
        let request = ReadRowsRequest {
            table_name: self.table_name(table_name),
            rows_limit: 1,
            rows: Some(RowSet {
                row_keys: vec![],
                row_ranges: vec![range],
            }),
            reversed: true,
            ..ReadRowsRequest::default()
        };
        self.read_rows(request).await
    }

    fn raw_object_key(object_key: &ObjectKey) -> Result<Vec<u8>> {
        let mut raw_key = object_key.0.to_vec();
        raw_key.extend(object_key.1.value().to_be_bytes());
        Ok(raw_key)
    }

    /// Splits a vector of `Entry` messages into batches such that the total
    /// serialized size of each batch does not exceed the gRPC message size
    /// limit (250 MB).
    ///
    /// This function serializes each entry to determine its size, and
    /// accumulates entries into a batch until adding another entry would
    /// exceed the size limit. It then starts a new batch. The result is a
    /// vector of batches, each of which can be safely sent as a single gRPC
    /// request without exceeding the configured maximum message size.
    fn batch_entries_by_size(entries: Vec<Entry>) -> Vec<Vec<Entry>> {
        let mut batches = Vec::new();
        let mut current_batch = Vec::new();
        let mut current_size = 0;

        for entry in entries {
            // Estimate size by serializing the entry
            let entry_size = entry.encoded_len();

            // If adding this entry would exceed the limit, start a new batch
            if current_size + entry_size > GRPC_MAX_REQUEST_SIZE && !current_batch.is_empty() {
                batches.push(current_batch);
                current_batch = Vec::new();
                current_size = 0;
            }

            current_batch.push(entry);
            current_size += entry_size;
        }

        // Push the last batch if not empty
        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        batches
    }
}

/// A smart, thread-safe wrapper around a gRPC channel that transparently
/// manages authentication and header injection for requests to Google Bigtable.
///
/// # Purpose
/// - Handles authentication using tokens, automatically injecting a valid
///   `Authorization` header into each outgoing request if a `token_provider` is
///   configured.
/// - Caches tokens and refreshes them only when expired, ensuring efficient and
///   secure communication.
/// - Injects additional headers (such as `bigtable-features`) required for
///   enabling specific Bigtable features.
///
/// # Behavior
/// - On each request, checks if a valid token is cached; if not, fetches a new
///   one.
/// - Adds the `Authorization: Bearer <token>` header when needed.
/// - Always adds the `bigtable-features` header.
/// - Implements the `Service` trait to act as middleware in the gRPC stack,
///   intercepting and modifying requests.
///
/// # Usage
/// Used internally by `BigTableClient` to ensure all requests are properly
/// authorized and feature-enabled when communicating with Google Bigtable.
#[derive(Clone)]
struct AuthChannel {
    // The underlying gRPC channel used for communication.
    channel: Channel,
    // The access policy (scope) for which tokens are requested.
    policy: String,
    // Provides tokens for authentication.
    token_provider: Option<Arc<dyn TokenProvider>>,
    // Caches the current token.
    token: Arc<RwLock<Option<Arc<Token>>>>,
}

impl AuthChannel {
    /// Creates a new `AuthChannel` for localhost communication. It does not
    /// require authentication.
    fn new_localhost(channel: Channel, policy: impl Into<String>) -> Self {
        Self {
            channel,
            policy: policy.into(),
            token_provider: None,
            token: Arc::new(RwLock::new(None)),
        }
    }

    /// Creates a new `AuthChannel` for remote communication. It requires
    /// authentication.
    fn new_remote(
        channel: Channel,
        policy: impl Into<String>,
        token_provider: Arc<dyn TokenProvider>,
    ) -> Self {
        Self {
            channel,
            policy: policy.into(),
            token_provider: Some(token_provider),
            token: Arc::new(RwLock::new(None)),
        }
    }
}

impl Service<Request<Body>> for AuthChannel {
    type Response = Response<Body>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    // Checks if the underlying channel is ready to send a request.
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.channel.poll_ready(cx).map_err(Into::into)
    }
    // Handles an outgoing request:
    // - Injects authentication and feature headers.
    // - Forwards the request to the underlying channel.
    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let cloned_channel = self.channel.clone();
        let cloned_token = self.token.clone();
        let mut inner = std::mem::replace(&mut self.channel, cloned_channel);
        let policy = self.policy.clone();
        let token_provider = self.token_provider.clone();

        let mut auth_token = None;
        // try to get a valid cached token if a provider exists.
        if token_provider.is_some() {
            let guard = self.token.read().expect("failed to acquire a read lock");
            if let Some(token) = &*guard {
                if !token.has_expired() {
                    auth_token = Some(token.clone());
                }
            }
        }

        Box::pin(async move {
            // if a token provider exists, ensure a valid token is present.
            if let Some(ref provider) = token_provider {
                let token = match auth_token {
                    // no valid token cached: fetch a new one and cache it.
                    None => {
                        let new_token = provider.token(&[policy.as_ref()]).await?;
                        let mut guard = cloned_token.write().unwrap();
                        *guard = Some(new_token.clone());
                        new_token
                    }
                    // use the cached valid token.
                    Some(token) => token,
                };
                // insert the Authorization header with the Bearer token.
                let token_string = token.as_str().parse::<String>()?;
                let header =
                    HeaderValue::from_str(format!("Bearer {}", token_string.as_str()).as_str())?;
                request.headers_mut().insert("authorization", header);
            }
            // always insert the Bigtable features header (e.g., to enable reverse scan).
            let header = HeaderValue::from_static("CAE=");
            request.headers_mut().insert("bigtable-features", header);

            // forward the request to the underlying channel and return the response.
            Ok(inner.call(request).await?)
        })
    }
}
