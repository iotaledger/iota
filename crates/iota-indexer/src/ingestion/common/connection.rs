// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use backoff::{self, ExponentialBackoff};
use futures::TryFutureExt;
use iota_data_ingestion_core::{
    create_remote_store_client, history::manifest::Manifest, reader::v2::RemoteUrl,
};
use iota_grpc_client::Client as GrpcClient;
use object_store::ObjectStoreExt;
use tracing::{debug, info};

use crate::{
    config::IngestionSources,
    errors::{IndexerError, IndexerResult},
};

/// Resolves the remote checkpoint source from the provided
/// [`remote_store_url`](IngestionSources::remote_store_url).
///
/// Since `remote_store_url` accepts either a fullnode gRPC endpoint or an
/// object store URL for historical checkpoint data, this function probes the
/// URL to determine which type it is:
///
/// 1. **gRPC health check**: attempts to connect and call `GetHealth`. If
///    successful, the URL is treated as a fullnode gRPC endpoint.
/// 2. **Historical manifest fetch**: if gRPC fails, attempts to fetch the
///    MANIFEST file from the URL as an S3-compatible object store. If
///    successful, the URL is treated as a historical checkpoint store.
///
/// Both probes are retried with exponential backoff within the given timeout.
/// If neither succeeds, returns an error.
///
/// When `live_checkpoints_store_url` is provided, the URL is assumed to be a
/// historical store and the live URL is included in the
/// [`RemoteUrl::HybridHistoricalStore`].
pub async fn resolve_remote_url(
    ingestion_sources: &IngestionSources,
    timeout: Duration,
) -> IndexerResult<Option<RemoteUrl>> {
    let Some(url) = ingestion_sources
        .remote_store_url
        .as_ref()
        .map(ToString::to_string)
    else {
        return Ok(None);
    };

    let live_url = ingestion_sources
        .live_checkpoints_store_url
        .as_ref()
        .map(ToString::to_string);

    // if live URL is provided, remote-store-url can be assumed as a historical
    // store
    if live_url.is_some() {
        return Ok(Some(RemoteUrl::HybridHistoricalStore {
            historical_url: url,
            live_url,
        }));
    }

    let backoff = ExponentialBackoff {
        max_elapsed_time: Some(timeout),
        multiplier: 2.0,
        ..Default::default()
    };

    backoff::future::retry(backoff, || {
        let url = url.clone();
        async move {
            let grpc_result = GrpcClient::connect(url.clone())
                .and_then(|client| async move { client.get_health(None).await })
                .await
                .inspect_err(|e| debug!("gRPC health check failed: {e}"));

            if grpc_result.is_ok() {
                info!("resolved remote store as fullnode gRPC: {url}");
                return Ok(Some(RemoteUrl::Fullnode(url)));
            }

            // we use a lightweight S3 client to check if the MANIFEST file exists.
            // we avoid HistoricalReader here as its internal manifest fetch retries
            // with a 15-minute default backoff and does not have a timeout.
            let store =
                create_remote_store_client(url.clone(), Default::default(), timeout.as_secs())
                    .map_err(|e| {
                        debug!("failed to create historical store client: {e}");
                        backoff::Error::transient(IndexerError::Generic(format!(
                            "remote store not reachable: {url}"
                        )))
                    })?;

            store.head(&Manifest::file_path()).await.map_err(|e| {
                debug!("historical store MANIFEST not found: {e}");
                backoff::Error::transient(IndexerError::Generic(format!(
                    "remote store not reachable: {url}"
                )))
            })?;

            info!("resolved remote store as historical object store: {url}");
            Ok(Some(RemoteUrl::HybridHistoricalStore {
                historical_url: url,
                live_url: None,
            }))
        }
    })
    .await
    .map_err(|_: IndexerError| IndexerError::Generic(format!(
        "failed to resolve remote store '{url}' after {timeout:?}: not reachable as gRPC or historical store"
    )))
}
