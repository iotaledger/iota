// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    io,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::SystemTime,
};

use anyhow::Result;
use async_trait::async_trait;
use iota_config::local_ip_utils::new_local_tcp_address_for_testing;
use iota_metrics::{histogram::Histogram as IotaHistogram, spawn_monitored_task};
use iota_network::{
    api::{Validator, ValidatorServer},
    tonic,
};
use iota_types::{
    effects::TransactionEffectsAPI,
    error::*,
    fp_ensure,
    iota_system_state::IotaSystemState,
    messages_checkpoint::{CheckpointRequest, CheckpointResponse},
    messages_consensus::ConsensusTransaction,
    messages_grpc::{
        HandleCertificateRequestV1, HandleCertificateResponseV1,
        HandleSoftBundleCertificatesRequestV1, HandleSoftBundleCertificatesResponseV1,
        HandleTransactionResponse, ObjectInfoRequest, ObjectInfoResponse,
        SubmitCertificateResponse, SystemStateRequest, TransactionInfoRequest,
        TransactionInfoResponse,
    },
    multiaddr::Multiaddr,
    traffic_control::{ClientIdSource, PolicyConfig, RemoteFirewallConfig, Weight},
    transaction::*,
};
use nonempty::{NonEmpty, nonempty};
use prometheus::{
    IntCounter, IntCounterVec, Registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry,
};
use tap::TapFallible;
use tokio::task::JoinHandle;
use tonic::{
    metadata::{Ascii, MetadataValue},
    transport::server::TcpConnectInfo,
};
use tracing::{Instrument, error, error_span, info};

use crate::{
    authority::{AuthorityState, authority_per_epoch_store::AuthorityPerEpochStore},
    consensus_adapter::{
        ConnectionMonitorStatusForTests, ConsensusAdapter, ConsensusAdapterMetrics,
    },
    mysticeti_adapter::LazyMysticetiClient,
    traffic_controller::{
        TrafficController, metrics::TrafficControllerMetrics, policies::TrafficTally,
    },
};

#[cfg(test)]
#[path = "unit_tests/server_tests.rs"]
mod server_tests;

/// A handle to the authority server.
pub struct AuthorityServerHandle {
    tx_cancellation: tokio::sync::oneshot::Sender<()>,
    local_addr: Multiaddr,
    handle: JoinHandle<Result<(), tonic::transport::Error>>,
}

impl AuthorityServerHandle {
    /// Waits for the server to complete.
    pub async fn join(self) -> Result<(), io::Error> {
        // Note that dropping `self.complete` would terminate the server.
        self.handle.await?.map_err(io::Error::other)?;
        Ok(())
    }

    /// Kills the server.
    pub async fn kill(self) -> Result<(), io::Error> {
        self.tx_cancellation
            .send(())
            .map_err(|_e| io::Error::other("could not send cancellation signal!"))?;
        self.handle.await?.map_err(io::Error::other)?;
        Ok(())
    }

    /// Returns the address of the server.
    pub fn address(&self) -> &Multiaddr {
        &self.local_addr
    }
}

/// An authority server that is used for testing.
pub struct AuthorityServer {
    address: Multiaddr,
    pub state: Arc<AuthorityState>,
    consensus_adapter: Arc<ConsensusAdapter>,
    pub metrics: Arc<ValidatorServiceMetrics>,
}

impl AuthorityServer {
    /// Creates a new `AuthorityServer` for testing with a consensus adapter.
    pub fn new_for_test_with_consensus_adapter(
        state: Arc<AuthorityState>,
        consensus_adapter: Arc<ConsensusAdapter>,
    ) -> Self {
        let address = new_local_tcp_address_for_testing();
        let metrics = Arc::new(ValidatorServiceMetrics::new_for_tests());

        Self {
            address,
            state,
            consensus_adapter,
            metrics,
        }
    }

    /// Creates a new `AuthorityServer` for testing.
    pub fn new_for_test(state: Arc<AuthorityState>) -> Self {
        let consensus_adapter = Arc::new(ConsensusAdapter::new(
            Arc::new(LazyMysticetiClient::new()),
            state.name,
            Arc::new(ConnectionMonitorStatusForTests {}),
            100_000,
            100_000,
            None,
            None,
            ConsensusAdapterMetrics::new_test(),
        ));
        Self::new_for_test_with_consensus_adapter(state, consensus_adapter)
    }

    /// Spawns the server.
    pub async fn spawn_for_test(self) -> Result<AuthorityServerHandle, io::Error> {
        let address = self.address.clone();
        self.spawn_with_bind_address_for_test(address).await
    }

    /// Spawns the server with a bind address.
    pub async fn spawn_with_bind_address_for_test(
        self,
        address: Multiaddr,
    ) -> Result<AuthorityServerHandle, io::Error> {
        let mut server = iota_network_stack::config::Config::new()
            .server_builder()
            .add_service(ValidatorServer::new(ValidatorService::new_for_tests(
                self.state,
                self.consensus_adapter,
                self.metrics,
            )))
            .bind(&address)
            .await
            .unwrap();
        let local_addr = server.local_addr().to_owned();
        info!("Listening to traffic on {local_addr}");
        let handle = AuthorityServerHandle {
            tx_cancellation: server.take_cancel_handle().unwrap(),
            local_addr,
            handle: spawn_monitored_task!(server.serve()),
        };
        Ok(handle)
    }
}

/// Metrics for the validator service.
pub struct ValidatorServiceMetrics {
    pub signature_errors: IntCounter,
    pub tx_verification_latency: IotaHistogram,
    pub cert_verification_latency: IotaHistogram,
    pub consensus_latency: IotaHistogram,
    pub handle_transaction_latency: IotaHistogram,
    pub submit_certificate_consensus_latency: IotaHistogram,
    pub handle_certificate_consensus_latency: IotaHistogram,
    pub handle_certificate_non_consensus_latency: IotaHistogram,
    pub handle_soft_bundle_certificates_consensus_latency: IotaHistogram,
    pub handle_soft_bundle_certificates_count: IotaHistogram,
    pub handle_soft_bundle_certificates_size_bytes: IotaHistogram,

    num_rejected_tx_in_epoch_boundary: IntCounter,
    num_rejected_cert_in_epoch_boundary: IntCounter,
    num_rejected_tx_during_overload: IntCounterVec,
    num_rejected_cert_during_overload: IntCounterVec,
    connection_ip_not_found: IntCounter,
    forwarded_header_parse_error: IntCounter,
    forwarded_header_invalid: IntCounter,
    forwarded_header_not_included: IntCounter,
}

impl ValidatorServiceMetrics {
    /// Creates a new `ValidatorServiceMetrics` with Prometheus registry.
    pub fn new(registry: &Registry) -> Self {
        Self {
            signature_errors: register_int_counter_with_registry!(
                "total_signature_errors",
                "Number of transaction signature errors",
                registry,
            )
            .unwrap(),
            tx_verification_latency: IotaHistogram::new_in_registry(
                "validator_service_tx_verification_latency",
                "Latency of verifying a transaction",
                registry,
            ),
            cert_verification_latency: IotaHistogram::new_in_registry(
                "validator_service_cert_verification_latency",
                "Latency of verifying a certificate",
                registry,
            ),
            consensus_latency: IotaHistogram::new_in_registry(
                "validator_service_consensus_latency",
                "Time spent between submitting a shared obj txn to consensus and getting result",
                registry,
            ),
            handle_transaction_latency: IotaHistogram::new_in_registry(
                "validator_service_handle_transaction_latency",
                "Latency of handling a transaction",
                registry,
            ),
            handle_certificate_consensus_latency: IotaHistogram::new_in_registry(
                "validator_service_handle_certificate_consensus_latency",
                "Latency of handling a consensus transaction certificate",
                registry,
            ),
            submit_certificate_consensus_latency: IotaHistogram::new_in_registry(
                "validator_service_submit_certificate_consensus_latency",
                "Latency of submit_certificate RPC handler",
                registry,
            ),
            handle_certificate_non_consensus_latency: IotaHistogram::new_in_registry(
                "validator_service_handle_certificate_non_consensus_latency",
                "Latency of handling a non-consensus transaction certificate",
                registry,
            ),
            handle_soft_bundle_certificates_consensus_latency: IotaHistogram::new_in_registry(
                "validator_service_handle_soft_bundle_certificates_consensus_latency",
                "Latency of handling a consensus soft bundle",
                registry,
            ),
            num_rejected_tx_in_epoch_boundary: register_int_counter_with_registry!(
                "validator_service_num_rejected_tx_in_epoch_boundary",
                "Number of rejected transaction during epoch transitioning",
                registry,
            )
            .unwrap(),
            num_rejected_cert_in_epoch_boundary: register_int_counter_with_registry!(
                "validator_service_num_rejected_cert_in_epoch_boundary",
                "Number of rejected transaction certificate during epoch transitioning",
                registry,
            )
            .unwrap(),
            num_rejected_tx_during_overload: register_int_counter_vec_with_registry!(
                "validator_service_num_rejected_tx_during_overload",
                "Number of rejected transaction due to system overload",
                &["error_type"],
                registry,
            )
            .unwrap(),
            num_rejected_cert_during_overload: register_int_counter_vec_with_registry!(
                "validator_service_num_rejected_cert_during_overload",
                "Number of rejected transaction certificate due to system overload",
                &["error_type"],
                registry,
            )
            .unwrap(),
            handle_soft_bundle_certificates_count: IotaHistogram::new_in_registry(
                "handle_soft_bundle_certificates_count",
                "The number of certificates included in a soft bundle",
                registry,
            ),
            handle_soft_bundle_certificates_size_bytes: IotaHistogram::new_in_registry(
                "handle_soft_bundle_certificates_size_bytes",
                "The size of soft bundle in bytes",
                registry,
            ),
            connection_ip_not_found: register_int_counter_with_registry!(
                "validator_service_connection_ip_not_found",
                "Number of times connection IP was not extractable from request",
                registry,
            )
            .unwrap(),
            forwarded_header_parse_error: register_int_counter_with_registry!(
                "validator_service_forwarded_header_parse_error",
                "Number of times x-forwarded-for header could not be parsed",
                registry,
            )
            .unwrap(),
            forwarded_header_invalid: register_int_counter_with_registry!(
                "validator_service_forwarded_header_invalid",
                "Number of times x-forwarded-for header was invalid",
                registry,
            )
            .unwrap(),
            forwarded_header_not_included: register_int_counter_with_registry!(
                "validator_service_forwarded_header_not_included",
                "Number of times x-forwarded-for header was (unexpectedly) not included in request",
                registry,
            )
            .unwrap(),
        }
    }

    /// Creates a new `ValidatorServiceMetrics` for testing.
    pub fn new_for_tests() -> Self {
        let registry = Registry::new();
        Self::new(&registry)
    }
}

/// The validator service.
#[derive(Clone)]
pub struct ValidatorService {
    state: Arc<AuthorityState>,
    consensus_adapter: Arc<ConsensusAdapter>,
    metrics: Arc<ValidatorServiceMetrics>,
    traffic_controller: Option<Arc<TrafficController>>,
    client_id_source: Option<ClientIdSource>,
}

impl ValidatorService {
    /// Creates a new `ValidatorService`.
    pub fn new(
        state: Arc<AuthorityState>,
        consensus_adapter: Arc<ConsensusAdapter>,
        validator_metrics: Arc<ValidatorServiceMetrics>,
        traffic_controller_metrics: TrafficControllerMetrics,
        policy_config: Option<PolicyConfig>,
        firewall_config: Option<RemoteFirewallConfig>,
    ) -> Self {
        Self {
            state,
            consensus_adapter,
            metrics: validator_metrics,
            traffic_controller: policy_config.clone().map(|policy| {
                Arc::new(TrafficController::spawn(
                    policy,
                    traffic_controller_metrics,
                    firewall_config,
                ))
            }),
            client_id_source: policy_config.map(|policy| policy.client_id_source),
        }
    }

    pub fn new_for_tests(
        state: Arc<AuthorityState>,
        consensus_adapter: Arc<ConsensusAdapter>,
        metrics: Arc<ValidatorServiceMetrics>,
    ) -> Self {
        Self {
            state,
            consensus_adapter,
            metrics,
            traffic_controller: None,
            client_id_source: None,
        }
    }

    /// Returns the validator state.
    pub fn validator_state(&self) -> &Arc<AuthorityState> {
        &self.state
    }

    /// Executes a `CertifiedTransaction` for testing.
    pub async fn execute_certificate_for_testing(
        &self,
        cert: CertifiedTransaction,
    ) -> Result<tonic::Response<HandleCertificateResponseV1>, tonic::Status> {
        let request = make_tonic_request_for_testing(HandleCertificateRequestV1::new(cert));
        self.handle_certificate_v1(request).await
    }

    /// Handles a `Transaction` request for benchmarking.
    pub async fn handle_transaction_for_benchmarking(
        &self,
        transaction: Transaction,
    ) -> Result<tonic::Response<HandleTransactionResponse>, tonic::Status> {
        let request = make_tonic_request_for_testing(transaction);
        self.transaction(request).await
    }

    /// Handles a `Transaction` request.
    async fn handle_transaction(
        &self,
        request: tonic::Request<Transaction>,
    ) -> WrappedServiceResponse<HandleTransactionResponse> {
        let Self {
            state,
            consensus_adapter,
            metrics,
            traffic_controller: _,
            client_id_source: _,
        } = self.clone();
        let transaction = request.into_inner();
        let epoch_store = state.load_epoch_store_one_call_per_task();

        transaction.validity_check(epoch_store.protocol_config(), epoch_store.epoch())?;

        // When authority is overloaded and decide to reject this tx, we still lock the
        // object and ask the client to retry in the future. This is because
        // without locking, the input objects can be locked by a different tx in
        // the future, however, the input objects may already be locked by this
        // tx in other validators. This can cause non of the txes to have enough
        // quorum to form a certificate, causing the objects to be locked for
        // the entire epoch. By doing locking but pushback, retrying transaction will
        // have higher chance to succeed.
        let mut validator_pushback_error = None;
        let overload_check_res = state.check_system_overload(
            &consensus_adapter,
            transaction.data(),
            state.check_system_overload_at_signing(),
        );
        if let Err(error) = overload_check_res {
            metrics
                .num_rejected_tx_during_overload
                .with_label_values(&[error.as_ref()])
                .inc();
            // TODO: consider change the behavior for other types of overload errors.
            match error {
                IotaError::ValidatorOverloadedRetryAfter { .. } => {
                    validator_pushback_error = Some(error)
                }
                _ => return Err(error.into()),
            }
        }

        let _handle_tx_metrics_guard = metrics.handle_transaction_latency.start_timer();

        let tx_verif_metrics_guard = metrics.tx_verification_latency.start_timer();
        let transaction = epoch_store.verify_transaction(transaction).tap_err(|_| {
            metrics.signature_errors.inc();
        })?;
        drop(tx_verif_metrics_guard);

        let tx_digest = transaction.digest();

        // Enable Trace Propagation across spans/processes using tx_digest
        let span = error_span!("validator_state_process_tx", ?tx_digest);

        let info = state
            .handle_transaction(&epoch_store, transaction.clone())
            .instrument(span)
            .await
            .tap_err(|e| {
                if let IotaError::ValidatorHaltedAtEpochEnd = e {
                    metrics.num_rejected_tx_in_epoch_boundary.inc();
                }
            })?;

        if let Some(error) = validator_pushback_error {
            // TODO: right now, we still sign the txn, but just don't return it. We can also
            // skip signing to save more CPU.
            return Err(error.into());
        }

        Ok((tonic::Response::new(info), Weight::zero()))
    }

    // In addition to the response from handling the certificates,
    // returns a bool indicating whether the request should be tallied
    // toward spam count. In general, this should be set to true for
    // requests that are read-only and thus do not consume gas, such
    // as when the transaction is already executed.
    async fn handle_certificates(
        &self,
        certificates: NonEmpty<CertifiedTransaction>,
        include_events: bool,
        include_input_objects: bool,
        include_output_objects: bool,
        _include_auxiliary_data: bool,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        wait_for_effects: bool,
    ) -> Result<(Option<Vec<HandleCertificateResponseV1>>, Weight), tonic::Status> {
        // Validate if cert can be executed
        // Fullnode does not serve handle_certificate call.
        fp_ensure!(
            !self.state.is_fullnode(epoch_store),
            IotaError::FullNodeCantHandleCertificate.into()
        );

        let shared_object_tx = certificates
            .iter()
            .any(|cert| cert.contains_shared_object());

        let metrics = if certificates.len() == 1 {
            if wait_for_effects {
                if shared_object_tx {
                    &self.metrics.handle_certificate_consensus_latency
                } else {
                    &self.metrics.handle_certificate_non_consensus_latency
                }
            } else {
                &self.metrics.submit_certificate_consensus_latency
            }
        } else {
            // `soft_bundle_validity_check` ensured that all certificates contain shared
            // objects.
            &self
                .metrics
                .handle_soft_bundle_certificates_consensus_latency
        };

        let _metrics_guard = metrics.start_timer();

        // 1) Check if the certificate is already executed. This is only needed when we
        //    have only one certificate (not a soft bundle). When multiple certificates
        //    are provided, we will either submit all of them or none of them to
        //    consensus.
        if certificates.len() == 1 {
            let tx_digest = *certificates[0].digest();

            if let Some(signed_effects) = self
                .state
                .get_signed_effects_and_maybe_resign(&tx_digest, epoch_store)?
            {
                let events = if include_events {
                    if let Some(digest) = signed_effects.events_digest() {
                        Some(self.state.get_transaction_events(digest)?)
                    } else {
                        None
                    }
                } else {
                    None
                };

                return Ok((
                    Some(vec![HandleCertificateResponseV1 {
                        signed_effects: signed_effects.into_inner(),
                        events,
                        input_objects: None,
                        output_objects: None,
                        auxiliary_data: None,
                    }]),
                    Weight::one(),
                ));
            };
        }

        // 2) Verify the certificates.
        // Check system overload
        for certificate in &certificates {
            let overload_check_res = self.state.check_system_overload(
                &self.consensus_adapter,
                certificate.data(),
                self.state.check_system_overload_at_execution(),
            );
            if let Err(error) = overload_check_res {
                self.metrics
                    .num_rejected_cert_during_overload
                    .with_label_values(&[error.as_ref()])
                    .inc();
                return Err(error.into());
            }
        }

        let verified_certificates = {
            let _timer = self.metrics.cert_verification_latency.start_timer();
            epoch_store
                .signature_verifier
                .multi_verify_certs(certificates.into())
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
        };

        {
            // code block within reconfiguration lock
            let reconfiguration_lock = epoch_store.get_reconfig_state_read_lock_guard();
            if !reconfiguration_lock.should_accept_user_certs() {
                self.metrics.num_rejected_cert_in_epoch_boundary.inc();
                return Err(IotaError::ValidatorHaltedAtEpochEnd.into());
            }

            // 3) All certificates are sent to consensus (at least by some authorities)
            // For shared objects this will wait until either timeout or we have heard back
            // from consensus. For owned objects this will return without
            // waiting for certificate to be sequenced First do quick dirty
            // non-async check.
            if !epoch_store
                .is_all_tx_certs_consensus_message_processed(verified_certificates.iter())?
            {
                let _metrics_guard = if shared_object_tx {
                    Some(self.metrics.consensus_latency.start_timer())
                } else {
                    None
                };
                let transactions = verified_certificates
                    .iter()
                    .map(|certificate| {
                        ConsensusTransaction::new_certificate_message(
                            &self.state.name,
                            certificate.clone().into(),
                        )
                    })
                    .collect::<Vec<_>>();
                self.consensus_adapter.submit_batch(
                    &transactions,
                    Some(&reconfiguration_lock),
                    epoch_store,
                )?;
                // Do not wait for the result, because the transaction might
                // have already executed. Instead, check or wait
                // for the existence of certificate effects below.
            }
        }

        if !wait_for_effects {
            // It is useful to enqueue owned object transaction for execution locally,
            // even when we are not returning effects to user
            let certificates_without_shared_objects = verified_certificates
                .iter()
                .filter(|certificate| !certificate.contains_shared_object())
                .cloned()
                .collect::<Vec<_>>();
            if !certificates_without_shared_objects.is_empty() {
                self.state.enqueue_certificates_for_execution(
                    certificates_without_shared_objects,
                    epoch_store,
                );
            }
            return Ok((None, Weight::zero()));
        }

        // 4) Execute the certificates immediately if they contain only owned object
        //    transactions,
        // or wait for the execution results if it contains shared objects.
        let responses = futures::future::try_join_all(verified_certificates.into_iter().map(
            |certificate| async move {
                let effects = self
                    .state
                    .execute_certificate(&certificate, epoch_store)
                    .await?;
                let events = if include_events {
                    if let Some(digest) = effects.events_digest() {
                        Some(self.state.get_transaction_events(digest)?)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let input_objects = include_input_objects
                    .then(|| self.state.get_transaction_input_objects(&effects))
                    .and_then(Result::ok);

                let output_objects = include_output_objects
                    .then(|| self.state.get_transaction_output_objects(&effects))
                    .and_then(Result::ok);

                let signed_effects = self.state.sign_effects(effects, epoch_store)?;
                epoch_store.insert_tx_cert_sig(certificate.digest(), certificate.auth_sig())?;

                Ok::<_, IotaError>(HandleCertificateResponseV1 {
                    signed_effects: signed_effects.into_inner(),
                    events,
                    input_objects,
                    output_objects,
                    auxiliary_data: None, // We don't have any aux data generated presently
                })
            },
        ))
        .await?;

        Ok((Some(responses), Weight::zero()))
    }
}

type WrappedServiceResponse<T> = Result<(tonic::Response<T>, Weight), tonic::Status>;

impl ValidatorService {
    async fn transaction_impl(
        &self,
        request: tonic::Request<Transaction>,
    ) -> WrappedServiceResponse<HandleTransactionResponse> {
        self.handle_transaction(request).await
    }

    async fn submit_certificate_impl(
        &self,
        request: tonic::Request<CertifiedTransaction>,
    ) -> WrappedServiceResponse<SubmitCertificateResponse> {
        let epoch_store = self.state.load_epoch_store_one_call_per_task();
        let certificate = request.into_inner();
        certificate.validity_check(epoch_store.protocol_config(), epoch_store.epoch())?;

        let span = error_span!("submit_certificate", tx_digest = ?certificate.digest());
        self.handle_certificates(
            nonempty![certificate],
            true,
            false,
            false,
            false,
            &epoch_store,
            false,
        )
        .instrument(span)
        .await
        .map(|(executed, spam_weight)| {
            (
                tonic::Response::new(SubmitCertificateResponse {
                    executed: executed.map(|mut x| x.remove(0)),
                }),
                spam_weight,
            )
        })
    }

    async fn handle_certificate_v1_impl(
        &self,
        request: tonic::Request<HandleCertificateRequestV1>,
    ) -> WrappedServiceResponse<HandleCertificateResponseV1> {
        let epoch_store = self.state.load_epoch_store_one_call_per_task();
        let request = request.into_inner();
        request
            .certificate
            .validity_check(epoch_store.protocol_config(), epoch_store.epoch())?;

        let span = error_span!("handle_certificate_v1", tx_digest = ?request.certificate.digest());
        self.handle_certificates(
            nonempty![request.certificate],
            request.include_events,
            request.include_input_objects,
            request.include_output_objects,
            request.include_auxiliary_data,
            &epoch_store,
            true,
        )
        .instrument(span)
        .await
        .map(|(resp, spam_weight)| {
            (
                tonic::Response::new(
                    resp.expect(
                        "handle_certificate should not return none with wait_for_effects=true",
                    )
                    .remove(0),
                ),
                spam_weight,
            )
        })
    }

    async fn soft_bundle_validity_check(
        &self,
        certificates: &NonEmpty<CertifiedTransaction>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> Result<(), tonic::Status> {
        let protocol_config = epoch_store.protocol_config();

        // Enforce these checks per [SIP-19](https://github.com/sui-foundation/sips/blob/main/sips/sip-19.md):
        // - All certs must access at least one shared object.
        // - All certs must not be already executed.
        // - All certs must have the same gas price.
        // - Number of certs must not exceed the max allowed.
        fp_ensure!(
            certificates.len() as u64 <= protocol_config.max_soft_bundle_size(),
            IotaError::UserInput {
                error: UserInputError::TooManyTransactionsInSoftBundle {
                    limit: protocol_config.max_soft_bundle_size()
                }
            }
            .into()
        );
        let mut gas_price = None;
        for certificate in certificates {
            let tx_digest = *certificate.digest();
            fp_ensure!(
                certificate.contains_shared_object(),
                IotaError::UserInput {
                    error: UserInputError::NoSharedObject { digest: tx_digest }
                }
                .into()
            );
            fp_ensure!(
                !self.state.is_tx_already_executed(&tx_digest)?,
                IotaError::UserInput {
                    error: UserInputError::AlreadyExecuted { digest: tx_digest }
                }
                .into()
            );
            if let Some(gas) = gas_price {
                fp_ensure!(
                    gas == certificate.gas_price(),
                    IotaError::UserInput {
                        error: UserInputError::GasPriceMismatch {
                            digest: tx_digest,
                            expected: gas,
                            actual: certificate.gas_price()
                        }
                    }
                    .into()
                );
            } else {
                gas_price = Some(certificate.gas_price());
            }
        }

        // For Soft Bundle, if at this point we know at least one certificate has
        // already been processed, reject the entire bundle.  Otherwise, submit
        // all certificates in one request. This is not a strict check as there
        // may be race conditions where one or more certificates are
        // already being processed by another actor, and we could not know it.
        fp_ensure!(
            !epoch_store.is_any_tx_certs_consensus_message_processed(certificates.iter())?,
            IotaError::UserInput {
                error: UserInputError::CertificateAlreadyProcessed
            }
            .into()
        );

        Ok(())
    }

    async fn handle_soft_bundle_certificates_v1_impl(
        &self,
        request: tonic::Request<HandleSoftBundleCertificatesRequestV1>,
    ) -> WrappedServiceResponse<HandleSoftBundleCertificatesResponseV1> {
        let epoch_store = self.state.load_epoch_store_one_call_per_task();
        let client_addr = if self.client_id_source.is_none() {
            self.get_client_ip_addr(&request, &ClientIdSource::SocketAddr)
        } else {
            self.get_client_ip_addr(&request, self.client_id_source.as_ref().unwrap())
        };
        let request = request.into_inner();

        let certificates =
            NonEmpty::from_vec(request.certificates).ok_or(IotaError::NoCertificateProvided)?;
        let mut total_size_bytes = 0;
        for certificate in &certificates {
            // We need to check this first because we haven't verified the cert signature.
            total_size_bytes +=
                certificate.validity_check(epoch_store.protocol_config(), epoch_store.epoch())?;
        }

        self.metrics
            .handle_soft_bundle_certificates_count
            .observe(certificates.len() as u64);

        self.metrics
            .handle_soft_bundle_certificates_size_bytes
            .observe(total_size_bytes as u64);

        // Now that individual certificates are valid, we check if the bundle is valid.
        self.soft_bundle_validity_check(&certificates, &epoch_store)
            .await?;

        info!(
            "Received Soft Bundle with {} certificates, from {}, tx digests are [{}]",
            certificates.len(),
            client_addr
                .map(|x| x.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            certificates
                .iter()
                .map(|x| x.digest().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );

        let span = error_span!("handle_soft_bundle_certificates_v1");
        self.handle_certificates(
            certificates,
            request.include_events,
            request.include_input_objects,
            request.include_output_objects,
            request.include_auxiliary_data,
            &epoch_store,
            request.wait_for_effects,
        )
        .instrument(span)
        .await
        .map(|(resp, spam_weight)| {
            (
                tonic::Response::new(HandleSoftBundleCertificatesResponseV1 {
                    responses: resp.unwrap_or_default(),
                }),
                spam_weight,
            )
        })
    }

    async fn object_info_impl(
        &self,
        request: tonic::Request<ObjectInfoRequest>,
    ) -> WrappedServiceResponse<ObjectInfoResponse> {
        let request = request.into_inner();
        let response = self.state.handle_object_info_request(request).await?;
        Ok((tonic::Response::new(response), Weight::one()))
    }

    async fn transaction_info_impl(
        &self,
        request: tonic::Request<TransactionInfoRequest>,
    ) -> WrappedServiceResponse<TransactionInfoResponse> {
        let request = request.into_inner();
        let response = self.state.handle_transaction_info_request(request).await?;
        Ok((tonic::Response::new(response), Weight::one()))
    }

    async fn checkpoint_impl(
        &self,
        request: tonic::Request<CheckpointRequest>,
    ) -> WrappedServiceResponse<CheckpointResponse> {
        let request = request.into_inner();
        let response = self.state.handle_checkpoint_request(&request)?;
        Ok((tonic::Response::new(response), Weight::one()))
    }

    async fn get_system_state_object_impl(
        &self,
        _request: tonic::Request<SystemStateRequest>,
    ) -> WrappedServiceResponse<IotaSystemState> {
        let response = self
            .state
            .get_object_cache_reader()
            .get_iota_system_state_object_unsafe()?;
        Ok((tonic::Response::new(response), Weight::one()))
    }

    fn get_client_ip_addr<T>(
        &self,
        request: &tonic::Request<T>,
        source: &ClientIdSource,
    ) -> Option<IpAddr> {
        match source {
            ClientIdSource::SocketAddr => {
                let socket_addr: Option<SocketAddr> = request.remote_addr();

                // We will hit this case if the IO type used does not
                // implement Connected or when using a unix domain socket.
                // TODO: once we have confirmed that no legitimate traffic
                // is hitting this case, we should reject such requests that
                // hit this case.
                if let Some(socket_addr) = socket_addr {
                    Some(socket_addr.ip())
                } else {
                    if cfg!(msim) {
                        // Ignore the error from simtests.
                    } else if cfg!(test) {
                        panic!("Failed to get remote address from request");
                    } else {
                        self.metrics.connection_ip_not_found.inc();
                        error!("Failed to get remote address from request");
                    }
                    None
                }
            }
            ClientIdSource::XForwardedFor(num_hops) => {
                let do_header_parse = |op: &MetadataValue<Ascii>| {
                    match op.to_str() {
                        Ok(header_val) => {
                            let header_contents =
                                header_val.split(',').map(str::trim).collect::<Vec<_>>();
                            if *num_hops == 0 {
                                error!(
                                    "x-forwarded-for: 0 specified. x-forwarded-for contents: {:?}. Please assign nonzero value for \
                                    number of hops here, or use `socket-addr` client-id-source type if requests are not being proxied \
                                    to this node. Skipping traffic controller request handling.",
                                    header_contents,
                                );
                                return None;
                            }
                            let contents_len = header_contents.len();
                            let Some(client_ip) = header_contents.get(contents_len - num_hops)
                            else {
                                error!(
                                    "x-forwarded-for header value of {:?} contains {} values, but {} hops were specified. \
                                    Expected at least {} values. Skipping traffic controller request handling.",
                                    header_contents, contents_len, num_hops, contents_len,
                                );
                                return None;
                            };
                            client_ip.parse::<IpAddr>().ok().or_else(|| {
                                client_ip.parse::<SocketAddr>().ok().map(|socket_addr| socket_addr.ip()).or_else(|| {
                                    self.metrics.forwarded_header_parse_error.inc();
                                    error!(
                                        "Failed to parse x-forwarded-for header value of {:?} to ip address or socket. \
                                        Please ensure that your proxy is configured to resolve client domains to an \
                                        IP address before writing header",
                                        client_ip,
                                    );
                                    None
                                })
                            })
                        }
                        Err(e) => {
                            // TODO: once we have confirmed that no legitimate traffic
                            // is hitting this case, we should reject such requests that
                            // hit this case.
                            self.metrics.forwarded_header_invalid.inc();
                            error!("Invalid UTF-8 in x-forwarded-for header: {:?}", e);
                            None
                        }
                    }
                };
                if let Some(op) = request.metadata().get("x-forwarded-for") {
                    do_header_parse(op)
                } else if let Some(op) = request.metadata().get("X-Forwarded-For") {
                    do_header_parse(op)
                } else {
                    self.metrics.forwarded_header_not_included.inc();
                    error!(
                        "x-forwarded-for header not present for request despite node configuring x-forwarded-for tracking type"
                    );
                    None
                }
            }
        }
    }

    async fn handle_traffic_req(&self, client: Option<IpAddr>) -> Result<(), tonic::Status> {
        if let Some(traffic_controller) = &self.traffic_controller {
            if !traffic_controller.check(&client, &None).await {
                // Entity in blocklist
                Err(tonic::Status::from_error(IotaError::TooManyRequests.into()))
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn handle_traffic_resp<T>(
        &self,
        client: Option<IpAddr>,
        wrapped_response: WrappedServiceResponse<T>,
    ) -> Result<tonic::Response<T>, tonic::Status> {
        let (error, spam_weight, unwrapped_response) = match wrapped_response {
            Ok((result, spam_weight)) => (None, spam_weight.clone(), Ok(result)),
            Err(status) => (
                Some(IotaError::from(status.clone())),
                Weight::zero(),
                Err(status.clone()),
            ),
        };

        if let Some(traffic_controller) = self.traffic_controller.clone() {
            traffic_controller.tally(TrafficTally {
                direct: client,
                through_fullnode: None,
                error_weight: error.map(normalize).unwrap_or(Weight::zero()),
                spam_weight,
                timestamp: SystemTime::now(),
            })
        }
        unwrapped_response
    }
}

fn make_tonic_request_for_testing<T>(message: T) -> tonic::Request<T> {
    // simulate a TCP connection, which would have added extensions to
    // the request object that would be used downstream
    let mut request = tonic::Request::new(message);
    let tcp_connect_info = TcpConnectInfo {
        local_addr: None,
        remote_addr: Some(SocketAddr::new([127, 0, 0, 1].into(), 0)),
    };
    request.extensions_mut().insert(tcp_connect_info);
    request
}

// TODO: refine error matching here
fn normalize(err: IotaError) -> Weight {
    match err {
        IotaError::UserInput { .. }
        | IotaError::InvalidSignature { .. }
        | IotaError::SignerSignatureAbsent { .. }
        | IotaError::SignerSignatureNumberMismatch { .. }
        | IotaError::IncorrectSigner { .. }
        | IotaError::UnknownSigner { .. }
        | IotaError::WrongEpoch { .. } => Weight::one(),
        _ => Weight::zero(),
    }
}

/// Implements generic pre- and post-processing. Since this is on the critical
/// path, any heavy lifting should be done in a separate non-blocking task
/// unless it is necessary to override the return value.
#[macro_export]
macro_rules! handle_with_decoration {
    ($self:ident, $func_name:ident, $request:ident) => {{
        if $self.client_id_source.is_none() {
            return $self.$func_name($request).await.map(|(result, _)| result);
        }

        let client = $self.get_client_ip_addr(&$request, $self.client_id_source.as_ref().unwrap());

        // check if either IP is blocked, in which case return early
        $self.handle_traffic_req(client.clone()).await?;

        // handle traffic tallying
        let wrapped_response = $self.$func_name($request).await;
        $self.handle_traffic_resp(client, wrapped_response)
    }};
}

#[async_trait]
impl Validator for ValidatorService {
    /// Handles a `Transaction` request.
    async fn transaction(
        &self,
        request: tonic::Request<Transaction>,
    ) -> Result<tonic::Response<HandleTransactionResponse>, tonic::Status> {
        let validator_service = self.clone();

        // Spawns a task which handles the transaction. The task will unconditionally
        // continue processing in the event that the client connection is
        // dropped.
        spawn_monitored_task!(async move {
            // NB: traffic tally wrapping handled within the task rather than on task exit
            // to prevent an attacker from subverting traffic control by severing the
            // connection
            handle_with_decoration!(validator_service, transaction_impl, request)
        })
        .await
        .unwrap()
    }

    /// Submits a `CertifiedTransaction` request.
    async fn submit_certificate(
        &self,
        request: tonic::Request<CertifiedTransaction>,
    ) -> Result<tonic::Response<SubmitCertificateResponse>, tonic::Status> {
        let validator_service = self.clone();

        // Spawns a task which handles the certificate. The task will unconditionally
        // continue processing in the event that the client connection is
        // dropped.
        spawn_monitored_task!(async move {
            // NB: traffic tally wrapping handled within the task rather than on task exit
            // to prevent an attacker from subverting traffic control by severing the
            // connection.
            handle_with_decoration!(validator_service, submit_certificate_impl, request)
        })
        .await
        .unwrap()
    }

    async fn handle_certificate_v1(
        &self,
        request: tonic::Request<HandleCertificateRequestV1>,
    ) -> Result<tonic::Response<HandleCertificateResponseV1>, tonic::Status> {
        handle_with_decoration!(self, handle_certificate_v1_impl, request)
    }

    async fn handle_soft_bundle_certificates_v1(
        &self,
        request: tonic::Request<HandleSoftBundleCertificatesRequestV1>,
    ) -> Result<tonic::Response<HandleSoftBundleCertificatesResponseV1>, tonic::Status> {
        handle_with_decoration!(self, handle_soft_bundle_certificates_v1_impl, request)
    }

    /// Handles an `ObjectInfoRequest` request.
    async fn object_info(
        &self,
        request: tonic::Request<ObjectInfoRequest>,
    ) -> Result<tonic::Response<ObjectInfoResponse>, tonic::Status> {
        handle_with_decoration!(self, object_info_impl, request)
    }

    /// Handles a `TransactionInfoRequest` request.
    async fn transaction_info(
        &self,
        request: tonic::Request<TransactionInfoRequest>,
    ) -> Result<tonic::Response<TransactionInfoResponse>, tonic::Status> {
        handle_with_decoration!(self, transaction_info_impl, request)
    }

    /// Handles a `CheckpointRequest` request.
    async fn checkpoint(
        &self,
        request: tonic::Request<CheckpointRequest>,
    ) -> Result<tonic::Response<CheckpointResponse>, tonic::Status> {
        handle_with_decoration!(self, checkpoint_impl, request)
    }

    /// Gets the `IotaSystemState` response.
    async fn get_system_state_object(
        &self,
        request: tonic::Request<SystemStateRequest>,
    ) -> Result<tonic::Response<IotaSystemState>, tonic::Status> {
        handle_with_decoration!(self, get_system_state_object_impl, request)
    }
}
