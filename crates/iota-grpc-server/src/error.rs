// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::google::rpc::{BadRequest, ErrorInfo, RetryInfo};
use iota_types::{base_types::ObjectID, digests::TransactionDigest};
use tonic::{Code, Status};

/// Main RPC error type
///
/// An error encountered while serving an RPC request.
/// The main purpose of this error type is to provide a convenient type for
/// converting between internal errors and a response that needs to be sent to a
/// calling client.
#[derive(Debug)]
pub struct RpcError {
    code: Code,
    message: Option<String>,
    details: Option<Box<ErrorDetails>>,
}

impl RpcError {
    pub fn new<T: std::fmt::Display>(code: Code, message: T) -> Self {
        Self {
            code,
            message: Some(message.to_string()),
            details: None,
        }
    }

    /// Add context to an existing error
    pub fn with_context<T: std::fmt::Display>(mut self, context: T) -> Self {
        self.message = Some(match self.message {
            Some(existing) => format!("{}: {}", context, existing),
            None => context.to_string(),
        });
        self
    }

    pub fn internal() -> Self {
        Self {
            code: Code::Internal,
            message: None,
            details: None,
        }
    }

    pub fn not_found() -> Self {
        Self {
            code: Code::NotFound,
            message: None,
            details: None,
        }
    }

    pub fn into_status_proto(self) -> iota_grpc_types::google::rpc::Status {
        iota_grpc_types::google::rpc::Status {
            code: self.code.into(),
            message: self.message.unwrap_or_default(),
            details: self
                .details
                .map(ErrorDetails::into_status_details)
                .unwrap_or_default(),
        }
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.message {
            Some(msg) => write!(f, "{:?}: {}", self.code, msg),
            None => write!(f, "{:?}", self.code),
        }
    }
}

impl From<RpcError> for Status {
    fn from(value: RpcError) -> Self {
        use prost::Message;

        let code = value.code;
        let status = value.into_status_proto();
        let details = status.encode_to_vec().into();
        let message = status.message;

        Status::with_details(code, message, details)
    }
}

impl From<anyhow::Error> for RpcError {
    fn from(value: anyhow::Error) -> Self {
        if value
            .chain()
            .any(|e| e.downcast_ref::<MissingIndexesError>().is_some())
        {
            return Self::new(Code::FailedPrecondition, value.to_string());
        }
        Self::internal().with_context(value)
    }
}

impl From<iota_types::iota_sdk_types_conversions::SdkTypeConversionError> for RpcError {
    fn from(value: iota_types::iota_sdk_types_conversions::SdkTypeConversionError) -> Self {
        Self::internal().with_context(value)
    }
}

impl From<bcs::Error> for RpcError {
    fn from(value: bcs::Error) -> Self {
        Self::internal().with_context(value)
    }
}

impl From<iota_grpc_types::proto::GrpcConversionError> for RpcError {
    fn from(value: iota_grpc_types::proto::GrpcConversionError) -> Self {
        Self::internal().with_context(value)
    }
}

impl From<iota_grpc_types::google::rpc::bad_request::FieldViolation> for RpcError {
    fn from(value: iota_grpc_types::google::rpc::bad_request::FieldViolation) -> Self {
        BadRequest::from(value).into()
    }
}

impl From<BadRequest> for RpcError {
    fn from(value: BadRequest) -> Self {
        let message = value
            .field_violations
            .first()
            .map(|violation| violation.description.clone());
        let details = ErrorDetails::new().with_bad_request(value);

        RpcError {
            code: Code::InvalidArgument,
            message,
            details: Some(Box::new(details)),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ErrorDetails {
    error_info: Option<ErrorInfo>,
    bad_request: Option<BadRequest>,
    retry_info: Option<RetryInfo>,
}

impl ErrorDetails {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_bad_request(mut self, bad_request: BadRequest) -> Self {
        self.bad_request = Some(bad_request);
        self
    }

    pub fn with_retry_info(mut self, retry_info: RetryInfo) -> Self {
        self.retry_info = Some(retry_info);
        self
    }

    #[allow(clippy::boxed_local)]
    fn into_status_details(self: Box<Self>) -> Vec<prost_types::Any> {
        let mut details = Vec::new();

        if let Some(error_info) = &self.error_info {
            details.push(
                prost_types::Any::from_msg(error_info).expect("Message encoding cannot fail"),
            );
        }

        if let Some(bad_request) = &self.bad_request {
            details.push(
                prost_types::Any::from_msg(bad_request).expect("Message encoding cannot fail"),
            );
        }

        if let Some(retry_info) = &self.retry_info {
            details.push(
                prost_types::Any::from_msg(retry_info).expect("Message encoding cannot fail"),
            );
        }
        details
    }
}

#[derive(Debug, Clone)]
pub struct ObjectNotFoundError {
    object_id: ObjectID,
    version: Option<u64>,
}

impl ObjectNotFoundError {
    pub fn new(object_id: ObjectID) -> Self {
        Self {
            object_id,
            version: None,
        }
    }

    pub fn new_with_version(object_id: ObjectID, version: u64) -> Self {
        Self {
            object_id,
            version: Some(version),
        }
    }
}

impl std::fmt::Display for ObjectNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.version {
            Some(version) => {
                write!(
                    f,
                    "Object {} at version {} not found",
                    self.object_id, version
                )
            }
            None => write!(f, "Object {} not found", self.object_id),
        }
    }
}

impl std::error::Error for ObjectNotFoundError {}

impl From<ObjectNotFoundError> for RpcError {
    fn from(value: ObjectNotFoundError) -> Self {
        Self::not_found().with_context(value)
    }
}

#[derive(Debug, Clone)]
pub struct TransactionNotFoundError(pub TransactionDigest);

impl std::fmt::Display for TransactionNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Transaction {} not found", self.0)
    }
}

impl std::error::Error for TransactionNotFoundError {}

impl From<TransactionNotFoundError> for RpcError {
    fn from(value: TransactionNotFoundError) -> Self {
        Self::not_found().with_context(value)
    }
}

/// Returned when an index table has not yet finished its background backfill.
/// Clients should retry after the suggested delay.
#[derive(Debug, Clone)]
pub struct IndexBackfillInProgressError {
    pub index_name: &'static str,
}

impl std::fmt::Display for IndexBackfillInProgressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} index backfill in progress, try again later",
            self.index_name
        )
    }
}

impl std::error::Error for IndexBackfillInProgressError {}

impl From<IndexBackfillInProgressError> for RpcError {
    fn from(e: IndexBackfillInProgressError) -> Self {
        let details = ErrorDetails::new().with_retry_info(RetryInfo {
            retry_delay: Some(prost_types::Duration {
                seconds: 30,
                nanos: 0,
            }),
        });
        RpcError {
            code: Code::Unavailable,
            message: Some(e.to_string()),
            details: Some(Box::new(details)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MissingIndexesError;

impl std::fmt::Display for MissingIndexesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "indexes are not available on this node")
    }
}

impl std::error::Error for MissingIndexesError {}

impl From<MissingIndexesError> for RpcError {
    fn from(value: MissingIndexesError) -> Self {
        Self::new(Code::FailedPrecondition, value)
    }
}

impl From<tonic::Status> for RpcError {
    fn from(status: tonic::Status) -> Self {
        Self::new(status.code(), status.message().to_string())
    }
}

impl From<iota_types::quorum_driver_types::QuorumDriverError> for RpcError {
    fn from(error: iota_types::quorum_driver_types::QuorumDriverError) -> Self {
        use iota_types::{error::IotaError, quorum_driver_types::QuorumDriverError::*};
        use itertools::Itertools;

        match error {
            InvalidUserSignature(err) => {
                let message = {
                    let err = match err {
                        IotaError::UserInput { error } => error.to_string(),
                        _ => err.to_string(),
                    };
                    format!("Invalid user signature: {err}")
                };
                RpcError::new(Code::InvalidArgument, message)
            }
            QuorumDriverInternal(err) => RpcError::new(Code::Internal, err.to_string()),
            ObjectsDoubleUsed { conflicting_txes } => {
                let new_map = conflicting_txes
                    .into_iter()
                    .map(|(digest, (pairs, _))| {
                        (
                            digest,
                            pairs.into_iter().map(|(_, obj_ref)| obj_ref).collect(),
                        )
                    })
                    .collect::<std::collections::BTreeMap<_, Vec<_>>>();

                let message = format!(
                    "Failed to sign transaction by a quorum of validators because of \
                     locked objects. Conflicting Transactions:\n{new_map:#?}",
                );
                RpcError::new(Code::Aborted, message)
            }
            TimeoutBeforeFinality | FailedWithTransientErrorAfterMaximumAttempts { .. } => {
                RpcError::new(
                    Code::Unavailable,
                    "timed-out before finality could be reached",
                )
            }
            NonRecoverableTransactionError { errors } => {
                let new_errors: Vec<String> = errors
                    .into_iter()
                    .sorted_by(|(_, a, _), (_, b, _)| b.cmp(a))
                    .filter_map(|(err, _, _)| match &err {
                        IotaError::UserInput { error } => Some(error.to_string()),
                        _ => {
                            if err.is_retryable().0 {
                                None
                            } else {
                                Some(err.to_string())
                            }
                        }
                    })
                    .collect();

                assert!(
                    !new_errors.is_empty(),
                    "NonRecoverableTransactionError should have at least one non-retryable error"
                );

                let error_list = new_errors.join(", ");
                let error_msg = format!(
                    "Transaction execution failed due to issues with transaction inputs, \
                     please review the errors and try again: {error_list}."
                );
                RpcError::new(Code::InvalidArgument, error_msg)
            }
            TxAlreadyFinalizedWithDifferentUserSignatures => RpcError::new(
                Code::Aborted,
                "The transaction is already finalized but with different user signatures",
            ),
            SystemOverload { .. } => RpcError::new(Code::Unavailable, "system is overloaded"),
            SystemOverloadRetryAfter {
                retry_after_secs, ..
            } => {
                let details = ErrorDetails::new().with_retry_info(RetryInfo {
                    retry_delay: Some(prost_types::Duration {
                        seconds: retry_after_secs as i64,
                        nanos: 0,
                    }),
                });
                RpcError {
                    code: Code::Unavailable,
                    message: Some("system is overloaded".to_string()),
                    details: Some(Box::new(details)),
                }
            }
        }
    }
}
