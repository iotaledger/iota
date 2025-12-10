// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.checkpoint.rs");
include!("../../../generated/iota.grpc.v0.checkpoint.field_info.rs");
include!("../../../generated/iota.grpc.v0.checkpoint.accessors.rs");

use crate::{field::FieldMaskTree, merge::Merge, proto::TryFromProtoError, v0::bcs::BcsData};

// CheckpointSummary
//

impl TryFrom<iota_sdk_types::CheckpointSummary> for CheckpointSummary {
    type Error = Box<dyn std::error::Error>;

    fn try_from(summary: iota_sdk_types::CheckpointSummary) -> Result<Self, Self::Error> {
        Self::merge_from(summary, &FieldMaskTree::new_wildcard())
    }
}

impl Merge<iota_sdk_types::CheckpointSummary> for CheckpointSummary {
    fn merge(
        &mut self,
        source: iota_sdk_types::CheckpointSummary,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = BcsData::serialize(&source).ok();
        }

        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(source.digest().into());
        }

        Ok(())
    }
}

impl Merge<&CheckpointSummary> for CheckpointSummary {
    fn merge(
        &mut self,
        source: &CheckpointSummary,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let CheckpointSummary { bcs, digest } = source;

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = bcs.clone();
        }

        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = digest.clone();
        }

        Ok(())
    }
}

impl TryFrom<&CheckpointSummary> for iota_sdk_types::CheckpointSummary {
    type Error = TryFromProtoError;

    fn try_from(
        CheckpointSummary { bcs, digest: _ }: &CheckpointSummary,
    ) -> Result<Self, Self::Error> {
        let bcs = bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(CheckpointSummary::BCS_FIELD.name))?;
        BcsData::deserialize(bcs)
            .map_err(|e| TryFromProtoError::invalid(CheckpointSummary::BCS_FIELD, e))
    }
}

// CheckpointContents
//

impl TryFrom<iota_sdk_types::CheckpointContents> for CheckpointContents {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: iota_sdk_types::CheckpointContents) -> Result<Self, Self::Error> {
        Self::merge_from(value, &FieldMaskTree::new_wildcard())
    }
}

impl Merge<iota_sdk_types::CheckpointContents> for CheckpointContents {
    fn merge(
        &mut self,
        source: iota_sdk_types::CheckpointContents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = BcsData::serialize(&source).ok();
        }

        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(source.digest().into());
        }

        Ok(())
    }
}

impl Merge<&CheckpointContents> for CheckpointContents {
    fn merge(
        &mut self,
        source: &CheckpointContents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let CheckpointContents { bcs, digest } = source;

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = bcs.clone();
        }

        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = digest.clone();
        }

        Ok(())
    }
}

impl TryFrom<&CheckpointContents> for iota_sdk_types::CheckpointContents {
    type Error = TryFromProtoError;

    fn try_from(value: &CheckpointContents) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(CheckpointContents::BCS_FIELD.name))?;
        BcsData::deserialize(bcs)
            .map_err(|e| TryFromProtoError::invalid(CheckpointContents::BCS_FIELD, e))
    }
}

// Checkpoint
//

impl Merge<&iota_sdk_types::CheckpointSummary> for Checkpoint {
    fn merge(
        &mut self,
        source: &iota_sdk_types::CheckpointSummary,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::SEQUENCE_NUMBER_FIELD.name) {
            self.sequence_number = Some(source.sequence_number);
        }

        if let Some(submask) = mask.subtree(Self::SUMMARY_FIELD.name) {
            self.summary = Some(CheckpointSummary::merge_from(source.clone(), &submask)?);
        }

        Ok(())
    }
}

impl Merge<iota_sdk_types::ValidatorAggregatedSignature> for Checkpoint {
    fn merge(
        &mut self,
        source: iota_sdk_types::ValidatorAggregatedSignature,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::SIGNATURE_FIELD.name) {
            self.signature = Some(source.into());
        }

        Ok(())
    }
}

impl Merge<iota_sdk_types::CheckpointContents> for Checkpoint {
    fn merge(
        &mut self,
        source: iota_sdk_types::CheckpointContents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(submask) = mask.subtree(Self::CONTENTS_FIELD.name) {
            self.contents = Some(CheckpointContents::merge_from(source, &submask)?);
        }

        Ok(())
    }
}

impl Merge<&Checkpoint> for Checkpoint {
    fn merge(
        &mut self,
        source: &Checkpoint,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Checkpoint {
            sequence_number,
            summary,
            signature,
            contents,
        } = source;

        if mask.contains(Self::SEQUENCE_NUMBER_FIELD.name) {
            self.sequence_number = *sequence_number;
        }

        if let Some(submask) = mask.subtree(Self::SUMMARY_FIELD.name) {
            self.summary = summary
                .as_ref()
                .map(|summary| CheckpointSummary::merge_from(summary, &submask))
                .transpose()?;
        }

        if mask.contains(Self::SIGNATURE_FIELD.name) {
            self.signature = signature.clone();
        }

        if let Some(submask) = mask.subtree(Self::CONTENTS_FIELD.name) {
            self.contents = contents
                .as_ref()
                .map(|contents| CheckpointContents::merge_from(contents, &submask))
                .transpose()?;
        }

        Ok(())
    }
}

// TODO: we need to reconsider this design

use serde::{Deserialize, Serialize};

/// Forward-compatible versioned checkpoint data for gRPC streaming.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CheckpointData {
    V1(iota_types::full_checkpoint_content::CheckpointData),
}

/// Forward-compatible versioned checkpoint summary for gRPC streaming.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CertifiedCheckpointSummary {
    V1(iota_types::messages_checkpoint::CertifiedCheckpointSummary),
}

impl From<iota_types::full_checkpoint_content::CheckpointData> for CheckpointData {
    fn from(data: iota_types::full_checkpoint_content::CheckpointData) -> Self {
        Self::V1(data)
    }
}

impl From<iota_types::messages_checkpoint::CertifiedCheckpointSummary>
    for CertifiedCheckpointSummary
{
    fn from(summary: iota_types::messages_checkpoint::CertifiedCheckpointSummary) -> Self {
        Self::V1(summary)
    }
}

impl CheckpointData {
    /// Extract the V1 checkpoint data, returning None for unknown versions
    pub fn into_v1(self) -> Option<iota_types::full_checkpoint_content::CheckpointData> {
        match self {
            Self::V1(data) => Some(data),
        }
    }

    /// Get a reference to the V1 checkpoint data, returning None for unknown
    /// versions
    pub fn as_v1(&self) -> Option<&iota_types::full_checkpoint_content::CheckpointData> {
        match self {
            Self::V1(data) => Some(data),
        }
    }

    /// Get the sequence number regardless of version
    pub fn sequence_number(&self) -> u64 {
        match self {
            Self::V1(data) => data.checkpoint_summary.sequence_number,
        }
    }
}

impl CertifiedCheckpointSummary {
    /// Extract the V1 checkpoint summary, returning None for unknown versions
    pub fn into_v1(self) -> Option<iota_types::messages_checkpoint::CertifiedCheckpointSummary> {
        match self {
            Self::V1(summary) => Some(summary),
        }
    }

    /// Get a reference to the V1 checkpoint summary, returning None for unknown
    /// versions
    pub fn as_v1(&self) -> Option<&iota_types::messages_checkpoint::CertifiedCheckpointSummary> {
        match self {
            Self::V1(summary) => Some(summary),
        }
    }

    /// Get the sequence number regardless of version
    pub fn sequence_number(&self) -> u64 {
        match self {
            Self::V1(summary) => summary.data().sequence_number,
        }
    }
}
