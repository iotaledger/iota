// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use strum::AsRefStr;
use thiserror::Error;

use crate::{
    base_types::{AuthorityName, EpochId, ObjectRef, TransactionDigest},
    committee::{QUORUM_THRESHOLD, StakeUnit, TOTAL_VOTING_POWER},
    crypto::{AuthorityStrongQuorumSignInfo, ConciseAuthorityPublicKeyBytes},
    effects::{
        CertifiedTransactionEffects, TransactionEffects, TransactionEvents,
        VerifiedCertifiedTransactionEffects,
    },
    error::IotaError,
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
    transaction::Transaction,
};

pub type QuorumDriverResult = Result<QuorumDriverResponse, QuorumDriverError>;

pub type QuorumDriverEffectsQueueResult =
    Result<(Transaction, QuorumDriverResponse), (TransactionDigest, QuorumDriverError)>;

pub const NON_RECOVERABLE_ERROR_MSG: &str =
    "Transaction has non recoverable errors from at least 1/3 of validators";

/// Client facing errors regarding transaction submission via Quorum Driver.
/// Every invariant needs detailed documents to instruct client handling.
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, Error, Hash, AsRefStr)]
pub enum QuorumDriverError {
    #[error("QuorumDriver internal error: {0}.")]
    QuorumDriverInternal(IotaError),
    #[error("Invalid user signature: {0}.")]
    InvalidUserSignature(IotaError),
    #[error(
        "Failed to sign transaction by a quorum of validators because of locked objects: {conflicting_txes:?}"
    )]
    ObjectsDoubleUsed {
        conflicting_txes: BTreeMap<TransactionDigest, (Vec<(AuthorityName, ObjectRef)>, StakeUnit)>,
    },
    #[error("Transaction timed out before reaching finality")]
    TimeoutBeforeFinality,
    #[error(
        "Transaction failed to reach finality with transient error after {total_attempts} attempts."
    )]
    FailedWithTransientErrorAfterMaximumAttempts { total_attempts: u32 },
    #[error("{NON_RECOVERABLE_ERROR_MSG}: {errors:?}.")]
    NonRecoverableTransactionError { errors: GroupedErrors },
    #[error(
        "Transaction is not processed because {overloaded_stake} of validators by stake are overloaded with certificates pending execution."
    )]
    SystemOverload {
        overloaded_stake: StakeUnit,
        errors: GroupedErrors,
    },
    #[error("Transaction is already finalized but with different user signatures")]
    TxAlreadyFinalizedWithDifferentUserSignatures,
    #[error(
        "Transaction is not processed because {overload_stake} of validators are overloaded and asked client to retry after {retry_after_secs}."
    )]
    SystemOverloadRetryAfter {
        overload_stake: StakeUnit,
        errors: GroupedErrors,
        retry_after_secs: u64,
    },
}

impl QuorumDriverError {
    pub fn to_error_message(&self) -> String {
        match self {
            QuorumDriverError::InvalidUserSignature(err) => {
                format!("Invalid user signature: {err}")
            }
            QuorumDriverError::TxAlreadyFinalizedWithDifferentUserSignatures => {
                "The transaction is already finalized but with different user signatures"
                    .to_string()
            }
            QuorumDriverError::TimeoutBeforeFinality
            | QuorumDriverError::FailedWithTransientErrorAfterMaximumAttempts { .. }
            | QuorumDriverError::SystemOverload { .. }
            | QuorumDriverError::SystemOverloadRetryAfter { .. } => self.to_string(),
            QuorumDriverError::ObjectsDoubleUsed { conflicting_txes } => {
                let weights: Vec<u64> =
                    conflicting_txes.values().map(|(_, stake)| *stake).collect();
                let remaining: u64 = TOTAL_VOTING_POWER - weights.iter().sum::<u64>();

                // better version of above
                let reason = if weights.iter().all(|w| remaining + w < QUORUM_THRESHOLD) {
                    "equivocated until the next epoch"
                } else {
                    "reserved for another transaction"
                };

                format!(
                    "Failed to sign transaction by a quorum of validators because one or more of its objects is {}. Other transactions locking these objects:\n{}",
                    reason,
                    conflicting_txes
                        .iter()
                        .sorted_by(|(_, (_, a)), (_, (_, b))| b.cmp(a))
                        .map(|(digest, (_, stake))| format!(
                            "- {} (stake {}.{})",
                            digest,
                            stake / 100,
                            stake % 100,
                        ))
                        .join("\n"),
                )
            }
            QuorumDriverError::NonRecoverableTransactionError { errors } => {
                let new_errors: Vec<String> = errors
                    .iter()
                    // sort by total stake, descending, so users see the most prominent one
                    // first
                    .sorted_by(|(_, a, _), (_, b, _)| b.cmp(a))
                    .filter_map(|(err, _, _)| {
                        match &err {
                            // Special handling of UserInputError:
                            // ObjectNotFound and DependentPackageNotFound are considered
                            // retryable errors but they have different treatment
                            // in AuthorityAggregator.
                            // The optimal fix would be to examine if the total stake
                            // of ObjectNotFound/DependentPackageNotFound exceeds the
                            // quorum threshold, but it takes a Committee here.
                            // So, we take an easier route and consider them non-retryable
                            // at all. Combining this with the sorting above, clients will
                            // see the dominant error first.
                            IotaError::UserInput { error } => Some(error.to_string()),
                            _ => {
                                if err.is_retryable().0 {
                                    None
                                } else {
                                    Some(err.to_string())
                                }
                            }
                        }
                    })
                    .collect();

                assert!(
                    !new_errors.is_empty(),
                    "NonRecoverableTransactionError should have at least one non-retryable error"
                );

                let mut error_list = vec![];
                for err in new_errors.iter() {
                    error_list.push(format!("- {err}"));
                }

                format!(
                    "Transaction execution failed due to issues with transaction inputs, please review the errors and try again:\n{}",
                    error_list.join("\n")
                )
            }
            QuorumDriverError::QuorumDriverInternal { .. } => {
                "Internal error occurred while executing transaction.".to_string()
            }
        }
    }
}

pub type GroupedErrors = Vec<(IotaError, StakeUnit, Vec<ConciseAuthorityPublicKeyBytes>)>;

#[derive(Serialize, Deserialize, Clone, Debug, schemars::JsonSchema)]
pub enum ExecuteTransactionRequestType {
    WaitForEffectsCert,
    WaitForLocalExecution,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum EffectsFinalityInfo {
    Certified(AuthorityStrongQuorumSignInfo),
    Checkpointed(EpochId, CheckpointSequenceNumber),
}

/// When requested to execute a transaction with WaitForLocalExecution,
/// TransactionOrchestrator attempts to execute this transaction locally
/// after it is finalized. This value represents whether the transaction
/// is confirmed to be executed on this node before the response returns.
pub type IsTransactionExecutedLocally = bool;

#[derive(Debug, Clone)]
pub struct QuorumDriverResponse {
    pub effects_cert: VerifiedCertifiedTransactionEffects,
    // pub events: TransactionEvents,
    pub events: Option<TransactionEvents>,
    // Input objects will only be populated in the happy path
    pub input_objects: Option<Vec<Object>>,
    // Output objects will only be populated in the happy path
    pub output_objects: Option<Vec<Object>>,
    pub auxiliary_data: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecuteTransactionRequestV1 {
    pub transaction: Transaction,

    pub include_events: bool,
    pub include_input_objects: bool,
    pub include_output_objects: bool,
    pub include_auxiliary_data: bool,
}

impl ExecuteTransactionRequestV1 {
    pub fn new<T: Into<Transaction>>(transaction: T) -> Self {
        Self {
            transaction: transaction.into(),
            include_events: true,
            include_input_objects: false,
            include_output_objects: false,
            include_auxiliary_data: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecuteTransactionResponseV1 {
    pub effects: FinalizedEffects,

    pub events: Option<TransactionEvents>,
    // Input objects will only be populated in the happy path
    pub input_objects: Option<Vec<Object>>,
    // Output objects will only be populated in the happy path
    pub output_objects: Option<Vec<Object>>,
    pub auxiliary_data: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FinalizedEffects {
    pub effects: TransactionEffects,
    pub finality_info: EffectsFinalityInfo,
}

impl FinalizedEffects {
    pub fn new_from_effects_cert(effects_cert: CertifiedTransactionEffects) -> Self {
        let (data, sig) = effects_cert.into_data_and_sig();
        Self {
            effects: data,
            finality_info: EffectsFinalityInfo::Certified(sig),
        }
    }

    pub fn epoch(&self) -> EpochId {
        match &self.finality_info {
            EffectsFinalityInfo::Certified(cert) => cert.epoch,
            EffectsFinalityInfo::Checkpointed(epoch, _) => *epoch,
        }
    }
}
