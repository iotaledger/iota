// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;
use iota_json_rpc_types::Checkpoint as RpcCheckpoint;
use iota_sdk_types::gas::GasCostSummary;
use iota_types::{
    base_types::TransactionDigest,
    digests::{CheckpointContentsDigest, CheckpointDigest},
    messages_checkpoint::CheckpointSummary,
};

use crate::{
    errors::IndexerError,
    schema::{chain_identifier, checkpoints, pruner_cp_watermark},
    types::IndexedCheckpoint,
};

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = chain_identifier)]
pub struct StoredChainIdentifier {
    pub checkpoint_digest: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = checkpoints)]
pub struct StoredCheckpoint {
    pub sequence_number: i64,
    pub checkpoint_digest: Vec<u8>,
    pub epoch: i64,
    pub network_total_transactions: i64,
    pub previous_checkpoint_digest: Option<Vec<u8>>,
    pub end_of_epoch: bool,
    pub tx_digests: Vec<Option<Vec<u8>>>,
    pub timestamp_ms: i64,
    pub total_gas_cost: i64,
    pub computation_cost: i64,
    pub storage_cost: i64,
    pub storage_rebate: i64,
    pub non_refundable_storage_fee: i64,
    pub checkpoint_commitments: Vec<u8>,
    pub validator_signature: Vec<u8>,
    pub end_of_epoch_data: Option<Vec<u8>>,
    pub min_tx_sequence_number: Option<i64>,
    pub max_tx_sequence_number: Option<i64>,
    pub computation_cost_burned: Option<i64>,
    pub content_digest: Option<Vec<u8>>,
    pub version_specific_data: Option<Vec<u8>>,
}

impl StoredCheckpoint {
    /// Get or derive the `computation_cost_burned`.
    pub fn computation_cost_burned(&self) -> u64 {
        self.computation_cost_burned
            .unwrap_or(self.computation_cost) as u64
    }
}

impl From<&IndexedCheckpoint> for StoredCheckpoint {
    fn from(c: &IndexedCheckpoint) -> Self {
        Self {
            sequence_number: c.sequence_number as i64,
            checkpoint_digest: c.checkpoint_digest.into_inner().to_vec(),
            epoch: c.epoch as i64,
            tx_digests: c
                .tx_digests
                .iter()
                .map(|tx| Some(tx.into_inner().to_vec()))
                .collect(),
            network_total_transactions: c.network_total_transactions as i64,
            previous_checkpoint_digest: c
                .previous_checkpoint_digest
                .as_ref()
                .map(|d| (*d).into_inner().to_vec()),
            timestamp_ms: c.timestamp_ms as i64,
            total_gas_cost: c.total_gas_cost,
            computation_cost: c.computation_cost as i64,
            computation_cost_burned: Some(c.computation_cost_burned as i64),
            storage_cost: c.storage_cost as i64,
            storage_rebate: c.storage_rebate as i64,
            non_refundable_storage_fee: c.non_refundable_storage_fee as i64,
            checkpoint_commitments: bcs::to_bytes(&c.checkpoint_commitments).unwrap(),
            validator_signature: bcs::to_bytes(&c.validator_signature).unwrap(),
            end_of_epoch_data: c
                .end_of_epoch_data
                .as_ref()
                .map(|d| bcs::to_bytes(d).unwrap()),
            end_of_epoch: c.end_of_epoch_data.is_some(),
            min_tx_sequence_number: Some(c.min_tx_sequence_number as i64),
            max_tx_sequence_number: Some(c.max_tx_sequence_number as i64),
            content_digest: Some(c.content_digest.into_inner().to_vec()),
            version_specific_data: Some(c.version_specific_data.clone()),
        }
    }
}

impl TryFrom<StoredCheckpoint> for RpcCheckpoint {
    type Error = IndexerError;
    fn try_from(checkpoint: StoredCheckpoint) -> Result<RpcCheckpoint, IndexerError> {
        let computation_cost_burned = checkpoint.computation_cost_burned();
        let parsed_digest = CheckpointDigest::from_bytes(checkpoint.checkpoint_digest.clone())
            .map_err(|e| {
                IndexerError::PersistentStorageDataCorruption(format!(
                    "Failed to decode checkpoint digest: {:?} with err: {:?}",
                    checkpoint.checkpoint_digest, e
                ))
            })?;

        let parsed_previous_digest: Option<CheckpointDigest> = checkpoint
            .previous_checkpoint_digest
            .map(|digest| {
                CheckpointDigest::from_bytes(digest.clone()).map_err(|e| {
                    IndexerError::PersistentStorageDataCorruption(format!(
                        "Failed to decode previous checkpoint digest: {digest:?} with err: {e:?}"
                    ))
                })
            })
            .transpose()?;

        let transactions: Vec<TransactionDigest> = {
            {
                checkpoint
                    .tx_digests
                    .into_iter()
                    .map(|tx_digest| match tx_digest {
                        None => Err(IndexerError::PersistentStorageDataCorruption(
                            "tx_digests should not contain null elements".to_string(),
                        )),
                        Some(tx_digest) => TransactionDigest::from_bytes(tx_digest.as_slice())
                            .map_err(|e| {
                                IndexerError::PersistentStorageDataCorruption(format!(
                                    "Failed to decode transaction digest: {tx_digest:?} with err: {e:?}"
                                ))
                            }),
                    })
                    .collect::<Result<Vec<TransactionDigest>, IndexerError>>()?
            }
        };
        let validator_signature =
            bcs::from_bytes(&checkpoint.validator_signature).map_err(|e| {
                IndexerError::PersistentStorageDataCorruption(format!(
                    "Failed to decode validator signature: {:?} with err: {:?}",
                    checkpoint.validator_signature, e
                ))
            })?;

        let checkpoint_commitments =
            bcs::from_bytes(&checkpoint.checkpoint_commitments).map_err(|e| {
                IndexerError::PersistentStorageDataCorruption(format!(
                    "Failed to decode checkpoint commitments: {:?} with err: {:?}",
                    checkpoint.checkpoint_commitments, e
                ))
            })?;

        let end_of_epoch_data = checkpoint
            .end_of_epoch_data
            .as_ref()
            .map(|data| {
                bcs::from_bytes(data).map_err(|e| {
                    IndexerError::PersistentStorageDataCorruption(format!(
                        "Failed to decode end of epoch data: {data:?} with err: {e:?}"
                    ))
                })
            })
            .transpose()?;

        Ok(RpcCheckpoint {
            epoch: checkpoint.epoch as u64,
            sequence_number: checkpoint.sequence_number as u64,
            digest: parsed_digest,
            previous_digest: parsed_previous_digest,
            end_of_epoch_data,
            epoch_rolling_gas_cost_summary: GasCostSummary {
                computation_cost: checkpoint.computation_cost as u64,
                computation_cost_burned,
                storage_cost: checkpoint.storage_cost as u64,
                storage_rebate: checkpoint.storage_rebate as u64,
                non_refundable_storage_fee: checkpoint.non_refundable_storage_fee as u64,
            },
            network_total_transactions: checkpoint.network_total_transactions as u64,
            timestamp_ms: checkpoint.timestamp_ms as u64,
            transactions,
            validator_signature,
            checkpoint_commitments,
        })
    }
}

impl TryFrom<StoredCheckpoint> for CheckpointSummary {
    type Error = IndexerError;

    fn try_from(checkpoint: StoredCheckpoint) -> Result<CheckpointSummary, IndexerError> {
        let computation_cost_burned = checkpoint.computation_cost_burned();

        let content_digest_bytes = checkpoint.content_digest.ok_or_else(|| {
            IndexerError::PersistentStorageDataCorruption(
                "checkpoint content_digest is missing; re-index to populate it".to_string(),
            )
        })?;
        let content_digest = CheckpointContentsDigest::from_bytes(content_digest_bytes.clone())
            .map_err(|e| {
                IndexerError::PersistentStorageDataCorruption(format!(
                    "Failed to decode content digest: {content_digest_bytes:?} with err: {e:?}"
                ))
            })?;

        let version_specific_data = checkpoint.version_specific_data.ok_or_else(|| {
            IndexerError::PersistentStorageDataCorruption(
                "checkpoint version_specific_data is missing; re-index to populate it".to_string(),
            )
        })?;

        let previous_digest = checkpoint
            .previous_checkpoint_digest
            .map(|digest| {
                CheckpointDigest::from_bytes(digest.clone()).map_err(|e| {
                    IndexerError::PersistentStorageDataCorruption(format!(
                        "Failed to decode previous checkpoint digest: {digest:?} with err: {e:?}"
                    ))
                })
            })
            .transpose()?;

        let checkpoint_commitments =
            bcs::from_bytes(&checkpoint.checkpoint_commitments).map_err(|e| {
                IndexerError::PersistentStorageDataCorruption(format!(
                    "Failed to decode checkpoint commitments: {:?} with err: {:?}",
                    checkpoint.checkpoint_commitments, e
                ))
            })?;

        let end_of_epoch_data = checkpoint
            .end_of_epoch_data
            .as_ref()
            .map(|data| {
                bcs::from_bytes(data).map_err(|e| {
                    IndexerError::PersistentStorageDataCorruption(format!(
                        "Failed to decode end of epoch data: {data:?} with err: {e:?}"
                    ))
                })
            })
            .transpose()?;

        Ok(CheckpointSummary {
            epoch: checkpoint.epoch as u64,
            sequence_number: checkpoint.sequence_number as u64,
            network_total_transactions: checkpoint.network_total_transactions as u64,
            content_digest,
            previous_digest,
            epoch_rolling_gas_cost_summary: GasCostSummary {
                computation_cost: checkpoint.computation_cost as u64,
                computation_cost_burned,
                storage_cost: checkpoint.storage_cost as u64,
                storage_rebate: checkpoint.storage_rebate as u64,
                non_refundable_storage_fee: checkpoint.non_refundable_storage_fee as u64,
            },
            timestamp_ms: checkpoint.timestamp_ms as u64,
            checkpoint_commitments,
            end_of_epoch_data,
            version_specific_data,
        })
    }
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = pruner_cp_watermark)]
pub struct StoredCpTx {
    pub checkpoint_sequence_number: i64,
    pub min_tx_sequence_number: i64,
    pub max_tx_sequence_number: i64,
}

impl From<&IndexedCheckpoint> for StoredCpTx {
    fn from(c: &IndexedCheckpoint) -> Self {
        Self {
            checkpoint_sequence_number: c.sequence_number as i64,
            min_tx_sequence_number: c.min_tx_sequence_number as i64,
            max_tx_sequence_number: c.max_tx_sequence_number as i64,
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_types::message_envelope::Message;

    use super::*;

    #[test]
    fn checkpoint_summary_rebuilds_from_stored_columns() {
        let summary = CheckpointSummary {
            epoch: 7,
            sequence_number: 42,
            network_total_transactions: 100,
            content_digest: CheckpointContentsDigest::new([1u8; 32]),
            previous_digest: Some(CheckpointDigest::new([2u8; 32])),
            epoch_rolling_gas_cost_summary: GasCostSummary {
                computation_cost: 10,
                computation_cost_burned: 4,
                storage_cost: 20,
                storage_rebate: 5,
                non_refundable_storage_fee: 1,
            },
            timestamp_ms: 1_700_000_000_000,
            checkpoint_commitments: vec![],
            end_of_epoch_data: None,
            version_specific_data: vec![1, 2, 3, 4],
        };

        let stored = StoredCheckpoint {
            sequence_number: summary.sequence_number as i64,
            checkpoint_digest: summary.digest().into_inner().to_vec(),
            epoch: summary.epoch as i64,
            network_total_transactions: summary.network_total_transactions as i64,
            previous_checkpoint_digest: summary.previous_digest.map(|d| d.into_inner().to_vec()),
            end_of_epoch: summary.end_of_epoch_data.is_some(),
            tx_digests: vec![],
            timestamp_ms: summary.timestamp_ms as i64,
            total_gas_cost: 0,
            computation_cost: summary.epoch_rolling_gas_cost_summary.computation_cost as i64,
            storage_cost: summary.epoch_rolling_gas_cost_summary.storage_cost as i64,
            storage_rebate: summary.epoch_rolling_gas_cost_summary.storage_rebate as i64,
            non_refundable_storage_fee: summary
                .epoch_rolling_gas_cost_summary
                .non_refundable_storage_fee as i64,
            checkpoint_commitments: bcs::to_bytes(&summary.checkpoint_commitments).unwrap(),
            validator_signature: vec![],
            end_of_epoch_data: summary
                .end_of_epoch_data
                .as_ref()
                .map(|d| bcs::to_bytes(d).unwrap()),
            min_tx_sequence_number: None,
            max_tx_sequence_number: None,
            computation_cost_burned: Some(
                summary.epoch_rolling_gas_cost_summary.computation_cost_burned as i64,
            ),
            content_digest: Some(summary.content_digest.into_inner().to_vec()),
            version_specific_data: Some(summary.version_specific_data.clone()),
        };

        let rebuilt = CheckpointSummary::try_from(stored).unwrap();
        assert_eq!(summary, rebuilt);
        // The rebuilt summary must hash to the same canonical checkpoint digest.
        assert_eq!(summary.digest(), rebuilt.digest());
    }
}
