// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use prometheus_filtered::{
    Gauge, Histogram, IntCounter, IntCounterVec, IntGauge, MetricLevel, Registry,
    register_gauge_with_registry, register_histogram_with_registry,
    register_int_counter_vec_with_registry, register_int_counter_with_registry,
    register_int_gauge_with_registry,
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
    pub num_rejected_tx_soft_lock_conflict: IntCounter,
    pub soft_lock_table_size: IntGauge,
    pub num_rejected_tx_recently_resubmitted: IntCounter,

    /// Latency of `attest_transaction` (the pre-consensus dry-run) for
    /// `UserTransactionV2` transactions. `tx_verification_latency` covers only
    /// signature verification, so this isolates the attestation cost. Spans the
    /// whole `spawn_blocking` call: pool wait + dry-run execution. The two
    /// metrics below split that total into its parts.
    pub validator_attestation_latency: Histogram,
    /// Time an attestation dry-run waits on the `spawn_blocking` pool before a
    /// worker starts running it (queue wait only — no execution).
    pub validator_attestation_queue_wait: Histogram,
    /// Wall-clock of the attestation Move-VM dry-run itself (the
    /// `spawn_blocking` closure body), excluding the pool wait above.
    pub validator_attestation_execution_latency: Histogram,
    /// Number of attestations performed (dry-runs that completed without
    /// panicking). Pairs with the latency to give CPU-per-attestation / rate.
    pub validator_attestations_total: IntCounter,
    /// Number of attestation dry-runs that panicked (surfaced as a `JoinError`
    /// in the spawned task). A robustness signal for the attestation path.
    pub validator_attestation_task_panics: IntCounter,
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
                registry;
                MetricLevel::Info,
            )
                .unwrap(),
            handle_certificate_consensus_latency: register_histogram_with_registry!(
                "validator_service_handle_certificate_consensus_latency",
                "Latency of handling a consensus transaction certificate",
                iota_metrics::COARSE_LATENCY_SEC_BUCKETS.to_vec(),
                registry;
                MetricLevel::Warn,
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
                registry;
                MetricLevel::Warn,
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
                registry;
                MetricLevel::Info,
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
            num_rejected_tx_soft_lock_conflict: register_int_counter_with_registry!(
                "validator_service_num_rejected_tx_soft_lock_conflict",
                "Number of transactions rejected due to pre-consensus soft lock conflict on owned objects",
                registry,
            )
                .unwrap(),
            num_rejected_tx_recently_resubmitted: register_int_counter_with_registry!(
                "validator_service_num_rejected_tx_recently_resubmitted",
                "Number of transactions rejected as duplicate resubmissions of a transaction whose soft locks are still held",
                registry,
            )
                .unwrap(),
            soft_lock_table_size: register_int_gauge_with_registry!(
                "validator_service_soft_lock_table_size",
                "Current number of object refs held in the pre-consensus soft lock table",
                registry,
            )
                .unwrap(),
            validator_attestation_latency: register_histogram_with_registry!(
                "validator_attestation_latency",
                "Latency of attest_transaction (the pre-consensus dry-run) for UserTransactionV2 transactions; spans spawn_blocking pool wait + dry-run execution",
                iota_metrics::SUBSECOND_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            validator_attestation_queue_wait: register_histogram_with_registry!(
                "validator_attestation_queue_wait",
                "Time an attestation dry-run waits on the spawn_blocking pool before a worker starts it (queue wait only)",
                iota_metrics::SUBSECOND_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            validator_attestation_execution_latency: register_histogram_with_registry!(
                "validator_attestation_execution_latency",
                "Wall-clock of the attestation Move-VM dry-run itself (spawn_blocking closure body), excluding pool wait",
                iota_metrics::SUBSECOND_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            validator_attestations_total: register_int_counter_with_registry!(
                "validator_attestations_total",
                "Number of attestations performed (dry-runs that completed without panicking)",
                registry,
            )
                .unwrap(),
            validator_attestation_task_panics: register_int_counter_with_registry!(
                "validator_attestation_task_panics",
                "Number of attestation dry-runs that panicked (surfaced as a JoinError)",
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
