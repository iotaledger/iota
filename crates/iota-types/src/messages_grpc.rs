// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use bytes::Bytes;
use iota_sdk_types::ObjectId;
use move_core_types::annotated_value::MoveStructLayout;
use serde::{Deserialize, Serialize};

use crate::{
    base_types::{SequenceNumber, TransactionDigest},
    committee::EpochId,
    crypto::{AuthoritySignInfo, AuthorityStrongQuorumSignInfo},
    digests::TransactionEffectsDigest,
    effects::{
        SignedTransactionEffects, TransactionEffects, TransactionEvents,
        VerifiedSignedTransactionEffects,
    },
    error::IotaError,
    messages_consensus::SignedAuthorityCapabilitiesV1,
    object::Object,
    transaction::{CertifiedTransaction, SenderSignedData, SignedTransaction, Transaction},
};

/// Request for validator health information.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ValidatorHealthRequest {}

/// Response with validator health metrics.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ValidatorHealthResponse {
    /// Number of in-flight execution transactions from execution scheduler.
    pub num_inflight_execution_transactions: u64,
    /// Number of in-flight consensus transactions.
    pub num_inflight_consensus_transactions: u64,
    /// Sequence number of the last locally built checkpoint.
    pub last_locally_built_checkpoint: u64,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub enum ObjectInfoRequestKind {
    /// Request the latest object state.
    LatestObjectInfo,
    /// Request a specific version of the object.
    /// This is used only for debugging purpose and will not work as a generic
    /// solution since we don't keep around all historic object versions.
    /// No production code should depend on this kind.
    PastObjectInfoDebug(SequenceNumber),
}

/// Layout generation options -- you can either generate or not generate the
/// layout.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub enum LayoutGenerationOption {
    Generate,
    None,
}

/// A request for information about an object and optionally its
/// parent certificate at a specific version.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct ObjectInfoRequest {
    /// The id of the object to retrieve, at the latest version.
    pub object_id: ObjectId,
    /// if true return the layout of the object.
    pub generate_layout: LayoutGenerationOption,
    /// The type of request, either latest object info or the past.
    pub request_kind: ObjectInfoRequestKind,
}

impl ObjectInfoRequest {
    pub fn past_object_info_debug_request(
        object_id: ObjectId,
        version: SequenceNumber,
        generate_layout: LayoutGenerationOption,
    ) -> Self {
        ObjectInfoRequest {
            object_id,
            generate_layout,
            request_kind: ObjectInfoRequestKind::PastObjectInfoDebug(version),
        }
    }

    pub fn latest_object_info_request(
        object_id: ObjectId,
        generate_layout: LayoutGenerationOption,
    ) -> Self {
        ObjectInfoRequest {
            object_id,
            generate_layout,
            request_kind: ObjectInfoRequestKind::LatestObjectInfo,
        }
    }
}

/// This message provides information about the latest object and its lock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectInfoResponse {
    /// Value of the requested object in this authority
    pub object: Object,
    /// Schema of the Move value inside this object.
    /// None if the object is a Move package, or the request did not ask for the
    /// layout
    pub layout: Option<MoveStructLayout>,
    /// Transaction the object is locked on in this authority.
    /// None if the object is not currently locked by this authority.
    /// This should be only used for debugging purpose, such as from iota-tool.
    /// No prod clients should rely on it.
    pub lock_for_debugging: Option<SignedTransaction>,
}

/// Verified version of `ObjectInfoResponse`. `layout` and `lock_for_debugging`
/// are skipped because they are not needed and we don't want to verify them.
#[derive(Debug, Clone)]
pub struct VerifiedObjectInfoResponse {
    /// Value of the requested object in this authority
    pub object: Object,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionInfoRequest {
    pub transaction_digest: TransactionDigest,
}

#[expect(clippy::large_enum_variant)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TransactionStatus {
    /// Signature over the transaction.
    Signed(AuthoritySignInfo),
    /// For executed transaction, we could return an optional certificate
    /// signature on the transaction (i.e. the signature part of the
    /// CertifiedTransaction), as well as the signed effects.
    /// The certificate signature is optional because for transactions executed
    /// in previous epochs, we won't keep around the certificate signatures.
    Executed(
        Option<AuthorityStrongQuorumSignInfo>,
        SignedTransactionEffects,
        TransactionEvents,
    ),
}

impl TransactionStatus {
    pub fn into_signed_for_testing(self) -> AuthoritySignInfo {
        match self {
            Self::Signed(s) => s,
            _ => unreachable!("Incorrect response type"),
        }
    }

    pub fn into_effects_for_testing(self) -> SignedTransactionEffects {
        match self {
            Self::Executed(_, e, _) => e,
            _ => unreachable!("Incorrect response type"),
        }
    }
}

impl PartialEq for TransactionStatus {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::Signed(s1) => match other {
                Self::Signed(s2) => s1.epoch == s2.epoch,
                _ => false,
            },
            Self::Executed(c1, e1, ev1) => match other {
                Self::Executed(c2, e2, ev2) => {
                    c1.as_ref().map(|a| a.epoch) == c2.as_ref().map(|a| a.epoch)
                        && e1.epoch() == e2.epoch()
                        && e1.digest() == e2.digest()
                        && ev1.digest() == ev2.digest()
                }
                _ => false,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HandleTransactionResponse {
    pub status: TransactionStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransactionInfoResponse {
    pub transaction: SenderSignedData,
    pub status: TransactionStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitCertificateResponse {
    /// If transaction is already executed, return same result as
    /// handle_certificate
    pub executed: Option<HandleCertificateResponseV1>,
}

#[derive(Clone, Debug)]
pub struct VerifiedHandleCertificateResponse {
    pub signed_effects: VerifiedSignedTransactionEffects,
    pub events: TransactionEvents,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemStateRequest {
    // This is needed to make gRPC happy.
    pub _unused: bool,
}

/// Response type for version 1 of the handle certificate validator API.
///
/// The corresponding version 1 request type allows for a client to request
/// events as well as input/output objects from a transaction's execution. Given
/// Validators operate with very aggressive object pruning, the return of
/// input/output objects is only done immediately after the transaction has been
/// executed locally on the validator and will not be returned for requests to
/// previously executed transactions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandleCertificateResponseV1 {
    pub signed_effects: SignedTransactionEffects,
    pub events: Option<TransactionEvents>,

    /// If requested, will include all initial versions of objects modified in
    /// this transaction. This includes owned objects included as input into
    /// the transaction as well as the assigned versions of shared objects.
    // TODO: In the future we may want to include shared objects or child objects which were read
    //  but not modified during execution.
    pub input_objects: Option<Vec<Object>>,

    /// If requested, will include all changed objects, including mutated,
    /// created and unwrapped objects. In other words, all objects that
    /// still exist in the object state after this transaction.
    pub output_objects: Option<Vec<Object>>,
    pub auxiliary_data: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandleCertificateRequestV1 {
    pub certificate: CertifiedTransaction,

    pub include_events: bool,
    pub include_input_objects: bool,
    pub include_output_objects: bool,
    pub include_auxiliary_data: bool,
}

impl HandleCertificateRequestV1 {
    pub fn new(certificate: CertifiedTransaction) -> Self {
        Self {
            certificate,
            include_events: false,
            include_input_objects: false,
            include_output_objects: false,
            include_auxiliary_data: false,
        }
    }

    pub fn with_events(mut self) -> Self {
        self.include_events = true;
        self
    }

    pub fn with_input_objects(mut self) -> Self {
        self.include_input_objects = true;
        self
    }

    pub fn with_output_objects(mut self) -> Self {
        self.include_output_objects = true;
        self
    }

    pub fn with_auxiliary_data(mut self) -> Self {
        self.include_auxiliary_data = true;
        self
    }
}

/// Response type for the handle Soft Bundle certificates validator API.
/// If `wait_for_effects` is true, it is guaranteed that:
///  - Number of responses will be equal to the number of input transactions.
///  - The order of the responses matches the order of the input transactions.
///
/// Otherwise, `responses` will be empty.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandleSoftBundleCertificatesResponseV1 {
    pub responses: Vec<HandleCertificateResponseV1>,
}

/// Soft Bundle request.  See [SIP-19](https://github.com/sui-foundation/sips/blob/main/sips/sip-19.md).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandleSoftBundleCertificatesRequestV1 {
    pub certificates: Vec<CertifiedTransaction>,

    pub wait_for_effects: bool,
    pub include_events: bool,
    pub include_input_objects: bool,
    pub include_output_objects: bool,
    pub include_auxiliary_data: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandleCapabilityNotificationRequestV1 {
    pub message: SignedAuthorityCapabilitiesV1,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandleCapabilityNotificationResponseV1 {
    // This is needed to make gRPC happy.
    pub _unused: bool,
}

// =========== TransactionDriver types ===========

/// Full executed transaction data returned from validators.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutedData {
    pub effects: TransactionEffects,
    pub events: Option<TransactionEvents>,
    pub input_objects: Vec<Object>,
    pub output_objects: Vec<Object>,
}

impl Default for ExecutedData {
    fn default() -> Self {
        use crate::effects::TransactionEffectsExt as _;
        Self {
            effects: TransactionEffects::new_empty_v1(TransactionDigest::default()),
            events: None,
            input_objects: Vec::new(),
            output_objects: Vec::new(),
        }
    }
}

/// Contains either a transaction or the type of Ping request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitTransactionsRequest {
    pub transactions: Vec<Transaction>,
}

impl From<Transaction> for SubmitTransactionsRequest {
    fn from(transaction: Transaction) -> Self {
        Self::new_transaction(transaction)
    }
}

impl SubmitTransactionsRequest {
    pub fn new_transaction(transaction: Transaction) -> Self {
        Self {
            transactions: vec![transaction],
        }
    }

    pub fn new_ping() -> Self {
        Self {
            transactions: vec![],
        }
    }

    /// Returns the digest of the transaction if it is a transaction request.
    /// Returns None if it is a ping request.
    pub fn tx_digest(&self) -> Vec<TransactionDigest> {
        self.transactions.iter().map(|t| *t.digest()).collect()
    }

    // TODO: are those checks ok or should we have a single method that returns an
    // enum?
    pub fn is_ping(&self) -> bool {
        self.transactions.is_empty()
    }
}

/// The result of submitting a transaction to a validator.
#[derive(Clone, Serialize, Deserialize)]
pub enum SubmitTransactionResult {
    /// The transaction was submitted to consensus.
    Submitted,
    /// The transaction has already been executed (finalized).
    Executed {
        effects_digest: TransactionEffectsDigest,
        details: Box<ExecutedData>,
    },
    /// The transaction was rejected.
    Rejected { error: IotaError },
}

impl std::fmt::Debug for SubmitTransactionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Submitted => f.debug_struct("Submitted").finish(),
            Self::Executed { effects_digest, .. } => f
                .debug_struct("Executed")
                .field("effects_digest", &format_args!("{effects_digest}"))
                .finish(),
            Self::Rejected { error } => f.debug_struct("Rejected").field("error", &error).finish(),
        }
    }
}

/// Response from the TransactionDriver submit_transaction endpoint.
/// Contains one result per submitted transaction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitTransactionsResponse {
    pub results: Vec<SubmitTransactionResult>,
}

/// A single wait-for-effects item within a batch request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaitForEffectRequest {
    pub transaction_digest: TransactionDigest,
    pub include_details: bool,
}

/// Batch request to wait for transaction effects from a validator.
/// An empty `requests` vec is treated as a ping.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaitForEffectsRequest {
    pub requests: Vec<WaitForEffectRequest>,
}

impl WaitForEffectsRequest {
    pub fn is_ping(&self) -> bool {
        self.requests.is_empty()
    }
}

/// Per-item response for a single wait-for-effects request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WaitForEffectResponse {
    Executed {
        effects_digest: TransactionEffectsDigest,
        details: Option<Box<ExecutedData>>,
    },
    /// The transaction was rejected by consensus.
    Rejected { error: Option<IotaError> },
    /// Transaction status has expired from the cache.
    Expired { epoch: EpochId },
}

/// Batch response from a validator to a wait for effects request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaitForEffectsResponse {
    pub results: Vec<WaitForEffectResponse>,
}

// =========== Raw prost wire-format types ===========

#[derive(Clone, prost::Message)]
pub struct RawExecutedData {
    #[prost(bytes = "bytes", tag = "1")]
    pub effects: Bytes,
    #[prost(bytes = "bytes", optional, tag = "2")]
    pub events: Option<Bytes>,
    #[prost(bytes = "bytes", repeated, tag = "3")]
    pub input_objects: Vec<Bytes>,
    #[prost(bytes = "bytes", repeated, tag = "4")]
    pub output_objects: Vec<Bytes>,
}

#[derive(Clone, prost::Message)]
pub struct RawSubmitTransactionsRequest {
    #[prost(bytes = "bytes", repeated, tag = "1")]
    pub transactions: Vec<Bytes>,
}

#[derive(Clone, prost::Message)]
pub struct RawSubmitTransactionsResponse {
    #[prost(message, repeated, tag = "1")]
    pub results: Vec<RawSubmitTransactionResult>,
}

#[derive(Clone, prost::Message)]
pub struct RawSubmitTransactionResult {
    #[prost(oneof = "RawSubmitStatus", tags = "1, 2, 3")]
    pub inner: Option<RawSubmitStatus>,
}

#[derive(Clone, prost::Oneof)]
pub enum RawSubmitStatus {
    #[prost(message, tag = "1")]
    Submitted(RawSubmittedStatus),
    #[prost(message, tag = "2")]
    Executed(RawExecutedStatus),
    #[prost(message, tag = "3")]
    Rejected(RawRejectedStatus),
}

#[derive(Clone, prost::Message)]
pub struct RawSubmittedStatus {}

#[derive(Clone, prost::Message)]
pub struct RawExecutedStatus {
    #[prost(bytes = "bytes", tag = "1")]
    pub effects_digest: Bytes,
    #[prost(message, optional, tag = "2")]
    pub details: Option<RawExecutedData>,
}

#[derive(Clone, prost::Message)]
pub struct RawRejectedStatus {
    #[prost(bytes = "bytes", optional, tag = "1")]
    pub error: Option<Bytes>,
}

/// A single wait-for-effect item in the raw (protobuf) batch request.
#[derive(Clone, prost::Message)]
pub struct RawWaitForEffectRequest {
    #[prost(bytes = "bytes", tag = "1")]
    pub transaction_digest: Bytes,
    #[prost(bool, tag = "2")]
    pub include_details: bool,
}

/// Batch request: repeated items. An empty `requests` vec is treated as a ping.
#[derive(Clone, prost::Message)]
pub struct RawWaitForEffectsRequest {
    #[prost(message, repeated, tag = "1")]
    pub requests: Vec<RawWaitForEffectRequest>,
}

/// Per-item response for a single wait-for-effect in the raw (protobuf) format.
#[derive(Clone, prost::Message)]
pub struct RawWaitForEffectResponse {
    #[prost(oneof = "RawWaitForEffectsStatus", tags = "1, 2, 3")]
    pub inner: Option<RawWaitForEffectsStatus>,
}

/// Batch response: repeated items.
#[derive(Clone, prost::Message)]
pub struct RawWaitForEffectsResponse {
    #[prost(message, repeated, tag = "1")]
    pub results: Vec<RawWaitForEffectResponse>,
}

#[derive(Clone, prost::Oneof)]
pub enum RawWaitForEffectsStatus {
    #[prost(message, tag = "1")]
    Executed(RawExecutedStatus),
    #[prost(message, tag = "2")]
    Rejected(RawRejectedStatus),
    #[prost(message, tag = "3")]
    Expired(RawExpiredStatus),
}

#[derive(Clone, prost::Message)]
pub struct RawExpiredStatus {
    #[prost(uint64, tag = "1")]
    pub epoch: u64,
}

#[derive(Clone, prost::Message)]
pub struct RawValidatorHealthRequest {}

#[derive(Clone, prost::Message)]
pub struct RawValidatorHealthResponse {
    #[prost(uint64, optional, tag = "1")]
    pub num_inflight_execution_transactions: Option<u64>,
    #[prost(uint64, optional, tag = "2")]
    pub num_inflight_consensus_transactions: Option<u64>,
    /// Sequence number of the last locally built checkpoint.
    #[prost(uint64, tag = "3")]
    pub last_locally_built_checkpoint: u64,
}

// =========== TryFrom conversions ===========

fn bcs_serialize<T: Serialize>(value: &T, type_info: &str) -> Result<Bytes, IotaError> {
    bcs::to_bytes(value)
        .map(Into::into)
        .map_err(|e| IotaError::TransactionSerialization {
            error: format!("{type_info}: {e}"),
        })
}

fn bcs_deserialize<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    type_info: &str,
) -> Result<T, IotaError> {
    bcs::from_bytes(bytes).map_err(|e| IotaError::TransactionSerialization {
        error: format!("{type_info}: {e}"),
    })
}

// --- ExecutedData ---

impl TryFrom<ExecutedData> for RawExecutedData {
    type Error = IotaError;

    fn try_from(value: ExecutedData) -> Result<Self, Self::Error> {
        let effects = bcs_serialize(&value.effects, "ExecutedData.effects")?;
        let events = value
            .events
            .as_ref()
            .map(|e| bcs_serialize(e, "ExecutedData.events"))
            .transpose()?;
        let input_objects = value
            .input_objects
            .iter()
            .map(|o| bcs_serialize(o, "ExecutedData.input_objects"))
            .collect::<Result<Vec<_>, _>>()?;
        let output_objects = value
            .output_objects
            .iter()
            .map(|o| bcs_serialize(o, "ExecutedData.output_objects"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RawExecutedData {
            effects,
            events,
            input_objects,
            output_objects,
        })
    }
}

impl TryFrom<RawExecutedData> for ExecutedData {
    type Error = IotaError;

    fn try_from(value: RawExecutedData) -> Result<Self, Self::Error> {
        let effects = bcs_deserialize(&value.effects, "RawExecutedData.effects")?;
        let events = value
            .events
            .as_ref()
            .map(|e| bcs_deserialize(e, "RawExecutedData.events"))
            .transpose()?;
        let input_objects = value
            .input_objects
            .iter()
            .map(|o| bcs_deserialize(o, "RawExecutedData.input_objects"))
            .collect::<Result<Vec<_>, _>>()?;
        let output_objects = value
            .output_objects
            .iter()
            .map(|o| bcs_deserialize(o, "RawExecutedData.output_objects"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ExecutedData {
            effects,
            events,
            input_objects,
            output_objects,
        })
    }
}

// --- Shared helpers for Executed/Rejected status ---

fn try_from_response_executed_submit(
    effects_digest: TransactionEffectsDigest,
    details: ExecutedData,
) -> Result<RawExecutedStatus, IotaError> {
    let effects_digest_bytes = bcs_serialize(&effects_digest, "effects_digest")?;
    let details = Some(details.try_into()?);
    Ok(RawExecutedStatus {
        effects_digest: effects_digest_bytes,
        details,
    })
}

fn try_from_raw_executed_status_submit(
    executed: RawExecutedStatus,
) -> Result<(TransactionEffectsDigest, Box<ExecutedData>), IotaError> {
    let effects_digest =
        bcs_deserialize(&executed.effects_digest, "RawExecutedStatus.effects_digest")?;
    let details = executed
        .details
        .ok_or_else(|| IotaError::TransactionSerialization {
            error: "RawExecutedStatus.details is None for SubmitTransactionResult".to_string(),
        })?
        .try_into()
        .map(Box::new)?;
    Ok((effects_digest, details))
}

fn try_from_response_executed_wait(
    effects_digest: TransactionEffectsDigest,
    details: Option<Box<ExecutedData>>,
) -> Result<RawExecutedStatus, IotaError> {
    let effects_digest_bytes = bcs_serialize(&effects_digest, "effects_digest")?;
    let details = details.map(|d| (*d).try_into()).transpose()?;
    Ok(RawExecutedStatus {
        effects_digest: effects_digest_bytes,
        details,
    })
}

fn try_from_raw_executed_status_wait(
    executed: RawExecutedStatus,
) -> Result<(TransactionEffectsDigest, Option<Box<ExecutedData>>), IotaError> {
    let effects_digest =
        bcs_deserialize(&executed.effects_digest, "RawExecutedStatus.effects_digest")?;
    let details = executed
        .details
        .map(|d| d.try_into().map(Box::new))
        .transpose()?;
    Ok((effects_digest, details))
}

fn try_from_response_rejected(error: Option<IotaError>) -> Result<RawRejectedStatus, IotaError> {
    let error = error
        .map(|e| bcs_serialize(&e, "RawRejectedStatus.error"))
        .transpose()?;
    Ok(RawRejectedStatus { error })
}

fn try_from_raw_rejected_status(
    rejected: RawRejectedStatus,
) -> Result<Option<IotaError>, IotaError> {
    rejected
        .error
        .as_ref()
        .map(|e| bcs_deserialize(e, "RawRejectedStatus.error"))
        .transpose()
}

// --- SubmitTransactions ---

impl TryFrom<SubmitTransactionsRequest> for RawSubmitTransactionsRequest {
    type Error = IotaError;

    fn try_from(value: SubmitTransactionsRequest) -> Result<Self, Self::Error> {
        let transactions = value
            .transactions
            .iter()
            .map(|t| bcs_serialize(t, "SubmitTransactionsRequest.transactions"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RawSubmitTransactionsRequest { transactions })
    }
}

impl TryFrom<SubmitTransactionResult> for RawSubmitTransactionResult {
    type Error = IotaError;

    fn try_from(value: SubmitTransactionResult) -> Result<Self, Self::Error> {
        let inner = match value {
            SubmitTransactionResult::Submitted => RawSubmitStatus::Submitted(RawSubmittedStatus {}),
            SubmitTransactionResult::Executed {
                effects_digest,
                details,
            } => RawSubmitStatus::Executed(try_from_response_executed_submit(
                effects_digest,
                *details,
            )?),
            SubmitTransactionResult::Rejected { error } => {
                RawSubmitStatus::Rejected(try_from_response_rejected(Some(error))?)
            }
        };
        Ok(RawSubmitTransactionResult { inner: Some(inner) })
    }
}

impl TryFrom<RawSubmitTransactionResult> for SubmitTransactionResult {
    type Error = IotaError;

    fn try_from(value: RawSubmitTransactionResult) -> Result<Self, Self::Error> {
        match value.inner {
            Some(RawSubmitStatus::Submitted(_)) => Ok(SubmitTransactionResult::Submitted),
            Some(RawSubmitStatus::Executed(executed)) => {
                let (effects_digest, details) = try_from_raw_executed_status_submit(executed)?;
                Ok(SubmitTransactionResult::Executed {
                    effects_digest,
                    details,
                })
            }
            Some(RawSubmitStatus::Rejected(rejected)) => {
                let error = try_from_raw_rejected_status(rejected)?.unwrap_or(
                    IotaError::TransactionSerialization {
                        error: "RawSubmitTransactionResult rejected error is None".to_string(),
                    },
                );
                Ok(SubmitTransactionResult::Rejected { error })
            }
            None => Err(IotaError::TransactionSerialization {
                error: "RawSubmitTransactionResult.inner is None".to_string(),
            }),
        }
    }
}

impl TryFrom<SubmitTransactionsResponse> for RawSubmitTransactionsResponse {
    type Error = IotaError;

    fn try_from(value: SubmitTransactionsResponse) -> Result<Self, Self::Error> {
        let results = value
            .results
            .into_iter()
            .map(|r| r.try_into())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RawSubmitTransactionsResponse { results })
    }
}

impl TryFrom<RawSubmitTransactionsResponse> for SubmitTransactionsResponse {
    type Error = IotaError;

    fn try_from(value: RawSubmitTransactionsResponse) -> Result<Self, Self::Error> {
        let results = value
            .results
            .into_iter()
            .map(|r| r.try_into())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SubmitTransactionsResponse { results })
    }
}

// --- WaitForEffects ---

impl TryFrom<WaitForEffectRequest> for RawWaitForEffectRequest {
    type Error = IotaError;

    fn try_from(value: WaitForEffectRequest) -> Result<Self, Self::Error> {
        let transaction_digest = bcs_serialize(
            &value.transaction_digest,
            "WaitForEffectRequest.transaction_digest",
        )?;
        Ok(RawWaitForEffectRequest {
            transaction_digest,
            include_details: value.include_details,
        })
    }
}

impl TryFrom<WaitForEffectsRequest> for RawWaitForEffectsRequest {
    type Error = IotaError;

    fn try_from(value: WaitForEffectsRequest) -> Result<Self, Self::Error> {
        let requests = value
            .requests
            .into_iter()
            .map(|r| r.try_into())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RawWaitForEffectsRequest { requests })
    }
}

impl TryFrom<WaitForEffectResponse> for RawWaitForEffectResponse {
    type Error = IotaError;

    fn try_from(value: WaitForEffectResponse) -> Result<Self, Self::Error> {
        let inner = match value {
            WaitForEffectResponse::Executed {
                effects_digest,
                details,
            } => RawWaitForEffectsStatus::Executed(try_from_response_executed_wait(
                effects_digest,
                details,
            )?),
            WaitForEffectResponse::Rejected { error } => {
                RawWaitForEffectsStatus::Rejected(try_from_response_rejected(error)?)
            }
            WaitForEffectResponse::Expired { epoch } => {
                RawWaitForEffectsStatus::Expired(RawExpiredStatus { epoch })
            }
        };
        Ok(RawWaitForEffectResponse { inner: Some(inner) })
    }
}

impl TryFrom<RawWaitForEffectResponse> for WaitForEffectResponse {
    type Error = IotaError;

    fn try_from(value: RawWaitForEffectResponse) -> Result<Self, Self::Error> {
        match value.inner {
            Some(RawWaitForEffectsStatus::Executed(executed)) => {
                let (effects_digest, details) = try_from_raw_executed_status_wait(executed)?;
                Ok(WaitForEffectResponse::Executed {
                    effects_digest,
                    details,
                })
            }
            Some(RawWaitForEffectsStatus::Rejected(rejected)) => {
                let error = try_from_raw_rejected_status(rejected)?;
                Ok(WaitForEffectResponse::Rejected { error })
            }
            Some(RawWaitForEffectsStatus::Expired(expired)) => Ok(WaitForEffectResponse::Expired {
                epoch: expired.epoch,
            }),
            None => Err(IotaError::TransactionSerialization {
                error: "RawWaitForEffectResponse.inner is None".to_string(),
            }),
        }
    }
}

impl TryFrom<WaitForEffectsResponse> for RawWaitForEffectsResponse {
    type Error = IotaError;

    fn try_from(value: WaitForEffectsResponse) -> Result<Self, Self::Error> {
        let results = value
            .results
            .into_iter()
            .map(|r| r.try_into())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RawWaitForEffectsResponse { results })
    }
}

impl TryFrom<RawWaitForEffectsResponse> for WaitForEffectsResponse {
    type Error = IotaError;

    fn try_from(value: RawWaitForEffectsResponse) -> Result<Self, Self::Error> {
        let results = value
            .results
            .into_iter()
            .map(|r| r.try_into())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(WaitForEffectsResponse { results })
    }
}

// --- ValidatorHealth ---

impl TryFrom<ValidatorHealthRequest> for RawValidatorHealthRequest {
    type Error = IotaError;

    fn try_from(_value: ValidatorHealthRequest) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl TryFrom<RawValidatorHealthRequest> for ValidatorHealthRequest {
    type Error = IotaError;

    fn try_from(_value: RawValidatorHealthRequest) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl TryFrom<ValidatorHealthResponse> for RawValidatorHealthResponse {
    type Error = IotaError;

    fn try_from(value: ValidatorHealthResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            num_inflight_execution_transactions: Some(value.num_inflight_execution_transactions),
            num_inflight_consensus_transactions: Some(value.num_inflight_consensus_transactions),
            last_locally_built_checkpoint: value.last_locally_built_checkpoint,
        })
    }
}

impl TryFrom<RawValidatorHealthResponse> for ValidatorHealthResponse {
    type Error = IotaError;

    fn try_from(value: RawValidatorHealthResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            num_inflight_execution_transactions: value
                .num_inflight_execution_transactions
                .unwrap_or(0),
            num_inflight_consensus_transactions: value
                .num_inflight_consensus_transactions
                .unwrap_or(0),
            last_locally_built_checkpoint: value.last_locally_built_checkpoint,
        })
    }
}

// =========== Tests ===========

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executed_data_round_trip() {
        let data = ExecutedData::default();
        let raw: RawExecutedData = data.clone().try_into().unwrap();
        let back: ExecutedData = raw.try_into().unwrap();
        assert_eq!(
            bcs::to_bytes(&data.effects).unwrap(),
            bcs::to_bytes(&back.effects).unwrap()
        );
        assert!(back.events.is_none());
        assert!(back.input_objects.is_empty());
        assert!(back.output_objects.is_empty());
    }

    #[test]
    fn test_submit_transactions_request_ping_to_raw() {
        let request = SubmitTransactionsRequest::new_ping();
        let raw: RawSubmitTransactionsRequest = request.try_into().unwrap();
        assert!(raw.transactions.is_empty());
    }

    #[test]
    fn test_submit_transaction_result_submitted_round_trip() {
        let result = SubmitTransactionResult::Submitted;
        let raw: RawSubmitTransactionResult = result.try_into().unwrap();
        let back: SubmitTransactionResult = raw.try_into().unwrap();
        assert!(matches!(back, SubmitTransactionResult::Submitted));
    }

    #[test]
    fn test_wait_for_effects_request_to_raw() {
        let request = WaitForEffectsRequest {
            requests: vec![WaitForEffectRequest {
                transaction_digest: TransactionDigest::default(),
                include_details: true,
            }],
        };
        let raw: RawWaitForEffectsRequest = request.try_into().unwrap();
        assert_eq!(raw.requests.len(), 1);
        assert!(raw.requests[0].include_details);
    }

    #[test]
    fn test_wait_for_effects_request_ping_to_raw() {
        let request = WaitForEffectsRequest { requests: vec![] };
        assert!(request.is_ping());
        let raw: RawWaitForEffectsRequest = request.try_into().unwrap();
        assert!(raw.requests.is_empty());
    }

    #[test]
    fn test_wait_for_effect_response_expired_round_trip() {
        let response = WaitForEffectResponse::Expired { epoch: 42 };
        let raw: RawWaitForEffectResponse = response.try_into().unwrap();
        let back: WaitForEffectResponse = raw.try_into().unwrap();
        match back {
            WaitForEffectResponse::Expired { epoch } => {
                assert_eq!(epoch, 42);
            }
            _ => panic!("Expected Expired variant"),
        }
    }

    #[test]
    fn test_wait_for_effects_response_batch_round_trip() {
        let response = WaitForEffectsResponse {
            results: vec![
                WaitForEffectResponse::Expired { epoch: 1 },
                WaitForEffectResponse::Rejected { error: None },
            ],
        };
        let raw: RawWaitForEffectsResponse = response.try_into().unwrap();
        assert_eq!(raw.results.len(), 2);
        let back: WaitForEffectsResponse = raw.try_into().unwrap();
        assert_eq!(back.results.len(), 2);
        assert!(matches!(
            back.results[0],
            WaitForEffectResponse::Expired { epoch: 1 }
        ));
        assert!(matches!(
            back.results[1],
            WaitForEffectResponse::Rejected { error: None }
        ));
    }

    #[test]
    fn test_validator_health_round_trip() {
        let request = ValidatorHealthRequest {};
        let raw_req: RawValidatorHealthRequest = request.try_into().unwrap();
        let _back_req: ValidatorHealthRequest = raw_req.try_into().unwrap();

        let response = ValidatorHealthResponse {
            num_inflight_execution_transactions: 10,
            num_inflight_consensus_transactions: 20,
            last_locally_built_checkpoint: 30,
        };
        let raw_resp: RawValidatorHealthResponse = response.try_into().unwrap();
        let back_resp: ValidatorHealthResponse = raw_resp.try_into().unwrap();
        assert_eq!(back_resp.num_inflight_execution_transactions, 10);
        assert_eq!(back_resp.num_inflight_consensus_transactions, 20);
        assert_eq!(back_resp.last_locally_built_checkpoint, 30);
    }
}
