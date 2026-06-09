// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `claim_registry` module.

#[cfg(msim)]
use iota_macros::sim_test;
#[cfg(msim)]
use iota_sdk_types::{ObjectId, Owner};
#[cfg(msim)]
use iota_types::iota_system_state::IotaSystemStateTrait;
#[cfg(msim)]
use test_cluster::TestClusterBuilder;

// ---------------------------------------------------------------------------
// Protocol-upgrade test (msim only)
// ---------------------------------------------------------------------------

/// Verify that `ClaimRegistry` is created via `EndOfEpochTransaction` during
/// the first epoch that runs protocol v26 (where `enable_claim_registry` first
/// activates). The object is created at the end of that epoch, so it becomes
/// visible at the start of the following epoch.
///
/// Timeline:
///   epoch 0 (v25) → epoch 1 (v26, no registry yet) → epoch 2 (registry exists)
#[cfg(msim)]
#[sim_test]
async fn test_claim_registry_created_on_protocol_upgrade() {
    use iota_protocol_config::ProtocolVersion;
    use iota_types::supported_protocol_versions::SupportedProtocolVersions;

    telemetry_subscribers::init_for_testing();

    const PRE: u64 = 28;
    const POST: u64 = 29;

    let test_cluster = TestClusterBuilder::new()
        .with_protocol_version(ProtocolVersion::new(PRE))
        .with_epoch_duration_ms(20000)
        .with_supported_protocol_versions(SupportedProtocolVersions::new_for_testing(PRE, POST))
        .build()
        .await;

    assert!(
        test_cluster
            .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
            .await
            .is_none(),
        "ClaimRegistry must NOT exist at genesis (protocol v{PRE})"
    );

    // Epoch 1: protocol upgrade to v26 has happened, but ClaimRegistry is
    // created at the *end* of this epoch (first epoch running v26).
    let system_state = test_cluster.wait_for_epoch(Some(1)).await;
    assert_eq!(
        system_state.protocol_version(),
        POST,
        "Expected protocol version {POST} after epoch 1"
    );
    assert!(
        test_cluster
            .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
            .await
            .is_none(),
        "ClaimRegistry must NOT exist yet at the start of epoch 1"
    );

    // Epoch 2: ClaimRegistry was created at the end of epoch 1.
    test_cluster.wait_for_epoch(Some(2)).await;

    let reg = test_cluster
        .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
        .await
        .expect("ClaimRegistry must exist at the start of epoch 2");
    assert!(
        matches!(reg.owner(), Owner::Shared { .. }),
        "ClaimRegistry must be a shared object; got {:?}",
        reg.owner()
    );
}
