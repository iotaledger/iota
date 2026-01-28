// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.types.rs");
include!("../../../generated/iota.grpc.v0.types.field_info.rs");
include!("../../../generated/iota.grpc.v0.types.accessors.rs");

impl From<iota_sdk_types::Digest> for Digest {
    fn from(value: iota_sdk_types::Digest) -> Self {
        Self {
            digest: value.into_inner().to_vec().into(),
        }
    }
}

impl From<iota_types::digests::TransactionDigest> for Digest {
    fn from(value: iota_types::digests::TransactionDigest) -> Self {
        Self {
            digest: value.into_inner().to_vec().into(),
        }
    }
}

impl TryFrom<&Digest> for iota_sdk_types::Digest {
    type Error = crate::proto::TryFromProtoError;

    fn try_from(value: &Digest) -> Result<Self, Self::Error> {
        iota_sdk_types::Digest::from_bytes(&value.digest)
            .map_err(|e| crate::proto::TryFromProtoError::invalid("digest", e))
    }
}
