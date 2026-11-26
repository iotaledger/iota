// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::ops::RangeInclusive;

use iota_types::messages_checkpoint::CheckpointSequenceNumber;

use crate::{IngestionError, IngestionResult};

/// The order in which checkpoints are ingested.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IngestionOrder {
    /// From the lowest checkpoint of the range to the highest.
    #[default]
    Ascending,
    /// From the highest checkpoint of the range to the lowest.
    Descending,
}

impl IngestionOrder {
    pub(crate) fn is_descending(&self) -> bool {
        matches!(self, Self::Descending)
    }

    /// Returns the checkpoint at `index` of `range` in this order.
    pub(crate) fn nth(
        self,
        range: &RangeInclusive<CheckpointSequenceNumber>,
        index: u64,
    ) -> Option<CheckpointSequenceNumber> {
        match self {
            Self::Ascending => range.clone().nth(index as usize),
            Self::Descending => range.clone().nth_back(index as usize),
        }
    }

    /// Returns the first and the last checkpoint of `range` in this order.
    pub(crate) fn first_and_last(
        self,
        range: &RangeInclusive<CheckpointSequenceNumber>,
    ) -> (CheckpointSequenceNumber, CheckpointSequenceNumber) {
        match self {
            Self::Ascending => (*range.start(), *range.end()),
            Self::Descending => (*range.end(), *range.start()),
        }
    }
}

/// The checkpoints a store still has to return, in ingestion order.
pub(crate) struct ExpectedCheckpoints {
    remaining: RangeInclusive<CheckpointSequenceNumber>,
    order: IngestionOrder,
}

impl ExpectedCheckpoints {
    pub(crate) fn new(
        remaining: RangeInclusive<CheckpointSequenceNumber>,
        order: IngestionOrder,
    ) -> Self {
        Self { remaining, order }
    }

    /// Checks that `sequence_number` is the checkpoint that has to come next, and moves on.
    pub(crate) fn verify_next(
        &mut self,
        sequence_number: CheckpointSequenceNumber,
    ) -> IngestionResult<()> {
        match self.advance() {
            Some(expected) if expected == sequence_number => Ok(()),
            Some(expected) => Err(IngestionError::Verification(format!(
                "expected checkpoint {expected}, the store returned {sequence_number}"
            ))),
            None => Err(IngestionError::Verification(format!(
                "the store returned checkpoint {sequence_number} after the end of the range"
            ))),
        }
    }

    /// Takes the checkpoint that has to come next, or `None` once all were returned.
    fn advance(&mut self) -> Option<CheckpointSequenceNumber> {
        match self.order {
            IngestionOrder::Ascending => self.remaining.next(),
            IngestionOrder::Descending => self.remaining.next_back(),
        }
    }

    /// Returns the checkpoints the store has not returned yet.
    pub(crate) fn remaining_checkpoints(&self) -> &RangeInclusive<CheckpointSequenceNumber> {
        &self.remaining
    }

    /// Returns `true` once the store has returned every checkpoint of the range.
    pub(crate) fn is_finished(&self) -> bool {
        self.remaining.is_empty()
    }
}
