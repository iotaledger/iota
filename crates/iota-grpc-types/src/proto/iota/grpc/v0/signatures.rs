// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.signatures.rs");
include!("../../../generated/iota.grpc.v0.signatures.field_info.rs");
include!("../../../generated/iota.grpc.v0.signatures.accessors.rs");

use crate::{field::FieldMaskTree, merge::Merge, proto::TryFromProtoError, v0::bcs::BcsData};

// ValidatorAggregatedSignature
//

impl From<iota_sdk2::types::ValidatorAggregatedSignature> for ValidatorAggregatedSignature {
    fn from(value: iota_sdk2::types::ValidatorAggregatedSignature) -> Self {
        Self {
            bcs: Some(BcsData::serialize(&value).unwrap()),
        }
    }
}

impl TryFrom<&ValidatorAggregatedSignature> for iota_sdk2::types::ValidatorAggregatedSignature {
    type Error = TryFromProtoError;

    fn try_from(value: &ValidatorAggregatedSignature) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing("bcs"))?;
        BcsData::deserialize(bcs)
            .map_err(|e| TryFromProtoError::invalid(ValidatorAggregatedSignature::BCS_FIELD, e))
    }
}

// UserSignature
//

impl From<iota_sdk2::types::UserSignature> for UserSignature {
    fn from(value: iota_sdk2::types::UserSignature) -> Self {
        Self::merge_from(value, &FieldMaskTree::new_wildcard())
    }
}

impl Merge<iota_sdk2::types::UserSignature> for UserSignature {
    fn merge(&mut self, source: iota_sdk2::types::UserSignature, mask: &FieldMaskTree) {
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = Some(BcsData::serialize(&source).unwrap());
        }
    }
}

impl Merge<&UserSignature> for UserSignature {
    fn merge(&mut self, source: &UserSignature, mask: &FieldMaskTree) {
        let UserSignature { bcs } = source;

        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = bcs.clone();
        }
    }
}

impl TryFrom<&UserSignature> for iota_sdk2::types::UserSignature {
    type Error = TryFromProtoError;

    fn try_from(value: &UserSignature) -> Result<Self, Self::Error> {
        let bcs = value
            .bcs
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing("bcs"))?;
        BcsData::deserialize(bcs)
            .map_err(|e| TryFromProtoError::invalid(UserSignature::BCS_FIELD, e))
    }
}
