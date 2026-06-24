// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use prometheus::{
    Gauge, Histogram, IntCounter, IntCounterVec, Registry, register_gauge_with_registry,
    register_histogram_with_registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry,
};

/// Metrics for the validator service.
pub struct ValidatorServiceMetrics {
    pub signature_errors: IntCounter,
    pub tx_verification_latency: Histogram,
    pub cert_verification_latency: Histogram,
    pub consensus_latency: Histogram,
    pub handle_transaction_latency: Histogram,
    pub submit_certificate_consensus_latency: Histogram,
    pub handle_certificate_consensus_latency: Histogram,
    pub handle_certificate_non_consensus_latency: Histogram,
    pub handle_soft_bundle_certificates_consensus_latency: Histogram,
    pub handle_soft_bundle_certificates_count: Histogram,
    pub handle_soft_bundle_certificates_size_bytes: Histogram,
    pub handle_capability_notification_latency: Histogram,

    pub num_rejected_tx_in_epoch_boundary: IntCounter,
    pub num_rejected_cert_in_epoch_boundary: IntCounter,
    pub num_rejected_tx_during_overload: IntCounterVec,
    pub num_rejected_cert_during_overload: IntCounterVec,
    pub num_rejected_capability_notifications_during_overload: IntCounterVec,
    pub connection_ip_not_found: IntCounter,
    pub forwarded_header_parse_error: IntCounter,
    pub forwarded_header_invalid: IntCounter,
    pub forwarded_header_not_included: IntCounter,
    pub client_id_source_config_mismatch: IntCounter,
    pub x_forwarded_for_num_hops: Gauge,
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
            tx_verification_latency: register_histogram_with_registry!(
                "validator_service_tx_verification_latency",
                "Latency of verifying a transaction",
                iota_metrics::SUBSECOND_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            cert_verification_latency: register_histogram_with_registry!(
                "validator_service_cert_verification_latency",
                "Latency of verifying a certificate",
                iota_metrics::SUBSECOND_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            consensus_latency: register_histogram_with_registry!(
                "validator_service_consensus_latency",
                "Time spent between submitting a shared obj txn to consensus and getting result",
                iota_metrics::SUBSECOND_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            handle_transaction_latency: register_histogram_with_registry!(
                "validator_service_handle_transaction_latency",
                "Latency of handling a transaction",
                iota_metrics::SUBSECOND_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            handle_certificate_consensus_latency: register_histogram_with_registry!(
                "validator_service_handle_certificate_consensus_latency",
                "Latency of handling a consensus transaction certificate",
                iota_metrics::COARSE_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            submit_certificate_consensus_latency: register_histogram_with_registry!(
                "validator_service_submit_certificate_consensus_latency",
                "Latency of submit_certificate RPC handler",
                iota_metrics::COARSE_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            handle_certificate_non_consensus_latency: register_histogram_with_registry!(
                "validator_service_handle_certificate_non_consensus_latency",
                "Latency of handling a non-consensus transaction certificate",
                iota_metrics::SUBSECOND_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            handle_soft_bundle_certificates_consensus_latency: register_histogram_with_registry!(
                "validator_service_handle_soft_bundle_certificates_consensus_latency",
                "Latency of handling a consensus soft bundle",
                iota_metrics::COARSE_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            handle_soft_bundle_certificates_count: register_histogram_with_registry!(
                "validator_service_handle_soft_bundle_certificates_count",
                "The number of certificates included in a soft bundle",
                iota_metrics::COUNT_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            handle_soft_bundle_certificates_size_bytes: register_histogram_with_registry!(
                "validator_service_handle_soft_bundle_certificates_size_bytes",
                "The size of soft bundle in bytes",
                iota_metrics::BYTES_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            handle_capability_notification_latency: register_histogram_with_registry!(
                "validator_service_handle_capability_notification_latency",
                "Latency of handling a capability notification",
                iota_metrics::SUBSECOND_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
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
            num_rejected_capability_notifications_during_overload: register_int_counter_vec_with_registry!(
                "num_rejected_capability_notifications_during_overload",
                "Number of rejected capability notifications from non-committee active validators due to system overload",
                &["error_type"],
                registry,
            )
                .unwrap(),
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
            client_id_source_config_mismatch: register_int_counter_with_registry!(
                "validator_service_client_id_source_config_mismatch",
                "Number of times detected that client id source config doesn't agree with x-forwarded-for header",
                registry,
            )
                .unwrap(),
            x_forwarded_for_num_hops: register_gauge_with_registry!(
                "validator_service_x_forwarded_for_num_hops",
                "Number of hops in x-forwarded-for header",
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
