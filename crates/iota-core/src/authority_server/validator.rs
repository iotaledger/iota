// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use futures::future::{Either, join_all};
use iota_metrics::spawn_monitored_task;
use iota_network::{api::Validator, tonic};
use iota_types::{
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::{IotaError, UserInputError},
    fp_ensure,
    iota_system_state::IotaSystemState,
    messages_checkpoint::{CheckpointRequest, CheckpointResponse},
    messages_consensus::ConsensusTransaction,
    messages_grpc::{
        ExecutedData, HandleCapabilityNotificationRequestV1,
        HandleCapabilityNotificationResponseV1, HandleCertificateRequestV1,
        HandleCertificateResponseV1, HandleSoftBundleCertificatesRequestV1,
        HandleSoftBundleCertificatesResponseV1, HandleTransactionResponse, ObjectInfoRequest,
        ObjectInfoResponse, SubmitCertificateResponse, SubmitTransactionResult, SystemStateRequest,
        TransactionInfoRequest, TransactionInfoResponse,
    },
    traffic_control::{ClientIdSource, Weight},
    transaction::*,
};
use nonempty::{NonEmpty, nonempty};
use tap::TapFallible;
use tracing::{Instrument, debug, error_span, info, trace_span, warn};

use crate::{
    authority::authority_per_epoch_store::AuthorityPerEpochStore,
    authority_server::{ValidatorService, WrappedServiceResponse, make_tonic_request_for_testing},
    handle_with_decoration,
};

#[cfg(test)]
#[path = "../unit_tests/server_tests.rs"]
mod server_tests;

impl ValidatorService {
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

        // Reject if white flag flow is enabled - transactions should use
        // submit_transaction instead
        fp_ensure!(
            !epoch_store.protocol_config().enable_white_flag_flow(),
            IotaError::UnsupportedFeature {
                error: "handle_transaction is disabled when white flag flow is enabled. Use submit_transaction instead.".to_string()
            }
            .into()
        );

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
                    if signed_effects.events_digest().is_some() {
                        Some(
                            self.state
                                .get_transaction_events(signed_effects.transaction_digest())?,
                        )
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
                .instrument(trace_span!("SignatureVerifier::multi_verify_certs"))
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
                    if effects.events_digest().is_some() {
                        Some(
                            self.state
                                .get_transaction_events(effects.transaction_digest())?,
                        )
                    } else {
                        None
                    }
                } else {
                    None
                };

                let input_objects = include_input_objects
                    .then(|| self.state.get_transaction_input_objects(&effects))
                    .and_then(|res| {
                        res.map_err(|e| {
                            warn!(
                                tx_digest = ?effects.transaction_digest(),
                                error = ?e,
                                "Failed to load transaction input objects requested by client",
                            )
                        })
                        .ok()
                    });

                let output_objects = include_output_objects
                    .then(|| self.state.get_transaction_output_objects(&effects))
                    .and_then(|res| {
                        res.map_err(|e| {
                            warn!(
                                tx_digest = ?effects.transaction_digest(),
                                error = ?e,
                                "Failed to load transaction output objects requested by client",
                            )
                        })
                        .ok()
                    });

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

    async fn transaction_impl(
        &self,
        request: tonic::Request<Transaction>,
    ) -> WrappedServiceResponse<HandleTransactionResponse> {
        self.handle_transaction(request)
            .instrument(trace_span!("ValidatorService::handle_transaction"))
            .await
    }

    async fn submit_certificate_impl(
        &self,
        request: tonic::Request<CertifiedTransaction>,
    ) -> WrappedServiceResponse<SubmitCertificateResponse> {
        let epoch_store = self.state.load_epoch_store_one_call_per_task();

        // Reject if white flag flow is enabled - certificates are not used in white
        // flag flow
        fp_ensure!(
            !epoch_store.protocol_config().enable_white_flag_flow(),
            IotaError::UnsupportedFeature {
                error: "handle_certificate_v1 is disabled when white flag flow is enabled. Transactions go directly to consensus.".to_string()
            }
            .into()
        );

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

        // Reject if white flag flow is enabled - certificates are not used in white
        // flag flow
        fp_ensure!(
            !epoch_store.protocol_config().enable_white_flag_flow(),
            IotaError::UnsupportedFeature {
                error: "handle_certificate_v1 is disabled when white flag flow is enabled. Transactions go directly to consensus.".to_string()
            }
            .into()
        );

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
        total_size_bytes: u64,
    ) -> Result<(), tonic::Status> {
        let protocol_config = epoch_store.protocol_config();

        // Enforce these checks per [SIP-19](https://github.com/sui-foundation/sips/blob/main/sips/sip-19.md):
        // - All certs must access at least one shared object.
        // - All certs must not be already executed.
        // - All certs must have the same gas price.
        // - Number of certs must not exceed the max allowed.
        // - Total size of all certs must not exceed the max allowed.
        fp_ensure!(
            certificates.len() as u64 <= protocol_config.max_soft_bundle_size(),
            IotaError::UserInput {
                error: UserInputError::TooManyTransactionsInSoftBundle {
                    limit: protocol_config.max_soft_bundle_size()
                }
            }
            .into()
        );

        // We set the soft bundle max size to be half of the consensus max transactions
        // in block size. We do this to account for serialization overheads and
        // to ensure that the soft bundle is not too large when is attempted to be
        // posted via consensus. Although half the block size is on the extreme
        // side, it's should be good enough for now.
        let soft_bundle_max_size_bytes =
            protocol_config.consensus_max_transactions_in_block_bytes() / 2;
        fp_ensure!(
            total_size_bytes <= soft_bundle_max_size_bytes,
            IotaError::UserInput {
                error: UserInputError::SoftBundleTooLarge {
                    size: total_size_bytes,
                    limit: soft_bundle_max_size_bytes,
                },
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
                !self.state.try_is_tx_already_executed(&tx_digest)?,
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

        // Reject if white flag flow is enabled - certificates are not used in white
        // flag flow
        fp_ensure!(
            !epoch_store.protocol_config().enable_white_flag_flow(),
            IotaError::UnsupportedFeature {
                error: "handle_soft_bundle_certificates_v1 is disabled when white flag flow is enabled. Use batch submission via submit_transaction instead.".to_string()
            }
            .into()
        );

        let client_addr = if let Some(client_id_source) = &self.client_id_source {
            self.get_client_ip_addr(&request, client_id_source)
        } else {
            self.get_client_ip_addr(&request, &ClientIdSource::SocketAddr)
        };

        let request = request.into_inner();

        let certificates =
            NonEmpty::from_vec(request.certificates).ok_or(IotaError::NoCertificateProvided)?;
        let mut total_size_bytes = 0;
        for certificate in &certificates {
            // We need to check this first because we haven't verified the cert signature.
            total_size_bytes += certificate
                .validity_check(epoch_store.protocol_config(), epoch_store.epoch())?
                as u64;
        }

        self.metrics
            .handle_soft_bundle_certificates_count
            .observe(certificates.len() as f64);

        self.metrics
            .handle_soft_bundle_certificates_size_bytes
            .observe(total_size_bytes as f64);

        // Now that individual certificates are valid, we check if the bundle is valid.
        self.soft_bundle_validity_check(&certificates, &epoch_store, total_size_bytes)
            .await?;

        info!(
            "Received Soft Bundle with {} certificates, from {}, tx digests are [{}], total size [{}]bytes",
            certificates.len(),
            client_addr
                .map(|x| x.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            certificates
                .iter()
                .map(|x| x.digest().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            total_size_bytes
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
            .try_get_iota_system_state_object_unsafe()?;
        Ok((tonic::Response::new(response), Weight::one()))
    }

    async fn handle_capability_notification_v1_impl(
        &self,
        request: tonic::Request<HandleCapabilityNotificationRequestV1>,
    ) -> WrappedServiceResponse<HandleCapabilityNotificationResponseV1> {
        let epoch_store = self.state.load_epoch_store_one_call_per_task();
        let request = request.into_inner();
        fp_ensure!(
            epoch_store
                .protocol_config()
                .track_non_committee_eligible_validators(),
            IotaError::UnsupportedFeature {
                error: "capability notification endpoint is not supported in this Protocol Version"
                    .to_string()
            }
            .into()
        );
        // Validate if the capability notification can be handled.
        // Fullnode does not serve capability notification requests.
        fp_ensure!(
            !self.state.is_fullnode(&epoch_store),
            IotaError::FullNodeCantHandleAuthorityCapabilities.into()
        );

        // Check if the capabilities notification has already been processed
        let existing_capabilities = epoch_store.get_capabilities_v1()?;
        let incoming_capability = request.message.data();

        info!(
            "Received capability notification: {:?}",
            incoming_capability
        );

        if let Some(existing) = existing_capabilities
            .iter()
            .find(|cap| cap.authority == incoming_capability.authority)
        {
            if incoming_capability.generation <= existing.generation {
                // Return successfully if generation is lower or equal - already processed
                return Ok((
                    tonic::Response::new(HandleCapabilityNotificationResponseV1 { _unused: false }),
                    Weight::one(),
                ));
            }
        }
        if let Err(error) = self.consensus_adapter.check_consensus_overload() {
            self.metrics
                .num_rejected_capability_notifications_during_overload
                .with_label_values(&[error.as_ref()])
                .inc();
            return Err(error.into());
        }

        let _handle_tx_metrics_guard = self
            .metrics
            .handle_capability_notification_latency
            .start_timer();

        let signed_authority_capabilities = request.message;
        // Verify the message signature
        let verified_authority_capabilities = epoch_store
            .verify_authority_capabilities(signed_authority_capabilities)
            .inspect_err(|_e| {
                self.metrics.signature_errors.inc();
            })?;

        let authority_name = verified_authority_capabilities.authority;
        // Process the verified capabilities
        debug!("Verified capability notification for authority {authority_name:?}");

        // Submit the signed capability notification to consensus instead of processing
        // directly
        let signed_authority_capabilities_transaction =
            ConsensusTransaction::new_signed_capability_notification_v1(
                verified_authority_capabilities.into_inner(),
            );

        // Submit to consensus - similar to how certificates are handled
        self.consensus_adapter.submit(
            signed_authority_capabilities_transaction,
            None,
            &epoch_store,
        )?;

        debug!("Submitted capability notification to consensus for authority {authority_name:?}");

        Ok((
            tonic::Response::new(HandleCapabilityNotificationResponseV1 { _unused: false }),
            Weight::one(),
        ))
    }

    async fn handle_submit_transactions_impl(
        &self,
        request: tonic::Request<iota_types::messages_grpc::RawSubmitTransactionsRequest>,
    ) -> WrappedServiceResponse<iota_types::messages_grpc::RawSubmitTransactionsResponse> {
        use iota_types::messages_grpc::{
            RawSubmitTransactionResult, RawSubmitTransactionsResponse,
        };

        let Self {
            state,
            consensus_adapter,
            metrics,
            ..
        } = self.clone();
        let epoch_store = state.load_epoch_store_one_call_per_task();

        // Ensure not a fullnode.
        fp_ensure!(
            !state.is_fullnode(&epoch_store),
            IotaError::FullNodeCantHandleSubmitTransactions.into()
        );

        // Check feature flag.
        fp_ensure!(
            epoch_store.protocol_config().enable_white_flag_flow(),
            IotaError::UnsupportedFeature {
                error: "White flag flow is not enabled in this protocol version".to_string()
            }
            .into()
        );

        let raw_request = request.into_inner();

        // Handle ping (empty request → empty response).
        if raw_request.transactions.is_empty() {
            return Ok((
                tonic::Response::new(RawSubmitTransactionsResponse { results: vec![] }),
                Weight::zero(),
            ));
        }

        let tx_count = raw_request.transactions.len();

        // Pre-allocate a results vector aligned 1:1 with the input transactions.
        let mut results: Vec<Option<SubmitTransactionResult>> = vec![None; tx_count];

        // Deserialize, validate, and check overload in a single pass.
        // Per-tx failures (BCS, validity, overload) are stored per-tx rather
        // than aborting the whole batch, so valid transactions can still proceed.
        let mut valid_transactions: Vec<(usize, Transaction)> = Vec::with_capacity(tx_count);

        for (idx, tx_bytes) in raw_request.transactions.iter().enumerate() {
            let tx: Transaction = match bcs::from_bytes(tx_bytes) {
                Ok(tx) => tx,
                Err(e) => {
                    results[idx] = Some(SubmitTransactionResult::Rejected {
                        error: IotaError::TransactionSerialization {
                            error: e.to_string(),
                        },
                    });
                    continue;
                }
            };

            if let Err(e) = tx.validity_check(epoch_store.protocol_config(), epoch_store.epoch()) {
                results[idx] = Some(SubmitTransactionResult::Rejected { error: e });
                continue;
            }

            if let Err(e) = state.check_system_overload(
                &consensus_adapter,
                tx.data(),
                state.check_system_overload_at_signing(),
            ) {
                metrics
                    .num_rejected_tx_during_overload
                    .with_label_values(&[e.as_ref()])
                    .inc();
                results[idx] = Some(SubmitTransactionResult::Rejected { error: e });
                continue;
            }

            valid_transactions.push((idx, tx));
        }

        // Latency timer starts after pre-flight checks, mirroring handle_transaction
        // where the timer also starts after the overload check.
        let _handle_tx_metrics_guard = metrics.handle_transaction_latency.start_timer();

        // Process each valid transaction independently in parallel.
        let epoch_store_ref = &epoch_store;
        let futures = valid_transactions.into_iter().map(|(idx, tx)| async move {
            let result = self
                .handle_submit_transaction_impl(tx, epoch_store_ref)
                .await;
            (idx, result)
        });
        let futures_result = join_all(futures).await;

        // Fill in results for transactions that passed pre-flight checks.
        for (idx, result) in futures_result {
            results[idx] = Some(
                result.unwrap_or_else(|e| SubmitTransactionResult::Rejected { error: e.into() }),
            );
        }

        // Convert each native result to its Raw protobuf representation.
        let results: Vec<RawSubmitTransactionResult> = results
            .into_iter()
            .map(|r| r.expect("every transaction slot must be filled"))
            .map(|r| r.try_into())
            .collect::<Result<Vec<_>, IotaError>>()
            .map_err(|e| tonic::Status::internal(e.to_string()))?;

        Ok((
            tonic::Response::new(RawSubmitTransactionsResponse { results }),
            Weight::one(),
        ))
    }

    /// Handles submission of a single transaction. Validates, checks for prior
    /// execution, verifies signature, runs deny checks, and submits to
    /// consensus. Returns the per-transaction result.
    async fn handle_submit_transaction_impl(
        &self,
        transaction: Transaction,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> Result<SubmitTransactionResult, tonic::Status> {
        let state = &self.state;
        let consensus_adapter = &self.consensus_adapter;
        let metrics = &self.metrics;

        let tx_digest = *transaction.digest();

        // Helper to build an Executed result from transaction effects.
        let build_executed_result =
            |effects: TransactionEffects| -> Result<SubmitTransactionResult, tonic::Status> {
                let effects_digest = effects.digest();
                let events = if effects.events_digest().is_some() {
                    state
                        .get_transaction_events(effects.transaction_digest())
                        .ok()
                } else {
                    None
                };
                let input_objects = state.get_transaction_input_objects(&effects).ok();
                let output_objects = state.get_transaction_output_objects(&effects).ok();
                let details = Box::new(ExecutedData {
                    effects,
                    events,
                    input_objects: input_objects.unwrap_or_default(),
                    output_objects: output_objects.unwrap_or_default(),
                });
                Ok(SubmitTransactionResult::Executed {
                    effects_digest,
                    details,
                })
            };

        // Check if already executed.
        if let Some(effects) = state
            .get_transaction_cache_reader()
            .try_get_executed_effects(&tx_digest)
            .map_err(tonic::Status::from)?
        {
            return build_executed_result(effects);
        }

        // Verify user signature.
        let tx_verif_guard = metrics.tx_verification_latency.start_timer();
        let verified_tx = match epoch_store.verify_transaction(transaction.clone()) {
            Ok(verified) => verified,
            Err(e) => {
                metrics.signature_errors.inc();
                return Err(e.into());
            }
        };
        drop(tx_verif_guard);

        // Early bail-out during epoch boundary, before running expensive deny checks.
        if !epoch_store
            .get_reconfig_state_read_lock_guard()
            .should_accept_user_certs()
        {
            metrics.num_rejected_tx_in_epoch_boundary.inc();
            return Err(IotaError::ValidatorHaltedAtEpochEnd.into());
        }

        // Content validation: deny checks + owned object version validation.
        let owned_objects = state
            .handle_transaction_validation_checks(&verified_tx, epoch_store)
            .await
            .map_err(tonic::Status::from)?;
        if let Err(e) = state
            .get_cache_writer()
            .validate_owned_object_versions(&owned_objects)
        {
            // Check if the transaction was executed while being validated, and that's why
            // the owned object version validation failed. This is an edge
            // case so checking executed effects twice is acceptable.
            if let Some(effects) = state
                .get_transaction_cache_reader()
                .try_get_executed_effects(&tx_digest)?
            {
                return build_executed_result(effects);
            }
            return Err(tonic::Status::from(e));
        }

        // Reconfig check.
        let reconfiguration_lock = epoch_store.get_reconfig_state_read_lock_guard();
        if !reconfiguration_lock.should_accept_user_certs() {
            metrics.num_rejected_tx_in_epoch_boundary.inc();
            return Err(IotaError::ValidatorHaltedAtEpochEnd.into());
        }

        // Submit to consensus.
        consensus_adapter
            .submit(
                ConsensusTransaction::new_user_transaction(transaction),
                Some(&reconfiguration_lock),
                epoch_store,
            )
            .map_err(tonic::Status::from)?;

        Ok(SubmitTransactionResult::Submitted)
    }

    async fn handle_wait_for_effects_impl(
        &self,
        request: tonic::Request<iota_types::messages_grpc::RawWaitForEffectsRequest>,
    ) -> WrappedServiceResponse<iota_types::messages_grpc::RawWaitForEffectsResponse> {
        use iota_types::messages_grpc::{
            RawWaitForEffectResponse, RawWaitForEffectsResponse, WaitForEffectRequest,
        };

        let raw_request = request.into_inner();

        // Handle ping (empty request → empty response).
        if raw_request.requests.is_empty() {
            return Ok((
                tonic::Response::new(RawWaitForEffectsResponse { results: vec![] }),
                Weight::one(),
            ));
        }

        // Deserialize each item's fields lazily and construct native requests.
        let mut native_requests = Vec::with_capacity(raw_request.requests.len());
        for raw_item in &raw_request.requests {
            let transaction_digest =
                bcs::from_bytes(&raw_item.transaction_digest).map_err(|e| {
                    tonic::Status::invalid_argument(format!(
                        "Failed to deserialize transaction_digest: {e}"
                    ))
                })?;
            native_requests.push(WaitForEffectRequest {
                transaction_digest,
                include_details: raw_item.include_details,
            });
        }

        let futures = native_requests
            .into_iter()
            .map(|req| self.handle_wait_for_effect_impl(req));
        let native_results = join_all(futures).await;

        // Convert each native result to its Raw protobuf representation.
        let results: Vec<RawWaitForEffectResponse> = native_results
            .into_iter()
            .map(|r| r.try_into())
            .collect::<Result<Vec<_>, IotaError>>()
            .map_err(|e| tonic::Status::internal(e.to_string()))?;

        Ok((
            tonic::Response::new(RawWaitForEffectsResponse { results }),
            Weight::one(),
        ))
    }

    /// Handles a single wait-for-effects request. Waits for the transaction to
    /// be executed or dropped, and returns the per-item response.
    async fn handle_wait_for_effect_impl(
        &self,
        request: iota_types::messages_grpc::WaitForEffectRequest,
    ) -> iota_types::messages_grpc::WaitForEffectResponse {
        use iota_types::messages_grpc::WaitForEffectResponse;

        let tx_digest = request.transaction_digest;
        let epoch_store = self.state.load_epoch_store_one_call_per_task();

        // Wait for the transaction to be executed and get the effects digest.
        // Race against the transaction being dropped by white-flag conflict
        // resolution, which gives an immediate Rejected response instead of
        // waiting for the gRPC deadline.
        let cache = self.state.get_transaction_cache_reader();

        const WAIT_TIMEOUT_SECS: u64 = 30;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(WAIT_TIMEOUT_SECS),
            async {
                let digests_to_watch = [tx_digest];
                tokio::select! {
                    biased;
                    effects_digests = cache.notify_read_executed_effects_digests(&digests_to_watch) => {
                        Either::Left(effects_digests)
                    }
                    dropped_error = epoch_store.notify_read_dropped_digests(tx_digest) => {
                        Either::Right(dropped_error)
                    }
                }
            },
        )
            .await;

        // The select! produces Either<Vec<EffectsDigest>, IotaError>.
        // Unpack the three outcomes: executed, dropped, or timeout.
        match result {
            Ok(Either::Left(effects_digests)) => {
                if let Some(effects_digest) = effects_digests.into_iter().next() {
                    // Fetch detailed execution data if requested.
                    let details = if request.include_details {
                        if let Some(effects) = cache.get_executed_effects(&tx_digest) {
                            let events = if effects.events_digest().is_some() {
                                self.state
                                    .get_transaction_events(effects.transaction_digest())
                                    .ok()
                            } else {
                                None
                            };
                            let input_objects =
                                self.state.get_transaction_input_objects(&effects).ok();
                            let output_objects =
                                self.state.get_transaction_output_objects(&effects).ok();
                            Some(Box::new(ExecutedData {
                                effects,
                                events,
                                input_objects: input_objects.unwrap_or_default(),
                                output_objects: output_objects.unwrap_or_default(),
                            }))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    WaitForEffectResponse::Executed {
                        effects_digest,
                        details,
                    }
                } else {
                    // Empty effects list — should not happen, but treat as expired.
                    WaitForEffectResponse::Expired {
                        epoch: epoch_store.epoch(),
                    }
                }
            }
            Ok(Either::Right(dropped_error)) => {
                // Transaction was dropped by white-flag conflict resolution.
                WaitForEffectResponse::Rejected {
                    error: Some(dropped_error),
                }
            }
            Err(_timeout) => {
                // Timed out waiting for effects.
                WaitForEffectResponse::Expired {
                    epoch: epoch_store.epoch(),
                }
            }
        }
    }

    async fn handle_validator_health_impl(
        &self,
        _request: tonic::Request<iota_types::messages_grpc::RawValidatorHealthRequest>,
    ) -> WrappedServiceResponse<iota_types::messages_grpc::RawValidatorHealthResponse> {
        use iota_types::messages_grpc::ValidatorHealthResponse;

        let epoch_store = self.state.load_epoch_store_one_call_per_task();

        let last_locally_built_checkpoint = epoch_store
            .last_built_checkpoint_summary()
            .ok()
            .flatten()
            .map(|(seq, _)| seq)
            .unwrap_or(0);

        let typed_response = ValidatorHealthResponse {
            num_inflight_execution_transactions: self
                .state
                .transaction_manager()
                .inflight_queue_len() as u64,
            num_inflight_consensus_transactions: self.consensus_adapter.num_inflight_transactions(),
            last_locally_built_checkpoint,
        };

        let raw_response = typed_response
            .try_into()
            .map_err(|e: IotaError| tonic::Status::internal(e.to_string()))?;

        Ok((tonic::Response::new(raw_response), Weight::zero()))
    }
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

    async fn handle_capability_notification_v1(
        &self,
        request: tonic::Request<HandleCapabilityNotificationRequestV1>,
    ) -> Result<tonic::Response<HandleCapabilityNotificationResponseV1>, tonic::Status> {
        handle_with_decoration!(self, handle_capability_notification_v1_impl, request)
    }

    async fn handle_submit_transactions(
        &self,
        request: tonic::Request<iota_types::messages_grpc::RawSubmitTransactionsRequest>,
    ) -> Result<
        tonic::Response<iota_types::messages_grpc::RawSubmitTransactionsResponse>,
        tonic::Status,
    > {
        let validator_service = self.clone();
        spawn_monitored_task!(async move {
            handle_with_decoration!(validator_service, handle_submit_transactions_impl, request)
        })
        .await
        .unwrap()
    }

    async fn handle_wait_for_effects(
        &self,
        request: tonic::Request<iota_types::messages_grpc::RawWaitForEffectsRequest>,
    ) -> Result<tonic::Response<iota_types::messages_grpc::RawWaitForEffectsResponse>, tonic::Status>
    {
        let validator_service = self.clone();
        spawn_monitored_task!(async move {
            handle_with_decoration!(validator_service, handle_wait_for_effects_impl, request)
        })
        .await
        .unwrap()
    }

    async fn handle_validator_health(
        &self,
        request: tonic::Request<iota_types::messages_grpc::RawValidatorHealthRequest>,
    ) -> Result<tonic::Response<iota_types::messages_grpc::RawValidatorHealthResponse>, tonic::Status>
    {
        handle_with_decoration!(self, handle_validator_health_impl, request)
    }
}
