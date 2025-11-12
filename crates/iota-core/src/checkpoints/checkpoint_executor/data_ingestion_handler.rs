// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, path::Path};

use iota_storage::blob::{Blob, BlobEncoding};
use iota_types::{
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult, UserInputError},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    storage::ObjectStore,
};

use crate::{
    checkpoints::checkpoint_executor::CheckpointExecutionData,
    execution_cache::{ObjectCacheRead, TransactionCacheRead},
};

pub(crate) fn load_checkpoint_data(
    checkpoint_exec_data: &CheckpointExecutionData,
    object_store: &dyn ObjectStore,
    transaction_cache_reader: &dyn TransactionCacheRead,
) -> IotaResult<CheckpointData> {
    let event_digests = checkpoint_exec_data
        .effects
        .iter()
        .flat_map(|fx| fx.events_digest().copied())
        .collect::<Vec<_>>();

    let events = transaction_cache_reader
        .try_multi_get_events(&event_digests)?
        .into_iter()
        .zip(&event_digests)
        .map(|(event, digest)| {
            event.ok_or(IotaError::TransactionEventsNotFound { digest: *digest })
        })
        .collect::<IotaResult<Vec<_>>>()?;

    let events: HashMap<_, _> = event_digests.into_iter().zip(events).collect();
    let mut full_transactions = Vec::with_capacity(checkpoint_exec_data.transactions.len());
    for (tx, fx) in checkpoint_exec_data
        .transactions
        .iter()
        .zip(checkpoint_exec_data.effects.iter())
    {
        let events = fx.events_digest().map(|event_digest| {
            events
                .get(event_digest)
                .cloned()
                .expect("event was already checked to be present")
        });

        let input_objects = iota_types::storage::get_transaction_input_objects(object_store, fx)
            .map_err(|e| IotaError::Unknown(e.to_string()))?;
        let output_objects = iota_types::storage::get_transaction_output_objects(object_store, fx)
            .map_err(|e| IotaError::Unknown(e.to_string()))?;

        let full_transaction = CheckpointTransaction {
            transaction: (*tx).clone().into_unsigned().into(),
            effects: fx.clone(),
            events,
            input_objects,
            output_objects,
        };
        full_transactions.push(full_transaction);
    }
    let checkpoint_data = CheckpointData {
        checkpoint_summary: checkpoint_exec_data.checkpoint.clone().into(),
        checkpoint_contents: checkpoint_exec_data.checkpoint_contents.clone(),
        transactions: full_transactions,
    };
    Ok(checkpoint_data)
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
