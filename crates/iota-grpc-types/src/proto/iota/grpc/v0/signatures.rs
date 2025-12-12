// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.signatures.rs");
include!("../../../generated/iota.grpc.v0.signatures.field_info.rs");
include!("../../../generated/iota.grpc.v0.signatures.accessors.rs");

use crate::{field::FieldMaskTree, merge::Merge, proto::TryFromProtoError, v0::bcs::BcsData};

// ValidatorAggregatedSignature
//

impl From<iota_sdk_types::ValidatorAggregatedSignature> for ValidatorAggregatedSignature {
    fn from(value: iota_sdk_types::ValidatorAggregatedSignature) -> Self {
        Self {
            bcs: BcsData::serialize(&value).ok(),
        }
    }
}

impl TryFrom<&ValidatorAggregatedSignature> for iota_sdk_types::ValidatorAggregatedSignature {
    type Error = TryFromProtoError;

    fn try_from(value: &ValidatorAggregatedSignature) -> Result<Self, Self::Error> {
        let bcs = value.bcs.as_ref().ok_or_else(|| {
            TryFromProtoError::missing(ValidatorAggregatedSignature::BCS_FIELD.name)
        })?;
        BcsData::deserialize(bcs)
            .map_err(|e| TryFromProtoError::invalid(ValidatorAggregatedSignature::BCS_FIELD, e))
    }
}

// UserSignature
//

impl TryFrom<iota_sdk_types::UserSignature> for UserSignature {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: iota_sdk_types::UserSignature) -> Result<Self, Self::Error> {
        Self::merge_from(value, &FieldMaskTree::new_wildcard())
    }
}

impl Merge<iota_types::signature::GenericSignature> for UserSignature {
    fn merge(
        &mut self,
        source: iota_types::signature::GenericSignature,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !mask.contains(Self::BCS_FIELD.name) {
            // No need to convert if no field is requested
            return Ok(());
        }

        let sdk_signature: iota_sdk_types::UserSignature = source
            .try_into()
            .map_err(|e| format!("Failed to convert SDK signature: {}", e))?;

        Merge::merge(self, sdk_signature, mask)
    }
}

impl Merge<iota_sdk_types::UserSignature> for UserSignature {
    fn merge(
        &mut self,
        source: iota_sdk_types::UserSignature,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = BcsData::serialize(&source).ok();
        }

        Ok(())
    }
}

impl Merge<&UserSignature> for UserSignature {
    fn merge(
        &mut self,
        source: &UserSignature,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let UserSignature { bcs } = source;

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = bcs.clone();
        }

        Ok(())
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

// UserSignatures
//

impl Merge<&iota_sdk_types::SignedTransaction> for UserSignatures {
    fn merge(
        &mut self,
        source: &iota_sdk_types::SignedTransaction,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(signatures_mask) = mask.subtree(Self::SIGNATURES_FIELD.name) {
            self.signatures = source
                .signatures
                .iter()
                .map(|sig| UserSignature::merge_from(sig.clone(), &signatures_mask))
                .collect::<Result<Vec<_>, _>>()?;
        }

        Ok(())
    }
}
