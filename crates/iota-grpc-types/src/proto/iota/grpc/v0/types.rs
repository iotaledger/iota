// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.types.rs");
include!("../../../generated/iota.grpc.v0.types.field_info.rs");
include!("../../../generated/iota.grpc.v0.types.accessors.rs");

impl From<iota_sdk2::types::Digest> for Digest {
    fn from(value: iota_sdk2::types::Digest) -> Self {
        Self {
            digest: value.into_inner().to_vec().into(),
        }
    }
}

impl From<iota_sdk2::types::CheckpointDigest> for Digest {
    fn from(value: iota_sdk2::types::CheckpointDigest) -> Self {
        Self {
            digest: value.into_inner().to_vec().into(),
        }
    }
}

impl From<iota_sdk2::types::CheckpointContentsDigest> for Digest {
    fn from(value: iota_sdk2::types::CheckpointContentsDigest) -> Self {
        Self {
            digest: value.into_inner().to_vec().into(),
        }
    }
}

impl From<iota_sdk2::types::ObjectDigest> for Digest {
    fn from(value: iota_sdk2::types::ObjectDigest) -> Self {
        Self {
            digest: value.into_inner().to_vec().into(),
        }
    }
}
