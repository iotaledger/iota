// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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

/// Discriminates the submission mode in ['SubmitTransactionsRequest']. TODO: Is
/// it even necessary though?
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubmitTransactionsType {
    /// Single transaction submission (default path).
    #[default]
    Default,
    /// Ping (health-check / latency measurement); no transaction data.
    Ping,
    /// Multiple transactions submitted together as a soft bundle for
    /// post-consensus owned-object conflict detection.
    SoftBundle,
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

    pub fn is_soft_bundle(&self) -> bool {
        self.transactions.len() > 1
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
        /// Response should always include details for executed transactions.
        details: Option<Box<ExecutedData>>,
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitTransactionsResponse {
    pub result: SubmitTransactionResult,
}

/// Request to wait for transaction effects from a validator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaitForEffectsRequest {
    pub transaction_digest: Option<TransactionDigest>,
    pub include_details: bool,
    pub ping_type: bool,
}

/// Response from a validator to a wait for effects request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WaitForEffectsResponse {
    Executed {
        effects_digest: TransactionEffectsDigest,
        details: Option<Box<ExecutedData>>,
    },
    /// The transaction was rejected by consensus.
    Rejected { error: Option<IotaError> },
    /// Transaction status has expired from the cache.
    Expired { epoch: EpochId },
}
