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
}

// Checkpoint
//

impl Checkpoint {
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
}
