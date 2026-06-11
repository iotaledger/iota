// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Conversions between `iota.validator.v2` proto types and native `iota-types`.

use iota_types::{
    digests::TransactionDigest,
    error::IotaError,
    messages_consensus::SignedAuthorityCapabilitiesV1,
    messages_grpc::{
        ExecutedData, GetTxStatusRequest, HandleCapabilityNotificationRequestV1,
        HandleCapabilityNotificationResponseV1, TxStatusQuery, TxStatusUpdate,
        ValidatorHealthRequest, ValidatorHealthResponse,
    },
    transaction::Transaction,
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

// --- TxStatusQuery (domain → proto) ---

impl TryFrom<TxStatusQuery> for api::TxStatusQuery {
    type Error = IotaError;

    fn try_from(value: TxStatusQuery) -> Result<Self, Self::Error> {
        Ok(Self {
            tx_digest: Some(value.transaction_digest.try_into()?),
            include_details: value.include_details,
        })
    }
}

// --- GetTxStatusRequest (domain → proto) ---

impl TryFrom<GetTxStatusRequest> for api::GetTxStatusRequest {
    type Error = IotaError;

    fn try_from(value: GetTxStatusRequest) -> Result<Self, Self::Error> {
        let queries = value
            .queries
            .into_iter()
            .map(api::TxStatusQuery::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { queries })
    }
}

// --- GetTxStatusRequest (proto → domain) ---

impl TryFrom<api::TxStatusQuery> for TxStatusQuery {
    type Error = IotaError;

    fn try_from(value: api::TxStatusQuery) -> Result<Self, Self::Error> {
        let tx_digest = value
            .tx_digest
            .ok_or_else(|| IotaError::TransactionSerialization {
                error: "TxStatusQuery.tx_digest is None".to_string(),
            })?;
        Ok(TxStatusQuery {
            transaction_digest: tx_digest.try_into()?,
            include_details: value.include_details,
        })
    }
}

impl TryFrom<api::GetTxStatusRequest> for GetTxStatusRequest {
    type Error = IotaError;

    fn try_from(value: api::GetTxStatusRequest) -> Result<Self, Self::Error> {
        let queries = value
            .queries
            .into_iter()
            .map(TxStatusQuery::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(GetTxStatusRequest { queries })
    }
}

// --- SubmitTxRequest (proto → domain) ---

impl TryFrom<api::SubmitTxRequest> for Vec<Transaction> {
    type Error = IotaError;

    fn try_from(value: api::SubmitTxRequest) -> Result<Self, Self::Error> {
        value
            .tx
            .iter()
            .map(|t| bcs_deserialize::<Transaction>(t, "SubmitTxRequest.tx"))
            .collect()
    }
}

// --- SubmitTxRequest (domain → proto, used by tests) ---

impl TryFrom<Vec<Transaction>> for api::SubmitTxRequest {
    type Error = IotaError;

    fn try_from(value: Vec<Transaction>) -> Result<Self, Self::Error> {
        let tx = value
            .iter()
            .map(|t| bcs_serialize(t, "SubmitTxRequest.tx"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(api::SubmitTxRequest { tx })
    }
}

// --- TxStatusUpdate → StatusDetail → TxStatus ---

impl TryFrom<TxStatusUpdate> for StatusDetail {
    type Error = IotaError;

    fn try_from(value: TxStatusUpdate) -> Result<Self, Self::Error> {
        let kind = match value {
            TxStatusUpdate::Submitted => Kind::Submitted(SubmittedStatus {}),
            TxStatusUpdate::Executed {
                effects_digest,
                details,
            } => Kind::Executed(ExecutedStatus {
                effects_digest: bcs_serialize(&effects_digest, "ExecutedStatus.effects_digest")?,
                details: details
                    .as_ref()
                    .map(|d| bcs_serialize(d.as_ref(), "ExecutedStatus.details"))
                    .transpose()?,
            }),
            TxStatusUpdate::Rejected { error } => Kind::Rejected(RejectedStatus {
                error: bcs_serialize(&error, "RejectedStatus.error")?,
            }),
            TxStatusUpdate::Expired { epoch } => Kind::Expired(ExpiredStatus { epoch }),
        };
        Ok(StatusDetail { kind: Some(kind) })
    }
}

impl TryFrom<(TransactionDigest, TxStatusUpdate)> for api::TxStatus {
    type Error = IotaError;

    fn try_from(
        (digest, update): (TransactionDigest, TxStatusUpdate),
    ) -> Result<Self, Self::Error> {
        Ok(api::TxStatus {
            tx_digest: Some(digest.try_into()?),
            status: Some(update.try_into()?),
        })
    }
}

// --- TxStatus (proto → domain) ---

impl TryFrom<StatusDetail> for TxStatusUpdate {
    type Error = IotaError;

    fn try_from(value: StatusDetail) -> Result<Self, Self::Error> {
        let kind = value
            .kind
            .ok_or_else(|| IotaError::TransactionSerialization {
                error: "StatusDetail.kind is None".to_string(),
            })?;
        match kind {
            Kind::Submitted(_) => Ok(TxStatusUpdate::Submitted),
            Kind::Executed(e) => {
                let effects_digest =
                    bcs_deserialize(&e.effects_digest, "ExecutedStatus.effects_digest")?;
                let details = e
                    .details
                    .as_ref()
                    .map(|d| bcs_deserialize::<ExecutedData>(d, "ExecutedStatus.details"))
                    .transpose()?
                    .map(Box::new);
                Ok(TxStatusUpdate::Executed {
                    effects_digest,
                    details,
                })
            }
            Kind::Rejected(r) => {
                let error = bcs_deserialize(&r.error, "RejectedStatus.error")?;
                Ok(TxStatusUpdate::Rejected { error })
            }
            Kind::Expired(e) => Ok(TxStatusUpdate::Expired { epoch: e.epoch }),
        }
    }
}

impl TryFrom<api::TxStatus> for (TransactionDigest, TxStatusUpdate) {
    type Error = IotaError;

    fn try_from(value: api::TxStatus) -> Result<Self, Self::Error> {
        let digest = value
            .tx_digest
            .ok_or_else(|| IotaError::TransactionSerialization {
                error: "TxStatus.tx_digest is None".to_string(),
            })?
            .try_into()?;
        let status = value
            .status
            .ok_or_else(|| IotaError::TransactionSerialization {
                error: "TxStatus.status is None".to_string(),
            })?
            .try_into()?;
        Ok((digest, status))
    }
}

// --- NotifyCapabilitiesRequest ↔ HandleCapabilityNotificationRequestV1 ---

impl TryFrom<HandleCapabilityNotificationRequestV1> for api::NotifyCapabilitiesRequest {
    type Error = IotaError;

    fn try_from(value: HandleCapabilityNotificationRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            capabilities: bcs_serialize(&value.message, "NotifyCapabilitiesRequest.capabilities")?,
        })
    }
}

impl TryFrom<api::NotifyCapabilitiesRequest> for HandleCapabilityNotificationRequestV1 {
    type Error = IotaError;

    fn try_from(value: api::NotifyCapabilitiesRequest) -> Result<Self, Self::Error> {
        let message: SignedAuthorityCapabilitiesV1 = bcs_deserialize(
            &value.capabilities,
            "NotifyCapabilitiesRequest.capabilities",
        )?;
        Ok(Self { message })
    }
}

// --- NotifyCapabilitiesResponse ↔ HandleCapabilityNotificationResponseV1 ---

impl From<HandleCapabilityNotificationResponseV1> for api::NotifyCapabilitiesResponse {
    fn from(_value: HandleCapabilityNotificationResponseV1) -> Self {
        Self {}
    }
}

impl From<api::NotifyCapabilitiesResponse> for HandleCapabilityNotificationResponseV1 {
    fn from(_value: api::NotifyCapabilitiesResponse) -> Self {
        Self { _unused: false }
    }
}

// --- HealthCheckRequest ↔ ValidatorHealthRequest ---

impl From<ValidatorHealthRequest> for api::HealthCheckRequest {
    fn from(_value: ValidatorHealthRequest) -> Self {
        Self {}
    }
}

impl From<api::HealthCheckRequest> for ValidatorHealthRequest {
    fn from(_value: api::HealthCheckRequest) -> Self {
        Self {}
    }
}

// --- HealthCheckResponse ↔ ValidatorHealthResponse ---

impl From<ValidatorHealthResponse> for api::HealthCheckResponse {
    fn from(value: ValidatorHealthResponse) -> Self {
        Self {
            num_inflight_execution_transactions: value.num_inflight_execution_transactions,
            num_inflight_consensus_transactions: value.num_inflight_consensus_transactions,
            last_locally_built_checkpoint: value.last_locally_built_checkpoint,
        }
    }
}

impl From<api::HealthCheckResponse> for ValidatorHealthResponse {
    fn from(value: api::HealthCheckResponse) -> Self {
        Self {
            num_inflight_execution_transactions: value.num_inflight_execution_transactions,
            num_inflight_consensus_transactions: value.num_inflight_consensus_transactions,
            last_locally_built_checkpoint: value.last_locally_built_checkpoint,
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_types::{
        digests::{TransactionDigest, TransactionEffectsDigest},
        error::IotaError,
        messages_grpc::{ExecutedData, GetTxStatusRequest, TxStatusUpdate},
        transaction::Transaction,
    };

    use crate::api::{self, SubmittedStatus, status_detail::Kind};

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

    // --- GetTxStatusRequest ---

    #[test]
    fn get_tx_status_request_converts() {
        let d1 = TransactionDigest::random();
        let d2 = TransactionDigest::random();
        let proto = api::GetTxStatusRequest {
            queries: vec![
                api::TxStatusQuery {
                    tx_digest: Some(d1.try_into().unwrap()),
                    include_details: true,
                },
                api::TxStatusQuery {
                    tx_digest: Some(d2.try_into().unwrap()),
                    include_details: false,
                },
            ],
        };
        let domain: GetTxStatusRequest = proto.try_into().unwrap();
        assert_eq!(domain.queries.len(), 2);
        assert_eq!(domain.queries[0].transaction_digest, d1);
        assert!(domain.queries[0].include_details);
        assert_eq!(domain.queries[1].transaction_digest, d2);
        assert!(!domain.queries[1].include_details);
    }

    #[test]
    fn get_tx_status_query_missing_digest_is_error() {
        let proto = api::GetTxStatusRequest {
            queries: vec![api::TxStatusQuery {
                tx_digest: None,
                include_details: false,
            }],
        };
        let result = GetTxStatusRequest::try_from(proto);
        assert!(result.is_err());
    }

    #[test]
    fn get_tx_status_request_empty_queries() {
        let proto = api::GetTxStatusRequest { queries: vec![] };
        let domain: GetTxStatusRequest = proto.try_into().unwrap();
        assert!(domain.queries.is_empty());
    }

    // --- TxStatus from (TransactionDigest, TxStatusUpdate) ---

    #[test]
    fn tx_status_update_submitted() {
        let digest = TransactionDigest::random();
        let tx_status: api::TxStatus = (digest, TxStatusUpdate::Submitted).try_into().unwrap();
        assert!(tx_status.tx_digest.is_some());
        assert!(matches!(
            tx_status.status.unwrap().kind,
            Some(Kind::Submitted(_))
        ));
    }

    #[test]
    fn tx_status_update_executed_with_details() {
        let digest = TransactionDigest::random();
        let update = TxStatusUpdate::Executed {
            effects_digest: TransactionEffectsDigest::random(),
            details: Some(Box::new(ExecutedData::default())),
        };
        let tx_status: api::TxStatus = (digest, update).try_into().unwrap();
        assert!(matches!(
            tx_status.status.unwrap().kind,
            Some(Kind::Executed(_))
        ));
    }

    #[test]
    fn tx_status_update_executed_without_details() {
        let digest = TransactionDigest::random();
        let update = TxStatusUpdate::Executed {
            effects_digest: TransactionEffectsDigest::random(),
            details: None,
        };
        let tx_status: api::TxStatus = (digest, update).try_into().unwrap();
        assert!(matches!(
            tx_status.status.unwrap().kind,
            Some(Kind::Executed(_))
        ));
    }

    #[test]
    fn tx_status_update_rejected() {
        let digest = TransactionDigest::random();
        let update = TxStatusUpdate::Rejected {
            error: IotaError::TransactionSerialization {
                error: "conflict".to_string(),
            },
        };
        let tx_status: api::TxStatus = (digest, update).try_into().unwrap();
        assert!(matches!(
            tx_status.status.unwrap().kind,
            Some(Kind::Rejected(_))
        ));
    }

    #[test]
    fn tx_status_update_expired() {
        let digest = TransactionDigest::random();
        let update = TxStatusUpdate::Expired { epoch: 42 };
        let tx_status: api::TxStatus = (digest, update).try_into().unwrap();
        match tx_status.status.unwrap().kind {
            Some(Kind::Expired(e)) => assert_eq!(e.epoch, 42),
            _ => panic!("expected Expired"),
        }
    }

    // --- TxStatusQuery domain → proto round-trip ---

    #[test]
    fn tx_status_query_round_trip() {
        use iota_types::messages_grpc::TxStatusQuery;
        let query = TxStatusQuery {
            transaction_digest: TransactionDigest::random(),
            include_details: true,
        };
        let proto: api::TxStatusQuery = query.clone().try_into().unwrap();
        let back: TxStatusQuery = proto.try_into().unwrap();
        assert_eq!(query.transaction_digest, back.transaction_digest);
        assert_eq!(query.include_details, back.include_details);
    }

    // --- GetTxStatusRequest domain → proto round-trip ---

    #[test]
    fn get_tx_status_request_round_trip() {
        use iota_types::messages_grpc::TxStatusQuery;
        let request = GetTxStatusRequest {
            queries: vec![
                TxStatusQuery {
                    transaction_digest: TransactionDigest::random(),
                    include_details: true,
                },
                TxStatusQuery {
                    transaction_digest: TransactionDigest::random(),
                    include_details: false,
                },
            ],
        };
        let proto: api::GetTxStatusRequest = request.clone().try_into().unwrap();
        let back: GetTxStatusRequest = proto.try_into().unwrap();
        assert_eq!(request.queries.len(), back.queries.len());
        for (a, b) in request.queries.iter().zip(back.queries.iter()) {
            assert_eq!(a.transaction_digest, b.transaction_digest);
            assert_eq!(a.include_details, b.include_details);
        }
    }

    // --- TxStatus proto → domain round-trips ---

    #[test]
    fn tx_status_submitted_round_trip() {
        let digest = TransactionDigest::random();
        let update = TxStatusUpdate::Submitted;
        let proto: api::TxStatus = (digest, update).try_into().unwrap();
        let (back_digest, back_update): (TransactionDigest, TxStatusUpdate) =
            proto.try_into().unwrap();
        assert_eq!(digest, back_digest);
        assert!(matches!(back_update, TxStatusUpdate::Submitted));
    }

    #[test]
    fn tx_status_executed_round_trip() {
        let digest = TransactionDigest::random();
        let effects_digest = TransactionEffectsDigest::random();
        let update = TxStatusUpdate::Executed {
            effects_digest,
            details: None,
        };
        let proto: api::TxStatus = (digest, update).try_into().unwrap();
        let (back_digest, back_update): (TransactionDigest, TxStatusUpdate) =
            proto.try_into().unwrap();
        assert_eq!(digest, back_digest);
        match back_update {
            TxStatusUpdate::Executed {
                effects_digest: ed,
                details,
            } => {
                assert_eq!(effects_digest, ed);
                assert!(details.is_none());
            }
            _ => panic!("expected Executed"),
        }
    }

    #[test]
    fn tx_status_rejected_round_trip() {
        let digest = TransactionDigest::random();
        let update = TxStatusUpdate::Rejected {
            error: IotaError::TransactionSerialization {
                error: "test".to_string(),
            },
        };
        let proto: api::TxStatus = (digest, update).try_into().unwrap();
        let (back_digest, back_update): (TransactionDigest, TxStatusUpdate) =
            proto.try_into().unwrap();
        assert_eq!(digest, back_digest);
        assert!(matches!(back_update, TxStatusUpdate::Rejected { .. }));
    }

    #[test]
    fn tx_status_expired_round_trip() {
        let digest = TransactionDigest::random();
        let update = TxStatusUpdate::Expired { epoch: 99 };
        let proto: api::TxStatus = (digest, update).try_into().unwrap();
        let (back_digest, back_update): (TransactionDigest, TxStatusUpdate) =
            proto.try_into().unwrap();
        assert_eq!(digest, back_digest);
        match back_update {
            TxStatusUpdate::Expired { epoch } => assert_eq!(epoch, 99),
            _ => panic!("expected Expired"),
        }
    }

    #[test]
    fn tx_status_missing_digest_is_error() {
        let proto = api::TxStatus {
            tx_digest: None,
            status: Some(api::StatusDetail {
                kind: Some(Kind::Submitted(SubmittedStatus {})),
            }),
        };
        let result = <(TransactionDigest, TxStatusUpdate)>::try_from(proto);
        assert!(result.is_err());
    }

    #[test]
    fn tx_status_missing_status_is_error() {
        let digest = TransactionDigest::random();
        let proto = api::TxStatus {
            tx_digest: Some(digest.try_into().unwrap()),
            status: None,
        };
        let result = <(TransactionDigest, TxStatusUpdate)>::try_from(proto);
        assert!(result.is_err());
    }

    // --- SubmitTxRequest round-trip ---

    #[test]
    fn submit_tx_request_empty_round_trip() {
        let request: Vec<Transaction> = vec![];
        let proto: api::SubmitTxRequest = request.try_into().unwrap();
        let back: Vec<Transaction> = proto.try_into().unwrap();
        assert!(back.is_empty());
    }

    // --- NotifyCapabilitiesResponse round-trip ---

    #[test]
    fn notify_capabilities_response_round_trip() {
        use iota_types::messages_grpc::HandleCapabilityNotificationResponseV1;
        let response = HandleCapabilityNotificationResponseV1 { _unused: false };
        let proto: api::NotifyCapabilitiesResponse = response.into();
        let back: HandleCapabilityNotificationResponseV1 = proto.into();
        assert!(!back._unused);
    }

    // --- HealthCheckRequest round-trip ---

    #[test]
    fn health_check_request_round_trip() {
        use iota_types::messages_grpc::ValidatorHealthRequest;
        let request = ValidatorHealthRequest {};
        let proto: api::HealthCheckRequest = request.into();
        let _back: ValidatorHealthRequest = proto.into();
    }

    // --- HealthCheckResponse round-trip ---

    #[test]
    fn health_check_response_round_trip() {
        use iota_types::messages_grpc::ValidatorHealthResponse;
        let response = ValidatorHealthResponse {
            num_inflight_execution_transactions: 42,
            num_inflight_consensus_transactions: 7,
            last_locally_built_checkpoint: 100,
        };
        let proto: api::HealthCheckResponse = response.clone().into();
        let back: ValidatorHealthResponse = proto.into();
        assert_eq!(
            response.num_inflight_execution_transactions,
            back.num_inflight_execution_transactions
        );
        assert_eq!(
            response.num_inflight_consensus_transactions,
            back.num_inflight_consensus_transactions
        );
        assert_eq!(
            response.last_locally_built_checkpoint,
            back.last_locally_built_checkpoint
        );
    }
}
