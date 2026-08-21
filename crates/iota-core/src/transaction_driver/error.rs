// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, time::Duration};

use iota_sdk_types::TransactionEffectsDigest;
use iota_types::{
    base_types::{AuthorityName, ConciseableName},
    committee::{EpochId, StakeUnit},
    error::{ErrorCategory, IotaError},
};
use itertools::Itertools as _;
use thiserror::Error;

/// Errors emitted from individual validators during transaction driver
/// operations.
///
/// These errors are associated with the transaction and authority externally,
/// so it is unnecessary to include those information in these messages.
///
/// NOTE: these errors will be aggregated across authorities by status and
/// reported to the caller. So the error messages should not contain authority
/// specific information, such as authority name.
#[derive(Eq, PartialEq, Clone, Debug, Error)]
pub(crate) enum TransactionRequestError {
    #[error("Request timed out submitting transaction")]
    TimedOutSubmittingTransaction,
    #[error("Request timed out getting full effects")]
    TimedOutGettingFullEffectsAtValidator,
    #[error("{0}")]
    ValidatorInternal(String),

    // Rejected by the validator when voting on the transaction.
    #[error("{0}")]
    RejectedAtValidator(IotaError),
    // Transaction status has been dropped from cache at the validator.
    #[error("Transaction status expired")]
    StatusExpired(EpochId),
    // Request to submit transaction or get full effects failed.
    #[error("{0}")]
    Aborted(IotaError),
}

impl TransactionRequestError {
    /// Retry delay requested by an overloaded validator.
    fn retry_after_secs(&self) -> Option<u64> {
        match self {
            TransactionRequestError::RejectedAtValidator(error)
            | TransactionRequestError::Aborted(error) => error
                .is_retryable_overload()
                .then(|| error.retry_after_secs()),
            _ => None,
        }
    }

    pub(crate) fn categorize(&self) -> ErrorCategory {
        match self {
            TransactionRequestError::TimedOutSubmittingTransaction => ErrorCategory::Unavailable,
            TransactionRequestError::TimedOutGettingFullEffectsAtValidator => {
                ErrorCategory::Unavailable
            }
            TransactionRequestError::ValidatorInternal(_) => ErrorCategory::Internal,

            TransactionRequestError::RejectedAtValidator(error) => error.categorize(),
            TransactionRequestError::StatusExpired(_) => ErrorCategory::Aborted,
            TransactionRequestError::Aborted(error) => error.categorize(),
        }
    }

    pub(crate) fn is_submission_retriable(&self) -> bool {
        self.categorize().is_submission_retriable()
    }
}

/// Client facing errors on transaction processing via Transaction Driver.
///
/// NOTE: every error should indicate if it is retriable.
#[derive(Eq, PartialEq, Clone)]
pub enum TransactionDriverError {
    /// TransactionDriver encountered an internal error.
    /// Non-retriable.
    ClientInternal { error: String },
    /// The transaction failed validation from local state.
    /// Non-retriable.
    ValidationFailed { error: String },
    /// Transient failure during transaction processing that prevents the
    /// transaction from finalization. Retriable with new transaction
    /// submission.
    Aborted {
        submission_non_retriable_errors: AggregatedRequestErrors,
        submission_retriable_errors: AggregatedRequestErrors,
        observed_effects_digests: AggregatedEffectsDigests,
    },
    /// Over validity threshold of validators rejected the transaction as
    /// invalid. Non-retriable.
    RejectedByValidators {
        submission_non_retriable_errors: AggregatedRequestErrors,
        submission_retriable_errors: AggregatedRequestErrors,
    },
    /// Transaction execution observed multiple effects digests, and it is no
    /// longer possible to certify any of them.
    /// Non-retriable.
    ForkedExecution {
        observed_effects_digests: AggregatedEffectsDigests,
        submission_non_retriable_errors: AggregatedRequestErrors,
        submission_retriable_errors: AggregatedRequestErrors,
    },
    /// Transaction timed out but we return last retriable error if it exists.
    /// Non-retriable.
    TimeoutWithLastRetriableError {
        last_error: Option<Box<TransactionDriverError>>,
        attempts: u32,
        timeout: Duration,
    },
    /// Transaction was successfully submitted to consensus, but the
    /// subsequent effects fetch from the submitting validator failed. The
    /// tx is expected to be finalized via the local checkpoint executor; the
    /// caller may recover by waiting for checkpoint inclusion and reading
    /// effects from the local cache. Retriable at the client level.
    SubmittedButFetchFailed {
        validator: AuthorityName,
        error: String,
    },
}

impl TransactionDriverError {
    pub(crate) fn is_submission_retriable(&self) -> bool {
        self.categorize().is_submission_retriable()
    }

    /// True when the overloaded stake of an aborted attempt exceeds all
    /// other error stake combined. Single rule shared by the driver's
    /// overload backoff and the client-facing overload mapping. `categorize`
    /// is unsuitable here: it reads only the largest message bucket and must
    /// keep preferring retriable errors for retriability.
    pub(crate) fn is_overload_dominated(&self) -> bool {
        let TransactionDriverError::Aborted {
            submission_non_retriable_errors,
            submission_retriable_errors,
            ..
        } = self
        else {
            return false;
        };
        let mut overloaded = 0;
        let mut other = 0;
        for (_, _, stake, category) in submission_retriable_errors
            .errors
            .iter()
            .chain(submission_non_retriable_errors.errors.iter())
        {
            if *category == ErrorCategory::ValidatorOverloaded {
                overloaded += *stake;
            } else {
                other += *stake;
            }
        }
        overloaded > 0 && overloaded > other
    }

    pub fn categorize(&self) -> ErrorCategory {
        match self {
            TransactionDriverError::ClientInternal { .. } => ErrorCategory::Internal,
            TransactionDriverError::ValidationFailed { .. } => ErrorCategory::InvalidTransaction,
            TransactionDriverError::Aborted {
                submission_retriable_errors,
                submission_non_retriable_errors,
                ..
            } => {
                if let Some((_, _, _, category)) = submission_retriable_errors.errors.first() {
                    *category
                } else if let Some((_, _, _, category)) =
                    submission_non_retriable_errors.errors.first()
                {
                    *category
                } else {
                    ErrorCategory::Aborted
                }
            }
            TransactionDriverError::RejectedByValidators {
                submission_non_retriable_errors,
                submission_retriable_errors,
                ..
            } => {
                if let Some((_, _, _, category)) = submission_non_retriable_errors.errors.first() {
                    *category
                } else if let Some((_, _, _, category)) = submission_retriable_errors.errors.first()
                {
                    *category
                } else {
                    // There should be at least one error.
                    ErrorCategory::Internal
                }
            }
            TransactionDriverError::ForkedExecution { .. } => ErrorCategory::Internal,
            TransactionDriverError::TimeoutWithLastRetriableError { .. } => {
                ErrorCategory::Unavailable
            }
            TransactionDriverError::SubmittedButFetchFailed { .. } => ErrorCategory::Unavailable,
        }
    }

    fn display_aborted(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let TransactionDriverError::Aborted {
            submission_non_retriable_errors,
            submission_retriable_errors,
            observed_effects_digests,
        } = self
        else {
            return Ok(());
        };
        let mut msgs =
            vec!["Transaction processing aborted (retriable with another submission).".to_string()];
        if submission_retriable_errors.total_stake > 0 {
            msgs.push(format!(
                "Retriable errors: [{submission_retriable_errors}]."
            ));
        }
        if submission_non_retriable_errors.total_stake > 0 {
            msgs.push(format!(
                "Non-retriable errors: [{submission_non_retriable_errors}]."
            ));
        }
        if !observed_effects_digests.digests.is_empty() {
            msgs.push(format!(
                "Observed effects digests: [{observed_effects_digests}]."
            ));
        }
        write!(f, "{}", msgs.join(" "))
    }

    fn display_validation_failed(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let TransactionDriverError::ValidationFailed { error } = self else {
            return Ok(());
        };
        write!(f, "Transaction failed validation: {error}")
    }

    fn display_invalid_transaction(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let TransactionDriverError::RejectedByValidators {
            submission_non_retriable_errors,
            submission_retriable_errors,
        } = self
        else {
            return Ok(());
        };
        let mut msgs = vec!["Transaction is rejected as invalid by more than 1/3 of validators by stake (non-retriable).".to_string()];
        if submission_non_retriable_errors.total_stake > 0 {
            msgs.push(format!(
                "Non-retriable errors: [{submission_non_retriable_errors}]."
            ));
        }
        if submission_retriable_errors.total_stake > 0 {
            msgs.push(format!(
                "Retriable errors: [{submission_retriable_errors}]."
            ));
        }
        write!(f, "{}", msgs.join(" "))
    }

    fn display_forked_execution(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let TransactionDriverError::ForkedExecution {
            observed_effects_digests,
            submission_non_retriable_errors,
            submission_retriable_errors,
        } = self
        else {
            return Ok(());
        };
        let mut msgs =
            vec!["Transaction execution observed forked outputs (non-retriable).".to_string()];
        msgs.push(format!(
            "Observed effects digests: [{observed_effects_digests}]."
        ));
        if submission_non_retriable_errors.total_stake > 0 {
            msgs.push(format!(
                "Non-retriable errors: [{submission_non_retriable_errors}]."
            ));
        }
        if submission_retriable_errors.total_stake > 0 {
            msgs.push(format!(
                "Retriable errors: [{submission_retriable_errors}]."
            ));
        }
        write!(f, "{}", msgs.join(" "))
    }
}

impl std::fmt::Display for TransactionDriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionDriverError::ClientInternal { error } => {
                write!(f, "TransactionDriver internal error: {error}")
            }
            TransactionDriverError::Aborted { .. } => self.display_aborted(f),
            TransactionDriverError::ValidationFailed { .. } => self.display_validation_failed(f),
            TransactionDriverError::RejectedByValidators { .. } => {
                self.display_invalid_transaction(f)
            }
            TransactionDriverError::ForkedExecution { .. } => self.display_forked_execution(f),
            TransactionDriverError::TimeoutWithLastRetriableError {
                last_error,
                attempts,
                timeout,
            } => {
                write!(
                    f,
                    "Transaction timed out after {} attempts. Timeout: {:?}. Last error: {}",
                    attempts,
                    timeout,
                    last_error
                        .as_ref()
                        .map(|e| e.to_string())
                        .unwrap_or_default()
                )
            }
            TransactionDriverError::SubmittedButFetchFailed { validator, error } => {
                write!(
                    f,
                    "Transaction submitted to consensus but failed to fetch effects from submitter {:?}: {error}",
                    validator.concise()
                )
            }
        }
    }
}

impl std::fmt::Debug for TransactionDriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl std::error::Error for TransactionDriverError {}

#[derive(Eq, PartialEq, Clone, Debug, Default)]
pub struct AggregatedRequestErrors {
    pub errors: Vec<(String, Vec<AuthorityName>, StakeUnit, ErrorCategory)>,
    // The total stake of all errors.
    pub total_stake: StakeUnit,
    /// Stake per requested retry delay in seconds. Mirrors the aggregator's
    /// `RetryableOverloadInfo`.
    pub stake_requested_retry_after: BTreeMap<u64, StakeUnit>,
}

impl AggregatedRequestErrors {
    /// Upper stake-weighted median of the requested retry delays. A minority
    /// of the hinting stake cannot dictate the result.
    pub fn median_retry_after_secs(&self) -> Option<u64> {
        let total: StakeUnit = self.stake_requested_retry_after.values().sum();
        if total == 0 {
            return None;
        }
        let mut cumulative = 0;
        for (secs, stake) in &self.stake_requested_retry_after {
            cumulative += stake;
            if cumulative * 2 > total {
                return Some(*secs);
            }
        }
        None
    }
}

impl std::fmt::Display for AggregatedRequestErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = self
            .errors
            .iter()
            .map(|(error, names, stake, _category)| {
                format!(
                    "{} {{ {} }} with {} stake",
                    error,
                    names.iter().map(|n| n.concise_owned()).join(", "),
                    stake
                )
            })
            .join("; ");
        write!(f, "{msg}")?;
        Ok(())
    }
}

fn format_transaction_request_error(error: &TransactionRequestError) -> String {
    match error {
        TransactionRequestError::RejectedAtValidator(iota_error) => match iota_error {
            IotaError::UserInput { error: user_error } => user_error.to_string(),
            _ => iota_error.to_string(),
        },
        _ => error.to_string(),
    }
}

pub(crate) fn aggregate_request_errors(
    errors: Vec<(AuthorityName, StakeUnit, TransactionRequestError)>,
) -> AggregatedRequestErrors {
    let mut total_stake = 0;
    let mut aggregated_errors =
        BTreeMap::<String, (Vec<AuthorityName>, StakeUnit, ErrorCategory)>::new();
    let mut stake_requested_retry_after = BTreeMap::new();

    for (name, stake, error) in errors {
        total_stake += stake;
        if let Some(secs) = error.retry_after_secs() {
            *stake_requested_retry_after.entry(secs).or_default() += stake;
        }
        let key = format_transaction_request_error(&error);
        let entry = aggregated_errors
            .entry(key)
            .or_insert_with(|| (vec![], 0, error.categorize()));
        entry.0.push(name);
        entry.1 += stake;
    }

    let mut errors: Vec<_> = aggregated_errors
        .into_iter()
        .map(|(error, (names, stake, category))| (error, names, stake, category))
        .collect();
    errors.sort_by_key(|(_, _, stake, _)| std::cmp::Reverse(*stake));

    AggregatedRequestErrors {
        errors,
        total_stake,
        stake_requested_retry_after,
    }
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct AggregatedEffectsDigests {
    pub digests: Vec<(TransactionEffectsDigest, Vec<AuthorityName>, StakeUnit)>,
}

impl std::fmt::Display for AggregatedEffectsDigests {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = self
            .digests
            .iter()
            .map(|(digest, names, stake)| {
                format!(
                    "{} {{ {} }} with {} stake",
                    digest,
                    names.iter().map(|n| n.concise_owned()).join(", "),
                    stake
                )
            })
            .join("; ");
        write!(f, "{msg}")?;
        Ok(())
    }
}

impl AggregatedEffectsDigests {
    #[cfg(test)]
    pub fn total_stake(&self) -> StakeUnit {
        self.digests.iter().map(|(_, _, stake)| stake).sum()
    }
}

#[cfg(test)]
mod tests {
    use iota_types::crypto::{AuthorityPublicKey, AuthorityPublicKeyBytes};

    use super::*;

    /// Aggregation records stake per requested delay. The median ignores
    /// low-stake outliers.
    #[test]
    fn aggregation_records_stake_per_retry_hint() {
        use fastcrypto::traits::VerifyingKey;

        let name = |i: u8| AuthorityPublicKeyBytes([i; AuthorityPublicKey::LENGTH]);
        let overloaded = |secs| {
            TransactionRequestError::RejectedAtValidator(IotaError::ValidatorOverloadedRetryAfter {
                retry_after_secs: secs,
            })
        };
        let aggregated = aggregate_request_errors(vec![
            (name(0), 4000, overloaded(10)),
            (name(1), 3000, overloaded(10)),
            (name(2), 1000, overloaded(3600)),
            (
                name(3),
                1000,
                TransactionRequestError::TimedOutSubmittingTransaction,
            ),
        ]);
        assert_eq!(
            aggregated.stake_requested_retry_after,
            BTreeMap::from([(10, 7000), (3600, 1000)])
        );
        assert_eq!(aggregated.total_stake, 9000);
        assert_eq!(aggregated.median_retry_after_secs(), Some(10));

        let aggregated = aggregate_request_errors(vec![(
            name(0),
            1000,
            TransactionRequestError::TimedOutSubmittingTransaction,
        )]);
        assert!(aggregated.stake_requested_retry_after.is_empty());
        assert_eq!(aggregated.median_retry_after_secs(), None);
    }

    /// Dominance requires overloaded stake to exceed all other error stake
    /// combined.
    #[test]
    fn overload_dominance_sums_stake_per_category() {
        use fastcrypto::traits::VerifyingKey;

        let name = |i: u8| AuthorityPublicKeyBytes([i; AuthorityPublicKey::LENGTH]);
        let overloaded = |secs| {
            TransactionRequestError::RejectedAtValidator(IotaError::ValidatorOverloadedRetryAfter {
                retry_after_secs: secs,
            })
        };
        let aborted = |retriable: Vec<_>| TransactionDriverError::Aborted {
            submission_non_retriable_errors: AggregatedRequestErrors::default(),
            submission_retriable_errors: aggregate_request_errors(retriable),
            observed_effects_digests: AggregatedEffectsDigests { digests: vec![] },
        };

        // Overload split across delays still outweighs a larger single
        // bucket.
        let dominated = aborted(vec![
            (
                name(0),
                4000,
                TransactionRequestError::TimedOutSubmittingTransaction,
            ),
            (name(1), 2500, overloaded(10)),
            (name(2), 2500, overloaded(30)),
        ]);
        assert!(dominated.is_overload_dominated());

        // An overload plurality that is an aggregate minority does not
        // dominate.
        let not_dominated = aborted(vec![
            (name(0), 3500, overloaded(10)),
            (
                name(1),
                3000,
                TransactionRequestError::TimedOutSubmittingTransaction,
            ),
            (
                name(2),
                2500,
                TransactionRequestError::TimedOutGettingFullEffectsAtValidator,
            ),
        ]);
        assert!(!not_dominated.is_overload_dominated());
    }
}
