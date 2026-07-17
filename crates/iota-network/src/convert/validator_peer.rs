// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Conversions between `iota.validator.peer` proto types and native
//! `iota-types`.

use iota_types::{
    error::IotaError,
    messages_checkpoint::{CheckpointRequest, CheckpointResponse, CheckpointSummaryResponse},
};

use super::{bcs_deserialize, bcs_serialize};
use crate::api;

// --- GetCheckpointRequest ↔ CheckpointRequest ---

impl From<CheckpointRequest> for api::GetCheckpointRequest {
    fn from(value: CheckpointRequest) -> Self {
        Self {
            sequence_number: value.sequence_number,
            request_content: value.request_content,
            certified: value.certified,
        }
    }
}

impl From<api::GetCheckpointRequest> for CheckpointRequest {
    fn from(value: api::GetCheckpointRequest) -> Self {
        Self {
            sequence_number: value.sequence_number,
            request_content: value.request_content,
            certified: value.certified,
        }
    }
}

// --- GetCheckpointResponse ↔ CheckpointResponse ---

impl TryFrom<CheckpointResponse> for api::GetCheckpointResponse {
    type Error = IotaError;

    fn try_from(value: CheckpointResponse) -> Result<Self, Self::Error> {
        let checkpoint = value
            .checkpoint
            .as_ref()
            .map(|c| bcs_serialize(c, "GetCheckpointResponse.checkpoint"))
            .transpose()?;
        let contents = value
            .contents
            .as_ref()
            .map(|c| bcs_serialize(c, "GetCheckpointResponse.contents"))
            .transpose()?;
        Ok(Self {
            checkpoint,
            contents,
        })
    }
}

impl TryFrom<api::GetCheckpointResponse> for CheckpointResponse {
    type Error = IotaError;

    fn try_from(value: api::GetCheckpointResponse) -> Result<Self, Self::Error> {
        let checkpoint: Option<CheckpointSummaryResponse> = value
            .checkpoint
            .as_ref()
            .map(|c| bcs_deserialize(c, "GetCheckpointResponse.checkpoint"))
            .transpose()?;
        let contents = value
            .contents
            .as_ref()
            .map(|c| bcs_deserialize(c, "GetCheckpointResponse.contents"))
            .transpose()?;
        Ok(Self {
            checkpoint,
            contents,
        })
    }
}

#[cfg(test)]
mod tests {
    use iota_types::messages_checkpoint::{CheckpointRequest, CheckpointResponse};

    use crate::api;

    // --- GetCheckpointRequest round-trips ---

    #[test]
    fn checkpoint_request_with_sequence_number_round_trip() {
        let original = CheckpointRequest {
            sequence_number: Some(100),
            request_content: true,
            certified: true,
        };
        let proto: api::GetCheckpointRequest = original.clone().into();
        assert_eq!(proto.sequence_number, Some(100));
        assert!(proto.request_content);
        assert!(proto.certified);
        let back: CheckpointRequest = proto.into();
        assert_eq!(back.sequence_number, original.sequence_number);
        assert_eq!(back.request_content, original.request_content);
        assert_eq!(back.certified, original.certified);
    }

    #[test]
    fn checkpoint_request_latest_round_trip() {
        let original = CheckpointRequest {
            sequence_number: None,
            request_content: false,
            certified: false,
        };
        let proto: api::GetCheckpointRequest = original.into();
        assert_eq!(proto.sequence_number, None);
        let back: CheckpointRequest = proto.into();
        assert_eq!(back.sequence_number, None);
        assert!(!back.request_content);
        assert!(!back.certified);
    }

    // --- GetCheckpointResponse round-trips ---

    #[test]
    fn checkpoint_response_empty_round_trip() {
        let original = CheckpointResponse {
            checkpoint: None,
            contents: None,
        };
        let proto: api::GetCheckpointResponse = original.try_into().unwrap();
        assert!(proto.checkpoint.is_none());
        assert!(proto.contents.is_none());
        let back: CheckpointResponse = proto.try_into().unwrap();
        assert!(back.checkpoint.is_none());
        assert!(back.contents.is_none());
    }
}
