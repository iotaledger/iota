// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::types::{
    CheckpointSequenceNumber, EpochId, Object, ObjectId, SignedTransaction, ValidatorCommittee,
    Version,
};
use iota_types::storage::{
    ObjectStore, RpcStateReader,
    error::{Error as StorageError, Result},
};
use tap::Pipe;

use crate::Direction;

#[derive(Clone)]
pub struct StateReader {
    inner: Arc<dyn RpcStateReader>,
}

impl StateReader {
    pub fn new(inner: Arc<dyn RpcStateReader>) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &Arc<dyn RpcStateReader> {
        &self.inner
    }

    pub fn get_object(&self, object_id: ObjectId) -> crate::Result<Option<Object>> {
        self.inner
            .get_object(&object_id.into())
            .map(TryInto::try_into)
            .transpose()
            .map_err(Into::into)
    }

    pub fn get_object_with_version(
        &self,
        object_id: ObjectId,
        version: Version,
    ) -> crate::Result<Option<Object>> {
        self.inner
            .get_object_by_key(&object_id.into(), version.into())
            .map(TryInto::try_into)
            .transpose()
            .map_err(Into::into)
    }

    pub fn get_committee(&self, epoch: EpochId) -> Option<ValidatorCommittee> {
        self.inner
            .get_committee(epoch)
            .map(|committee| (*committee).clone().into())
    }

    pub fn get_system_state_summary(&self) -> Result<super::rest::system::SystemStateSummary> {
        use iota_types::iota_system_state::IotaSystemStateTrait;

        let system_state = iota_types::iota_system_state::get_iota_system_state(self.inner())
            .map_err(StorageError::custom)?;
        let summary = system_state.into_iota_system_state_summary().into();

        Ok(summary)
    }

    pub fn get_transaction(
        &self,
        digest: iota_sdk_types::types::TransactionDigest,
    ) -> crate::Result<(
        iota_sdk_types::types::SignedTransaction,
        iota_sdk_types::types::TransactionEffects,
        Option<iota_sdk_types::types::TransactionEvents>,
    )> {
        use iota_types::effects::TransactionEffectsAPI;

        use super::rest::transactions::TransactionNotFoundError;

        let transaction_digest = digest.into();

        let transaction = (*self
            .inner()
            .get_transaction(&transaction_digest)
            .ok_or(TransactionNotFoundError(digest))?)
        .clone()
        .into_inner();
        let effects = self
            .inner()
            .get_transaction_effects(&transaction_digest)
            .ok_or(TransactionNotFoundError(digest))?;
        let events = if let Some(event_digest) = effects.events_digest() {
            self.inner()
                .get_events(event_digest)
                .ok_or(TransactionNotFoundError(digest))?
                .pipe(Some)
        } else {
            None
        };

        Ok((
            transaction.try_into()?,
            effects.try_into()?,
            events.map(TryInto::try_into).transpose()?,
        ))
    }

    pub fn get_transaction_checkpoint(
        &self,
        digest: &iota_types::digests::TransactionDigest,
    ) -> Option<CheckpointSequenceNumber> {
        self.inner()
            .indexes()?
            .get_transaction_checkpoint(digest)
            .ok()?
    }

    pub fn get_transaction_read(
        &self,
        digest: iota_sdk_types::types::TransactionDigest,
    ) -> crate::Result<TransactionRead> {
        let (
            SignedTransaction {
                transaction,
                signatures,
            },
            effects,
            events,
        ) = self.get_transaction(digest)?;

        let checkpoint = self.get_transaction_checkpoint(&(digest.into()));
        let timestamp_ms = if let Some(checkpoint) = checkpoint {
            self.inner()
                .get_checkpoint_by_sequence_number(checkpoint)
                .map(|checkpoint| checkpoint.timestamp_ms)
        } else {
            None
        };

        Ok(TransactionRead {
            digest: transaction.digest(),
            transaction,
            signatures,
            effects,
            events,
            checkpoint,
            timestamp_ms,
        })
    }

    pub fn checkpoint_iter(
        &self,
        direction: Direction,
        start: CheckpointSequenceNumber,
    ) -> CheckpointIter {
        CheckpointIter::new(self.clone(), direction, start)
    }

    pub fn transaction_iter(
        &self,
        direction: Direction,
        cursor: (CheckpointSequenceNumber, Option<usize>),
    ) -> CheckpointTransactionsIter {
        CheckpointTransactionsIter::new(self.clone(), direction, cursor)
    }
}

#[derive(Debug)]
pub struct TransactionRead {
    pub digest: iota_sdk_types::types::TransactionDigest,
    pub transaction: iota_sdk_types::types::Transaction,
    pub signatures: Vec<iota_sdk_types::types::UserSignature>,
    pub effects: iota_sdk_types::types::TransactionEffects,
    pub events: Option<iota_sdk_types::types::TransactionEvents>,
    pub checkpoint: Option<u64>,
    pub timestamp_ms: Option<u64>,
}

pub struct CheckpointTransactionsIter {
    reader: StateReader,
    direction: Direction,

    next_cursor: Option<(CheckpointSequenceNumber, Option<usize>)>,
    checkpoint: Option<(
        iota_types::messages_checkpoint::CheckpointSummary,
        iota_types::messages_checkpoint::CheckpointContents,
    )>,
}

impl CheckpointTransactionsIter {
    pub fn new(
        reader: StateReader,
        direction: Direction,
        start: (CheckpointSequenceNumber, Option<usize>),
    ) -> Self {
        Self {
            reader,
            direction,
            next_cursor: Some(start),
            checkpoint: None,
        }
    }
}

impl Iterator for CheckpointTransactionsIter {
    type Item = Result<(CursorInfo, iota_types::digests::TransactionDigest)>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (current_checkpoint, transaction_index) = self.next_cursor?;

            let (checkpoint, contents) = if let Some(checkpoint) = &self.checkpoint {
                if checkpoint.0.sequence_number != current_checkpoint {
                    self.checkpoint = None;
                    continue;
                } else {
                    checkpoint
                }
            } else {
                let checkpoint = match self
                    .reader
                    .inner()
                    .get_checkpoint_by_sequence_number(current_checkpoint)
                {
                    Some(checkpoint) => checkpoint,
                    None => return None,
                };
                let contents = match self
                    .reader
                    .inner()
                    .get_checkpoint_contents_by_sequence_number(checkpoint.sequence_number)
                {
                    Some(contents) => contents,
                    None => return None,
                };

                self.checkpoint = Some((checkpoint.into_inner().into_data(), contents));
                self.checkpoint.as_ref().unwrap()
            };

            let index = transaction_index
                .map(|idx| idx.clamp(0, contents.size().saturating_sub(1)))
                .unwrap_or_else(|| match self.direction {
                    Direction::Ascending => 0,
                    Direction::Descending => contents.size().saturating_sub(1),
                });

            self.next_cursor = {
                let next_index = match self.direction {
                    Direction::Ascending => {
                        let next_index = index + 1;
                        if next_index >= contents.size() {
                            None
                        } else {
                            Some(next_index)
                        }
                    }
                    Direction::Descending => index.checked_sub(1),
                };

                let next_checkpoint = if next_index.is_some() {
                    Some(current_checkpoint)
                } else {
                    match self.direction {
                        Direction::Ascending => current_checkpoint.checked_add(1),
                        Direction::Descending => current_checkpoint.checked_sub(1),
                    }
                };

                next_checkpoint.map(|checkpoint| (checkpoint, next_index))
            };

            if contents.size() == 0 {
                continue;
            }

            let digest = contents.inner()[index].transaction;

            let cursor_info = CursorInfo {
                checkpoint: checkpoint.sequence_number,
                timestamp_ms: checkpoint.timestamp_ms,
                index: index as u64,
                next_cursor: self.next_cursor,
            };

            return Some(Ok((cursor_info, digest)));
        }
    }
}

pub struct CursorInfo {
    pub checkpoint: CheckpointSequenceNumber,
    pub timestamp_ms: u64,
    #[allow(unused)]
    pub index: u64,

    // None if there are no more transactions in the store
    pub next_cursor: Option<(CheckpointSequenceNumber, Option<usize>)>,
}

pub struct CheckpointIter {
    reader: StateReader,
    direction: Direction,

    next_cursor: Option<CheckpointSequenceNumber>,
}

impl CheckpointIter {
    pub fn new(reader: StateReader, direction: Direction, start: CheckpointSequenceNumber) -> Self {
        Self {
            reader,
            direction,
            next_cursor: Some(start),
        }
    }
}

impl Iterator for CheckpointIter {
    type Item = Result<(
        iota_types::messages_checkpoint::CertifiedCheckpointSummary,
        iota_types::messages_checkpoint::CheckpointContents,
    )>;

    fn next(&mut self) -> Option<Self::Item> {
        let current_checkpoint = self.next_cursor?;

        let checkpoint = match self
            .reader
            .inner()
            .get_checkpoint_by_sequence_number(current_checkpoint)
        {
            Some(checkpoint) => checkpoint,
            None => return None,
        }
        .into_inner();
        let contents = match self
            .reader
            .inner()
            .get_checkpoint_contents_by_sequence_number(checkpoint.sequence_number)
        {
            Some(contents) => contents,
            None => return None,
        };

        self.next_cursor = match self.direction {
            Direction::Ascending => current_checkpoint.checked_add(1),
            Direction::Descending => current_checkpoint.checked_sub(1),
        };

        Some(Ok((checkpoint, contents)))
    }
}
