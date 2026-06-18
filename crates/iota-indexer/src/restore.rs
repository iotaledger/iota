// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Restore the Indexer database through formal snapshots.

use std::{num::NonZeroUsize, path::Path, sync::Arc};

use anyhow::{anyhow, bail};
use bytes::Bytes;
use fastcrypto::hash::{HashFunction, MultisetHash, Sha3_256};
use futures::{FutureExt, TryFutureExt, future::AbortHandle};
use indicatif::MultiProgress;
use iota_config::{
    genesis::Genesis,
    node::ArchiveReaderConfig,
    object_storage_config::{ObjectStoreConfig, ObjectStoreType},
};
use iota_core::db_checkpoint_handler::SUCCESS_MARKER;
use iota_data_ingestion_core::{
    IngestionError,
    history::{reader::HistoricalReader, verifier::EpochBoundaryVerifier},
};
use iota_protocol_config::Chain;
use iota_snapshot::{
    FileMetadata,
    reader::{LiveObjectIter, StateSnapshotReaderV1},
    restore::Restore,
};
use iota_storage::{
    SHA3_BYTES,
    object_store::{
        ObjectStoreGetExt,
        http::HttpDownloaderBuilder,
        util::{MANIFEST_FILENAME, Manifest, exists, get_path},
    },
};
use iota_types::{
    global_state_hash::GlobalStateHash,
    messages_checkpoint::{CheckpointCommitment, ECMHLiveObjectSetDigest},
};
use itertools::Itertools;
use tokio::sync::mpsc;
use tracing::info;

use crate::{
    chunk, errors::IndexerError, ingestion::common::prepare::LiveObject, store::PgIndexerStore,
    types::IndexerResult,
};

const MAINNET_FORMAL_SNAPSHOT_ENDPOINT: &str = "https://formal-snapshot.mainnet.iota.cafe";
const TESTNET_FORMAL_SNAPSHOT_ENDPOINT: &str = "https://formal-snapshot.testnet.iota.cafe";

const MAINNET_HISTORICAL_CHECKPOINTS_ENDPOINT: &str =
    "https://checkpoints.mainnet.iota.cafe/ingestion/historical";
const TESTNET_HISTORICAL_CHECKPOINTS_ENDPOINT: &str =
    "https://checkpoints.testnet.iota.cafe/ingestion/historical";

/// Restores the indexer database from the formal snapshot for the given network
/// and epoch.
///
/// This guarantees that the formal snapshot is verified, by comparing
/// the root state hash of the live objects against the verified commitment of
/// the network at the given epoch through public archives.
///
/// # Errors
///
/// Returns an error if:
///
/// - The reader or verifier cannot be instantiated.
/// - The persist pipeline fails
/// - The snapshot fails verification.
pub async fn start(
    network: Chain,
    epoch: Option<u64>,
    staging_path: &Path,
    genesis_path: &Path,
    num_parallel_downloads: NonZeroUsize,
    pg_indexer_store: PgIndexerStore,
) -> IndexerResult<()> {
    let (mut reader, epoch) =
        setup_reader(network, epoch, staging_path, num_parallel_downloads).await?;
    let verifier =
        build_epoch_boundary_verifier(network, epoch, genesis_path, num_parallel_downloads).await?;

    // It's ok to ignore the handle. Cancellation is effected by dropping the
    // `read_to_db` future below, so we don't need to call `abort` explicitly.
    let (_abort_handle, abort_registration) = AbortHandle::new_pair();
    let (state_hash_tx, state_hash_rx) =
        mpsc::channel::<(GlobalStateHash, u64)>(num_parallel_downloads.get());

    let ((), num_objects) = tokio::try_join!(
        reader
            .read_to_db(&pg_indexer_store, abort_registration, Some(state_hash_tx))
            .map_err(IndexerError::from),
        verify_state_hash(state_hash_rx, verifier),
    )?;

    info!(
        epoch,
        num_objects, "formal snapshot restore complete and verified"
    );
    Ok(())
}

/// Verifies the root state hash evaluated from the formal snapshot.
///
/// This is done by comparing the value against the verified commitment of the
/// snapshot epoch.
///
/// Returns the number of live objects accumulated.
///
/// # Errors
///
/// Returns an error if:
///
/// - Epoch-boundary verification fails.
/// - The verified checkpoint carries no end-of-epoch commitment.
/// - The accumulated root state hash does not match that commitment.
async fn verify_state_hash(
    state_hash_rx: mpsc::Receiver<(GlobalStateHash, u64)>,
    verifier: EpochBoundaryVerifier,
) -> IndexerResult<u64> {
    let ((root_state_hash, num_objects), verified_checkpoint) = tokio::try_join!(
        accumulate_state_hash(state_hash_rx).map(Ok::<_, IndexerError>),
        verifier
            .verify_target_epoch_boundary()
            .map_err(IndexerError::from),
    )?;

    let commitment = verified_checkpoint
        .end_of_epoch_data
        .as_ref()
        .and_then(|end_of_epoch| end_of_epoch.epoch_commitments.last())
        .ok_or_else(|| {
            IndexerError::Ingestion(IngestionError::Verification(
                "verified checkpoint has no end-of-epoch commitment".to_string(),
            ))
        })?;
    let CheckpointCommitment::ECMHLiveObjectSetDigest(verified_digest) = commitment;
    let local_digest = ECMHLiveObjectSetDigest::from(root_state_hash.digest());
    if *verified_digest != local_digest {
        return Err(IndexerError::Ingestion(IngestionError::Verification(
            format!(
                "root state hash {local_digest:?} does not match the verified commitment \
             {verified_digest:?}"
            ),
        )));
    }
    Ok(num_objects)
}

/// Evaluates the root state hash of the live object set in this snapshot.
///
/// This is done by accumulating the partial state hashes received during a
/// restore.
///
/// The total number of objects found in the live set is returned along with the
/// root state hash.
async fn accumulate_state_hash(
    mut state_hash_rx: mpsc::Receiver<(GlobalStateHash, u64)>,
) -> (GlobalStateHash, u64) {
    let mut root_state_hash = GlobalStateHash::default();
    let mut num_objects = 0u64;
    while let Some((partial_hash, count)) = state_hash_rx.recv().await {
        root_state_hash.union(&partial_hash);
        num_objects += count;
    }
    (root_state_hash, num_objects)
}

impl Restore for PgIndexerStore {
    async fn insert_partition(
        &self,
        file_metadata: FileMetadata,
        bytes: Bytes,
        expected_checksum: &[u8; SHA3_BYTES],
    ) -> anyhow::Result<()> {
        let mut hasher = Sha3_256::default();
        let partition = LiveObjectIter::new(&file_metadata, bytes)?
            .filter_map(|snapshot_object| snapshot_object.to_normal())
            .scan(&mut hasher, |hasher, object| {
                hasher.update(object.object_ref().digest.inner());
                Some(LiveObject::new(0, object))
            });
        let chunks = chunk!(partition, self.config.parallel_objects_chunk_size);
        let sha3_digest = hasher.finalize().digest;
        if *expected_checksum != sha3_digest {
            tracing::error!(
                "Sha does not match! expected: {expected_checksum:?}, actual: {sha3_digest:?}",
            );
            anyhow::bail!(
                "checksum verification failed for bucket/partition: {}/{}",
                file_metadata.bucket_num,
                file_metadata.part_num
            );
        }

        let persist_tasks = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_changed_objects(c)));
        futures::future::try_join_all(persist_tasks)
            .await
            .map_err(|e| {
                tracing::error!(
                    "failed to join futures for persisting formal snapshot partition: {e}"
                );
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "failed to persist all formal snapshot object chunks: {e:?}",
                ))
            })?;
        Ok(())
    }
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
    network: Chain,
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
    Ok((reader, epoch))
}

/// Builds an [`EpochBoundaryVerifier`] for the network's public historical
/// checkpoint store, targeting the given epoch.
///
/// # Errors
///
/// Returns an error if:
///
/// - The network is not `mainnet` or `testnet`.
/// - The genesis cannot be loaded from `genesis_path`.
/// - The historical reader cannot be built, or the epoch boundaries cannot be
///   read from the remote store.
async fn build_epoch_boundary_verifier(
    network: Chain,
    epoch: u64,
    genesis_path: &Path,
    download_concurrency: NonZeroUsize,
) -> IndexerResult<EpochBoundaryVerifier> {
    let endpoint = match network {
        Chain::Mainnet => MAINNET_HISTORICAL_CHECKPOINTS_ENDPOINT,
        Chain::Testnet => TESTNET_HISTORICAL_CHECKPOINTS_ENDPOINT,
        Chain::Unknown => {
            return Err(IndexerError::InvalidArgument(
                "formal snapshot network must be Mainnet or Testnet".into(),
            ));
        }
    };
    let reader = HistoricalReader::new(ArchiveReaderConfig {
        remote_store_config: unsigned_http_store_config(endpoint),
        download_concurrency,
        use_for_pruning_watermark: false,
    })?;
    let genesis = Genesis::load(genesis_path)?;
    let verifier = EpochBoundaryVerifier::from_genesis(reader, &genesis, epoch).await?;
    Ok(verifier)
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
        let config = unsigned_http_store_config(aws_endpoint);
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
