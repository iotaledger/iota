// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{num::NonZeroUsize, path::Path};

use futures::{TryFutureExt, future::AbortHandle};
use iota_config::genesis::Genesis;
use iota_types::global_state_hash::GlobalStateHash;
use tokio::sync::mpsc;
use tracing::info;

use super::{
    setup::{Network, setup_reader},
    verify::verify_state_hash,
};
use crate::{
    errors::IndexerError,
    restore::{persist::populate_remaining_tables, verify::verify_epoch_info},
    store::PgIndexerStore,
    types::IndexerResult,
};

/// Restores the indexer database from the formal snapshot for the given network
/// and epoch.
///
/// This guarantees that the formal snapshot is verified, by comparing
/// the root state hash of the live objects against the verified commitment of
/// the network at the given epoch.
///
/// # Errors
///
/// Returns an error if:
///
/// - The reader cannot be instantiated.
/// - The persist pipeline fails
/// - The snapshot fails verification.
pub async fn start(
    network: Network,
    epoch: Option<u64>,
    staging_path: &Path,
    genesis_path: &Path,
    num_parallel_downloads: NonZeroUsize,
    pg_indexer_store: PgIndexerStore,
) -> IndexerResult<()> {
    let (mut reader, epoch) =
        setup_reader(network, epoch, staging_path, num_parallel_downloads).await?;
    let epoch_info = reader
        .read_epoch_info()
        .await
        .map_err(|e| IndexerError::Restore(format!("failed to read epoch info: {e}")))?;
    let snapshot_chain_id = reader.chain_id();
    let genesis = Genesis::load(genesis_path)?;

    // It's ok to ignore the handle. Cancellation is effected by dropping the
    // `read_to_db` future below, so we don't need to call `abort` explicitly.
    let (_abort_handle, abort_registration) = AbortHandle::new_pair();
    let (state_hash_tx, state_hash_rx) =
        mpsc::channel::<(GlobalStateHash, u64)>(num_parallel_downloads.get());

    let verified_epoch_info = verify_epoch_info(epoch_info, genesis, snapshot_chain_id).await?;
    let ((), num_objects) = tokio::try_join!(
        reader
            .read_to_db(&pg_indexer_store, abort_registration, Some(state_hash_tx))
            .map_err(IndexerError::from),
        verify_state_hash(state_hash_rx, &verified_epoch_info),
    )?;
    populate_remaining_tables(&pg_indexer_store, verified_epoch_info, snapshot_chain_id).await?;

    info!(
        epoch,
        num_objects, "formal snapshot restore complete and verified"
    );
    Ok(())
}
