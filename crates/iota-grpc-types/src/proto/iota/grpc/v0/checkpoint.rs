// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.checkpoint.rs");
include!("../../../generated/iota.grpc.v0.checkpoint.field_info.rs");

use crate::{proto::TryFromProtoError, v0::bcs::BcsData};

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
    pub fn summary_bcs(&self) -> Option<&[u8]> {
        self.bcs.as_ref().map(BcsData::as_bytes)
    }

    /// Get the digest of this checkpoint summary if present.
    ///
    /// This is a low-level accessor that returns `Ok(None)` if the digest field
    /// is not set in the proto message. Use [`Checkpoint::summary_digest()`]
    /// for stricter validation that requires the digest when the summary
    /// exists.
    pub fn summary_digest(&self) -> Result<Option<iota_sdk_types::Digest>, TryFromProtoError> {
        self.digest
            .as_ref()
            .map(|d| {
                d.try_into()
                    .map_err(|e: TryFromProtoError| e.nested(Self::DIGEST_FIELD.name))
            })
            .transpose()
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
    pub fn contents_bcs(&self) -> Option<&[u8]> {
        self.bcs.as_ref().map(BcsData::as_bytes)
    }

    /// Get the digest of this checkpoint contents if present.
    ///
    /// This is a low-level accessor that returns `Ok(None)` if the digest field
    /// is not set in the proto message. Use [`Checkpoint::contents_digest()`]
    /// for stricter validation that requires the digest when the contents
    /// exists.
    pub fn contents_digest(&self) -> Result<Option<iota_sdk_types::Digest>, TryFromProtoError> {
        self.digest
            .as_ref()
            .map(|d| {
                d.try_into()
                    .map_err(|e: TryFromProtoError| e.nested(Self::DIGEST_FIELD.name))
            })
            .transpose()
    }
}

// Checkpoint
//

impl Checkpoint {
    /// Get the checkpoint sequence number (height).
    pub fn checkpoint_sequence_number(&self) -> Option<u64> {
        self.sequence_number
    }

    /// Get the raw BCS bytes of the checkpoint summary.
    pub fn summary_bcs(&self) -> Option<&[u8]> {
        self.summary.as_ref().and_then(|s| s.summary_bcs())
    }

    /// Get the raw BCS bytes of the checkpoint contents.
    pub fn contents_bcs(&self) -> Option<&[u8]> {
        self.contents.as_ref().and_then(|c| c.contents_bcs())
    }

    /// Deserialize checkpoint summary.
    pub fn summary(&self) -> Result<Option<iota_sdk_types::CheckpointSummary>, TryFromProtoError> {
        self.summary
            .as_ref()
            .map(|s| s.summary().map_err(|e| e.nested(Self::SUMMARY_FIELD.name)))
            .transpose()
    }

    /// Deserialize checkpoint contents.
    pub fn contents(
        &self,
    ) -> Result<Option<iota_sdk_types::CheckpointContents>, TryFromProtoError> {
        self.contents
            .as_ref()
            .map(|c| {
                c.contents()
                    .map_err(|e| e.nested(Self::CONTENTS_FIELD.name))
            })
            .transpose()
    }

    /// Deserialize validator signature.
    pub fn signature(
        &self,
    ) -> Result<Option<iota_sdk_types::ValidatorAggregatedSignature>, TryFromProtoError> {
        self.signature
            .as_ref()
            .map(|s| {
                <&super::signatures::ValidatorAggregatedSignature as TryInto<
                    iota_sdk_types::ValidatorAggregatedSignature,
                >>::try_into(s)
                .map_err(|e: TryFromProtoError| e.nested(Self::SIGNATURE_FIELD.name))
            })
            .transpose()
    }

    /// Get the raw BCS bytes of the validator signature.
    pub fn signature_bcs(&self) -> Option<&[u8]> {
        self.signature
            .as_ref()
            .and_then(|s| s.bcs.as_ref())
            .map(BcsData::as_bytes)
    }

    /// Get the summary digest directly from the nested summary.
    ///
    /// This method enforces that a well-formed checkpoint summary includes its
    /// digest. Unlike [`CheckpointSummary::summary_digest()`] which is a
    /// low-level accessor, this method treats a missing digest as an error when
    /// the summary itself is present.
    ///
    /// Returns `Ok(None)` if the summary field is not present.
    /// Returns `Err` if the summary is present but its digest field is missing.
    pub fn summary_digest(&self) -> Result<Option<iota_sdk_types::Digest>, TryFromProtoError> {
        self.summary
            .as_ref()
            .map(|s| {
                s.summary_digest()
                    .map_err(|e| e.nested(Self::SUMMARY_FIELD.name))?
                    .ok_or_else(|| {
                        TryFromProtoError::missing(CheckpointSummary::DIGEST_FIELD.name)
                            .nested(Self::SUMMARY_FIELD.name)
                    })
            })
            .transpose()
    }

    /// Get the contents digest directly from the nested contents.
    ///
    /// This method enforces that well-formed checkpoint contents includes its
    /// digest. Unlike [`CheckpointContents::contents_digest()`] which is a
    /// low-level accessor, this method treats a missing digest as an error when
    /// the contents itself is present.
    ///
    /// Returns `Ok(None)` if the contents field is not present.
    /// Returns `Err` if the contents is present but its digest field is
    /// missing.
    pub fn contents_digest(&self) -> Result<Option<iota_sdk_types::Digest>, TryFromProtoError> {
        self.contents
            .as_ref()
            .map(|c| {
                c.contents_digest()
                    .map_err(|e| e.nested(Self::CONTENTS_FIELD.name))?
                    .ok_or_else(|| {
                        TryFromProtoError::missing(CheckpointContents::DIGEST_FIELD.name)
                            .nested(Self::CONTENTS_FIELD.name)
                    })
            })
            .transpose()
    }
}
