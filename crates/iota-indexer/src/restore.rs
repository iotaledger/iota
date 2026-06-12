// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Restore the Indexer database through formal snapshots.

use std::{num::NonZeroUsize, path::Path, sync::Arc};

use anyhow::{anyhow, bail};
use indicatif::MultiProgress;
use iota_config::object_storage_config::{ObjectStoreConfig, ObjectStoreType};
use iota_core::db_checkpoint_handler::SUCCESS_MARKER;
use iota_protocol_config::Chain;
use iota_snapshot::reader::StateSnapshotReaderV1;
use iota_storage::object_store::{
    ObjectStoreGetExt,
    http::HttpDownloaderBuilder,
    util::{MANIFEST_FILENAME, Manifest, exists, get_path},
};
use tracing::info;

use crate::{errors::IndexerError, types::IndexerResult};

const MAINNET_FORMAL_SNAPSHOT_ENDPOINT: &str = "https://formal-snapshot.mainnet.iota.cafe";
const TESTNET_FORMAL_SNAPSHOT_ENDPOINT: &str = "https://formal-snapshot.testnet.iota.cafe";

pub async fn start(
    network: Chain,
    epoch: Option<u64>,
    staging_path: &Path,
    num_parallel_downloads: NonZeroUsize,
) -> IndexerResult<()> {
    let reader = setup_reader(network, epoch, staging_path, num_parallel_downloads).await?;
    Ok(())
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
/// # Errors
///
/// Returns an error if:
///
/// - The network is not `mainnet` or `testnet`.
/// - The snapshot for the resolved epoch is incomplete.
/// - Downloading the MANIFEST or reference files fails.
pub(crate) async fn setup_reader(
    network: Chain,
    epoch: Option<u64>,
    staging_path: &Path,
    num_parallel_downloads: NonZeroUsize,
) -> IndexerResult<StateSnapshotReaderV1> {
    let remote_store = FormalSnapshotStore::new(network)?;
    let local_store_config = local_store_config(staging_path);

    let epoch = match epoch {
        Some(epoch) => epoch,
        None => remote_store.latest_available_epoch().await?,
    };
    remote_store.verify_completed_snapshot(epoch).await?;

    info!(
        network = network.as_str(),
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
    Ok(reader)
}

/// Read client for a network's public formal snapshot store.
struct FormalSnapshotStore {
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
    fn new(network: Chain) -> IndexerResult<Self> {
        let aws_endpoint = match network {
            Chain::Mainnet => MAINNET_FORMAL_SNAPSHOT_ENDPOINT,
            Chain::Testnet => TESTNET_FORMAL_SNAPSHOT_ENDPOINT,
            Chain::Unknown => {
                return Err(IndexerError::InvalidArgument(
                    "formal snapshot network must be Mainnet or Testnet".into(),
                ));
            }
        };
        let config = ObjectStoreConfig {
            object_store: Some(ObjectStoreType::S3),
            aws_endpoint: Some(aws_endpoint.to_string()),
            aws_virtual_hosted_style_request: true,
            object_store_connection_limit: 200,
            no_sign_request: true,
            ..Default::default()
        };
        let store = config.make_http()?;
        Ok(Self { config, store })
    }

    /// Returns the latest epoch with a formal snapshot available in the remote
    /// store, according to the root MANIFEST.
    async fn latest_available_epoch(&self) -> Result<u64, anyhow::Error> {
        let manifest_contents = self.store.get_bytes(&get_path(MANIFEST_FILENAME)).await?;
        let root_manifest: Manifest = serde_json::from_slice(&manifest_contents)
            .map_err(|err| anyhow!("Error parsing MANIFEST from bytes: {}", err))?;
        root_manifest
            .available_epochs
            .iter()
            .map(|(epoch, _)| *epoch)
            .max()
            .ok_or(anyhow!("No snapshot found in manifest"))
    }

    /// Verifies that the formal snapshot upload for the given epoch has
    /// completed.
    async fn verify_completed_snapshot(&self, epoch: u64) -> Result<(), anyhow::Error> {
        let success_marker = format!("epoch_{epoch}/{SUCCESS_MARKER}");
        if exists(&self.store, &get_path(success_marker.as_str())).await {
            Ok(())
        } else {
            bail!(
                "missing success marker at {}/{}",
                self.config
                    .aws_endpoint
                    .as_deref()
                    .unwrap_or("unknown endpoint"),
                success_marker
            )
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
