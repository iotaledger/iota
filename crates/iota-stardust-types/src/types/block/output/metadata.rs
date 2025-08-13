// Copyright 2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::block::{BlockId, output::OutputId, payload::transaction::TransactionId};

/// Metadata of an [`Output`](crate::types::block::output::Output).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct OutputMetadata {
    /// The identifier of the block in which the output was included.
    block_id: BlockId,
    /// The identifier of the output.
    output_id: OutputId,
    /// Whether the output is spent or not.
    is_spent: bool,
    /// If spent, the index of the milestone in which the output was spent.
    milestone_index_spent: Option<u32>,
    /// If spent, the timestamp of the milestone in which the output was spent.
    milestone_timestamp_spent: Option<u32>,
    /// If spent, the identifier of the transaction that spent the output.
    transaction_id_spent: Option<TransactionId>,
    /// The index of the milestone that booked the output.
    milestone_index_booked: u32,
    /// The timestamp of the milestone that booked the output.
    milestone_timestamp_booked: u32,
    /// The index of ledger when the output was fetched.
    ledger_index: u32,
}

impl OutputMetadata {
    /// Creates a new [`OutputMetadata`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        block_id: BlockId,
        output_id: OutputId,
        is_spent: bool,
        milestone_index_spent: Option<u32>,
        milestone_timestamp_spent: Option<u32>,
        transaction_id_spent: Option<TransactionId>,
        milestone_index_booked: u32,
        milestone_timestamp_booked: u32,
        ledger_index: u32,
    ) -> Self {
        Self {
            block_id,
            output_id,
            is_spent,
            milestone_index_spent,
            milestone_timestamp_spent,
            transaction_id_spent,
            milestone_index_booked,
            milestone_timestamp_booked,
            ledger_index,
        }
    }

    /// Returns the block id of the [`OutputMetadata`].
    pub fn block_id(&self) -> &BlockId {
        &self.block_id
    }

    /// Returns the output id of the [`OutputMetadata`].
    pub fn output_id(&self) -> &OutputId {
        &self.output_id
    }

    /// Returns the transaction id of the [`OutputMetadata`].
    pub fn transaction_id(&self) -> &TransactionId {
        self.output_id.transaction_id()
    }

    /// Returns the output index of the [`OutputMetadata`].
    pub fn output_index(&self) -> u16 {
        self.output_id.index()
    }

    /// Returns whether the output is spent or not.
    pub fn is_spent(&self) -> bool {
        self.is_spent
    }

    /// Sets whether the output is spent or not.
    pub fn set_spent(&mut self, spent: bool) {
        self.is_spent = spent;
    }

    /// Returns the milestone index spent of the [`OutputMetadata`].
    pub fn milestone_index_spent(&self) -> Option<u32> {
        self.milestone_index_spent
    }

    /// Returns the milestone timestamp spent of the [`OutputMetadata`].
    pub fn milestone_timestamp_spent(&self) -> Option<u32> {
        self.milestone_timestamp_spent
    }

    /// Returns the transaction id spent of the [`OutputMetadata`].
    pub fn transaction_id_spent(&self) -> Option<&TransactionId> {
        self.transaction_id_spent.as_ref()
    }

    /// Returns the milestone index booked of the [`OutputMetadata`].
    pub fn milestone_index_booked(&self) -> u32 {
        self.milestone_index_booked
    }

    /// Returns the milestone timestamp booked of the [`OutputMetadata`].
    pub fn milestone_timestamp_booked(&self) -> u32 {
        self.milestone_timestamp_booked
    }

    /// Returns the ledger index of the [`OutputMetadata`].
    pub fn ledger_index(&self) -> u32 {
        self.ledger_index
    }
}

#[cfg(feature = "serde")]
mod dto {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct OutputMetadataDto {
        block_id: BlockId,
        transaction_id: TransactionId,
        output_index: u16,
        is_spent: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        milestone_index_spent: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        milestone_timestamp_spent: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transaction_id_spent: Option<TransactionId>,
        milestone_index_booked: u32,
        milestone_timestamp_booked: u32,
        ledger_index: u32,
    }

    impl TryFrom<OutputMetadataDto> for OutputMetadata {
        type Error = crate::types::block::Error;

        fn try_from(value: OutputMetadataDto) -> Result<Self, Self::Error> {
            Ok(Self {
                block_id: value.block_id,
                output_id: OutputId::new(value.transaction_id, value.output_index)?,
                is_spent: value.is_spent,
                milestone_index_spent: value.milestone_index_spent,
                milestone_timestamp_spent: value.milestone_timestamp_spent,
                transaction_id_spent: value.transaction_id_spent,
                milestone_index_booked: value.milestone_index_booked,
                milestone_timestamp_booked: value.milestone_timestamp_booked,
                ledger_index: value.ledger_index,
            })
        }
    }

    impl From<&OutputMetadata> for OutputMetadataDto {
        fn from(value: &OutputMetadata) -> Self {
            Self {
                block_id: value.block_id,
                transaction_id: *value.transaction_id(),
                output_index: value.output_index(),
                is_spent: value.is_spent,
                milestone_index_spent: value.milestone_index_spent,
                milestone_timestamp_spent: value.milestone_timestamp_spent,
                transaction_id_spent: value.transaction_id_spent,
                milestone_index_booked: value.milestone_index_booked,
                milestone_timestamp_booked: value.milestone_timestamp_booked,
                ledger_index: value.ledger_index,
            }
        }
    }

    impl<'de> Deserialize<'de> for OutputMetadata {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            OutputMetadataDto::deserialize(d)
                .and_then(|dto| dto.try_into().map_err(serde::de::Error::custom))
        }
    }

    impl Serialize for OutputMetadata {
        fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            OutputMetadataDto::from(self).serialize(s)
        }
    }
}
