// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use fastcrypto::error::FastCryptoError;
use hyper::header::InvalidHeaderValue;
use iota_json_rpc_api::{
    TRANSACTION_EXECUTION_CLIENT_ERROR_CODE, TRANSIENT_ERROR_CODE, error_object_from_rpc,
};
use iota_names::error::IotaNamesError;
use iota_types::{
    error::{IotaError, IotaObjectResponseError, UserInputError},
    quorum_driver_types::QuorumDriverError,
};
use jsonrpsee::{
    core::{ClientError as RpcError, RegisterMethodError},
    types::{
        ErrorObject, ErrorObjectOwned,
        error::{CALL_EXECUTION_FAILED_CODE, ErrorCode, INTERNAL_ERROR_CODE},
    },
};
use thiserror::Error;
use tokio::task::JoinError;

use crate::authority_state::StateReadError;

pub type RpcInterimResult<T = ()> = Result<T, Error>;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Iota(IotaError),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),

    #[error("Deserialization error: {0}")]
    Bcs(#[from] bcs::Error),
    #[error("Unexpected error: {0}")]
    Unexpected(String),

    #[error(transparent)]
    RPCServer(#[from] RpcError),
    #[error(transparent)]
    RPCRegisterMethod(#[from] RegisterMethodError),

    #[error(transparent)]
    InvalidHeaderValue(#[from] InvalidHeaderValue),

    #[error(transparent)]
    UserInput(#[from] UserInputError),

    #[error(transparent)]
    Encoding(#[from] eyre::Report),

    #[error(transparent)]
    TokioJoin(#[from] JoinError),

    #[error(transparent)]
    QuorumDriver(#[from] QuorumDriverError),

    #[error(transparent)]
    FastCrypto(#[from] FastCryptoError),

    #[error(transparent)]
    IotaObjectResponse(#[from] IotaObjectResponseError),

    #[error(transparent)]
    IotaRpcInput(#[from] IotaRpcInputError),

    // TODO(wlmyng): convert StateReadError::Internal message to generic internal error message.
    #[error(transparent)]
    StateRead(#[from] StateReadError),

    #[error("Unsupported Feature: {0}")]
    UnsupportedFeature(String),

    #[error(transparent)]
    IotaNames(#[from] IotaNamesError),
}

impl From<IotaError> for Error {
    fn from(e: IotaError) -> Self {
        match e {
            IotaError::UserInput { error } => Self::UserInput(error),
            IotaError::IotaObjectResponse { error } => Self::IotaObjectResponse(error),
            IotaError::UnsupportedFeature { error } => Self::UnsupportedFeature(error),
            IotaError::IndexStoreNotAvailable => Self::UnsupportedFeature(
                "Required indexes are not available on this node".to_string(),
            ),
            other => Self::Iota(other),
        }
    }
}

impl From<Error> for RpcError {
    /// `InvalidParams`/`INVALID_PARAMS_CODE` for client errors.
    fn from(e: Error) -> RpcError {
        match e {
            Error::UserInput(_) | Error::UnsupportedFeature(_) => RpcError::Call(
                ErrorObject::owned::<()>(ErrorCode::InvalidRequest.code(), e.to_string(), None),
            ),
            Error::IotaObjectResponse(err) => match err {
                IotaObjectResponseError::NotExists { .. }
                | IotaObjectResponseError::DynamicFieldNotFound { .. }
                | IotaObjectResponseError::Deleted { .. }
                | IotaObjectResponseError::Display { .. } => {
                    RpcError::Call(ErrorObject::owned::<()>(
                        ErrorCode::InvalidParams.code(),
                        err.to_string(),
                        None,
                    ))
                }
                _ => RpcError::Call(ErrorObject::owned::<()>(
                    CALL_EXECUTION_FAILED_CODE,
                    err.to_string(),
                    None,
                )),
            },
            Error::IotaRpcInput(err) => RpcError::Call(ErrorObject::owned::<()>(
                ErrorCode::InvalidParams.code(),
                err.to_string(),
                None,
            )),
            Error::Iota(iota_error) => match iota_error {
                IotaError::TransactionNotFound { .. }
                | IotaError::TransactionsNotFound { .. }
                | IotaError::TransactionEventsNotFound { .. } => {
                    RpcError::Call(ErrorObject::owned::<()>(
                        ErrorCode::InvalidParams.code(),
                        iota_error.to_string(),
                        None,
                    ))
                }
                _ => RpcError::Call(ErrorObject::owned::<()>(
                    CALL_EXECUTION_FAILED_CODE,
                    iota_error.to_string(),
                    None,
                )),
            },
            Error::StateRead(err) => match err {
                StateReadError::Client(_) => RpcError::Call(ErrorObject::owned::<()>(
                    ErrorCode::InvalidParams.code(),
                    err.to_string(),
                    None,
                )),
                _ => {
                    let error_object = ErrorObject::owned::<()>(
                        jsonrpsee::types::error::INTERNAL_ERROR_CODE,
                        err.to_string(),
                        None,
                    );
                    RpcError::Call(error_object)
                }
            },
            Error::QuorumDriver(err) => {
                let error_msg = err.to_error_message();

                match err {
                    QuorumDriverError::InvalidUserSignature { .. }
                    | QuorumDriverError::TxAlreadyFinalizedWithDifferentUserSignatures
                    | QuorumDriverError::NonRecoverableTransactionError { .. } => {
                        let error_object = ErrorObject::owned::<()>(
                            TRANSACTION_EXECUTION_CLIENT_ERROR_CODE,
                            error_msg,
                            None,
                        );
                        RpcError::Call(error_object)
                    }
                    QuorumDriverError::ObjectsDoubleUsed { conflicting_txes } => {
                        let new_map = conflicting_txes
                            .into_iter()
                            .map(|(digest, (pairs, _))| {
                                (
                                    digest,
                                    pairs.into_iter().map(|(_, obj_ref)| obj_ref).collect(),
                                )
                            })
                            .collect::<BTreeMap<_, Vec<_>>>();

                        let error_object = ErrorObject::owned(
                            TRANSACTION_EXECUTION_CLIENT_ERROR_CODE,
                            error_msg,
                            Some(new_map),
                        );
                        RpcError::Call(error_object)
                    }
                    QuorumDriverError::TimeoutBeforeFinality
                    | QuorumDriverError::FailedWithTransientErrorAfterMaximumAttempts { .. }
                    | QuorumDriverError::SystemOverload { .. }
                    | QuorumDriverError::SystemOverloadRetryAfter { .. } => {
                        let error_object =
                            ErrorObject::owned::<()>(TRANSIENT_ERROR_CODE, error_msg, None);
                        RpcError::Call(error_object)
                    }
                    QuorumDriverError::QuorumDriverInternal(_) => {
                        let error_object =
                            ErrorObject::owned::<()>(INTERNAL_ERROR_CODE, error_msg, None);
                        RpcError::Call(error_object)
                    }
                }
            }
            _ => RpcError::Call(ErrorObject::owned::<()>(
                CALL_EXECUTION_FAILED_CODE,
                e.to_string(),
                None,
            )),
        }
    }
}

impl From<Error> for ErrorObjectOwned {
    fn from(value: Error) -> Self {
        error_object_from_rpc(value.into())
    }
}

#[derive(Debug, Error)]
pub enum IotaRpcInputError {
    #[error("Input contains duplicates")]
    ContainsDuplicates,

    #[error("Input exceeds limit of {0}")]
    SizeLimitExceeded(String),

    #[error("{0}")]
    GenericNotFound(String),

    #[error("{0}")]
    GenericInvalid(String),

    #[error(
        "request_type` must set to `None` or `WaitForLocalExecution` if effects is required in the response"
    )]
    InvalidExecuteTransactionRequestType,

    #[error("Unsupported protocol version requested. Min supported: {0}, max supported: {1}")]
    ProtocolVersionUnsupported(u64, u64),

    #[error("{0}")]
    CannotParseIotaStructTag(String),

    #[error(transparent)]
    Base64(#[from] eyre::Report),

    #[error("Deserialization error: {0}")]
    Bcs(#[from] bcs::Error),

    #[error(transparent)]
    FastCrypto(#[from] FastCryptoError),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    #[error(transparent)]
    UserInput(#[from] UserInputError),
}

impl From<IotaRpcInputError> for RpcError {
    fn from(e: IotaRpcInputError) -> Self {
        RpcError::Call(ErrorObject::owned::<()>(
            ErrorCode::InvalidParams.code(),
            e.to_string(),
            None,
        ))
    }
}

impl From<IotaRpcInputError> for ErrorObjectOwned {
    fn from(value: IotaRpcInputError) -> Self {
        error_object_from_rpc(value.into())
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use iota_types::{
        base_types::{AuthorityName, ObjectID, ObjectRef, SequenceNumber},
        committee::StakeUnit,
        crypto::{AuthorityPublicKey, AuthorityPublicKeyBytes},
        digests::{ObjectDigest, TransactionDigest},
    };

    use super::*;

    fn test_object_ref() -> ObjectRef {
        (
            ObjectID::ZERO,
            SequenceNumber::from_u64(0),
            ObjectDigest::new([0; 32]),
        )
    }

    mod match_quorum_driver_error_tests {
        use super::*;

        #[test]
        fn test_invalid_user_signature() {
            let quorum_driver_error =
                QuorumDriverError::InvalidUserSignature(IotaError::InvalidSignature {
                    error: "Test inner invalid signature".to_string(),
                });

            let rpc_error: RpcError = Error::QuorumDriver(quorum_driver_error).into();

            let error_object = error_object_from_rpc(rpc_error);
            let expected_code = expect!["-32002"];
            expected_code.assert_eq(&error_object.code().to_string());
            let expected_message = expect![
                "Invalid user signature: Signature is not valid: Test inner invalid signature"
            ];
            expected_message.assert_eq(error_object.message());
        }

        #[test]
        fn test_timeout_before_finality() {
            let quorum_driver_error = QuorumDriverError::TimeoutBeforeFinality;

            let rpc_error: RpcError = Error::QuorumDriver(quorum_driver_error).into();

            let error_object = error_object_from_rpc(rpc_error);
            let expected_code = expect!["-32050"];
            expected_code.assert_eq(&error_object.code().to_string());
            let expected_message = expect!["Transaction timed out before reaching finality"];
            expected_message.assert_eq(error_object.message());
        }

        #[test]
        fn test_failed_with_transient_error_after_maximum_attempts() {
            let quorum_driver_error =
                QuorumDriverError::FailedWithTransientErrorAfterMaximumAttempts {
                    total_attempts: 10,
                };

            let rpc_error: RpcError = Error::QuorumDriver(quorum_driver_error).into();

            let error_object = error_object_from_rpc(rpc_error);
            let expected_code = expect!["-32050"];
            expected_code.assert_eq(&error_object.code().to_string());
            let expected_message = expect![
                "Transaction failed to reach finality with transient error after 10 attempts."
            ];
            expected_message.assert_eq(error_object.message());
        }

        #[test]
        fn test_objects_double_used() {
            use iota_types::crypto::VerifyingKey;
            let mut conflicting_txes: BTreeMap<
                TransactionDigest,
                (Vec<(AuthorityName, ObjectRef)>, StakeUnit),
            > = BTreeMap::new();
            let tx_digest = TransactionDigest::from([1; 32]);
            let object_ref = test_object_ref();
            // 4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi has enough stake to escape
            // equivocation
            let stake_unit: StakeUnit = 8000;
            let authority_name = AuthorityPublicKeyBytes([0; AuthorityPublicKey::LENGTH]);
            conflicting_txes.insert(tx_digest, (vec![(authority_name, object_ref)], stake_unit));

            // 8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR stake below quorum threshold
            let tx_digest = TransactionDigest::from([2; 32]);
            let stake_unit: StakeUnit = 500;
            let authority_name = AuthorityPublicKeyBytes([1; AuthorityPublicKey::LENGTH]);
            conflicting_txes.insert(tx_digest, (vec![(authority_name, object_ref)], stake_unit));

            let quorum_driver_error = QuorumDriverError::ObjectsDoubleUsed { conflicting_txes };

            let error_object: ErrorObjectOwned = Error::QuorumDriver(quorum_driver_error).into();

            let expected_code = expect!["-32002"];
            expected_code.assert_eq(&error_object.code().to_string());
            println!("error_object.message() {}", error_object.message());
            let expected_message = expect![[r#"
                Failed to sign transaction by a quorum of validators because one or more of its objects is reserved for another transaction. Other transactions locking these objects:
                - 4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi (stake 80.0)
                - 8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR (stake 5.0)"#]];
            expected_message.assert_eq(error_object.message());
            let expected_data = expect![[
                r#"{"4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi":[["0x0000000000000000000000000000000000000000000000000000000000000000",0,"11111111111111111111111111111111"]],"8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR":[["0x0000000000000000000000000000000000000000000000000000000000000000",0,"11111111111111111111111111111111"]]}"#
            ]];
            let actual_data = error_object.data().unwrap().to_string();
            expected_data.assert_eq(&actual_data);
        }

        #[test]
        fn test_objects_double_used_equivocated() {
            use iota_types::crypto::VerifyingKey;
            let mut conflicting_txes: BTreeMap<
                TransactionDigest,
                (Vec<(AuthorityName, ObjectRef)>, StakeUnit),
            > = BTreeMap::new();
            let tx_digest = TransactionDigest::from([1; 32]);
            let object_ref = test_object_ref();

            // 4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi has lower stake at 10
            let stake_unit: StakeUnit = 4000;
            let authority_name = AuthorityPublicKeyBytes([0; AuthorityPublicKey::LENGTH]);
            conflicting_txes.insert(tx_digest, (vec![(authority_name, object_ref)], stake_unit));

            // 8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR is a higher stake and should be
            // first in the list
            let tx_digest = TransactionDigest::from([2; 32]);
            let stake_unit: StakeUnit = 5000;
            let authority_name = AuthorityPublicKeyBytes([1; AuthorityPublicKey::LENGTH]);
            conflicting_txes.insert(tx_digest, (vec![(authority_name, object_ref)], stake_unit));

            let quorum_driver_error = QuorumDriverError::ObjectsDoubleUsed { conflicting_txes };

            let rpc_error: RpcError = Error::QuorumDriver(quorum_driver_error).into();

            let error_object = error_object_from_rpc(rpc_error);
            let expected_code = expect!["-32002"];
            expected_code.assert_eq(&error_object.code().to_string());
            let expected_message = expect![[r#"
                Failed to sign transaction by a quorum of validators because one or more of its objects is equivocated until the next epoch. Other transactions locking these objects:
                - 8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR (stake 50.0)
                - 4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi (stake 40.0)"#]];
            expected_message.assert_eq(error_object.message());
            let expected_data = expect![[
                r#"{"4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi":[["0x0000000000000000000000000000000000000000000000000000000000000000",0,"11111111111111111111111111111111"]],"8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR":[["0x0000000000000000000000000000000000000000000000000000000000000000",0,"11111111111111111111111111111111"]]}"#
            ]];
            let actual_data = error_object.data().unwrap().to_string();
            expected_data.assert_eq(&actual_data);
        }

        #[test]
        fn test_non_recoverable_transaction_error() {
            let quorum_driver_error = QuorumDriverError::NonRecoverableTransactionError {
                errors: vec![
                    (
                        IotaError::UserInput {
                            error: UserInputError::GasBalanceTooLow {
                                gas_balance: 10,
                                needed_gas_amount: 100,
                            },
                        },
                        0,
                        vec![],
                    ),
                    (
                        IotaError::UserInput {
                            error: UserInputError::ObjectVersionUnavailableForConsumption {
                                provided_obj_ref: test_object_ref(),
                                current_version: 10.into(),
                            },
                        },
                        0,
                        vec![],
                    ),
                ],
            };

            let rpc_error: RpcError = Error::QuorumDriver(quorum_driver_error).into();

            let error_object = error_object_from_rpc(rpc_error);
            let expected_code = expect!["-32002"];
            expected_code.assert_eq(&error_object.code().to_string());
            let expected_message = expect![
                "Transaction execution failed due to issues with transaction inputs, please review the errors and try again:\n- Balance of gas object 10 is lower than the needed amount: 100\n- Object ID 0x0000000000000000000000000000000000000000000000000000000000000000 Version 0x0 Digest 11111111111111111111111111111111 is not available for consumption, current version: 0xa"
            ];
            expected_message.assert_eq(error_object.message());
        }

        #[test]
        fn test_non_recoverable_transaction_error_with_transient_errors() {
            let quorum_driver_error = QuorumDriverError::NonRecoverableTransactionError {
                errors: vec![
                    (
                        IotaError::UserInput {
                            error: UserInputError::ObjectNotFound {
                                object_id: test_object_ref().0,
                                version: None,
                            },
                        },
                        0,
                        vec![],
                    ),
                    (
                        IotaError::Rpc("Hello".to_string(), "Testing".to_string()),
                        0,
                        vec![],
                    ),
                ],
            };

            let rpc_error: RpcError = Error::QuorumDriver(quorum_driver_error).into();

            let error_object = error_object_from_rpc(rpc_error);
            let expected_code = expect!["-32002"];
            expected_code.assert_eq(&error_object.code().to_string());
            let expected_message = expect![
                "Transaction execution failed due to issues with transaction inputs, please review the errors and try again:\n- Could not find the referenced object 0x0000000000000000000000000000000000000000000000000000000000000000 at version None"
            ];
            expected_message.assert_eq(error_object.message());
        }

        #[test]
        fn test_quorum_driver_internal_error() {
            let quorum_driver_error =
                QuorumDriverError::QuorumDriverInternal(IotaError::UnexpectedMessage);

            let rpc_error: RpcError = Error::QuorumDriver(quorum_driver_error).into();

            let error_object = error_object_from_rpc(rpc_error);
            let expected_code = expect!["-32603"];
            expected_code.assert_eq(&error_object.code().to_string());
            let expected_message = expect!["Internal error occurred while executing transaction."];
            expected_message.assert_eq(error_object.message());
        }

        #[test]
        fn test_system_overload() {
            let quorum_driver_error = QuorumDriverError::SystemOverload {
                overloaded_stake: 10,
                errors: vec![(IotaError::UnexpectedMessage, 0, vec![])],
            };

            let rpc_error: RpcError = Error::QuorumDriver(quorum_driver_error).into();

            let error_object = error_object_from_rpc(rpc_error);
            let expected_code = expect!["-32050"];
            expected_code.assert_eq(&error_object.code().to_string());
            let expected_message = expect![
                "Transaction is not processed because 10 of validators by stake are overloaded with certificates pending execution."
            ];
            expected_message.assert_eq(error_object.message());
        }
    }
}
