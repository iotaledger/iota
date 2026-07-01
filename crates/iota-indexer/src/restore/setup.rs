// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cmp::Reverse, num::NonZeroUsize, path::Path, sync::Arc};

use clap::ValueEnum;
use indicatif::MultiProgress;
use iota_config::object_storage_config::{ObjectStoreConfig, ObjectStoreType};
use iota_core::db_checkpoint_handler::SUCCESS_MARKER;
use iota_snapshot::reader::StateSnapshotReaderV1;
use iota_storage::object_store::{
    ObjectStoreGetExt,
    http::HttpDownloaderBuilder,
    util::{MANIFEST_FILENAME, RootManifest, exists, get_path},
};
use tracing::info;

use crate::{errors::IndexerError, types::IndexerResult};

const MAINNET_FORMAL_SNAPSHOT_ENDPOINT: &str = "https://formal-snapshot.mainnet.iota.cafe";
const TESTNET_FORMAL_SNAPSHOT_ENDPOINT: &str = "https://formal-snapshot.testnet.iota.cafe";
const DEVNET_FORMAL_SNAPSHOT_ENDPOINT: &str = "https://formal-snapshot.devnet.iota.cafe";

#[derive(Debug, Copy, Clone, strum_macros::AsRefStr, ValueEnum)]
#[strum(serialize_all = "lowercase")]
pub enum Network {
    Mainnet,
    Testnet,
    Devnet,
}

/// Downloads the formal snapshot metadata for the given network and epoch, and
/// instantiates a [`StateSnapshotReaderV1`] staged at `staging_path`.
///
/// 1. Builds a [`FormalSnapshotStore`] over the network's public snapshot
///    bucket and the local store configuration for the staging directory.
/// 2. Resolves the target epoch: the given one, or the latest epoch available
///    in the bucket.
/// 3. Verifies that the snapshot upload for that epoch has completed.
/// 4. Instantiates the reader, which downloads the snapshot's MANIFEST and
///    reference files into the staging directory.
///
/// Returns the reader and the resolved epoch.
///
/// # Errors
///
/// Returns an error if:
///
/// - The network is not `mainnet` or `testnet`.
/// - The snapshot for the resolved epoch is incomplete.
/// - Downloading the MANIFEST or reference files fails.
pub(crate) async fn setup_reader(
    network: Network,
    epoch: Option<u64>,
    staging_path: &Path,
    num_parallel_downloads: NonZeroUsize,
) -> IndexerResult<(StateSnapshotReaderV1, u64)> {
    let remote_store = FormalSnapshotStore::new(network)?;
    let local_store_config = local_store_config(staging_path);

    let epoch = match epoch {
        Some(epoch) => epoch,
        None => remote_store.latest_available_epoch().await?,
    };
    remote_store.verify_completed_snapshot(epoch).await?;

    info!(
        network = network.as_ref(),
        epoch,
        num_parallel_downloads = num_parallel_downloads.get(),
        "setting up formal snapshot reader"
    );
    let reader = StateSnapshotReaderV1::new(
        epoch,
        &remote_store.config,
        &local_store_config,
        num_parallel_downloads,
        MultiProgress::new(),
        false,
    )
    .await?;

    info!(
        epoch,
        staging_path = %staging_path.display(),
        "formal snapshot reader ready; MANIFEST and reference files downloaded"
    );
    Ok((reader, epoch))
}

/// Read client for a network's public formal snapshot store.
pub struct FormalSnapshotStore {
    config: ObjectStoreConfig,
    store: Arc<dyn ObjectStoreGetExt>,
}

impl FormalSnapshotStore {
    /// Builds the read client for the network's public formal snapshot store.
    ///
    /// # Errors
    ///
    /// Returns an error if the network is not `mainnet` or `testnet`, or if the
    /// client cannot be constructed.
    pub fn new(network: Network) -> IndexerResult<Self> {
        let aws_endpoint = match network {
            Network::Mainnet => MAINNET_FORMAL_SNAPSHOT_ENDPOINT,
            Network::Testnet => TESTNET_FORMAL_SNAPSHOT_ENDPOINT,
            Network::Devnet => DEVNET_FORMAL_SNAPSHOT_ENDPOINT,
        };
        let config = unsigned_http_store_config(aws_endpoint);
        let store = config.make_http()?;
        Ok(Self { config, store })
    }

    /// Returns the latest epoch with a formal snapshot available in the remote
    /// store, according to the root MANIFEST.
    async fn latest_available_epoch(&self) -> IndexerResult<u64> {
        self.available_epochs()
            .await?
            .into_iter()
            .max()
            .ok_or_else(|| IndexerError::Restore("No snapshot found in manifest".to_string()))
    }

    /// Returns the epochs for which there is an available formal snapshot of
    /// the ntwork, in descending order.
    ///
    /// # Errors
    ///
    /// Return an error if the read client cannot be build, or the root MANIFEST
    /// cannot be fetched or parsed.
    pub async fn available_epochs(&self) -> IndexerResult<Vec<u64>> {
        let manifest_contents = self.store.get_bytes(&get_path(MANIFEST_FILENAME)).await?;
        let root_manifest = RootManifest::from_bytes(&manifest_contents)?;
        let mut epochs: Vec<_> = root_manifest
            .available_epochs
            .iter()
            .map(|(epoch, _)| *epoch)
            .collect();
        epochs.sort_by_key(|&epoch| Reverse(epoch));
        Ok(epochs)
    }

    /// Verifies that the formal snapshot upload for the given epoch has
    /// completed.
    async fn verify_completed_snapshot(&self, epoch: u64) -> IndexerResult<()> {
        let success_marker = format!("epoch_{epoch}/{SUCCESS_MARKER}");
        // TODO: sort out failure modes of exists
        if exists(&self.store, &get_path(success_marker.as_str())).await {
            Ok(())
        } else {
            Err(IndexerError::Restore(format!(
                "missing success marker at {}/{}",
                self.config
                    .aws_endpoint
                    .as_deref()
                    .unwrap_or("unknown endpoint"),
                success_marker
            )))
        }
    }
}

/// Builds the local store configuration for the staging directory.
fn local_store_config(staging_path: &Path) -> ObjectStoreConfig {
    ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(staging_path.to_path_buf()),
        ..Default::default()
    }
}

/// Builds the config for an unsigned (anonymous) HTTPS object store served at
/// the given public endpoint.
fn unsigned_http_store_config(endpoint: &str) -> ObjectStoreConfig {
    ObjectStoreConfig {
        object_store: Some(ObjectStoreType::S3),
        aws_endpoint: Some(endpoint.to_string()),
        aws_virtual_hosted_style_request: true,
        object_store_connection_limit: 200,
        no_sign_request: true,
        ..Default::default()
    }
}
