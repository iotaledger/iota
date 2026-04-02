// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v1.signatures.rs");
include!("../../../generated/iota.grpc.v1.signatures.field_info.rs");
include!("../../../generated/iota.grpc.v1.signatures.accessors.rs");

use crate::{
    proto::{GrpcConversionError, TryFromProtoError},
    v1::{bcs::BcsData, versioned::VersionedValidatorAggregatedSignature},
};

// ValidatorAggregatedSignature
//

impl From<iota_sdk_types::ValidatorAggregatedSignature> for ValidatorAggregatedSignature {
    fn from(value: iota_sdk_types::ValidatorAggregatedSignature) -> Self {
        Self {
            bcs: BcsData::serialize(&VersionedValidatorAggregatedSignature::V1(value)).ok(),
        }
    }
}

impl TryFrom<&ValidatorAggregatedSignature> for iota_sdk_types::ValidatorAggregatedSignature {
    type Error = TryFromProtoError;

    fn try_from(value: &ValidatorAggregatedSignature) -> Result<Self, Self::Error> {
        let bcs = value.bcs.as_ref().ok_or_else(|| {
            TryFromProtoError::missing(ValidatorAggregatedSignature::BCS_FIELD.name)
        })?;
        bcs.deserialize::<VersionedValidatorAggregatedSignature>()
            .map_err(|e| TryFromProtoError::invalid(ValidatorAggregatedSignature::BCS_FIELD, e))?
            .try_into_v1()
            .map_err(|_| {
                TryFromProtoError::invalid(
                    ValidatorAggregatedSignature::BCS_FIELD,
                    "unsupported ValidatorAggregatedSignature version",
                )
            })
    }
}

impl ValidatorAggregatedSignature {
    /// Deserialize the validator aggregated signature.
    ///
    /// Requires `bcs` in the read_mask.
    pub fn signature(
        &self,
    ) -> Result<iota_sdk_types::ValidatorAggregatedSignature, TryFromProtoError> {
        self.try_into()
    }
}

// UserSignature
//

impl TryFrom<iota_sdk_types::UserSignature> for UserSignature {
    type Error = GrpcConversionError;

    fn try_from(value: iota_sdk_types::UserSignature) -> Result<Self, Self::Error> {
        Ok(Self {
            bcs: Some(BcsData::serialize(&value).map_err(|e| {
                GrpcConversionError::BcsSerializationFailed {
                    message: e.to_string(),
                }
            })?),
        })
    }
}

impl TryFrom<&UserSignature> for iota_sdk_types::UserSignature {
    type Error = TryFromProtoError;

    fn try_from(value: &UserSignature) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(UserSignature::BCS_FIELD.name))?;
        BcsData::deserialize(bcs)
            .map_err(|e| TryFromProtoError::invalid(UserSignature::BCS_FIELD, e))
    }
}

impl UserSignature {
    /// Deserialize the user signature.
    ///
    /// Requires `bcs` in the read_mask.
    pub fn signature(&self) -> Result<iota_sdk_types::UserSignature, TryFromProtoError> {
        self.try_into()
    }
}

// UserSignatures
//

// TryFrom implementation for UserSignatures
impl TryFrom<&UserSignatures> for Vec<iota_sdk_types::UserSignature> {
    type Error = TryFromProtoError;

    fn try_from(value: &UserSignatures) -> Result<Self, Self::Error> {
        value
            .signatures
            .iter()
            .enumerate()
            .map(|(i, sig)| {
                <&UserSignature as TryInto<iota_sdk_types::UserSignature>>::try_into(sig).map_err(
                    |e: TryFromProtoError| e.nested_at(UserSignatures::SIGNATURES_FIELD.name, i),
                )
            })
            .collect()
    }
}
