// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Conversions between `iota.validator.v2` proto types and native `iota-types`.

use iota_types::{
    digests::TransactionDigest,
    error::IotaError,
    messages_grpc::{ExecutedData, SubmitTransactionResult, WaitForEffectResponse},
};

use super::{bcs_deserialize, bcs_serialize};
use crate::api::{
    self, ExecutedStatus, ExpiredStatus, RejectedStatus, StatusDetail, SubmittedStatus,
    status_detail::Kind,
};

// --- TxDigest ↔ TransactionDigest ---

impl TryFrom<TransactionDigest> for api::TxDigest {
    type Error = IotaError;

    fn try_from(value: TransactionDigest) -> Result<Self, Self::Error> {
        Ok(Self {
            digest: bytes::Bytes::copy_from_slice(value.inner()),
        })
    }
}

impl TryFrom<api::TxDigest> for TransactionDigest {
    type Error = IotaError;

    fn try_from(value: api::TxDigest) -> Result<Self, Self::Error> {
        TransactionDigest::from_bytes(value.digest.as_ref()).map_err(|e| {
            IotaError::TransactionSerialization {
                error: format!("TxDigest: {e}"),
            }
        })
    }
}

// --- StatusDetail ↔ SubmitTransactionResult ---

impl TryFrom<SubmitTransactionResult> for StatusDetail {
    type Error = IotaError;

    fn try_from(value: SubmitTransactionResult) -> Result<Self, Self::Error> {
        let kind = match value {
            SubmitTransactionResult::Submitted => Kind::Submitted(SubmittedStatus {}),
            SubmitTransactionResult::Executed {
                effects_digest,
                details,
            } => Kind::Executed(ExecutedStatus {
                effects_digest: bcs_serialize(&effects_digest, "ExecutedStatus.effects_digest")?,
                details: Some(bcs_serialize(&*details, "ExecutedStatus.details")?),
            }),
            SubmitTransactionResult::Rejected { error } => Kind::Rejected(RejectedStatus {
                error: Some(bcs_serialize(&error, "RejectedStatus.error")?),
            }),
        };
        Ok(StatusDetail { kind: Some(kind) })
    }
}

impl TryFrom<StatusDetail> for SubmitTransactionResult {
    type Error = IotaError;

    fn try_from(value: StatusDetail) -> Result<Self, Self::Error> {
        let kind = value
            .kind
            .ok_or_else(|| IotaError::TransactionSerialization {
                error: "StatusDetail.kind is None".to_string(),
            })?;
        match kind {
            Kind::Submitted(_) => Ok(SubmitTransactionResult::Submitted),
            Kind::Executed(e) => {
                let effects_digest =
                    bcs_deserialize(&e.effects_digest, "ExecutedStatus.effects_digest")?;
                let details_bytes =
                    e.details
                        .ok_or_else(|| IotaError::TransactionSerialization {
                            error: "ExecutedStatus.details is None for SubmitTransactionResult"
                                .to_string(),
                        })?;
                let details: ExecutedData =
                    bcs_deserialize(&details_bytes, "ExecutedStatus.details")?;
                Ok(SubmitTransactionResult::Executed {
                    effects_digest,
                    details: Box::new(details),
                })
            }
            Kind::Rejected(r) => {
                let error = r
                    .error
                    .as_ref()
                    .map(|e| bcs_deserialize(e, "RejectedStatus.error"))
                    .transpose()?
                    .ok_or_else(|| IotaError::TransactionSerialization {
                        error: "RejectedStatus.error is None for SubmitTransactionResult"
                            .to_string(),
                    })?;
                Ok(SubmitTransactionResult::Rejected { error })
            }
            Kind::Expired(_) => Err(IotaError::TransactionSerialization {
                error: "Expired status is not valid for SubmitTransactionResult".to_string(),
            }),
        }
    }
}

// --- StatusDetail ↔ WaitForEffectResponse ---

impl TryFrom<WaitForEffectResponse> for StatusDetail {
    type Error = IotaError;

    fn try_from(value: WaitForEffectResponse) -> Result<Self, Self::Error> {
        let kind = match value {
            WaitForEffectResponse::Executed {
                effects_digest,
                details,
            } => Kind::Executed(ExecutedStatus {
                effects_digest: bcs_serialize(&effects_digest, "ExecutedStatus.effects_digest")?,
                details: details
                    .as_ref()
                    .map(|d| bcs_serialize(d.as_ref(), "ExecutedStatus.details"))
                    .transpose()?,
            }),
            WaitForEffectResponse::Rejected { error } => Kind::Rejected(RejectedStatus {
                error: error
                    .as_ref()
                    .map(|e| bcs_serialize(e, "RejectedStatus.error"))
                    .transpose()?,
            }),
            WaitForEffectResponse::Expired { epoch } => Kind::Expired(ExpiredStatus { epoch }),
        };
        Ok(StatusDetail { kind: Some(kind) })
    }
}

impl TryFrom<StatusDetail> for WaitForEffectResponse {
    type Error = IotaError;

    fn try_from(value: StatusDetail) -> Result<Self, Self::Error> {
        let kind = value
            .kind
            .ok_or_else(|| IotaError::TransactionSerialization {
                error: "StatusDetail.kind is None".to_string(),
            })?;
        match kind {
            Kind::Submitted(_) => Err(IotaError::TransactionSerialization {
                error: "Submitted status is not valid for WaitForEffectResponse".to_string(),
            }),
            Kind::Executed(e) => {
                let effects_digest =
                    bcs_deserialize(&e.effects_digest, "ExecutedStatus.effects_digest")?;
                let details = e
                    .details
                    .as_ref()
                    .map(|d| bcs_deserialize::<ExecutedData>(d, "ExecutedStatus.details"))
                    .transpose()?
                    .map(Box::new);
                Ok(WaitForEffectResponse::Executed {
                    effects_digest,
                    details,
                })
            }
            Kind::Rejected(r) => {
                let error = r
                    .error
                    .as_ref()
                    .map(|e| bcs_deserialize(e, "RejectedStatus.error"))
                    .transpose()?;
                Ok(WaitForEffectResponse::Rejected { error })
            }
            Kind::Expired(e) => Ok(WaitForEffectResponse::Expired { epoch: e.epoch }),
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_types::{
        digests::{TransactionDigest, TransactionEffectsDigest},
        error::IotaError,
        messages_grpc::{ExecutedData, SubmitTransactionResult, WaitForEffectResponse},
    };

    use crate::api::{self, StatusDetail, status_detail::Kind};

    // --- TxDigest round-trip ---

    #[test]
    fn tx_digest_round_trip() {
        let digest = TransactionDigest::random();
        let proto: api::TxDigest = digest.try_into().unwrap();
        let back: TransactionDigest = proto.try_into().unwrap();
        assert_eq!(digest, back);
    }

    #[test]
    fn tx_digest_invalid_length() {
        let proto = api::TxDigest {
            digest: bytes::Bytes::from_static(&[0u8; 5]),
        };
        let result = TransactionDigest::try_from(proto);
        assert!(result.is_err());
    }

    // --- SubmitTransactionResult round-trips ---

    #[test]
    fn submit_submitted_round_trip() {
        let original = SubmitTransactionResult::Submitted;
        let proto: StatusDetail = original.try_into().unwrap();
        assert!(matches!(proto.kind, Some(Kind::Submitted(_))));
        let back: SubmitTransactionResult = proto.try_into().unwrap();
        assert!(matches!(back, SubmitTransactionResult::Submitted));
    }

    #[test]
    fn submit_executed_round_trip() {
        let original = SubmitTransactionResult::Executed {
            effects_digest: TransactionEffectsDigest::random(),
            details: Box::new(ExecutedData::default()),
        };
        let proto: StatusDetail = original.try_into().unwrap();
        assert!(matches!(proto.kind, Some(Kind::Executed(_))));
        let back: SubmitTransactionResult = proto.try_into().unwrap();
        assert!(matches!(back, SubmitTransactionResult::Executed { .. }));
    }

    #[test]
    fn submit_rejected_round_trip() {
        let error = IotaError::TransactionSerialization {
            error: "test error".to_string(),
        };
        let original = SubmitTransactionResult::Rejected {
            error: error.clone(),
        };
        let proto: StatusDetail = original.try_into().unwrap();
        assert!(matches!(proto.kind, Some(Kind::Rejected(_))));
        let back: SubmitTransactionResult = proto.try_into().unwrap();
        match back {
            SubmitTransactionResult::Rejected { error: e } => {
                assert_eq!(e.to_string(), error.to_string());
            }
            _ => panic!("expected Rejected"),
        }
    }

    #[test]
    fn submit_rejects_expired_status() {
        let proto = StatusDetail {
            kind: Some(Kind::Expired(crate::api::ExpiredStatus { epoch: 1 })),
        };
        let result = SubmitTransactionResult::try_from(proto);
        assert!(result.is_err());
    }

    // --- WaitForEffectResponse round-trips ---

    #[test]
    fn wait_executed_round_trip() {
        let digest = TransactionEffectsDigest::random();
        let original = WaitForEffectResponse::Executed {
            effects_digest: digest,
            details: Some(Box::new(ExecutedData::default())),
        };
        let proto: StatusDetail = original.try_into().unwrap();
        assert!(matches!(proto.kind, Some(Kind::Executed(_))));
        let back: WaitForEffectResponse = proto.try_into().unwrap();
        match back {
            WaitForEffectResponse::Executed {
                effects_digest,
                details,
            } => {
                assert_eq!(effects_digest, digest);
                assert!(details.is_some());
            }
            _ => panic!("expected Executed"),
        }
    }

    #[test]
    fn wait_executed_without_details_round_trip() {
        let digest = TransactionEffectsDigest::random();
        let original = WaitForEffectResponse::Executed {
            effects_digest: digest,
            details: None,
        };
        let proto: StatusDetail = original.try_into().unwrap();
        let back: WaitForEffectResponse = proto.try_into().unwrap();
        match back {
            WaitForEffectResponse::Executed {
                effects_digest,
                details,
            } => {
                assert_eq!(effects_digest, digest);
                assert!(details.is_none());
            }
            _ => panic!("expected Executed"),
        }
    }

    #[test]
    fn wait_rejected_with_error_round_trip() {
        let error = IotaError::TransactionSerialization {
            error: "conflict".to_string(),
        };
        let original = WaitForEffectResponse::Rejected {
            error: Some(error.clone()),
        };
        let proto: StatusDetail = original.try_into().unwrap();
        let back: WaitForEffectResponse = proto.try_into().unwrap();
        match back {
            WaitForEffectResponse::Rejected { error: Some(e) } => {
                assert_eq!(e.to_string(), error.to_string());
            }
            _ => panic!("expected Rejected with error"),
        }
    }

    #[test]
    fn wait_rejected_without_error_round_trip() {
        let original = WaitForEffectResponse::Rejected { error: None };
        let proto: StatusDetail = original.try_into().unwrap();
        let back: WaitForEffectResponse = proto.try_into().unwrap();
        assert!(matches!(
            back,
            WaitForEffectResponse::Rejected { error: None }
        ));
    }

    #[test]
    fn wait_expired_round_trip() {
        let original = WaitForEffectResponse::Expired { epoch: 42 };
        let proto: StatusDetail = original.try_into().unwrap();
        let back: WaitForEffectResponse = proto.try_into().unwrap();
        match back {
            WaitForEffectResponse::Expired { epoch } => assert_eq!(epoch, 42),
            _ => panic!("expected Expired"),
        }
    }

    #[test]
    fn wait_rejects_submitted_status() {
        let proto = StatusDetail {
            kind: Some(Kind::Submitted(crate::api::SubmittedStatus {})),
        };
        let result = WaitForEffectResponse::try_from(proto);
        assert!(result.is_err());
    }

    #[test]
    fn status_detail_none_kind_is_error() {
        let proto = StatusDetail { kind: None };
        assert!(SubmitTransactionResult::try_from(proto.clone()).is_err());
        assert!(WaitForEffectResponse::try_from(proto).is_err());
    }
}
