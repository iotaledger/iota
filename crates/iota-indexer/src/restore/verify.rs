// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::hash::MultisetHash;
use iota_config::genesis::Genesis;
use iota_snapshot::{EpochInfo, VerifiedEpochInfo, verify_epoch_info_chain};
use iota_types::{
    digests::ChainIdentifier, global_state_hash::GlobalStateHash,
    messages_checkpoint::ECMHLiveObjectSetDigest,
};
use tokio::sync::mpsc;

use crate::{errors::IndexerError, types::IndexerResult};

pub(crate) async fn verify_epoch_info(
    epoch_info: EpochInfo,
    genesis: Genesis,
    snapshot_chain_id: ChainIdentifier,
) -> IndexerResult<VerifiedEpochInfo> {
    let genesis_committee = genesis.committee().expect("genesis committee");
    let genesis_system_state = genesis.iota_system_object();
    let genesis_chain_id = ChainIdentifier::from(*genesis.checkpoint().digest());
    tokio::task::spawn_blocking(move || {
        verify_epoch_info_chain(
            epoch_info,
            genesis_committee,
            genesis_system_state,
            snapshot_chain_id,
            genesis_chain_id,
        )
    })
    .await?
    .map_err(|e| IndexerError::Restore(format!("snapshot verification failed: {e}")))
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
pub(crate) async fn verify_state_hash(
    state_hash_rx: mpsc::Receiver<(GlobalStateHash, u64)>,
    verified_epoch_info: &VerifiedEpochInfo,
) -> IndexerResult<u64> {
    let (root_state_hash, num_objects) = accumulate_state_hash(state_hash_rx).await;

    let last_checkpoint_summary = &verified_epoch_info
        .entries()
        .last()
        .expect("there should be an entry for the associated epoch")
        .last_checkpoint_summary;
    let commitment = last_checkpoint_summary
        .end_of_epoch_data
        .as_ref()
        .and_then(|end_of_epoch| end_of_epoch.epoch_commitments.last())
        .ok_or_else(|| {
            IndexerError::Restore("verified checkpoint has no end-of-epoch commitment".to_string())
        })?;
    let verified_digest = commitment.as_ecmh_live_object_set_digest();
    let root_state_hash = ECMHLiveObjectSetDigest::from(root_state_hash.digest()).digest;
    if *verified_digest != root_state_hash {
        return Err(IndexerError::Restore(format!(
            "root state hash {root_state_hash:?} does not match the verified commitment \
             {verified_digest:?}"
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
