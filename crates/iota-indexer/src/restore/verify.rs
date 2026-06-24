// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::hash::MultisetHash;
use futures::FutureExt;
use iota_config::genesis::Genesis;
use iota_data_ingestion_core::IngestionError;
use iota_snapshot::{EpochInfo, verify_epoch_info_chain};
use iota_types::{
    digests::ChainIdentifier,
    global_state_hash::GlobalStateHash,
    messages_checkpoint::{CheckpointCommitment, ECMHLiveObjectSetDigest, VerifiedCheckpoint},
};
use tokio::sync::mpsc;

use crate::{errors::IndexerError, types::IndexerResult};

async fn verify_last_checkpoint(
    epoch_info: EpochInfo,
    genesis: Genesis,
    snapshot_chain_id: ChainIdentifier,
) -> IndexerResult<VerifiedCheckpoint> {
    let genesis_committee = genesis.committee().expect("genesis committee");
    let genesis_system_state = genesis.iota_system_object();
    let genesis_chain_id = ChainIdentifier::from(*genesis.checkpoint().digest());
    let verified_epoch_info = tokio::task::spawn_blocking(move || {
        verify_epoch_info_chain(
            epoch_info,
            genesis_committee,
            genesis_system_state,
            snapshot_chain_id,
            genesis_chain_id,
        )
    })
    .await?
    .map_err(|e| IndexerError::Restore(format!("snapshot verification failed: {e}")))?;
    Ok(VerifiedCheckpoint::new_unchecked(
        verified_epoch_info
            .entries()
            .last()
            .expect("there should be an entry for the associated epoch")
            .last_checkpoint_summary
            .clone(),
    ))
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
/// - State hash verification fails.
/// - The verified checkpoint carries no end-of-epoch commitment.
/// - The accumulated root state hash does not match that commitment.
pub(super) async fn verify_state_hash(
    state_hash_rx: mpsc::Receiver<(GlobalStateHash, u64)>,
    epoch_info: EpochInfo,
    genesis: Genesis,
    snapshot_chain_id: ChainIdentifier,
) -> IndexerResult<u64> {
    let ((root_state_hash, num_objects), verified_checkpoint) = tokio::try_join!(
        accumulate_state_hash(state_hash_rx).map(Ok::<_, IndexerError>),
        verify_last_checkpoint(epoch_info, genesis, snapshot_chain_id)
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

/// Evaluates the root state hash of the live object set stored in a formal
/// snapshot.
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
