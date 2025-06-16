// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use iota_metrics::histogram::{Histogram, HistogramVec};
use iota_types::{
    base_types::*,
    committee::*,
    crypto::AuthorityPublicKeyBytes,
    effects::{SignedTransactionEffects, TransactionEffectsAPI},
    error::{IotaError, IotaResult},
    fp_ensure,
    iota_system_state::IotaSystemState,
    messages_grpc::{
        HandleCertificateRequestV1, HandleCertificateResponseV1, ObjectInfoRequest,
        ObjectInfoResponse, SystemStateRequest, TransactionInfoRequest, TransactionStatus,
        VerifiedObjectInfoResponse,
    },
    messages_safe_client::PlainTransactionInfoResponse,
    transaction::*,
};
use prometheus::{
    IntCounterVec, Registry, core::GenericCounter, register_int_counter_vec_with_registry,
};
use tap::TapFallible;
use tracing::{debug, error, instrument};

use crate::{authority_client::AuthorityAPI, epoch::committee_store::CommitteeStore};

macro_rules! check_error {
    ($address:expr, $cond:expr, $msg:expr) => {
        $cond.tap_err(|err| {
            if err.individual_error_indicates_epoch_change() {
                debug!(?err, authority=?$address, "Not a real client error");
            } else {
                error!(?err, authority=?$address, $msg);
            }
        })
    }
}

#[derive(Clone)]
pub struct SafeClientMetricsBase {
    total_requests_by_address_method: IntCounterVec,
    total_responses_by_address_method: IntCounterVec,
    latency: HistogramVec,
}

impl SafeClientMetricsBase {
    pub fn new(registry: &Registry) -> Self {
        Self {
            total_requests_by_address_method: register_int_counter_vec_with_registry!(
                "safe_client_total_requests_by_address_method",
                "Total requests to validators group by address and method",
                &["address", "method"],
                registry,
            )
            .unwrap(),
            total_responses_by_address_method: register_int_counter_vec_with_registry!(
                "safe_client_total_responses_by_address_method",
                "Total good (OK) responses from validators group by address and method",
                &["address", "method"],
                registry,
            )
            .unwrap(),
            latency: HistogramVec::new_in_registry(
                "safe_client_latency",
                "RPC latency observed by safe client aggregator, group by address and method",
                &["address", "method"],
                registry,
            ),
        }
    }
}

/// Prometheus metrics which can be displayed in Grafana, queried and alerted on
#[derive(Clone)]
pub struct SafeClientMetrics {
    total_requests_handle_transaction_info_request: GenericCounter<prometheus::core::AtomicU64>,
    total_ok_responses_handle_transaction_info_request: GenericCounter<prometheus::core::AtomicU64>,
    total_requests_handle_object_info_request: GenericCounter<prometheus::core::AtomicU64>,
    total_ok_responses_handle_object_info_request: GenericCounter<prometheus::core::AtomicU64>,
    handle_transaction_latency: Histogram,
    handle_certificate_latency: Histogram,
    handle_obj_info_latency: Histogram,
    handle_tx_info_latency: Histogram,
}

impl SafeClientMetrics {
    pub fn new(metrics_base: &SafeClientMetricsBase, validator_address: AuthorityName) -> Self {
        let validator_address = validator_address.to_string();

        let total_requests_handle_transaction_info_request = metrics_base
            .total_requests_by_address_method
            .with_label_values(&[
                validator_address.as_str(),
                "handle_transaction_info_request",
            ]);
        let total_ok_responses_handle_transaction_info_request = metrics_base
            .total_responses_by_address_method
            .with_label_values(&[
                validator_address.as_str(),
                "handle_transaction_info_request",
            ]);

        let total_requests_handle_object_info_request = metrics_base
            .total_requests_by_address_method
            .with_label_values(&[validator_address.as_str(), "handle_object_info_request"]);
        let total_ok_responses_handle_object_info_request = metrics_base
            .total_responses_by_address_method
            .with_label_values(&[validator_address.as_str(), "handle_object_info_request"]);

        let handle_transaction_latency = metrics_base
            .latency
            .with_label_values(&[validator_address.as_str(), "handle_transaction"]);
        let handle_certificate_latency = metrics_base
            .latency
            .with_label_values(&[validator_address.as_str(), "handle_certificate"]);
        let handle_obj_info_latency = metrics_base
            .latency
            .with_label_values(&[validator_address.as_str(), "handle_object_info_request"]);
        let handle_tx_info_latency = metrics_base.latency.with_label_values(&[
            validator_address.as_str(),
            "handle_transaction_info_request",
        ]);

        Self {
            total_requests_handle_transaction_info_request,
            total_ok_responses_handle_transaction_info_request,
            total_requests_handle_object_info_request,
            total_ok_responses_handle_object_info_request,
            handle_transaction_latency,
            handle_certificate_latency,
            handle_obj_info_latency,
            handle_tx_info_latency,
        }
    }

    pub fn new_for_tests(validator_address: AuthorityName) -> Self {
        let registry = Registry::new();
        let metrics_base = SafeClientMetricsBase::new(&registry);
        Self::new(&metrics_base, validator_address)
    }
}

/// See `SafeClientMetrics::new` for description of each metrics.
/// The metrics are per validator client.
#[derive(Clone)]
pub struct SafeClient<C>
where
    C: Clone,
{
    authority_client: C,
    committee_store: Arc<CommitteeStore>,
    address: AuthorityPublicKeyBytes,
    metrics: SafeClientMetrics,
}

impl<C: Clone> SafeClient<C> {
    pub fn new(
        authority_client: C,
        committee_store: Arc<CommitteeStore>,
        address: AuthorityPublicKeyBytes,
        metrics: SafeClientMetrics,
    ) -> Self {
        Self {
            authority_client,
            committee_store,
            address,
            metrics,
        }
    }
}

impl<C: Clone> SafeClient<C> {
    pub fn authority_client(&self) -> &C {
        &self.authority_client
    }

    #[cfg(test)]
    pub fn authority_client_mut(&mut self) -> &mut C {
        &mut self.authority_client
    }

    fn get_committee(&self, epoch_id: &EpochId) -> IotaResult<Arc<Committee>> {
        self.committee_store
            .get_committee(epoch_id)?
            .ok_or(IotaError::MissingCommitteeAtEpoch(*epoch_id))
    }

    fn check_signed_effects_plain(
        &self,
        digest: &TransactionDigest,
        signed_effects: SignedTransactionEffects,
        expected_effects_digest: Option<&TransactionEffectsDigest>,
    ) -> IotaResult<SignedTransactionEffects> {
        // Check it has the right signer
        fp_ensure!(
            signed_effects.auth_sig().authority == self.address,
            IotaError::ByzantineAuthoritySuspicion {
                authority: self.address,
                reason: format!(
                    "Unexpected validator address in the signed effects signature: {:?}",
                    signed_effects.auth_sig().authority
                ),
            }
        );
        // Checks it concerns the right tx
        fp_ensure!(
            signed_effects.data().transaction_digest() == digest,
            IotaError::ByzantineAuthoritySuspicion {
                authority: self.address,
                reason: "Unexpected tx digest in the signed effects".to_string()
            }
        );
        // check that the effects digest is correct.
        if let Some(effects_digest) = expected_effects_digest {
            fp_ensure!(
                signed_effects.digest() == effects_digest,
                IotaError::ByzantineAuthoritySuspicion {
                    authority: self.address,
                    reason: "Effects digest does not match with expected digest".to_string()
                }
            );
        }
        self.get_committee(&signed_effects.epoch())?;
        Ok(signed_effects)
    }

    fn check_transaction_info(
        &self,
        digest: &TransactionDigest,
        transaction: Transaction,
        status: TransactionStatus,
    ) -> IotaResult<PlainTransactionInfoResponse> {
        fp_ensure!(
            digest == transaction.digest(),
            IotaError::ByzantineAuthoritySuspicion {
                authority: self.address,
                reason: "Signed transaction digest does not match with expected digest".to_string()
            }
        );
        match status {
            TransactionStatus::Signed(signed) => {
                self.get_committee(&signed.epoch)?;
                Ok(PlainTransactionInfoResponse::Signed(
                    SignedTransaction::new_from_data_and_sig(transaction.into_data(), signed),
                ))
            }
            TransactionStatus::Executed(cert_opt, effects, events) => {
                let signed_effects = self.check_signed_effects_plain(digest, effects, None)?;
                match cert_opt {
                    Some(cert) => {
                        let committee = self.get_committee(&cert.epoch)?;
                        let ct = CertifiedTransaction::new_from_data_and_sig(
                            transaction.into_data(),
                            cert,
                        );
                        ct.verify_committee_sigs_only(&committee).map_err(|e| {
                            IotaError::FailedToVerifyTxCertWithExecutedEffects {
                                validator_name: self.address,
                                error: e.to_string(),
                            }
                        })?;
                        Ok(PlainTransactionInfoResponse::ExecutedWithCert(
                            ct,
                            signed_effects,
                            events,
                        ))
                    }
                    None => Ok(PlainTransactionInfoResponse::ExecutedWithoutCert(
                        transaction,
                        signed_effects,
                        events,
                    )),
                }
            }
        }
    }

    fn check_object_response(
        &self,
        request: &ObjectInfoRequest,
        response: ObjectInfoResponse,
    ) -> IotaResult<VerifiedObjectInfoResponse> {
        let ObjectInfoResponse {
            object,
            layout: _,
            lock_for_debugging: _,
        } = response;

        fp_ensure!(
            request.object_id == object.id(),
            IotaError::ByzantineAuthoritySuspicion {
                authority: self.address,
                reason: "Object id mismatch in the response".to_string()
            }
        );

        Ok(VerifiedObjectInfoResponse { object })
    }

    pub fn address(&self) -> &AuthorityPublicKeyBytes {
        &self.address
    }
}

impl<C> SafeClient<C>
where
    C: AuthorityAPI + Send + Sync + Clone + 'static,
{
    /// Initiate a new transfer to an IOTA or Primary account.
    pub async fn handle_transaction(
        &self,
        transaction: Transaction,
        client_addr: Option<SocketAddr>,
    ) -> Result<PlainTransactionInfoResponse, IotaError> {
        let _timer = self.metrics.handle_transaction_latency.start_timer();
        let digest = *transaction.digest();
        let response = self
            .authority_client
            .handle_transaction(transaction.clone(), client_addr)
            .await?;
        let response = check_error!(
            self.address,
            self.check_transaction_info(&digest, transaction, response.status),
            "Client error in handle_transaction"
        )?;
        Ok(response)
    }

    fn verify_certificate_response_v1(
        &self,
        digest: &TransactionDigest,
        HandleCertificateResponseV1 {
            signed_effects,
            events,
            input_objects,
            output_objects,
            auxiliary_data,
        }: HandleCertificateResponseV1,
    ) -> IotaResult<HandleCertificateResponseV1> {
        let signed_effects = self.check_signed_effects_plain(digest, signed_effects, None)?;

        // Check Events
        match (&events, signed_effects.events_digest()) {
            (None, None) | (None, Some(_)) => {}
            (Some(events), None) => {
                if !events.data.is_empty() {
                    return Err(IotaError::ByzantineAuthoritySuspicion {
                        authority: self.address,
                        reason: "Returned events but no event digest present in the signed effects"
                            .to_string(),
                    });
                }
            }
            (Some(events), Some(events_digest)) => {
                fp_ensure!(
                    &events.digest() == events_digest,
                    IotaError::ByzantineAuthoritySuspicion {
                        authority: self.address,
                        reason: "Returned events don't match events digest in the signed effects"
                            .to_string()
                    }
                );
            }
        }

        // Check Input Objects
        if let Some(input_objects) = &input_objects {
            let expected: HashMap<_, _> = signed_effects
                .old_object_metadata()
                .into_iter()
                .map(|(object_ref, _owner)| (object_ref.0, object_ref))
                .collect();

            for object in input_objects {
                let object_ref = object.compute_object_reference();
                if expected
                    .get(&object_ref.0)
                    .is_none_or(|expect| &object_ref != expect)
                {
                    return Err(IotaError::ByzantineAuthoritySuspicion {
                        authority: self.address,
                        reason: "Returned input object that wasn't present in the signed effects"
                            .to_string(),
                    });
                }
            }
        }

        // Check Output Objects
        if let Some(output_objects) = &output_objects {
            let expected: HashMap<_, _> = signed_effects
                .all_changed_objects()
                .into_iter()
                .map(|(object_ref, _, _)| (object_ref.0, object_ref))
                .collect();

            for object in output_objects {
                let object_ref = object.compute_object_reference();
                if expected
                    .get(&object_ref.0)
                    .is_none_or(|expect| &object_ref != expect)
                {
                    return Err(IotaError::ByzantineAuthoritySuspicion {
                        authority: self.address,
                        reason: "Returned output object that wasn't present in the signed effects"
                            .to_string(),
                    });
                }
            }
        }

        Ok(HandleCertificateResponseV1 {
            signed_effects,
            events,
            input_objects,
            output_objects,
            auxiliary_data,
        })
    }

    /// Execute a certificate.
    pub async fn handle_certificate_v1(
        &self,
        request: HandleCertificateRequestV1,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleCertificateResponseV1, IotaError> {
        let digest = *request.certificate.digest();
        let _timer = self.metrics.handle_certificate_latency.start_timer();
        let response = self
            .authority_client
            .handle_certificate_v1(request, client_addr)
            .await?;

        let verified = check_error!(
            self.address,
            self.verify_certificate_response_v1(&digest, response),
            "Client error in handle_certificate"
        )?;
        Ok(verified)
    }

    pub async fn handle_object_info_request(
        &self,
        request: ObjectInfoRequest,
    ) -> Result<VerifiedObjectInfoResponse, IotaError> {
        self.metrics.total_requests_handle_object_info_request.inc();

        let _timer = self.metrics.handle_obj_info_latency.start_timer();
        let response = self
            .authority_client
            .handle_object_info_request(request.clone())
            .await?;
        let response = self
            .check_object_response(&request, response)
            .tap_err(|err| error!(?err, authority=?self.address, "Client error in handle_object_info_request"))?;

        self.metrics
            .total_ok_responses_handle_object_info_request
            .inc();
        Ok(response)
    }

    /// Handle Transaction information requests for a given digest.
    #[instrument(level = "trace", skip_all, fields(authority = ?self.address.concise()))]
    pub async fn handle_transaction_info_request(
        &self,
        request: TransactionInfoRequest,
    ) -> Result<PlainTransactionInfoResponse, IotaError> {
        self.metrics
            .total_requests_handle_transaction_info_request
            .inc();

        let _timer = self.metrics.handle_tx_info_latency.start_timer();

        let transaction_info = self
            .authority_client
            .handle_transaction_info_request(request.clone())
            .await?;

        let transaction = Transaction::new(transaction_info.transaction);
        let transaction_info = self.check_transaction_info(
            &request.transaction_digest,
            transaction,
            transaction_info.status,
        ).tap_err(|err| {
            error!(?err, authority=?self.address, "Client error in handle_transaction_info_request");
        })?;
        self.metrics
            .total_ok_responses_handle_transaction_info_request
            .inc();
        Ok(transaction_info)
    }

    #[instrument(level = "trace", skip_all, fields(authority = ?self.address.concise()))]
    pub async fn handle_system_state_object(&self) -> Result<IotaSystemState, IotaError> {
        self.authority_client
            .handle_system_state_object(SystemStateRequest { _unused: false })
            .await
    }
}
