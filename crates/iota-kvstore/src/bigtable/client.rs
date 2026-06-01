// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::num::NonZeroUsize;

use async_trait::async_trait;
use iota_bigtable::{
    BigTableClient, Cell, Row,
    proto::bigtable::v2::{
        RowFilter,
        row_filter::Filter,
        row_range::{EndKey, StartKey},
    },
};
use iota_types::{
    base_types::{IotaAddress, TransactionDigest},
    digests::CheckpointDigest,
    effects::{TransactionEffects, TransactionEvents},
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
    },
    object::Object,
    storage::ObjectKey,
    transaction::Transaction,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{Checkpoint, KeyValueStoreReader, KeyValueStoreWriter, TransactionData};

pub const OBJECTS_TABLE: &str = "objects";
pub const TRANSACTIONS_TABLE: &str = "transactions";
pub const CHECKPOINTS_TABLE: &str = "checkpoints";
pub const CHECKPOINTS_BY_DIGEST_TABLE: &str = "checkpoints_by_digest";
pub const TRANSACTIONS_BY_ADDRESS_TABLE: &str = "transactions_by_address";

pub const DEFAULT_COLUMN_QUALIFIER: &str = "";
pub const CHECKPOINT_SUMMARY_COLUMN_QUALIFIER: &str = "cs";
pub const CHECKPOINT_CONTENTS_COLUMN_QUALIFIER: &str = "cc";
pub const TRANSACTION_COLUMN_QUALIFIER: &str = "tx";
pub const EFFECTS_COLUMN_QUALIFIER: &str = "fx";
pub const EVENTS_COLUMN_QUALIFIER: &str = "evtx";
pub const TRANSACTION_TO_CHECKPOINT: &str = "tx2c";

pub type TransactionSequenceNumber = u64;

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

    async fn save_transactions_by_address<I>(&mut self, entries: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = (IotaAddress, u64, TransactionDigest)> + Send,
        I::IntoIter: Send,
    {
        let rows = entries
            .into_iter()
            .map(|(address, seq, transaction_digest)| {
                let key = encode_transaction_by_address_key(&address, seq.into());
                let cells = [Cell::new(
                    DEFAULT_COLUMN_QUALIFIER.as_bytes().into(),
                    transaction_digest.inner().into(),
                )];
                Row::new(key.into(), cells.into())
            });

        self.multi_set(TRANSACTIONS_BY_ADDRESS_TABLE, rows)
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
}

#[async_trait]
impl KeyValueStoreReader for BigTableClient {
    type Error = anyhow::Error;

    async fn get_objects(&mut self, object_keys: &[ObjectKey]) -> Result<Vec<Object>, Self::Error> {
        let keys = object_keys.iter().map(raw_object_key).collect();
        let mut objects = vec![];
        for row in self.multi_get(OBJECTS_TABLE, keys, None).await? {
            for cell in row.cells {
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
        for row in self.multi_get(TRANSACTIONS_TABLE, keys, None).await? {
            let mut transaction = None;
            let mut effects = None;
            let mut events = None;
            let mut checkpoint_number = 0;

            for Cell { name, value } in row.cells {
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

    async fn get_transaction_digests_by_address(
        &mut self,
        address: IotaAddress,
        cursor: impl Into<Option<TransactionSequenceNumber>> + Send,
        limit: impl TryInto<NonZeroUsize> + Send,
        order: TransactionsOrder,
    ) -> Result<Vec<(TransactionSequenceNumber, TransactionDigest)>, Self::Error> {
        let limit = limit
            .try_into()
            .map_err(|_| anyhow::anyhow!("limit must be greater than 0"))?;

        let cursor = cursor.into();

        let newest = encode_transaction_by_address_key(&address, u64::MAX.into()).to_vec();
        let oldest = encode_transaction_by_address_key(&address, 0.into()).to_vec();

        let (start_key, end_key) = match (cursor, order) {
            // no cursor: whole address block, direction handled by order.
            (None, _) => (
                StartKey::StartKeyClosed(newest),
                EndKey::EndKeyClosed(oldest),
            ),
            // newest-first, continue past cursor: tx seq < cursor.
            (Some(cursor), TransactionsOrder::NewestFirst) => {
                // no transactions can have seq < 0, the scan would be empty
                // and BigTable rejects empty ranges.
                if cursor == 0 {
                    return Ok(vec![]);
                }
                let k = encode_transaction_by_address_key(&address, cursor.into()).to_vec();
                (StartKey::StartKeyOpen(k), EndKey::EndKeyClosed(oldest))
            }
            // oldest-first, continue past cursor: tx seq > cursor.
            (Some(cursor), TransactionsOrder::OldestFirst) => {
                // no transactions can have seq > u64::MAX, the scan would be
                // empty and BigTable rejects empty ranges.
                if cursor == u64::MAX {
                    return Ok(vec![]);
                }
                let k = encode_transaction_by_address_key(&address, cursor.into()).to_vec();
                (StartKey::StartKeyClosed(newest), EndKey::EndKeyOpen(k))
            }
        };

        // row keys are encoded as `address || ReverseSequenceNumber`, so a
        // forward scan already returns newest-first. OldestFirst requires a
        // reverse scan.
        let descending = matches!(order, TransactionsOrder::OldestFirst);

        let rows = self
            .range_scan(
                TRANSACTIONS_BY_ADDRESS_TABLE,
                Some(start_key),
                Some(end_key),
                limit.get(),
                descending,
                Some(RowFilter {
                    filter: Some(Filter::ColumnQualifierRegexFilter(
                        format!("^{DEFAULT_COLUMN_QUALIFIER}$").into_bytes(),
                    )),
                }),
            )
            .await?;

        rows.into_iter()
            .filter_map(|row| row.cells.into_iter().next().map(|cell| (row.key, cell)))
            .map(|(key, cell)| -> Result<_, anyhow::Error> {
                let (_addr, seq) = decode_transaction_by_address_key(&key)?;
                let digest = TransactionDigest::from_bytes(&cell.value)?;
                Ok((u64::from(seq), digest))
            })
            .collect()
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
        for row in self.multi_get(CHECKPOINTS_TABLE, keys, None).await? {
            let mut summary = None;
            let mut contents = None;
            for Cell { name, value } in row.cells {
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

    async fn get_checkpoints_by_digest(
        &mut self,
        digests: &[CheckpointDigest],
    ) -> Result<Vec<Checkpoint>, Self::Error> {
        let keys = digests
            .iter()
            .map(|digest| digest.inner().to_vec())
            .collect::<Vec<_>>();
        let seq_nums = self
            .multi_get(CHECKPOINTS_BY_DIGEST_TABLE, keys, None)
            .await?
            .into_iter()
            .filter_map(|row| {
                row.cells
                    .into_iter()
                    .next()
                    .map(|cell| cell.value.as_slice().try_into().map(u64::from_be_bytes))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.get_checkpoints(&seq_nums).await
    }
}

pub fn raw_object_key(object_key: &ObjectKey) -> Vec<u8> {
    let mut raw_key = object_key.0.as_bytes().to_vec();
    raw_key.extend(object_key.1.as_u64().to_be_bytes());
    raw_key
}

/// Represents the order of transactions returned by a BigTable range scan.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize,
)]
pub enum TransactionsOrder {
    #[default]
    NewestFirst,
    OldestFirst,
}

/// A sequence number stored as its bitwise complement (`!seq`), so that
/// BigTable's ascending lexicographic row-key order yields newest-first
/// results on a forward range scan.
///
/// By storing the complement, higher original sequence numbers map to
/// smaller byte representations and therefore sort earlier. This turns
/// "newest first" into a cheap forward scan instead of an expensive
/// reverse one.
///
/// # Conversions
/// - `From<u64>`: produces a `ReverseSequenceNumber` holding `!seq`.
/// - `Into<u64>`: returns the original `seq` from the wrapped `!seq`.
/// - [`ReverseSequenceNumber::to_be_bytes`]: encodes the stored value as
///   big-endian bytes for BigTable storage.
/// - [`ReverseSequenceNumber::from_be_bytes`]: decodes the stored value from
///   big-endian bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReverseSequenceNumber(u64);

impl ReverseSequenceNumber {
    pub const LENGTH: usize = std::mem::size_of::<u64>();

    /// Encodes the reversed sequence number into a big-endian byte array.
    pub fn to_be_bytes(self) -> [u8; Self::LENGTH] {
        self.0.to_be_bytes()
    }

    /// Decodes a reversed sequence number from a big-endian byte array.
    pub fn from_be_bytes(bytes: [u8; Self::LENGTH]) -> Self {
        Self(u64::from_be_bytes(bytes))
    }
}

impl From<u64> for ReverseSequenceNumber {
    fn from(seq: u64) -> Self {
        Self(!seq)
    }
}
impl From<ReverseSequenceNumber> for u64 {
    fn from(rev: ReverseSequenceNumber) -> u64 {
        !rev.0
    }
}

/// The length of an address-to-transaction row key, in bytes.
pub const ADDRESS_TX_KEY_LEN: usize = IotaAddress::LENGTH + ReverseSequenceNumber::LENGTH;

/// Encodes a row key for the address-to-transaction index.
///
/// Layout: `address (32 bytes) || complement (8 bytes)`.
///
/// Storing the complement ensures that transactions are ordered
/// lexicographically by descending sequence number, allowing for efficient
/// forward scans when retrieving the most recent transactions for an address.
///
/// See [`decode_transaction_by_address_key`] for the inverse operation.
pub fn encode_transaction_by_address_key(
    address: &IotaAddress,
    sequence_number: ReverseSequenceNumber,
) -> [u8; ADDRESS_TX_KEY_LEN] {
    let mut key = [0u8; ADDRESS_TX_KEY_LEN];
    key[..IotaAddress::LENGTH].copy_from_slice(address.as_ref());
    key[IotaAddress::LENGTH..].copy_from_slice(&sequence_number.to_be_bytes());
    key
}

/// Decodes an address-to-transaction row key.
///
/// This is the inverse of [`encode_transaction_by_address_key`]. It
/// extracts the address and restores the reversed sequence number.
pub fn decode_transaction_by_address_key(
    key: &[u8],
) -> Result<(IotaAddress, ReverseSequenceNumber), anyhow::Error> {
    anyhow::ensure!(key.len() == ADDRESS_TX_KEY_LEN, "invalid key length");
    let address = IotaAddress::from_bytes(&key[..IotaAddress::LENGTH])?;
    let reversed_tx_sequence =
        ReverseSequenceNumber::from_be_bytes(key[IotaAddress::LENGTH..].try_into()?);
    Ok((address, reversed_tx_sequence))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_sequence_number_round_trips_through_bytes() {
        for seq in [0u64, 1, 42, u64::MAX - 1, u64::MAX] {
            let rev = ReverseSequenceNumber::from(seq);
            let recovered = ReverseSequenceNumber::from_be_bytes(rev.to_be_bytes());
            assert_eq!(u64::from(recovered), seq);
        }
    }

    #[test]
    fn transaction_sequence_number_orders_descending() {
        // higher seq must sort earlier (smaller bytes) so forward range scans
        // return newest transactions first.
        let older = ReverseSequenceNumber::from(1).to_be_bytes();
        let newer = ReverseSequenceNumber::from(2).to_be_bytes();
        assert!(newer < older);

        // boundary: u64::MAX (newest possible) sorts before 0 (oldest possible).
        let newest = ReverseSequenceNumber::from(u64::MAX).to_be_bytes();
        let oldest = ReverseSequenceNumber::from(0).to_be_bytes();
        assert!(newest < oldest);
        assert_eq!(newest, [0; 8]);
        assert_eq!(oldest, [0xFF; 8]);
    }

    #[test]
    fn transaction_by_address_key_encode() {
        let address = IotaAddress::random();
        let transaction_sequence_number = ReverseSequenceNumber::from(42);
        let key = encode_transaction_by_address_key(&address, transaction_sequence_number);
        assert_eq!(key[..IotaAddress::LENGTH], address.into_bytes());
        assert_eq!(key[IotaAddress::LENGTH..], (u64::MAX - 42).to_be_bytes());
    }

    #[test]
    fn transaction_by_address_key_decode() {
        let address = IotaAddress::random();
        let transaction_sequence_number = ReverseSequenceNumber::from(42);
        let key = encode_transaction_by_address_key(&address, transaction_sequence_number);
        let (decoded_address, decoded_sequence_number) =
            decode_transaction_by_address_key(&key).unwrap();
        assert_eq!(decoded_address, address);
        assert_eq!(decoded_sequence_number, transaction_sequence_number);
        assert_eq!(u64::from(decoded_sequence_number), 42);
    }
}
