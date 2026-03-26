// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use backoff::{self, ExponentialBackoff, backoff::Backoff};
use iota_data_ingestion_core::{
    create_remote_store_client, history::manifest::Manifest, reader::v2::RemoteUrl,
};
use iota_grpc_client::Client as GrpcClient;
use object_store::ObjectStoreExt;
use tracing::{info, warn};

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

    let grpc_client = GrpcClient::connect(url.clone()).await?;

    // Use a lightweight S3 client to check if the MANIFEST file exists.
    // We avoid HistoricalReader here as its internal manifest fetch retries
    // with a 15-minute default backoff and does not have a timeout.
    let historical =
        create_remote_store_client(url.clone(), Default::default(), timeout.as_secs())?;

    let mut backoff = ExponentialBackoff {
        max_elapsed_time: Some(timeout),
        initial_interval: Duration::from_millis(500),
        multiplier: 2.0,
        ..Default::default()
    };

    loop {
        if grpc_client.get_health(None).await.is_ok() {
            info!("resolved remote store as fullnode gRPC: {url}");
            return Ok(Some(RemoteUrl::Fullnode(url)));
        }

        if historical.head(&Manifest::file_path()).await.is_ok() {
            info!("resolved remote store as historical object store: {url}");
            let live_url = ingestion_sources
                .current_epoch_store_url
                .as_ref()
                .map(ToString::to_string);
            return Ok(Some(RemoteUrl::HybridHistoricalStore {
                historical_url: url,
                live_url,
            }));
        }

        match backoff.next_backoff() {
            Some(duration) => {
                warn!(
                    "remote store not reachable as fullnode gRPC or historical connection, retrying in {}ms",
                    duration.as_millis()
                );
                tokio::time::sleep(duration).await;
            }
            None => {
                return Err(IndexerError::Generic(format!(
                    "unable to resolve remote store URL after {timeout:?}: {url}"
                )));
            }
        }
    }
}
