// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, path::Path};

use iota_storage::blob::{Blob, BlobEncoding};
use iota_types::{
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    storage::ObjectStore,
};

use crate::{
    checkpoints::checkpoint_executor::{CheckpointExecutionData, CheckpointTransactionData},
    execution_cache::TransactionCacheRead,
};

pub(crate) fn load_checkpoint_data(
    checkpoint_exec_data: &CheckpointExecutionData,
    checkpoint_tx_data: &CheckpointTransactionData,
    object_store: &dyn ObjectStore,
    transaction_cache_reader: &dyn TransactionCacheRead,
) -> IotaResult<CheckpointData> {
    let event_tx_digests = checkpoint_tx_data
        .effects
        .iter()
        .flat_map(|fx| fx.events_digest().map(|_| fx.transaction_digest()).copied())
        .collect::<Vec<_>>();

    let events = transaction_cache_reader
        .try_multi_get_events(&event_tx_digests)?
        .into_iter()
        .zip(event_tx_digests)
        .map(|(maybe_event, tx_digest)| {
            maybe_event
                .ok_or(IotaError::TransactionEventsNotFound { digest: tx_digest })
                .map(|event| (tx_digest, event))
        })
        .collect::<IotaResult<HashMap<_, _>>>()?;

    let mut full_transactions = Vec::with_capacity(checkpoint_tx_data.transactions.len());
    for (tx, fx) in checkpoint_tx_data
        .transactions
        .iter()
        .zip(checkpoint_tx_data.effects.iter())
    {
        let events = fx.events_digest().map(|_event_digest| {
            events
                .get(fx.transaction_digest())
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
