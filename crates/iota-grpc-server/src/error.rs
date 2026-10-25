// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::google::rpc::{BadRequest, ErrorInfo, RetryInfo};
use iota_sdk_types::{ObjectId, TransactionDigest};
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
            Some(existing) => format!("{context}: {existing}"),
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
        let mut status = iota_grpc_types::google::rpc::Status::default();
        status.code = self.code.into();
        status.message = self.message.unwrap_or_default();
        status.details = self
            .details
            .map(ErrorDetails::into_status_details)
            .unwrap_or_default();
        status
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

impl From<iota_sdk_types::BcsError> for RpcError {
    fn from(value: iota_sdk_types::BcsError) -> Self {
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

    pub fn with_error_info(mut self, error_info: ErrorInfo) -> Self {
        self.error_info = Some(error_info);
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
    object_id: ObjectId,
    version: Option<u64>,
}

impl ObjectNotFoundError {
    pub fn new(object_id: ObjectId) -> Self {
        Self {
            object_id,
            version: None,
        }
    }

    pub fn new_with_version(object_id: ObjectId, version: u64) -> Self {
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

impl From<iota_types::storage::error::Error> for RpcError {
    fn from(value: iota_types::storage::error::Error) -> Self {
        Self::internal().with_context(value)
    }
}

impl From<tonic::Status> for RpcError {
    fn from(status: tonic::Status) -> Self {
        Self::new(status.code(), status.message().to_string())
    }
}

impl From<iota_types::quorum_driver_types::QuorumDriverError> for RpcError {
    fn from(error: iota_types::quorum_driver_types::QuorumDriverError) -> Self {
        use iota_types::{
            error::IotaError,
            quorum_driver_types::{NON_RECOVERABLE_ERROR_MSG, QuorumDriverError::*},
        };
        use itertools::Itertools;

        // gRPC mirror of the JSON-RPC `data.reason` discriminant.
        let reason = error.reason().to_uppercase();

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
            InvalidTransaction(err) | RejectedByValidators(err) => {
                let message = {
                    let err = match err {
                        IotaError::UserInput { error } => error.to_string(),
                        _ => err.to_string(),
                    };
                    format!("Invalid transaction: {err}")
                };
                let mut error_info = ErrorInfo::default();
                error_info.reason = reason;
                error_info.domain = "iota.grpc".to_string();
                let details = ErrorDetails::new().with_error_info(error_info);
                RpcError {
                    code: Code::InvalidArgument,
                    message: Some(message),
                    details: Some(Box::new(details)),
                }
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
                    .iter()
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

                // The aggregator builds this variant only from errors with
                // non-retryable stake, but rendering an error must never
                // panic: fall back to the unfiltered list. Every error in it
                // is then retryable, so the code is the one of a transient
                // failure.
                if new_errors.is_empty() {
                    let error_list = errors.iter().map(|(err, _, _)| err.to_string()).join(", ");
                    return RpcError::new(
                        Code::Unavailable,
                        format!("{NON_RECOVERABLE_ERROR_MSG}: {error_list}."),
                    );
                }

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
                let mut retry_info = RetryInfo::default();
                retry_info.retry_delay = Some(prost_types::Duration {
                    seconds: retry_after_secs as i64,
                    nanos: 0,
                });
                let details = ErrorDetails::new().with_retry_info(retry_info);
                RpcError {
                    code: Code::Unavailable,
                    message: Some("system is overloaded".to_string()),
                    details: Some(Box::new(details)),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Missing indexes are a client-visible contract: `FailedPrecondition`,
    /// both directly and through an anyhow chain.
    #[test]
    fn missing_indexes_render_as_failed_precondition() {
        let expected: i32 = Code::FailedPrecondition.into();

        let direct = RpcError::from(MissingIndexesError);
        assert_eq!(direct.into_status_proto().code, expected);

        let chained = anyhow::Error::from(MissingIndexesError).context("listing owned objects");
        let via_chain = RpcError::from(chained);
        assert_eq!(via_chain.into_status_proto().code, expected);
    }

    fn quorum_driver_status(
        error: iota_types::quorum_driver_types::QuorumDriverError,
    ) -> iota_grpc_types::google::rpc::Status {
        RpcError::from(error).into_status_proto()
    }

    fn error_info_reason(status: &iota_grpc_types::google::rpc::Status) -> String {
        assert_eq!(status.details.len(), 1);
        status.details[0]
            .to_msg::<ErrorInfo>()
            .expect("detail should be an ErrorInfo")
            .reason
    }

    #[test]
    fn quorum_driver_errors_without_details() {
        use iota_types::{
            error::{IotaError, UserInputError},
            quorum_driver_types::QuorumDriverError::*,
        };

        let cases = [
            (
                InvalidUserSignature(IotaError::UserInput {
                    error: UserInputError::EmptyCommandInput,
                }),
                Code::InvalidArgument,
                format!(
                    "Invalid user signature: {}",
                    UserInputError::EmptyCommandInput
                ),
            ),
            (
                QuorumDriverInternal(IotaError::Unknown("boom".to_string())),
                Code::Internal,
                IotaError::Unknown("boom".to_string()).to_string(),
            ),
            (
                TimeoutBeforeFinality,
                Code::Unavailable,
                "timed-out before finality could be reached".to_string(),
            ),
            (
                FailedWithTransientErrorAfterMaximumAttempts { total_attempts: 3 },
                Code::Unavailable,
                "timed-out before finality could be reached".to_string(),
            ),
            (
                SystemOverload {
                    overloaded_stake: 1,
                    errors: vec![],
                },
                Code::Unavailable,
                "system is overloaded".to_string(),
            ),
            (
                TxAlreadyFinalizedWithDifferentUserSignatures,
                Code::Aborted,
                "The transaction is already finalized but with different user signatures"
                    .to_string(),
            ),
        ];

        for (error, code, message) in cases {
            let status = quorum_driver_status(error.clone());
            assert_eq!(status.code, i32::from(code), "{error:?}");
            assert_eq!(status.message, message, "{error:?}");
            assert!(status.details.is_empty(), "{error:?}");
        }
    }

    #[test]
    fn invalid_and_rejected_transactions_carry_the_reason() {
        use iota_types::{
            error::{IotaError, UserInputError},
            quorum_driver_types::QuorumDriverError::*,
        };

        let user_input = IotaError::UserInput {
            error: UserInputError::EmptyCommandInput,
        };
        let expected_message =
            format!("Invalid transaction: {}", UserInputError::EmptyCommandInput);

        let status = quorum_driver_status(InvalidTransaction(user_input.clone()));
        assert_eq!(status.code, i32::from(Code::InvalidArgument));
        assert_eq!(status.message, expected_message);
        assert_eq!(error_info_reason(&status), "INVALID_TRANSACTION");

        let status = quorum_driver_status(RejectedByValidators(user_input));
        assert_eq!(status.code, i32::from(Code::InvalidArgument));
        assert_eq!(status.message, expected_message);
        assert_eq!(error_info_reason(&status), "REJECTED_BY_VALIDATORS");

        let other = IotaError::Unknown("rejected during submission".to_string());
        let status = quorum_driver_status(RejectedByValidators(other.clone()));
        assert_eq!(status.message, format!("Invalid transaction: {other}"));
    }

    #[test]
    fn objects_double_used_is_aborted() {
        use iota_sdk_types::{ObjectDigest, Version};
        use iota_types::{
            base_types::AuthorityName, crypto::AuthorityPublicKeyBytes,
            quorum_driver_types::QuorumDriverError::ObjectsDoubleUsed,
        };

        let conflicting_txes = std::collections::BTreeMap::from([(
            TransactionDigest::from([1; 32]),
            (
                vec![(
                    AuthorityName::from(AuthorityPublicKeyBytes::default()),
                    iota_sdk_types::ObjectReference::new(
                        ObjectId::ZERO,
                        Version::from_u64(7),
                        ObjectDigest::new([0; 32]),
                    ),
                )],
                8000,
            ),
        )]);

        let status = quorum_driver_status(ObjectsDoubleUsed { conflicting_txes });
        assert_eq!(status.code, i32::from(Code::Aborted));
        assert!(
            status
                .message
                .starts_with("Failed to sign transaction by a quorum of validators because of locked objects. Conflicting Transactions:"),
            "{}",
            status.message
        );
        assert!(status.details.is_empty());
    }

    #[test]
    fn overload_retry_after_carries_the_delay() {
        use iota_types::quorum_driver_types::QuorumDriverError::SystemOverloadRetryAfter;

        let status = quorum_driver_status(SystemOverloadRetryAfter {
            overload_stake: 1,
            errors: vec![],
            retry_after_secs: 42,
        });
        assert_eq!(status.code, i32::from(Code::Unavailable));
        assert_eq!(status.message, "system is overloaded");
        assert_eq!(status.details.len(), 1);
        let retry_info = status.details[0]
            .to_msg::<RetryInfo>()
            .expect("detail should be a RetryInfo");
        assert_eq!(retry_info.retry_delay.unwrap().seconds, 42);
    }

    #[test]
    fn non_recoverable_transaction_error_lists_non_retryable_errors() {
        use iota_types::{
            error::{IotaError, UserInputError},
            quorum_driver_types::{NON_RECOVERABLE_ERROR_MSG, QuorumDriverError::*},
        };

        let gas_error = UserInputError::GasBalanceTooLow {
            gas_balance: 10,
            needed_gas_amount: 100,
        };
        let retryable = IotaError::Rpc("Hello".to_string(), "Testing".to_string());

        let status = quorum_driver_status(NonRecoverableTransactionError {
            errors: vec![
                (
                    IotaError::UserInput {
                        error: gas_error.clone(),
                    },
                    0,
                    vec![],
                ),
                (retryable.clone(), 0, vec![]),
            ],
        });
        assert_eq!(status.code, i32::from(Code::InvalidArgument));
        assert_eq!(
            status.message,
            format!(
                "Transaction execution failed due to issues with transaction inputs, \
                 please review the errors and try again: {gas_error}."
            )
        );

        let status = quorum_driver_status(NonRecoverableTransactionError {
            errors: vec![(retryable.clone(), 0, vec![])],
        });
        assert_eq!(status.code, i32::from(Code::Unavailable));
        assert_eq!(
            status.message,
            format!("{NON_RECOVERABLE_ERROR_MSG}: {retryable}.")
        );
    }
}
