// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.checkpoint.rs");
include!("../../../generated/iota.grpc.v0.checkpoint.field_info.rs");
include!("../../../generated/iota.grpc.v0.checkpoint.accessors.rs");

use crate::{
    proto::{TryFromProtoError, get_inner_field},
    v0::bcs::BcsData,
};

// CheckpointSummary
//

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

impl CheckpointSummary {
    /// Deserialize checkpoint summary.
    pub fn summary(&self) -> Result<iota_sdk_types::CheckpointSummary, TryFromProtoError> {
        self.try_into()
    }

    /// Get the raw BCS bytes of this checkpoint summary.
    pub fn summary_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        self.bcs
            .as_ref()
            .map(BcsData::as_bytes)
            .ok_or_else(|| TryFromProtoError::missing(Self::BCS_FIELD.name))
    }

    /// Get the digest of this checkpoint summary.
    pub fn summary_digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.digest, Self::DIGEST_FIELD, try_into)
    }
}

// CheckpointContents
//

impl TryFrom<&CheckpointContents> for iota_sdk_types::CheckpointContents {
    type Error = TryFromProtoError;

    fn try_from(value: &CheckpointContents) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(CheckpointContents::BCS_FIELD.name))?;
        // TODO: add version
        BcsData::deserialize(bcs)
            .map_err(|e| TryFromProtoError::invalid(CheckpointContents::BCS_FIELD, e))
    }
}

impl CheckpointContents {
    /// Deserialize checkpoint contents.
    pub fn contents(&self) -> Result<iota_sdk_types::CheckpointContents, TryFromProtoError> {
        self.try_into()
    }

    /// Get the raw BCS bytes of this checkpoint contents.
    pub fn contents_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        self.bcs
            .as_ref()
            .map(BcsData::as_bytes)
            .ok_or_else(|| TryFromProtoError::missing(Self::BCS_FIELD.name))
    }

    /// Get the digest of this checkpoint contents.
    pub fn contents_digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.digest, Self::DIGEST_FIELD, try_into)
    }
}

// Checkpoint
//

impl Checkpoint {
    /// Get the checkpoint sequence number (height).
    pub fn checkpoint_sequence_number(&self) -> Result<u64, TryFromProtoError> {
        self.sequence_number
            .ok_or_else(|| TryFromProtoError::missing(Self::SEQUENCE_NUMBER_FIELD.name))
    }

    /// Get the raw BCS bytes of the checkpoint summary.
    pub fn summary_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        get_inner_field!(self.summary, Self::SUMMARY_FIELD, summary_bcs)
    }

    /// Get the raw BCS bytes of the checkpoint contents.
    pub fn contents_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        get_inner_field!(self.contents, Self::CONTENTS_FIELD, contents_bcs)
    }

    /// Deserialize checkpoint summary.
    pub fn summary(&self) -> Result<iota_sdk_types::CheckpointSummary, TryFromProtoError> {
        get_inner_field!(self.summary, Self::SUMMARY_FIELD, summary)
    }

    /// Deserialize checkpoint contents.
    pub fn contents(&self) -> Result<iota_sdk_types::CheckpointContents, TryFromProtoError> {
        get_inner_field!(self.contents, Self::CONTENTS_FIELD, contents)
    }

    /// Deserialize validator signature.
    pub fn signature(
        &self,
    ) -> Result<iota_sdk_types::ValidatorAggregatedSignature, TryFromProtoError> {
        let sig = self
            .signature
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::SIGNATURE_FIELD.name))?;
        <&super::signatures::ValidatorAggregatedSignature as TryInto<
            iota_sdk_types::ValidatorAggregatedSignature,
        >>::try_into(sig)
        .map_err(|e: TryFromProtoError| e.nested(Self::SIGNATURE_FIELD.name))
    }

    /// Get the raw BCS bytes of the validator signature.
    pub fn signature_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        let sig = self
            .signature
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::SIGNATURE_FIELD.name))?;
        sig.bcs.as_ref().map(BcsData::as_bytes).ok_or_else(|| {
            TryFromProtoError::missing(
                super::signatures::ValidatorAggregatedSignature::BCS_FIELD.name,
            )
            .nested(Self::SIGNATURE_FIELD.name)
        })
    }

    /// Get the summary digest directly from the nested summary.
    pub fn summary_digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.summary, Self::SUMMARY_FIELD, summary_digest)
    }

    /// Get the contents digest directly from the nested contents.
    pub fn contents_digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.contents, Self::CONTENTS_FIELD, contents_digest)
    }
}
