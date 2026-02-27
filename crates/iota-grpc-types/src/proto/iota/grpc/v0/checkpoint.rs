// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.checkpoint.rs");
include!("../../../generated/iota.grpc.v0.checkpoint.field_info.rs");
include!("../../../generated/iota.grpc.v0.checkpoint.accessors.rs");

use crate::{
    proto::{TryFromProtoError, get_inner_field},
    v0::{bcs::BcsData, versioned::VersionedCheckpointSummary},
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
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.digest, Self::DIGEST_FIELD, try_into)
    }

    /// Deserialize checkpoint summary.
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
    pub fn digest(&self) -> Result<iota_sdk_types::Digest, TryFromProtoError> {
        get_inner_field!(self.digest, Self::DIGEST_FIELD, try_into)
    }

    /// Deserialize checkpoint contents.
    pub fn contents(&self) -> Result<iota_sdk_types::CheckpointContents, TryFromProtoError> {
        self.try_into()
    }
}
