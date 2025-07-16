// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_bigtable::{
    BigTableClient, Cell, Row,
    proto::bigtable::v2::{RowFilter, row_filter::Filter},
};
use iota_types::{
    base_types::{ObjectID, TransactionDigest},
    digests::CheckpointDigest,
    effects::{TransactionEffects, TransactionEvents},
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber, CheckpointSummary,
    },
    object::Object,
    storage::ObjectKey,
    transaction::Transaction,
};
use tracing::error;

use crate::{Checkpoint, KeyValueStoreReader, KeyValueStoreWriter, TransactionData};

const OBJECTS_TABLE: &str = "objects";
const TRANSACTIONS_TABLE: &str = "transactions";
const CHECKPOINTS_TABLE: &str = "checkpoints";
const CHECKPOINTS_BY_DIGEST_TABLE: &str = "checkpoints_by_digest";
const WATERMARK_TABLE: &str = "watermark";

const DEFAULT_COLUMN_QUALIFIER: &str = "";
const CHECKPOINT_SUMMARY_COLUMN_QUALIFIER: &str = "cs";
const CHECKPOINT_CONTENTS_COLUMN_QUALIFIER: &str = "cc";
const TRANSACTION_COLUMN_QUALIFIER: &str = "tx";
const EFFECTS_COLUMN_QUALIFIER: &str = "fx";
const EVENTS_COLUMN_QUALIFIER: &str = "evtx";
const TRANSACTION_TO_CHECKPOINT: &str = "tx2c";

#[async_trait]
impl KeyValueStoreWriter for BigTableClient {
    type Error = anyhow::Error;

    async fn save_objects(&mut self, objects: &[&Object]) -> Result<(), Self::Error> {
        let mut rows = Vec::with_capacity(objects.len());
        for object in objects {
            let object_key = ObjectKey(object.id(), object.version());
            let cells = vec![Cell::new(
                DEFAULT_COLUMN_QUALIFIER.as_bytes().to_vec(),
                bcs::to_bytes(object)?,
            )];
            rows.push(Row::new(raw_object_key(&object_key), cells));
        }
        self.multi_set(OBJECTS_TABLE, rows)
            .await
            .map_err(Into::into)
    }

    async fn save_transactions(
        &mut self,
        transactions: &[TransactionData],
    ) -> Result<(), Self::Error> {
        let mut rows = Vec::with_capacity(transactions.len());
        for TransactionData {
            transaction,
            effects,
            events,
            checkpoint_number,
        } in transactions
        {
            let cells = vec![
                Cell::new(
                    TRANSACTION_COLUMN_QUALIFIER.as_bytes().to_vec(),
                    bcs::to_bytes(transaction)?,
                ),
                Cell::new(
                    EFFECTS_COLUMN_QUALIFIER.as_bytes().to_vec(),
                    bcs::to_bytes(effects)?,
                ),
                Cell::new(
                    EVENTS_COLUMN_QUALIFIER.as_bytes().to_vec(),
                    bcs::to_bytes(events)?,
                ),
                Cell::new(
                    TRANSACTION_TO_CHECKPOINT.as_bytes().to_vec(),
                    bcs::to_bytes(checkpoint_number)?,
                ),
            ];
            rows.push(Row::new(transaction.digest().inner().to_vec(), cells));
        }
        self.multi_set(TRANSACTIONS_TABLE, rows)
            .await
            .map_err(Into::into)
    }

    async fn save_checkpoint(&mut self, checkpoint: &CheckpointData) -> Result<(), Self::Error> {
        let summary = &checkpoint.checkpoint_summary;
        let contents = &checkpoint.checkpoint_contents;
        let key = summary.sequence_number.to_be_bytes().to_vec();
        let cells = vec![
            Cell::new(
                CHECKPOINT_SUMMARY_COLUMN_QUALIFIER.as_bytes().to_vec(),
                bcs::to_bytes(summary)?,
            ),
            Cell::new(
                CHECKPOINT_CONTENTS_COLUMN_QUALIFIER.as_bytes().to_vec(),
                bcs::to_bytes(contents)?,
            ),
        ];
        let row = Row::new(key.clone(), cells);
        self.multi_set(CHECKPOINTS_TABLE, [row]).await?;

        let cells = vec![Cell::new(DEFAULT_COLUMN_QUALIFIER.as_bytes().to_vec(), key)];
        let row = Row::new(
            checkpoint.checkpoint_summary.digest().inner().to_vec(),
            cells,
        );
        self.multi_set(CHECKPOINTS_BY_DIGEST_TABLE, [row])
            .await
            .map_err(Into::into)
    }

    async fn save_watermark(
        &mut self,
        watermark: CheckpointSequenceNumber,
    ) -> Result<(), Self::Error> {
        let cells = vec![Cell::new(
            DEFAULT_COLUMN_QUALIFIER.as_bytes().to_vec(),
            vec![],
        )];
        let row = Row::new(watermark.to_be_bytes().to_vec(), cells);
        self.multi_set(WATERMARK_TABLE, [row])
            .await
            .map_err(Into::into)
    }
}

#[async_trait]
impl KeyValueStoreReader for BigTableClient {
    type Error = anyhow::Error;

    async fn get_objects(&mut self, object_keys: &[ObjectKey]) -> Result<Vec<Object>, Self::Error> {
        let keys = object_keys.iter().map(raw_object_key).collect();
        let mut objects = vec![];
        for row_cells in self.multi_get(OBJECTS_TABLE, keys, None).await? {
            for cell in row_cells {
                let obj = bcs::from_bytes::<Object>(&cell.value)?;
                objects.push(obj);
            }
        }
        Ok(objects)
    }

    async fn get_transactions(
        &mut self,
        transactions: &[TransactionDigest],
    ) -> Result<Vec<TransactionData>, Self::Error> {
        let keys = transactions.iter().map(|tx| tx.inner().to_vec()).collect();
        let mut result = vec![];
        for row_cells in self.multi_get(TRANSACTIONS_TABLE, keys, None).await? {
            let mut transaction = None;
            let mut effects = None;
            let mut events = None;
            let mut checkpoint_number = 0;

            for Cell { name, value } in row_cells {
                match std::str::from_utf8(&name)? {
                    TRANSACTION_COLUMN_QUALIFIER => {
                        transaction = Some(bcs::from_bytes::<Transaction>(&value)?)
                    }
                    EFFECTS_COLUMN_QUALIFIER => {
                        effects = Some(bcs::from_bytes::<TransactionEffects>(&value)?)
                    }
                    EVENTS_COLUMN_QUALIFIER => {
                        events = Some(bcs::from_bytes::<Option<TransactionEvents>>(&value)?)
                    }
                    TRANSACTION_TO_CHECKPOINT => {
                        checkpoint_number = bcs::from_bytes::<CheckpointSequenceNumber>(&value)?
                    }
                    unexpected_cell_name => {
                        error!("unexpected column {unexpected_cell_name:?} in transactions table")
                    }
                }
            }
            result.push(TransactionData {
                transaction: transaction
                    .ok_or_else(|| anyhow::anyhow!("transaction field is missing"))?,
                effects: effects.ok_or_else(|| anyhow::anyhow!("effects field is missing"))?,
                events: events.ok_or_else(|| anyhow::anyhow!("events field is missing"))?,
                checkpoint_number,
            })
        }
        Ok(result)
    }

    async fn get_checkpoints(
        &mut self,
        sequence_numbers: &[CheckpointSequenceNumber],
    ) -> Result<Vec<Checkpoint>, Self::Error> {
        let keys = sequence_numbers
            .iter()
            .map(|sq| sq.to_be_bytes().to_vec())
            .collect();
        let mut checkpoints = vec![];
        for row_cells in self.multi_get(CHECKPOINTS_TABLE, keys, None).await? {
            let mut summary = None;
            let mut contents = None;
            for Cell { name, value } in row_cells {
                match std::str::from_utf8(&name)? {
                    CHECKPOINT_SUMMARY_COLUMN_QUALIFIER => {
                        summary = Some(bcs::from_bytes::<CertifiedCheckpointSummary>(&value)?)
                    }
                    CHECKPOINT_CONTENTS_COLUMN_QUALIFIER => {
                        contents = Some(bcs::from_bytes::<CheckpointContents>(&value)?)
                    }
                    unexpected_cell_name => {
                        error!("unexpected column {unexpected_cell_name:?} in checkpoints table")
                    }
                }
            }
            let checkpoint = Checkpoint {
                summary: summary.ok_or_else(|| anyhow::anyhow!("summary field is missing"))?,
                contents: contents.ok_or_else(|| anyhow::anyhow!("contents field is missing"))?,
            };
            checkpoints.push(checkpoint);
        }
        Ok(checkpoints)
    }

    async fn get_checkpoint_by_digest(
        &mut self,
        digest: CheckpointDigest,
    ) -> Result<Option<Checkpoint>, Self::Error> {
        let key = digest.inner().to_vec();
        let latest_row_cells = self
            .multi_get(CHECKPOINTS_BY_DIGEST_TABLE, vec![key], None)
            .await?
            .pop();
        if let Some(cells) = latest_row_cells {
            if let Some(cell) = cells.into_iter().next() {
                let sequence_number = u64::from_be_bytes(cell.value.as_slice().try_into()?);
                if let Some(chk) = self.get_checkpoints(&[sequence_number]).await?.pop() {
                    return Ok(Some(chk));
                }
            }
        }
        Ok(None)
    }

    async fn get_latest_checkpoint(&mut self) -> Result<CheckpointSequenceNumber, Self::Error> {
        let upper_limit = u64::MAX.to_be_bytes().to_vec();
        match self
            .reversed_scan(WATERMARK_TABLE, upper_limit, 1)
            .await?
            .pop()
        {
            Some(latest_row) => {
                let latest_watermark = u64::from_be_bytes(latest_row.key.as_slice().try_into()?);
                let latest_checkpoint = latest_watermark.saturating_sub(1);
                Ok(latest_checkpoint)
            }
            None => Ok(0),
        }
    }

    async fn get_latest_checkpoint_summary(
        &mut self,
    ) -> Result<Option<CheckpointSummary>, Self::Error> {
        let sequence_number = self.get_latest_checkpoint().await?;
        if sequence_number == 0 {
            return Ok(None);
        }

        // Fetch just the summary for the latest checkpoint sequence number.
        let mut rows_cells = self
            .multi_get(
                CHECKPOINTS_TABLE,
                vec![(sequence_number).to_be_bytes().to_vec()],
                Some(RowFilter {
                    filter: Some(Filter::ColumnQualifierRegexFilter(
                        CHECKPOINT_SUMMARY_COLUMN_QUALIFIER.into(),
                    )),
                }),
            )
            .await?;

        let Some(latest_row_cells) = rows_cells.pop() else {
            return Ok(None);
        };

        let mut summary = None;
        for Cell { name, value } in latest_row_cells {
            match std::str::from_utf8(&name)? {
                CHECKPOINT_SUMMARY_COLUMN_QUALIFIER => {
                    summary = Some(bcs::from_bytes::<CheckpointSummary>(&value)?)
                }
                unexpected_cell_name => {
                    error!("unexpected column {unexpected_cell_name:?} in checkpoints table")
                }
            }
        }

        Ok(summary)
    }

    async fn get_latest_object(
        &mut self,
        object_id: &ObjectID,
    ) -> Result<Option<Object>, Self::Error> {
        let upper_limit = raw_object_key(&ObjectKey::max_for_id(object_id));
        if let Some(latest_row) = self
            .reversed_scan(OBJECTS_TABLE, upper_limit, 1)
            .await?
            .pop()
        {
            if let Some(latest_cell) = latest_row.cells.into_iter().next() {
                return Ok(Some(bcs::from_bytes(&latest_cell.value)?));
            }
        }
        Ok(None)
    }
}

fn raw_object_key(object_key: &ObjectKey) -> Vec<u8> {
    let mut raw_key = object_key.0.to_vec();
    raw_key.extend(object_key.1.value().to_be_bytes());
    raw_key
}
