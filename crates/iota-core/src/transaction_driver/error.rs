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
    /// Transaction shed due to execution congestion. The client should resubmit
    /// a new transaction with a gas price of at least `suggested_gas_price`.
    /// Non-retriable by the driver itself (the same signed bytes would be
    /// shed again at the same price).
    Congested { suggested_gas_price: u64 },
    /// Transaction shed due to execution congestion while already priced at the
    /// maximum gas price. No resubmission at a higher price is possible, so the
    /// client must wait for congestion to clear. Non-retriable.
    CongestedAtMaxGasPrice { max_gas_price: u64 },
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
            TransactionDriverError::Congested { .. }
            | TransactionDriverError::CongestedAtMaxGasPrice { .. } => {
                ErrorCategory::TransactionCongested
            }
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
            TransactionDriverError::Congested {
                suggested_gas_price,
            } => {
                write!(
                    f,
                    "Transaction shed due to execution congestion (non-retriable). Resubmit a new \
                    transaction with a gas price of at least {suggested_gas_price}."
                )
            }
            TransactionDriverError::CongestedAtMaxGasPrice { max_gas_price } => {
                write!(
                    f,
                    "Transaction shed due to execution congestion (non-retriable) while already at \
                    the maximum gas price of {max_gas_price}. Resubmitting at a higher gas price is \
                    not possible; retry once congestion clears."
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

/// Outcome of checking whether rejections are dominated by execution
/// congestion, and whether the congestion feedback can be trusted yet.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CongestionCheck {
    /// Congestion rejections do not account for a validity threshold of the
    /// rejection stake — the rejections point at something other than
    /// congestion.
    Contradicted,
    /// A single reported (variant, price) is backed by at least a validity
    /// threshold of stake, so the error can be surfaced to the client.
    Corroborated(TransactionDriverError),
    /// Rejections are congestion-dominated, but the reported values disagree:
    /// none has validity-threshold backing. Honest reports are identical, so
    /// disagreement implies a dishonest reporter; honest validators that have
    /// not yet processed the dropping commit (they respond without a
    /// congestion report) can keep the honest value below threshold backing.
    /// The caller should keep collecting responses until either a value is
    /// corroborated or a quorum of stake has responded, and then surface the
    /// aggregated rejections without endorsing any reported price.
    Uncorroborated,
}

/// Checks whether congestion rejections account for a validity threshold of
/// the rejection stake, and if so whether a single reported (variant, price)
/// is backed by that much stake.
///
/// Honest reports are identical (the shed decision is deterministic per
/// commit), so a value backed by a validity threshold of stake includes an
/// honest validator and is the honest value; a dishonest reporter can only
/// delay corroboration, never corrupt it. No uncorroborated value is ever
/// endorsed — rather than guess, callers surface the aggregated rejections
/// and leave the resubmission price to the client.
pub(crate) fn check_congestion(
    errors: &[(AuthorityName, StakeUnit, TransactionRequestError)],
    validity_threshold: StakeUnit,
) -> CongestionCheck {
    // Supporting stake per reported (price, at_max) value.
    let mut support: BTreeMap<(u64, bool), StakeUnit> = BTreeMap::new();
    let mut congested_stake: StakeUnit = 0;
    for (_, stake, error) in errors {
        let report = match error {
            TransactionRequestError::RejectedAtValidator(
                IotaError::ValidatorTransactionCongested {
                    suggested_gas_price,
                },
            ) => (*suggested_gas_price, false),
            TransactionRequestError::RejectedAtValidator(
                IotaError::ValidatorTransactionCongestedAtMaxGasPrice { max_gas_price },
            ) => (*max_gas_price, true),
            _ => continue,
        };
        congested_stake += *stake;
        *support.entry(report).or_default() += *stake;
    }
    if congested_stake < validity_threshold {
        return CongestionCheck::Contradicted;
    }
    // At most one value can have validity-threshold support: all honest
    // congested reports are identical, and dishonest stake alone stays below
    // the threshold.
    for ((price, at_max), stake) in &support {
        if *stake >= validity_threshold {
            return CongestionCheck::Corroborated(if *at_max {
                TransactionDriverError::CongestedAtMaxGasPrice {
                    max_gas_price: *price,
                }
            } else {
                TransactionDriverError::Congested {
                    suggested_gas_price: *price,
                }
            });
        }
    }
    CongestionCheck::Uncorroborated
}

pub(crate) fn aggregate_request_errors(
    errors: Vec<(AuthorityName, StakeUnit, TransactionRequestError)>,
) -> AggregatedRequestErrors {
    let mut total_stake = 0;
    let mut aggregated_errors =
        BTreeMap::<String, (Vec<AuthorityName>, StakeUnit, ErrorCategory)>::new();

    for (name, stake, error) in errors {
        total_stake += stake;
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
    use iota_types::crypto::{AuthorityKeyPair, KeypairTraits, get_key_pair};

    use super::*;

    fn congested_error(suggested_gas_price: u64) -> TransactionRequestError {
        TransactionRequestError::RejectedAtValidator(IotaError::ValidatorTransactionCongested {
            suggested_gas_price,
        })
    }

    fn congested_at_max_error(max_gas_price: u64) -> TransactionRequestError {
        TransactionRequestError::RejectedAtValidator(
            IotaError::ValidatorTransactionCongestedAtMaxGasPrice { max_gas_price },
        )
    }

    fn random_authority() -> AuthorityName {
        let (_, key_pair): (_, AuthorityKeyPair) = get_key_pair();
        key_pair.public().into()
    }

    #[test]
    fn validator_transaction_congested_is_not_submission_retriable() {
        let error = IotaError::ValidatorTransactionCongested {
            suggested_gas_price: 1000,
        };
        assert_eq!(error.is_retryable(), (false, true));
        assert_eq!(error.categorize(), ErrorCategory::TransactionCongested);
        assert!(!error.categorize().is_submission_retriable());
        assert!(!congested_error(1000).is_submission_retriable());
    }

    #[test]
    fn congested_driver_error_is_not_submission_retriable() {
        let error = TransactionDriverError::Congested {
            suggested_gas_price: 1000,
        };
        assert_eq!(error.categorize(), ErrorCategory::TransactionCongested);
        assert!(!error.is_submission_retriable());
    }

    #[test]
    fn congested_at_max_gas_price_driver_error_is_not_submission_retriable() {
        let error = TransactionDriverError::CongestedAtMaxGasPrice {
            max_gas_price: 1000,
        };
        assert_eq!(error.categorize(), ErrorCategory::TransactionCongested);
        assert!(!error.is_submission_retriable());
    }

    #[test]
    fn check_congestion_requires_validity_threshold() {
        let errors = vec![
            (random_authority(), 1, congested_error(1000)),
            (random_authority(), 1, congested_error(1000)),
            (
                random_authority(),
                1,
                TransactionRequestError::RejectedAtValidator(IotaError::TransactionExpired),
            ),
        ];

        // Congested stake (2) at the threshold and agreeing on the price:
        // the agreed value is surfaced; other errors don't count toward it.
        assert_eq!(
            check_congestion(&errors, 2),
            CongestionCheck::Corroborated(TransactionDriverError::Congested {
                suggested_gas_price: 1000
            })
        );
        // Congested stake below the threshold.
        assert_eq!(check_congestion(&errors, 3), CongestionCheck::Contradicted);
        // No congested errors at all.
        assert_eq!(
            check_congestion(&errors[2..], 1),
            CongestionCheck::Contradicted
        );
    }

    #[test]
    fn check_congestion_at_max_gas_price_needs_threshold_support() {
        // An agreed at-max-gas-price report is surfaced as such.
        let errors = vec![(random_authority(), 2, congested_at_max_error(2000))];
        assert_eq!(
            check_congestion(&errors, 2),
            CongestionCheck::Corroborated(TransactionDriverError::CongestedAtMaxGasPrice {
                max_gas_price: 2000
            })
        );
    }

    #[test]
    fn check_congestion_single_high_report_cannot_inflate_price() {
        // Honest validators (stake 2) report the deterministic price; one
        // dishonest report (stake 1) cannot raise the suggestion.
        let errors = vec![
            (random_authority(), 2, congested_error(1000)),
            (random_authority(), 1, congested_error(1_000_000)),
        ];
        assert_eq!(
            check_congestion(&errors, 2),
            CongestionCheck::Corroborated(TransactionDriverError::Congested {
                suggested_gas_price: 1000
            })
        );
    }

    #[test]
    fn check_congestion_single_low_report_cannot_deflate_price() {
        // A dishonest low report (stake 1) does not pull the suggestion under
        // the price backed by threshold stake.
        let errors = vec![
            (random_authority(), 1, congested_error(1)),
            (random_authority(), 2, congested_error(1000)),
        ];
        assert_eq!(
            check_congestion(&errors, 2),
            CongestionCheck::Corroborated(TransactionDriverError::Congested {
                suggested_gas_price: 1000
            })
        );
    }

    #[test]
    fn check_congestion_single_at_max_report_cannot_flip_variant() {
        // One dishonest at-max report (stake 1) cannot make the client give
        // up on resubmission when threshold stake reports a beatable price.
        let errors = vec![
            (random_authority(), 2, congested_error(1000)),
            (random_authority(), 1, congested_at_max_error(2000)),
        ];
        assert_eq!(
            check_congestion(&errors, 2),
            CongestionCheck::Corroborated(TransactionDriverError::Congested {
                suggested_gas_price: 1000
            })
        );
    }

    #[test]
    fn check_congestion_uncorroborated() {
        // Congestion-dominated but no value has threshold backing: no
        // reported price is endorsed.
        let errors = vec![
            (random_authority(), 1, congested_error(1200)),
            (random_authority(), 1, congested_at_max_error(2000)),
        ];
        assert_eq!(
            check_congestion(&errors, 2),
            CongestionCheck::Uncorroborated
        );

        // Agreement formed by a later response resolves it.
        let errors = vec![
            (random_authority(), 1, congested_error(1200)),
            (random_authority(), 1, congested_at_max_error(2000)),
            (random_authority(), 1, congested_error(1200)),
        ];
        assert_eq!(
            check_congestion(&errors, 2),
            CongestionCheck::Corroborated(TransactionDriverError::Congested {
                suggested_gas_price: 1200
            })
        );
    }
}
