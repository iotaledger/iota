// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use futures::stream::{FuturesUnordered, StreamExt};
use iota_types::{
    base_types::AuthorityName,
    error::{ErrorCategory, IotaError},
    messages_grpc::TxStatusUpdate,
    transaction::TransactionEnvelope,
};
use tokio::time::timeout;
use tracing::instrument;

use crate::{
    authority_aggregator::AuthorityAggregator,
    authority_client::AuthorityAPI,
    safe_client::SafeClient,
    transaction_driver::{
        SubmitTransactionOptions, TransactionDriverMetrics,
        error::{
            AggregatedEffectsDigests, TransactionDriverError, TransactionRequestError,
            aggregate_request_errors,
        },
        request_retrier::RequestRetrier,
    },
    validator_client_monitor::{OperationFeedback, OperationType, ValidatorClientMonitor},
};

// Using a long timeout for transaction submission is ok, because good
// performing validators are chosen first.
const SUBMIT_TRANSACTION_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct TransactionSubmitter {
    metrics: Arc<TransactionDriverMetrics>,
}

impl TransactionSubmitter {
    pub(crate) fn new(metrics: Arc<TransactionDriverMetrics>) -> Self {
        Self { metrics }
    }

    #[instrument(level = "debug", skip_all, err(level = "debug"))]
    pub(crate) async fn submit_transaction<A>(
        &self,
        authority_aggregator: &AuthorityAggregator<A>,
        client_monitor: &ValidatorClientMonitor,
        amplification_factor: u64,
        transaction: Option<TransactionEnvelope>,
        options: &SubmitTransactionOptions,
    ) -> Result<(AuthorityName, TxStatusUpdate), TransactionDriverError>
    where
        A: AuthorityAPI + Send + Sync + 'static,
    {
        let start_time = Instant::now();

        self.metrics
            .submit_amplification_factor
            .observe(amplification_factor as f64);

        let mut retrier = RequestRetrier::new(
            authority_aggregator,
            client_monitor,
            &options.allowed_validators,
            &options.blocked_validators,
        );
        retrier.exclude(&options.cut_off_validators);

        let mut retries = 0;
        let mut request_rpcs = FuturesUnordered::new();

        // This loop terminates when there are enough (f+1) non-retriable errors when
        // submitting the transaction, or all feasible targets returned errors
        // or timed out.
        loop {
            // Try to fill up to amplification_factor concurrent requests
            while request_rpcs.len() < amplification_factor as usize {
                match retrier.next_target() {
                    Ok((name, client)) => {
                        let display_name = authority_aggregator.get_display_name(&name);
                        self.metrics
                            .validator_selections
                            .with_label_values(&[display_name.as_str()])
                            .inc();

                        // Create a future that returns the name and display_name along with the
                        // result
                        let submit_fut = self.submit_transaction_once(
                            client,
                            &transaction,
                            options,
                            client_monitor,
                            name,
                            display_name.clone(),
                        );

                        let wrapped_fut = async move {
                            let result = submit_fut.await;
                            (name, display_name, result)
                        };

                        request_rpcs.push(wrapped_fut);
                    }
                    Err(_) if request_rpcs.is_empty() => {
                        // No more targets and no requests in flight
                        return Err(TransactionDriverError::Aborted {
                            submission_non_retriable_errors: aggregate_request_errors(
                                retrier
                                    .non_retriable_errors_aggregator
                                    .status_by_authority(),
                            ),
                            submission_retriable_errors: aggregate_request_errors(
                                retrier.retriable_errors_aggregator.status_by_authority(),
                            ),
                            observed_effects_digests: AggregatedEffectsDigests {
                                digests: Vec::new(),
                            },
                        });
                    }
                    Err(_) => {
                        // No more targets but still have requests in flight
                        break;
                    }
                }
            }

            match request_rpcs.next().await {
                Some((name, display_name, Ok(result))) => {
                    self.metrics
                        .validator_submit_transaction_successes
                        .with_label_values(&[display_name.as_str()])
                        .inc();
                    self.metrics
                        .submit_transaction_retries
                        .observe(retries as f64);
                    let elapsed = start_time.elapsed().as_secs_f64();
                    self.metrics.submit_transaction_latency.observe(elapsed);

                    return Ok((name, result));
                }
                Some((name, display_name, Err(e))) => {
                    let error_type = e.categorize().into();
                    self.metrics
                        .validator_submit_transaction_errors
                        .with_label_values(&[display_name.as_str(), error_type])
                        .inc();

                    retries += 1;
                    retrier.add_error(name, e)?;
                }
                None => {
                    // All requests have been processed.
                    return Err(TransactionDriverError::Aborted {
                        submission_non_retriable_errors: aggregate_request_errors(
                            retrier
                                .non_retriable_errors_aggregator
                                .status_by_authority(),
                        ),
                        submission_retriable_errors: aggregate_request_errors(
                            retrier.retriable_errors_aggregator.status_by_authority(),
                        ),
                        observed_effects_digests: AggregatedEffectsDigests {
                            digests: Vec::new(),
                        },
                    });
                }
            };

            // Yield to prevent this retry loop from starving other tasks.
            tokio::task::yield_now().await;
        }
    }

    #[instrument(level = "debug", skip_all, err(level = "debug"), ret, fields(validator_display_name = ?display_name))]
    pub(crate) async fn submit_transaction_once<A>(
        &self,
        client: Arc<SafeClient<A>>,
        transaction: &Option<TransactionEnvelope>,
        options: &SubmitTransactionOptions,
        client_monitor: &ValidatorClientMonitor,
        validator: AuthorityName,
        display_name: String,
    ) -> Result<TxStatusUpdate, TransactionRequestError>
    where
        A: AuthorityAPI + Send + Sync + 'static,
    {
        let feedback_builder =
            &OperationFeedback::builder(validator, display_name, OperationType::Submit);
        let submit_start = Instant::now();

        let statuses = timeout(
            SUBMIT_TRANSACTION_TIMEOUT,
            client.submit_tx(
                transaction.clone().into_iter().collect(),
                options.forwarded_client_addr,
            ),
        )
        .await
        .map_err(|_| {
            client_monitor.record_interaction_result(feedback_builder.clone().err_now());
            TransactionRequestError::TimedOutSubmittingTransaction
        })?
        .map_err(|error| {
            if is_validator_error(error.categorize()) {
                client_monitor.record_interaction_result(feedback_builder.clone().err_now());
            }
            TransactionRequestError::RejectedAtValidator(error)
        })?;

        let result = statuses
            .into_iter()
            .next()
            .map(|(_digest, update)| update)
            .unwrap_or(TxStatusUpdate::Rejected {
                error: IotaError::Unknown("No result returned".to_string()),
            });

        // Since only one transaction is submitted, it is ok to return error when the
        // submission is rejected, expired or timed out.
        match &result {
            TxStatusUpdate::Rejected { error } => {
                let err = error.clone();
                if is_validator_error(err.categorize()) {
                    client_monitor.record_interaction_result(feedback_builder.clone().err_now());
                }
                return Err(TransactionRequestError::RejectedAtValidator(err));
            }
            TxStatusUpdate::Expired { epoch } => {
                return Err(TransactionRequestError::StatusExpired(*epoch));
            }
            TxStatusUpdate::TimedOut => {
                return Err(TransactionRequestError::TimedOutSubmittingTransaction);
            }
            _ => {}
        }

        let latency = submit_start.elapsed();
        client_monitor.record_interaction_result(feedback_builder.clone().ok_now(latency));
        Ok(result)
    }

    /// Pings `validator` with a submit request that carries no transaction
    /// and records the round trip with `client_monitor` as a submit
    /// interaction. The validator answers such a request with an empty
    /// stream, so an answer within `timeout_duration` is a successful ping
    /// and its latency sample. The sample measures the submit RPC's round
    /// trip only: nothing is admitted or submitted to consensus. A validator
    /// error or no answer in time is recorded as a failed submit, as for a
    /// transaction.
    #[instrument(level = "debug", skip_all, err(level = "debug"), fields(validator_display_name = ?display_name))]
    pub(crate) async fn ping<A>(
        &self,
        client: Arc<SafeClient<A>>,
        client_monitor: &ValidatorClientMonitor,
        validator: AuthorityName,
        display_name: String,
        timeout_duration: Duration,
    ) -> Result<Duration, IotaError>
    where
        A: AuthorityAPI + Send + Sync + 'static,
    {
        let feedback_builder =
            OperationFeedback::builder(validator, display_name, OperationType::Submit);
        let start = Instant::now();
        match timeout(timeout_duration, client.submit_tx(Vec::new(), None)).await {
            Ok(Ok(_)) => {
                let latency = start.elapsed();
                client_monitor.record_interaction_result(feedback_builder.ok_now(latency));
                Ok(latency)
            }
            Ok(Err(error)) => {
                if is_validator_error(error.categorize()) {
                    client_monitor.record_interaction_result(feedback_builder.err_now());
                }
                Err(error)
            }
            Err(_) => {
                client_monitor.record_interaction_result(feedback_builder.err_now());
                Err(IotaError::Timeout)
            }
        }
    }
}

// Whether the failure is caused by the peer validator, as opposed to the user
// or this node.
fn is_validator_error(category: ErrorCategory) -> bool {
    matches!(
        category,
        ErrorCategory::Aborted
            | ErrorCategory::Internal
            | ErrorCategory::ValidatorOverloaded
            | ErrorCategory::Unavailable
    )
}

#[cfg(test)]
mod tests {
    use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
    use iota_sdk_types::{ObjectId, Version};
    use iota_types::error::UserInputError;

    use super::*;
    use crate::{
        authority_aggregator::AuthorityAggregatorBuilder, test_authority_clients::MockAuthorityApi,
        validator_client_monitor::ValidatorClientMetrics,
    };

    /// Every validator rejects the transaction for an input object it does
    /// not have. The submission is rejected once a quorum answered, without
    /// asking the rest, and the error names the object.
    #[tokio::test(start_paused = true)]
    async fn submission_rejected_by_a_quorum_for_a_missing_input_is_final() {
        let aggregator =
            AuthorityAggregatorBuilder::from_committee_size(4).build_mock_authority_aggregator();
        let transaction = iota_types::utils::create_fake_transaction();
        let rejected = TxStatusUpdate::Rejected {
            error: IotaError::from(UserInputError::ObjectNotFound {
                object_id: ObjectId::ZERO,
                version: Some(Version::from_u64(1)),
            }),
        };
        for client in aggregator.authority_clients.values() {
            client
                .authority_client()
                .stub_submit_tx(Ok(vec![(*transaction.digest(), rejected.clone())]));
        }
        let submitter =
            TransactionSubmitter::new(Arc::new(TransactionDriverMetrics::new_for_tests()));
        let monitor = ValidatorClientMonitor::new_for_test();

        let error = submitter
            .submit_transaction(
                &aggregator,
                &monitor,
                1,
                Some(transaction),
                &SubmitTransactionOptions::default(),
            )
            .await
            .expect_err("a quorum rejecting for a missing input is final");

        assert!(
            matches!(error, TransactionDriverError::RejectedByValidators { .. }),
            "{error}"
        );
        assert!(!error.is_submission_retriable(), "{error}");
        assert!(
            error
                .to_string()
                .contains("Could not find the referenced object"),
            "{error}"
        );
        let asked = aggregator
            .authority_clients
            .values()
            .filter(|client| !client.authority_client().submit_requests().is_empty())
            .count();
        assert_eq!(asked, 3);
    }

    /// Longer than the mock client's delay.
    const PING_TIMEOUT: Duration = Duration::from_secs(1);

    struct PingSetup {
        submitter: TransactionSubmitter,
        monitor: ValidatorClientMonitor,
        metrics: Arc<ValidatorClientMetrics>,
        client: Arc<SafeClient<MockAuthorityApi>>,
        name: AuthorityName,
        display_name: String,
    }

    impl PingSetup {
        fn new() -> Self {
            let aggregator = AuthorityAggregatorBuilder::from_committee_size(4)
                .build_mock_authority_aggregator();
            let (name, client) = aggregator
                .authority_clients
                .iter()
                .next()
                .expect("the committee has members");
            let metrics = Arc::new(ValidatorClientMetrics::new_for_tests());
            Self {
                submitter: TransactionSubmitter::new(Arc::new(
                    TransactionDriverMetrics::new_for_tests(),
                )),
                monitor: ValidatorClientMonitor::new(
                    ValidatorClientMonitorConfig::default(),
                    metrics.clone(),
                ),
                metrics,
                client: client.clone(),
                name: *name,
                display_name: aggregator.get_display_name(name),
            }
        }

        async fn ping(&self, timeout_duration: Duration) -> Result<Duration, IotaError> {
            self.submitter
                .ping(
                    self.client.clone(),
                    &self.monitor,
                    self.name,
                    self.display_name.clone(),
                    timeout_duration,
                )
                .await
        }

        /// The successful and failed submit interactions recorded for the
        /// pinged validator.
        fn submit_counts(&self) -> (u64, u64) {
            self.metrics
                .operation_counts(&self.display_name, OperationType::Submit)
        }
    }

    /// The ping sends no transaction, and the validator answers it with an
    /// empty stream, which is a successful ping with a submit latency sample.
    #[tokio::test(start_paused = true)]
    async fn ping_answered_with_an_empty_stream_is_a_submit_success() {
        let setup = PingSetup::new();
        setup
            .client
            .authority_client()
            .stub_submit_tx(Ok(Vec::new()));

        let latency = setup
            .ping(PING_TIMEOUT)
            .await
            .expect("an answered ping succeeds");

        let requests = setup.client.authority_client().submit_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].is_empty());
        assert!(latency > Duration::ZERO);
        assert_eq!(setup.submit_counts(), (1, 0));
    }

    /// A validator error is a failed ping recorded against the validator.
    #[tokio::test(start_paused = true)]
    async fn ping_failed_by_the_validator_is_a_submit_failure() {
        let setup = PingSetup::new();
        setup
            .client
            .authority_client()
            .stub_submit_tx(Err(IotaError::Unknown("validator failure".to_string())));

        setup
            .ping(PING_TIMEOUT)
            .await
            .expect_err("a failed ping errs");

        assert_eq!(setup.submit_counts(), (0, 1));
    }

    /// A ping the validator does not answer in time is a failed ping.
    #[tokio::test(start_paused = true)]
    async fn ping_not_answered_in_time_is_a_submit_failure() {
        let setup = PingSetup::new();
        setup
            .client
            .authority_client()
            .stub_submit_tx(Ok(Vec::new()));

        let error = setup
            .ping(Duration::from_millis(1))
            .await
            .expect_err("an unanswered ping errs");

        assert!(matches!(error, IotaError::Timeout));
        assert_eq!(setup.submit_counts(), (0, 1));
    }
}
