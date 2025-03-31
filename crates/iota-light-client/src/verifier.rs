// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::{Result, anyhow};
use iota_config::genesis::Genesis;
use iota_json_rpc_types::{IotaObjectDataOptions, IotaTransactionBlockResponseOptions};
use iota_sdk::IotaClientBuilder;
use iota_types::{
    base_types::{ObjectID, TransactionDigest},
    committee::Committee,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
};
use tracing::info;

use crate::{
    checkpoint::{CheckpointList, read_checkpoint, read_checkpoint_list},
    config::Config,
    object_store::IotaObjectStore,
};

pub fn extract_verified_effects_and_events(
    checkpoint: &CheckpointData,
    committee: &Committee,
    tid: TransactionDigest,
) -> Result<(TransactionEffects, Option<TransactionEvents>)> {
    let summary = &checkpoint.checkpoint_summary;

    // Verify the checkpoint summary using the committee
    summary.verify_with_contents(committee, Some(&checkpoint.checkpoint_contents))?;

    // Check the validity of the transaction
    let contents = &checkpoint.checkpoint_contents;
    let (matching_tx, _) = checkpoint
        .transactions
        .iter()
        .zip(contents.iter())
        // Note that we get the digest of the effects to ensure this is
        // indeed the correct effects that are authenticated in the contents.
        .find(|(tx, digest)| {
            tx.effects.execution_digests() == **digest && digest.transaction == tid
        })
        .ok_or(anyhow!("Transaction not found in checkpoint contents"))?;

    // Check the events are all correct.
    let events_digest = matching_tx.events.as_ref().map(|events| events.digest());
    anyhow::ensure!(
        events_digest.as_ref() == matching_tx.effects.events_digest(),
        "Events digest does not match"
    );

    // Since we do not check objects we do not return them
    Ok((matching_tx.effects.clone(), matching_tx.events.clone()))
}

pub async fn get_verified_object(config: &Config, id: ObjectID) -> Result<Object> {
    let iota_client: Arc<iota_sdk::IotaClient> = Arc::new(
        IotaClientBuilder::default()
            .build(config.full_node_url.as_str())
            .await?,
    );

    info!("Getting object: {}", id);

    let read_api = iota_client.read_api();
    let object_json = read_api
        .get_object_with_options(id, IotaObjectDataOptions::bcs_lossless())
        .await
        .expect("Cannot get object");
    let object = object_json
        .into_object()
        .expect("Cannot make into object data");
    let object: Object = object.try_into().expect("Cannot reconstruct object");

    // Need to authenticate this object
    let (effects, _) = get_verified_effects_and_events(config, object.previous_transaction)
        .await
        .expect("Cannot get effects and events");

    // check that this object ID, version and hash is in the effects
    let target_object_ref = object.compute_object_reference();
    effects
        .all_changed_objects()
        .iter()
        .find(|object_ref| object_ref.0 == target_object_ref)
        .ok_or(anyhow!("Object not found"))
        .expect("Object not found");

    Ok(object)
}

pub async fn get_verified_effects_and_events(
    config: &Config,
    tid: TransactionDigest,
) -> Result<(TransactionEffects, Option<TransactionEvents>)> {
    let iota_client: iota_sdk::IotaClient = IotaClientBuilder::default()
        .build(config.full_node_url.as_str())
        .await?;
    let read_api = iota_client.read_api();

    info!("Getting effects and events for TID: {}", tid);

    // Lookup the transaction id and get the checkpoint sequence number
    let options = IotaTransactionBlockResponseOptions::new();
    let seq = read_api
        .get_transaction_with_options(tid, options)
        .await
        .map_err(|e| anyhow!(format!("Cannot get transaction: {e}")))?
        .checkpoint
        .ok_or(anyhow!("Transaction not found"))?;

    // Create object store
    let object_store = IotaObjectStore::new(config)?;

    // Download the full checkpoint for this sequence number
    let full_check_point = object_store
        .get_full_checkpoint(seq)
        .await
        .map_err(|e| anyhow!(format!("Cannot get full checkpoint: {e}")))?;

    // Load the list of stored checkpoints
    let checkpoints_list: CheckpointList = read_checkpoint_list(config)?;

    // find the stored checkpoint before the seq checkpoint
    let prev_ckp_id = checkpoints_list
        .checkpoints()
        .iter()
        .filter(|ckp_id| **ckp_id < seq)
        .last();

    let committee = if let Some(prev_ckp_id) = prev_ckp_id {
        // Read it from the store
        let prev_ckp = read_checkpoint(config, *prev_ckp_id)?;

        // Check we have the right checkpoint
        anyhow::ensure!(
            prev_ckp.epoch().checked_add(1).unwrap() == full_check_point.checkpoint_summary.epoch(),
            "Checkpoint sequence number does not match. Need to Sync."
        );

        // Get the committee from the previous checkpoint
        let current_committee = prev_ckp
            .end_of_epoch_data
            .as_ref()
            .ok_or(anyhow!(
                "Expected all checkpoints to be end-of-epoch checkpoints"
            ))?
            .next_epoch_committee
            .iter()
            .cloned()
            .collect();

        // Make a committee object using this
        Committee::new(prev_ckp.epoch().checked_add(1).unwrap(), current_committee)
    } else {
        // Since we did not find a small committee checkpoint we use the genesis
        let mut genesis_path = config.checkpoints_sync_dir.clone();
        genesis_path.push(&config.genesis_filename);
        Genesis::load(&genesis_path)?
            .committee()
            .map_err(|e| anyhow!(format!("Cannot load Genesis: {e}")))?
    };

    info!("Extracting effects and events for TID: {}", tid);
    extract_verified_effects_and_events(&full_check_point, &committee, tid)
        .map_err(|e| anyhow!(format!("Cannot extract effects and events: {e}")))
}

/// Get the verified checkpoint sequence number for an object.
/// This function will verify that the object is in the transaction's effects,
/// and that the transaction is in the checkpoint
/// and that the checkpoint is signed by the committee
/// and the committee is read from the verified checkpoint summary
/// which is signed by the previous committee.
pub async fn get_verified_checkpoint(
    id: ObjectID,
    config: &Config,
) -> Result<CheckpointSequenceNumber> {
    let iota_client: iota_sdk::IotaClient = IotaClientBuilder::default()
        .build(config.full_node_url.as_str())
        .await?;
    let read_api = iota_client.read_api();
    let object_json = read_api
        .get_object_with_options(id, IotaObjectDataOptions::bcs_lossless())
        .await
        .expect("Cannot get object");
    let object = object_json
        .into_object()
        .expect("Cannot make into object data");
    let object: Object = object.try_into().expect("Cannot reconstruct object");

    // Lookup the transaction id and get the checkpoint sequence number
    let options = IotaTransactionBlockResponseOptions::new();
    let seq = read_api
        .get_transaction_with_options(object.previous_transaction, options)
        .await
        .map_err(|e| anyhow!(format!("Cannot get transaction: {e}")))?
        .checkpoint
        .ok_or(anyhow!("Transaction not found"))?;

    // Need to authenticate this object
    let (effects, _) = get_verified_effects_and_events(config, object.previous_transaction)
        .await
        .expect("Cannot get effects and events");

    // check that this object ID, version and hash is in the effects
    let target_object_ref = object.compute_object_reference();
    effects
        .all_changed_objects()
        .iter()
        .find(|object_ref| object_ref.0 == target_object_ref)
        .ok_or(anyhow!("Object not found"))
        .expect("Object not found");

    // Create object store
    let object_store = IotaObjectStore::new(config)?;

    // Download the full checkpoint for this sequence number
    let full_check_point = object_store
        .get_full_checkpoint(seq)
        .await
        .map_err(|e| anyhow!(format!("Cannot get full checkpoint: {e}")))?;

    // Load the list of stored checkpoints
    let checkpoints_list: CheckpointList = read_checkpoint_list(config)?;

    // find the stored checkpoint before the seq checkpoint
    let prev_ckp_id = checkpoints_list
        .checkpoints()
        .iter()
        .filter(|ckp_id| **ckp_id < seq)
        .last();

    let committee = if let Some(prev_ckp_id) = prev_ckp_id {
        // Read it from the store
        let prev_ckp = read_checkpoint(config, *prev_ckp_id)?;

        // Check we have the right checkpoint
        anyhow::ensure!(
            prev_ckp.epoch().checked_add(1).unwrap() == full_check_point.checkpoint_summary.epoch(),
            "Checkpoint sequence number does not match. Need to Sync."
        );

        // Get the committee from the previous checkpoint
        let current_committee = prev_ckp
            .end_of_epoch_data
            .as_ref()
            .ok_or(anyhow!(
                "Expected all checkpoints to be end-of-epoch checkpoints"
            ))?
            .next_epoch_committee
            .iter()
            .cloned()
            .collect();

        // Make a committee object using this
        Committee::new(prev_ckp.epoch().checked_add(1).unwrap(), current_committee)
    } else {
        // Since we did not find a small committee checkpoint we use the genesis
        let mut genesis_path = config.checkpoints_sync_dir.clone();
        genesis_path.push(&config.genesis_filename);
        Genesis::load(&genesis_path)?
            .committee()
            .map_err(|e| anyhow!(format!("Cannot load Genesis: {e}")))?
    };

    // Verify that committee signed this checkpoint and checkpoint contents with
    // digest
    full_check_point
        .checkpoint_summary
        .verify_with_contents(&committee, Some(&full_check_point.checkpoint_contents))?;

    if full_check_point
        .transactions
        .iter()
        .any(|t| *t.transaction.digest() == object.previous_transaction)
    {
        Ok(seq)
    } else {
        Err(anyhow!("Transaction not found in checkpoint"))
    }
}

// Make a test namespace
#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Read, Write},
        path::{Path, PathBuf},
        str::FromStr,
    };

    use iota_types::{
        crypto::AuthorityQuorumSignInfo,
        event::Event,
        message_envelope::Envelope,
        messages_checkpoint::{
            CertifiedCheckpointSummary, CheckpointSummary, FullCheckpointContents,
        },
    };

    use super::*;

    const FIXTURES_DIR: &str = "tests/fixtures";
    const FIXTURE_1: &str = "235.sum";
    const FIXTURE_2: &str = "469.chk";

    async fn read_checkpoint_summary(
        checkpoint_path: &PathBuf,
    ) -> anyhow::Result<CertifiedCheckpointSummary> {
        let mut reader = fs::File::open(checkpoint_path.clone()).unwrap();
        let metadata = fs::metadata(&checkpoint_path).unwrap();
        let mut buffer = vec![0; metadata.len() as usize];
        reader.read_exact(&mut buffer).unwrap();
        bcs::from_bytes(&buffer).map_err(|_| anyhow!("Unable to parse checkpoint summary file"))
    }

    async fn read_full_checkpoint(checkpoint_path: &PathBuf) -> anyhow::Result<CheckpointData> {
        let mut reader = fs::File::open(checkpoint_path.clone())?;
        let metadata = fs::metadata(checkpoint_path)?;
        let mut buffer = vec![0; metadata.len() as usize];
        reader.read_exact(&mut buffer)?;
        bcs::from_bytes(&buffer).map_err(|_| anyhow!("Unable to parse full checkpoint file"))
    }

    async fn read_data() -> (Committee, CheckpointData) {
        let checkpoint_summary_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURES_DIR)
            .join(FIXTURE_1);
        let summary_checkpoint = read_checkpoint_summary(&checkpoint_summary_path)
            .await
            .unwrap();
        let prev_committee = summary_checkpoint
            .end_of_epoch_data
            .as_ref()
            .ok_or(anyhow!(
                "Expected all checkpoints to be end-of-epoch checkpoints"
            ))
            .unwrap()
            .next_epoch_committee
            .iter()
            .cloned()
            .collect();
        let committee = Committee::new(
            summary_checkpoint.epoch().checked_add(1).unwrap(),
            prev_committee,
        );
        let full_checkpoint_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURES_DIR)
            .join(FIXTURE_2);
        let full_checkpoint = read_full_checkpoint(&full_checkpoint_path).await.unwrap();

        (committee, full_checkpoint)
    }

    #[tokio::test]
    async fn test_checkpoint_all_good() {
        let (committee, full_checkpoint) = read_data().await;
        let tx_digest_0 = *full_checkpoint.transactions[0].transaction.digest();

        extract_verified_effects_and_events(&full_checkpoint, &committee, tx_digest_0).unwrap();
    }

    #[tokio::test]
    async fn test_checkpoint_bad_committee() {
        let (mut committee, full_checkpoint) = read_data().await;
        let tx_digest_0 = *full_checkpoint.transactions[0].transaction.digest();

        // Change committee
        committee.epoch += 10;

        assert!(
            extract_verified_effects_and_events(&full_checkpoint, &committee, tx_digest_0,)
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_checkpoint_no_transaction() {
        let (committee, full_checkpoint) = read_data().await;

        assert!(
            extract_verified_effects_and_events(
                &full_checkpoint,
                &committee,
                // tx does not exist
                TransactionDigest::from_str("11111111111111111111111111111111").unwrap(),
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn test_checkpoint_bad_contents() {
        let (committee, mut full_checkpoint) = read_data().await;
        let tx_digest_0 = *full_checkpoint.transactions[0].transaction.digest();

        // Change contents
        let random_contents = FullCheckpointContents::random_for_testing();
        full_checkpoint.checkpoint_contents = random_contents.checkpoint_contents();

        assert!(
            extract_verified_effects_and_events(&full_checkpoint, &committee, tx_digest_0,)
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_checkpoint_bad_events() {
        let (committee, mut full_checkpoint) = read_data().await;
        // Add a random event to the transaction, so the event digest doesn't match
        let tx0 = &mut full_checkpoint.transactions[0];
        let tx_digest_0 = *tx0.transaction.digest();

        if tx0.events.is_none() {
            // if there are no events yet, add them
            tx0.events = Some(TransactionEvents {
                data: vec![Event::random_for_testing()],
            });
        } else {
            tx0.events
                .as_mut()
                .unwrap()
                .data
                .push(Event::random_for_testing());
        }

        assert!(
            extract_verified_effects_and_events(&full_checkpoint, &committee, tx_digest_0,)
                .is_err()
        );
    }
}
