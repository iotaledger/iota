// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Conversions between `iota.validator.v2` proto types and native `iota-types`.

use iota_types::{
    digests::TransactionDigest,
    error::IotaError,
    messages_consensus::SignedAuthorityCapabilitiesV1,
    messages_grpc::{
        GetTxStatusRequest, HandleCapabilityNotificationRequestV1,
        HandleCapabilityNotificationResponseV1, SubmitTransactionsRequest, TxStatusQuery,
        TxStatusUpdate,
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

impl TryFrom<api::SubmitTxRequest> for SubmitTransactionsRequest {
    type Error = IotaError;

    fn try_from(value: api::SubmitTxRequest) -> Result<Self, Self::Error> {
        let transactions = value
            .tx
            .iter()
            .map(|t| bcs_deserialize::<Transaction>(t, "SubmitTxRequest.tx"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SubmitTransactionsRequest { transactions })
    }
}

// --- SubmitTxRequest (domain → proto, used by tests) ---

impl TryFrom<SubmitTransactionsRequest> for api::SubmitTxRequest {
    type Error = IotaError;

    fn try_from(value: SubmitTransactionsRequest) -> Result<Self, Self::Error> {
        let tx = value
            .transactions
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
                error: error
                    .as_ref()
                    .map(|e| bcs_serialize(e, "RejectedStatus.error"))
                    .transpose()?,
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

#[cfg(test)]
mod tests {
    use iota_types::{
        digests::{TransactionDigest, TransactionEffectsDigest},
        error::IotaError,
        messages_grpc::{ExecutedData, GetTxStatusRequest, TxStatusUpdate},
    };

    use crate::api::{self, status_detail::Kind};

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
            error: Some(IotaError::TransactionSerialization {
                error: "conflict".to_string(),
            }),
        };
        let tx_status: api::TxStatus = (digest, update).try_into().unwrap();
        assert!(matches!(
            tx_status.status.unwrap().kind,
            Some(Kind::Rejected(_))
        ));
    }

    #[test]
    fn tx_status_update_rejected_no_error() {
        let digest = TransactionDigest::random();
        let update = TxStatusUpdate::Rejected { error: None };
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
}
