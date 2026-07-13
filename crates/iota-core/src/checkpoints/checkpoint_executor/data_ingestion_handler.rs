// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, path::Path};

use iota_storage::blob::{Blob, BlobEncoding};
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::{IotaError, IotaResult},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    object::Object,
    storage::{ObjectKey, ObjectStore},
};

use crate::{
    authority::historic_store::HistoricStore,
    checkpoints::checkpoint_executor::{CheckpointExecutionData, CheckpointTransactionData},
    execution_cache::TransactionCacheRead,
};

/// The input pre-images of `fx`, from the transaction's still-buffered
/// in-memory outputs when available (the common case: checkpoint data is
/// assembled before the outputs are committed), otherwise from the store,
/// with a final fallback to the historic epoch buckets for replay after a
/// restart, where the versions were already relocated.
fn transaction_input_objects(
    fx: &iota_types::effects::TransactionEffects,
    outputs: Option<&crate::transaction_outputs::TransactionOutputs>,
    object_store: &dyn ObjectStore,
    historic_store: &HistoricStore,
) -> IotaResult<Vec<Object>> {
    let carried: HashMap<ObjectKey, &Object> = outputs
        .map(|outputs| {
            outputs
                .superseded
                .iter()
                .map(|(key, object)| (*key, object))
                .collect()
        })
        .unwrap_or_default();

    fx.modified_at_versions()
        .into_iter()
        .map(|(object_id, version)| {
            let key = ObjectKey(object_id, version);
            if let Some(object) = carried.get(&key) {
                return Ok((*object).clone());
            }
            if let Some(object) = object_store
                .try_get_object_by_key(&object_id, version)
                .map_err(|e| IotaError::Unknown(e.to_string()))?
            {
                return Ok(object);
            }
            historic_store
                .get_object(&key)?
                .ok_or(IotaError::UserInput {
                    error: iota_types::error::UserInputError::ObjectNotFound {
                        object_id,
                        version: Some(version),
                    },
                })
        })
        .collect()
}

pub(crate) fn load_checkpoint_data(
    checkpoint_exec_data: &CheckpointExecutionData,
    checkpoint_tx_data: &CheckpointTransactionData,
    object_store: &dyn ObjectStore,
    transaction_cache_reader: &dyn TransactionCacheRead,
    historic_store: &HistoricStore,
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

        let outputs =
            transaction_cache_reader.try_get_pending_transaction_outputs(fx.transaction_digest());
        let input_objects =
            transaction_input_objects(fx, outputs.as_deref(), object_store, historic_store)?;
        let output_objects = match &outputs {
            // Written objects are carried in the buffered outputs; no store
            // lookups needed.
            Some(outputs) => fx
                .all_changed_objects()
                .into_iter()
                .map(|(object_ref, _, _)| {
                    outputs
                        .written
                        .get(&object_ref.object_id)
                        .filter(|object| object.version() == object_ref.version)
                        .cloned()
                        .ok_or(IotaError::UserInput {
                            error: iota_types::error::UserInputError::ObjectNotFound {
                                object_id: object_ref.object_id,
                                version: Some(object_ref.version),
                            },
                        })
                })
                .collect::<IotaResult<Vec<_>>>()?,
            // Without buffered outputs (replay after restart, or a stage
            // lagging behind later commits), read the store with a historic
            // fallback: a later transaction may already have superseded an
            // output here and relocated it (e.g. the clock output of an old
            // checkpoint during catch-up).
            None => fx
                .all_changed_objects()
                .into_iter()
                .map(|(object_ref, _, _)| {
                    let key = ObjectKey::from(object_ref);
                    if let Some(object) = object_store
                        .try_get_object_by_key(&key.0, key.1)
                        .map_err(|e| IotaError::Unknown(e.to_string()))?
                    {
                        return Ok(object);
                    }
                    historic_store
                        .get_object(&key)?
                        .ok_or(IotaError::UserInput {
                            error: iota_types::error::UserInputError::ObjectNotFound {
                                object_id: key.0,
                                version: Some(key.1),
                            },
                        })
                })
                .collect::<IotaResult<Vec<_>>>()?,
        };

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
