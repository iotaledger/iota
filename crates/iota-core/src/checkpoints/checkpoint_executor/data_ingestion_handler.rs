// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
};

use iota_storage::blob::{Blob, BlobEncoding};
use iota_types::{
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
    full_checkpoint_content::{Checkpoint, CheckpointData, ExecutedTransaction},
    object::ObjectSet,
    storage::ObjectStore,
};

use crate::{
    checkpoints::checkpoint_executor::{CheckpointExecutionData, CheckpointTransactionData},
    execution_cache::TransactionCacheRead,
};

pub(crate) fn load_checkpoint(
    checkpoint_exec_data: &CheckpointExecutionData,
    checkpoint_tx_data: &CheckpointTransactionData,
    object_store: &dyn ObjectStore,
    transaction_cache_reader: &dyn TransactionCacheRead,
) -> IotaResult<Checkpoint> {
    let event_tx_digests = checkpoint_tx_data
        .effects
        .iter()
        .flat_map(|fx| fx.events_digest().map(|_| fx.transaction_digest()).copied())
        .collect::<Vec<_>>();

    let mut events = transaction_cache_reader
        .try_multi_get_events(&event_tx_digests)?
        .into_iter()
        .zip(event_tx_digests)
        .map(|(maybe_event, tx_digest)| {
            maybe_event
                .ok_or(IotaError::TransactionEventsNotFound { digest: tx_digest })
                .map(|event| (tx_digest, event))
        })
        .collect::<IotaResult<HashMap<_, _>>>()?;

    let mut transactions = Vec::with_capacity(checkpoint_tx_data.transactions.len());
    for (tx, fx) in checkpoint_tx_data
        .transactions
        .iter()
        .zip(checkpoint_tx_data.effects.iter())
    {
        let events = fx.events_digest().map(|_event_digest| {
            events
                .remove(fx.transaction_digest())
                .expect("event was already checked to be present")
        });

        let transaction = ExecutedTransaction {
            transaction: tx.data().transaction().clone(),
            signatures: tx.data().signatures().to_vec(),
            effects: fx.clone(),
            events,
            unchanged_loaded_runtime_objects: transaction_cache_reader
                .get_unchanged_loaded_runtime_objects(tx.digest())
                // We don't write empty sets to the DB to save space, so if this load went
                // through the writeback cache to the DB itself it wouldn't find an entry.
                .unwrap_or_default(),
        };
        transactions.push(transaction);
    }

    let object_set = {
        let refs = transactions
            .iter()
            .flat_map(|tx| {
                iota_types::storage::get_transaction_object_set(
                    &tx.transaction,
                    &tx.effects,
                    &tx.unchanged_loaded_runtime_objects,
                )
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        let objects = object_store.multi_get_objects_by_key(&refs);

        let mut object_set = ObjectSet::default();
        for (idx, object) in objects.into_iter().enumerate() {
            object_set.insert(object.ok_or_else(|| {
                iota_types::storage::error::Error::custom(format!(
                    "unable to load object {:?}",
                    refs[idx]
                ))
            })?);
        }
        object_set
    };
    let checkpoint = Checkpoint {
        summary: checkpoint_exec_data.checkpoint.clone().into(),
        contents: checkpoint_exec_data.checkpoint_contents.clone(),
        transactions,
        object_set,
    };
    Ok(checkpoint)
}

pub(crate) fn store_checkpoint_locally(
    path: impl AsRef<Path>,
    checkpoint_data: &CheckpointData,
) -> IotaResult {
    let path = path.as_ref();
    let file_name = format!("{}.chk", checkpoint_data.checkpoint_summary.sequence_number);

    std::fs::create_dir_all(path).map_err(|err| {
        IotaError::FileIO(format!(
            "failed to save full checkpoint content locally {err:?}"
        ))
    })?;

    Blob::encode(&checkpoint_data, BlobEncoding::Bcs)
        .map_err(|_| IotaError::TransactionSerialization {
            error: "failed to serialize full checkpoint content".to_string(),
        }) // Map the first error
        .and_then(|blob| {
            std::fs::write(path.join(file_name), blob.to_bytes()).map_err(|_| {
                IotaError::FileIO("failed to save full checkpoint content locally".to_string())
            })
        })?;

    Ok(())
}
