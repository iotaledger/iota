// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v1.checkpoint.rs");
include!("../../../generated/iota.grpc.v1.checkpoint.field_info.rs");
include!("../../../generated/iota.grpc.v1.checkpoint.accessors.rs");

use crate::{
    proto::{TryFromProtoError, get_inner_field},
    v1::{bcs::BcsData, versioned::VersionedCheckpointSummary},
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
        bcs.deserialize::<VersionedCheckpointSummary>()
            .map_err(|e| TryFromProtoError::invalid(CheckpointSummary::BCS_FIELD, e))?
            .try_into_v1()
            .map_err(|_| {
                TryFromProtoError::invalid(
                    CheckpointSummary::BCS_FIELD,
                    "unsupported CheckpointSummary version",
                )
            })
    }
}

impl CheckpointSummary {
    /// Get the digest of this checkpoint summary.
    ///
    /// **Read mask:** `"checkpoint.summary.digest"` (see
    /// [`CHECKPOINT_SUMMARY_DIGEST`]).
    ///
    /// [`CHECKPOINT_SUMMARY_DIGEST`]: crate::read_masks::CHECKPOINT_SUMMARY_DIGEST
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.digest, Self::DIGEST_FIELD, try_into)
    }

    /// Deserialize the checkpoint summary from BCS.
    ///
    /// **Read mask:** `"checkpoint.summary.bcs"` (see
    /// [`CHECKPOINT_SUMMARY_BCS`]).
    ///
    /// [`CHECKPOINT_SUMMARY_BCS`]: crate::read_masks::CHECKPOINT_SUMMARY_BCS
    pub fn summary(&self) -> Result<iota_sdk_types::CheckpointSummary, TryFromProtoError> {
        self.try_into()
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
        // CheckpointContents has a custom Serialize impl that embeds
        // a BCS enum discriminant byte, so no versioned wrapper needed.
        BcsData::deserialize(bcs)
            .map_err(|e| TryFromProtoError::invalid(CheckpointContents::BCS_FIELD, e))
    }
}

impl CheckpointContents {
    /// Get the digest of this checkpoint contents.
    ///
    /// **Read mask:** `"checkpoint.contents.digest"` (see
    /// [`CHECKPOINT_CONTENTS_DIGEST`]).
    ///
    /// [`CHECKPOINT_CONTENTS_DIGEST`]: crate::read_masks::CHECKPOINT_CONTENTS_DIGEST
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.digest, Self::DIGEST_FIELD, try_into)
    }

    /// Deserialize the checkpoint contents from BCS.
    ///
    /// **Read mask:** `"checkpoint.contents.bcs"` (see
    /// [`CHECKPOINT_CONTENTS_BCS`]).
    ///
    /// [`CHECKPOINT_CONTENTS_BCS`]: crate::read_masks::CHECKPOINT_CONTENTS_BCS
    pub fn contents(&self) -> Result<iota_sdk_types::CheckpointContents, TryFromProtoError> {
        self.try_into()
    }
}
