// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use iota_types::{base_types::AuthorityName, committee::Committee};

use crate::{
    authority_aggregator::AuthorityAggregator,
    safe_client::SafeClient,
    status_aggregator::StatusAggregator,
    transaction_driver::error::{
        AggregatedEffectsDigests, MissingInput, TransactionDriverError, TransactionRequestError,
        aggregate_request_errors,
    },
    validator_client_monitor::ValidatorClientMonitor,
};

/// Provides the next target validator to retry operations,
/// and gathers the errors along with the operations.
///
/// In TransactionDriver, submitting a transaction and getting full effects
/// follow the same pattern:
/// 1. Retry against all validators until the operation succeeds.
/// 2. If f+1 stake of validators return non-retriable errors, or a quorum rejects the transaction
///    for the same input object or package they do not have, the operation fails permanently.
///
/// When an `allowed_validators` is provided, only the validators in the list
/// will be used to submit the transaction to. When the allowed validator list
/// is empty, any validator can be used an then the validators are selected
/// based on their scores.
///
/// When a `blocked_validators` is provided, the validators in the list cannot
/// be used to submit the transaction to. When the blocked validator list is
/// empty, no restrictions are applied.
///
/// This component helps to manage this retry pattern.
pub(crate) struct RequestRetrier<A> {
    ranked_clients: Vec<(AuthorityName, Arc<SafeClient<A>>)>,
    committee: Arc<Committee>,
    pub(crate) non_retriable_errors_aggregator: StatusAggregator<TransactionRequestError>,
    pub(crate) retriable_errors_aggregator: StatusAggregator<TransactionRequestError>,
    /// Validators that rejected the transaction for an input they do not
    /// have, per input. One such rejection is retriable, since the validator
    /// may not have executed the transaction that created the input yet. A
    /// quorum of them is final: finalizing the creating transaction takes
    /// 2f+1 stake that executed it, and any two quorums share an honest
    /// validator, so an input a quorum lacks was never finalized as
    /// referenced.
    missing_input_aggregators: BTreeMap<MissingInput, StatusAggregator<()>>,
}

impl<A> RequestRetrier<A> {
    pub(crate) fn new(
        auth_agg: &AuthorityAggregator<A>,
        client_monitor: &ValidatorClientMonitor,
        allowed_validators: &[String],
        blocked_validators: &[String],
    ) -> Self {
        let committee = auth_agg
            .committee
            .names()
            .map(|name| (name, auth_agg.get_display_name(name)))
            .filter(|(_name, display_name)| {
                allowed_validators.is_empty() || allowed_validators.contains(display_name)
            })
            .filter(|(_name, display_name)| {
                blocked_validators.is_empty() || !blocked_validators.contains(display_name)
            })
            .map(|(name, _display_name)| name);

        let mut ranked_clients = client_monitor
            .select_shuffled_preferred_validators(committee)
            .into_iter()
            .map(|name| {
                // There is a guarantee that the `name` is in the
                // `auth_agg.authority_clients`, as the provided `auth_agg` is the same.
                let client = auth_agg
                    .authority_clients
                    .get(name)
                    .expect("selected validators names must be known to authority aggregator");
                (*name, client.clone())
            })
            .collect::<Vec<_>>();
        // Ranked clients are consumed in reverse order.
        ranked_clients.reverse();
        let non_retriable_errors_aggregator = StatusAggregator::new(auth_agg.committee.clone());
        let retriable_errors_aggregator = StatusAggregator::new(auth_agg.committee.clone());
        Self {
            ranked_clients,
            committee: auth_agg.committee.clone(),
            non_retriable_errors_aggregator,
            retriable_errors_aggregator,
            missing_input_aggregators: BTreeMap::new(),
        }
    }

    // Selects the next target validator to attempt an operation.
    pub(crate) fn next_target(
        &mut self,
    ) -> Result<(AuthorityName, Arc<SafeClient<A>>), TransactionDriverError> {
        if let Some((name, client)) = self.ranked_clients.pop() {
            return Ok((name, client));
        };

        if self.is_rejected() {
            Err(self.rejected_by_validators())
        } else {
            Err(TransactionDriverError::Aborted {
                submission_non_retriable_errors: aggregate_request_errors(
                    self.non_retriable_errors_aggregator.status_by_authority(),
                ),
                submission_retriable_errors: aggregate_request_errors(
                    self.retriable_errors_aggregator.status_by_authority(),
                ),
                observed_effects_digests: AggregatedEffectsDigests {
                    digests: Vec::new(),
                },
            })
        }
    }

    // Adds an error associated with the operation against the authority.
    //
    // Returns an error once the validators' rejections are final: f+1 stake
    // of non-retriable errors, or a quorum rejecting for the same missing
    // input. Resubmitting the transaction as is then gets the same verdict
    // until the network's state changes, unless there is a software bug or
    // > f malicious validators.
    pub(crate) fn add_error(
        &mut self,
        name: AuthorityName,
        error: TransactionRequestError,
    ) -> Result<(), TransactionDriverError> {
        if error.is_submission_retriable() {
            if let Some(input) = error.missing_input() {
                self.missing_input_aggregators
                    .entry(input)
                    .or_insert_with(|| StatusAggregator::new(self.committee.clone()))
                    .insert(name, ());
            }
            self.retriable_errors_aggregator.insert(name, error);
        } else {
            self.non_retriable_errors_aggregator.insert(name, error);
        }
        if self.is_rejected() {
            return Err(self.rejected_by_validators());
        }

        Ok(())
    }

    /// Whether the validators' rejections are final.
    fn is_rejected(&self) -> bool {
        self.non_retriable_errors_aggregator
            .reached_validity_threshold()
            || self
                .missing_input_aggregators
                .values()
                .any(StatusAggregator::reached_quorum_threshold)
    }

    /// Whether a quorum of validators rejected the transaction for `input`.
    fn lacked_by_quorum(&self, input: &MissingInput) -> bool {
        self.missing_input_aggregators
            .get(input)
            .is_some_and(StatusAggregator::reached_quorum_threshold)
    }

    /// The error for final rejections. Rejections for an input a quorum
    /// lacks are listed as non-retriable, so the error names it.
    fn rejected_by_validators(&self) -> TransactionDriverError {
        let mut non_retriable_errors = self.non_retriable_errors_aggregator.status_by_authority();
        let (rejected_for_missing_input, retriable_errors): (Vec<_>, Vec<_>) = self
            .retriable_errors_aggregator
            .status_by_authority()
            .into_iter()
            .partition(|(_, _, error)| {
                error
                    .missing_input()
                    .is_some_and(|input| self.lacked_by_quorum(&input))
            });
        non_retriable_errors.extend(rejected_for_missing_input);
        TransactionDriverError::RejectedByValidators {
            submission_non_retriable_errors: aggregate_request_errors(non_retriable_errors),
            submission_retriable_errors: aggregate_request_errors(retriable_errors),
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{ObjectId, TransactionDigest, Version};
    use iota_types::{
        base_types::{ConciseableName, random_object_ref},
        error::{ErrorCategory, IotaError, UserInputError},
    };

    use super::*;
    use crate::{
        authority_aggregator::{AuthorityAggregatorBuilder, TimeoutConfig},
        test_authority_clients::MockAuthorityApi,
    };

    pub(crate) fn get_authority_aggregator(
        committee_size: usize,
    ) -> AuthorityAggregator<MockAuthorityApi> {
        let timeouts_config = TimeoutConfig::default();
        AuthorityAggregatorBuilder::from_committee_size(committee_size)
            .with_timeouts_config(timeouts_config)
            .build_mock_authority_aggregator()
    }

    #[tokio::test]
    async fn test_next_target() {
        let auth_agg = get_authority_aggregator(4);
        let client_monitor = ValidatorClientMonitor::new_for_test();
        let mut retrier = RequestRetrier::new(&auth_agg, &client_monitor, &[], &[]);

        for name in auth_agg.committee.names() {
            retrier.next_target().unwrap();
            retrier
                .add_error(
                    *name,
                    TransactionRequestError::TimedOutSubmittingTransaction,
                )
                .unwrap();
        }

        let Err(error) = retrier.next_target() else {
            panic!("Expected an error");
        };
        assert!(error.is_submission_retriable(), "{}", error);
    }

    #[tokio::test]
    async fn test_allowed_validators() {
        use iota_types::crypto::{AuthorityKeyPair, get_key_pair};

        let auth_agg = get_authority_aggregator(4);
        let client_monitor = ValidatorClientMonitor::new_for_test();

        // Create validators that don't exist in the auth_agg
        let (_, key_pair1): (_, AuthorityKeyPair) = get_key_pair();
        let (_, key_pair2): (_, AuthorityKeyPair) = get_key_pair();
        let unknown_validator1: AuthorityName = (&key_pair1.verifying_key()).into();
        let unknown_validator2: AuthorityName = (&key_pair2.verifying_key()).into();

        // Mix of unknown validators and one known validator
        let authorities: Vec<_> = auth_agg.committee.names().copied().collect();

        println!("Case 1. Mix of unknown validators and one known validator");
        {
            let allowed_validators = [
                unknown_validator1.concise().to_string(),
                unknown_validator2.concise().to_string(),
                authorities[0].concise().to_string(), // This one exists in auth_agg
            ];

            let retrier = RequestRetrier::new(&auth_agg, &client_monitor, &allowed_validators, &[]);

            // Should only have 1 remaining client (the known validator)
            assert_eq!(retrier.ranked_clients.len(), 1);
            assert_eq!(retrier.ranked_clients[0].0, authorities[0]);
        }

        println!("Case 2. Only unknown validators are provided");
        {
            let allowed_validators = [
                unknown_validator1.concise().to_string(),
                unknown_validator2.concise().to_string(),
            ];

            let retrier = RequestRetrier::new(&auth_agg, &client_monitor, &allowed_validators, &[]);

            // Should have no remaining clients since none of the allowed validators exist
            assert_eq!(retrier.ranked_clients.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_blocked_validators() {
        let auth_agg = get_authority_aggregator(4);
        let client_monitor = ValidatorClientMonitor::new_for_test();

        // Create a list of validators that should be blocked and never picked up by the
        // retrier.
        let blocked_validators = auth_agg
            .committee
            .names()
            .take(3)
            .copied()
            .collect::<Vec<_>>();
        let blocked_display_names = blocked_validators
            .iter()
            .map(|name| auth_agg.get_display_name(name))
            .collect::<Vec<_>>();

        // Only the last validator will be picked up.
        let allowed_validator = auth_agg.committee.names().nth(3).unwrap();

        let mut retrier =
            RequestRetrier::new(&auth_agg, &client_monitor, &[], &blocked_display_names);

        // The last validator will be picked up.
        assert_eq!(retrier.next_target().unwrap().0, *allowed_validator);
        // No more validators will be picked up.
        assert!(retrier.next_target().is_err());
    }

    #[tokio::test]
    async fn test_add_error() {
        let auth_agg = get_authority_aggregator(4);
        let authorities: Vec<_> = auth_agg.committee.names().copied().collect();

        // Add retriable errors.
        {
            let client_monitor = ValidatorClientMonitor::new_for_test();
            let mut retrier = RequestRetrier::new(&auth_agg, &client_monitor, &[], &[]);

            // 25% stake.
            retrier
                .add_error(
                    authorities[0],
                    TransactionRequestError::TimedOutSubmittingTransaction,
                )
                .unwrap();
            // 50% stake.
            retrier
                .add_error(
                    authorities[1],
                    TransactionRequestError::TimedOutSubmittingTransaction,
                )
                .unwrap();
            // 75% stake.
            retrier
                .add_error(
                    authorities[1],
                    TransactionRequestError::TimedOutSubmittingTransaction,
                )
                .unwrap();
            // 100% stake.
            retrier
                .add_error(
                    authorities[1],
                    TransactionRequestError::TimedOutSubmittingTransaction,
                )
                .unwrap();
            // Still there is no aggregated error.
        }

        // Add mix of retriable and non-retriable errors.
        {
            let client_monitor = ValidatorClientMonitor::new_for_test();
            let mut retrier = RequestRetrier::new(&auth_agg, &client_monitor, &[], &[]);

            // 25% stake retriable error.
            retrier
                .add_error(
                    authorities[0],
                    TransactionRequestError::TimedOutSubmittingTransaction,
                )
                .unwrap();
            // 25% stake non-retriable error.
            retrier
                .add_error(
                    authorities[1],
                    TransactionRequestError::RejectedAtValidator(IotaError::UserInput {
                        error: UserInputError::EmptyCommandInput,
                    }),
                )
                .unwrap();
            // 50% stake non-retriable error. Above validity threshold.
            let aggregated_error = retrier
                .add_error(
                    authorities[2],
                    TransactionRequestError::RejectedAtValidator(IotaError::UserInput {
                        error: UserInputError::EmptyCommandInput,
                    }),
                )
                .unwrap_err();
            // The aggregated error is non-retriable.
            assert!(!aggregated_error.is_submission_retriable());
        }
    }

    fn missing_object(object_id: ObjectId) -> TransactionRequestError {
        TransactionRequestError::RejectedAtValidator(IotaError::from(
            UserInputError::ObjectNotFound {
                object_id,
                version: Some(Version::from_u64(1)),
            },
        ))
    }

    fn lock_conflict() -> TransactionRequestError {
        TransactionRequestError::RejectedAtValidator(IotaError::ObjectLockConflict {
            obj_ref: random_object_ref(),
            pending_transaction: TransactionDigest::random(),
        })
    }

    /// A quorum rejecting for a missing input is final, and the error lists
    /// the rejections as non-retriable so it names the object.
    #[tokio::test]
    async fn missing_input_rejections_from_a_quorum_are_final() {
        let auth_agg = get_authority_aggregator(4);
        let authorities: Vec<_> = auth_agg.committee.names().copied().collect();
        let client_monitor = ValidatorClientMonitor::new_for_test();
        let mut retrier = RequestRetrier::new(&auth_agg, &client_monitor, &[], &[]);

        // 25% stake times out, 50% rejects for the missing input.
        retrier
            .add_error(
                authorities[0],
                TransactionRequestError::TimedOutSubmittingTransaction,
            )
            .unwrap();
        retrier
            .add_error(authorities[1], missing_object(ObjectId::ZERO))
            .unwrap();
        retrier
            .add_error(authorities[2], missing_object(ObjectId::ZERO))
            .unwrap();
        // 75% stake rejects for the missing input.
        let error = retrier
            .add_error(authorities[3], missing_object(ObjectId::ZERO))
            .unwrap_err();

        assert!(!error.is_submission_retriable(), "{error}");
        let TransactionDriverError::RejectedByValidators {
            submission_non_retriable_errors,
            submission_retriable_errors,
        } = &error
        else {
            panic!("expected RejectedByValidators, got {error}");
        };
        assert!(
            submission_non_retriable_errors.total_stake >= auth_agg.committee.quorum_threshold()
        );
        assert_eq!(submission_non_retriable_errors.errors.len(), 1);
        assert!(
            submission_non_retriable_errors.errors[0]
                .0
                .contains("Could not find the referenced object"),
            "{error}"
        );
        assert_eq!(submission_retriable_errors.errors.len(), 1);
        assert!(
            submission_retriable_errors.errors[0]
                .0
                .contains("timed out"),
            "{error}"
        );
    }

    /// Below a quorum, a missing-input rejection stays retriable, since the
    /// validator may catch up with the object's creation.
    #[tokio::test]
    async fn missing_input_rejections_below_a_quorum_stay_retriable() {
        let auth_agg = get_authority_aggregator(4);
        let authorities: Vec<_> = auth_agg.committee.names().copied().collect();
        let client_monitor = ValidatorClientMonitor::new_for_test();
        let mut retrier = RequestRetrier::new(&auth_agg, &client_monitor, &[], &[]);
        for _ in &authorities {
            retrier.next_target().unwrap();
        }

        // 50% stake rejects for the missing input, 50% times out.
        retrier
            .add_error(authorities[0], missing_object(ObjectId::ZERO))
            .unwrap();
        retrier
            .add_error(authorities[1], missing_object(ObjectId::ZERO))
            .unwrap();
        for name in &authorities[2..] {
            retrier
                .add_error(
                    *name,
                    TransactionRequestError::TimedOutSubmittingTransaction,
                )
                .unwrap();
        }

        let Err(error) = retrier.next_target() else {
            panic!("expected an error");
        };
        assert!(error.is_submission_retriable(), "{error}");
        let TransactionDriverError::Aborted {
            submission_non_retriable_errors,
            submission_retriable_errors,
            ..
        } = &error
        else {
            panic!("expected Aborted, got {error}");
        };
        assert_eq!(submission_non_retriable_errors.total_stake, 0);
        assert_eq!(
            submission_retriable_errors.total_stake,
            auth_agg.committee.total_votes()
        );
    }

    /// Rejections for different missing inputs do not add up to a quorum.
    #[tokio::test]
    async fn missing_input_rejections_for_different_inputs_stay_retriable() {
        let auth_agg = get_authority_aggregator(4);
        let authorities: Vec<_> = auth_agg.committee.names().copied().collect();
        let client_monitor = ValidatorClientMonitor::new_for_test();
        let mut retrier = RequestRetrier::new(&auth_agg, &client_monitor, &[], &[]);
        for _ in &authorities {
            retrier.next_target().unwrap();
        }

        // 75% stake rejects for a missing input, each for a different one.
        for name in &authorities[..3] {
            retrier
                .add_error(*name, missing_object(ObjectId::random()))
                .unwrap();
        }
        retrier
            .add_error(
                authorities[3],
                TransactionRequestError::TimedOutSubmittingTransaction,
            )
            .unwrap();

        let Err(error) = retrier.next_target() else {
            panic!("expected an error");
        };
        assert!(error.is_submission_retriable(), "{error}");
        assert!(
            matches!(error, TransactionDriverError::Aborted { .. }),
            "{error}"
        );
    }

    /// Only the rejections for the input a quorum lacks become non-retriable.
    #[tokio::test]
    async fn only_rejections_for_the_input_a_quorum_lacks_are_final() {
        let auth_agg = get_authority_aggregator(4);
        let authorities: Vec<_> = auth_agg.committee.names().copied().collect();
        let client_monitor = ValidatorClientMonitor::new_for_test();
        let mut retrier = RequestRetrier::new(&auth_agg, &client_monitor, &[], &[]);
        let lacked_by_quorum = ObjectId::random();
        let lacked_by_one = ObjectId::random();

        retrier
            .add_error(authorities[0], missing_object(lacked_by_one))
            .unwrap();
        retrier
            .add_error(authorities[1], missing_object(lacked_by_quorum))
            .unwrap();
        retrier
            .add_error(authorities[2], missing_object(lacked_by_quorum))
            .unwrap();
        let error = retrier
            .add_error(authorities[3], missing_object(lacked_by_quorum))
            .unwrap_err();

        let TransactionDriverError::RejectedByValidators {
            submission_non_retriable_errors,
            submission_retriable_errors,
        } = &error
        else {
            panic!("expected RejectedByValidators, got {error}");
        };
        assert_eq!(submission_non_retriable_errors.errors.len(), 1);
        assert!(
            submission_non_retriable_errors.errors[0]
                .0
                .contains(&lacked_by_quorum.to_string()),
            "{error}"
        );
        assert!(
            submission_non_retriable_errors.total_stake >= auth_agg.committee.quorum_threshold()
        );
        assert_eq!(submission_retriable_errors.errors.len(), 1);
        assert!(
            submission_retriable_errors.errors[0]
                .0
                .contains(&lacked_by_one.to_string()),
            "{error}"
        );
    }

    /// A non-retriable rejection next to a missing-input quorum is listed
    /// with it, and the quorum decides the category.
    #[tokio::test]
    async fn missing_input_quorum_decides_the_category() {
        let auth_agg = get_authority_aggregator(4);
        let authorities: Vec<_> = auth_agg.committee.names().copied().collect();
        let client_monitor = ValidatorClientMonitor::new_for_test();
        let mut retrier = RequestRetrier::new(&auth_agg, &client_monitor, &[], &[]);

        retrier.add_error(authorities[0], lock_conflict()).unwrap();
        retrier
            .add_error(authorities[1], missing_object(ObjectId::ZERO))
            .unwrap();
        retrier
            .add_error(authorities[2], missing_object(ObjectId::ZERO))
            .unwrap();
        let error = retrier
            .add_error(authorities[3], missing_object(ObjectId::ZERO))
            .unwrap_err();

        assert!(!error.is_submission_retriable(), "{error}");
        assert_eq!(error.categorize(), ErrorCategory::InvalidTransaction);
        let TransactionDriverError::RejectedByValidators {
            submission_non_retriable_errors,
            submission_retriable_errors,
        } = &error
        else {
            panic!("expected RejectedByValidators, got {error}");
        };
        assert_eq!(submission_non_retriable_errors.errors.len(), 2);
        assert_eq!(
            submission_non_retriable_errors.errors[0].3,
            ErrorCategory::Aborted
        );
        assert_eq!(
            submission_non_retriable_errors.errors[1].3,
            ErrorCategory::LockConflict
        );
        assert_eq!(submission_retriable_errors.total_stake, 0);
    }
}
