// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `claim_registry` module.

#[cfg(msim)]
use iota_macros::sim_test;
#[cfg(msim)]
use iota_types::{IOTA_CLAIM_REGISTRY_OBJECT_ID, object::Owner};
#[cfg(msim)]
use test_cluster::TestClusterBuilder;

// ---------------------------------------------------------------------------
// Protocol-upgrade test (msim only)
// ---------------------------------------------------------------------------

/// Verify that `ClaimRegistry` is created via `EndOfEpochTransaction` when a
/// network started at protocol v25 upgrades to v26 (where
/// `enable_claim_registry` first activates).
#[cfg(msim)]
#[sim_test]
async fn test_claim_registry_created_on_protocol_upgrade() {
    use iota_protocol_config::ProtocolVersion;
    use iota_types::supported_protocol_versions::SupportedProtocolVersions;

    telemetry_subscribers::init_for_testing();

    const PRE: u64 = 25;
    const POST: u64 = 26;

    let test_cluster = TestClusterBuilder::new()
        .with_protocol_version(ProtocolVersion::new(PRE))
        .with_epoch_duration_ms(20000)
        .with_supported_protocol_versions(SupportedProtocolVersions::new_for_testing(PRE, POST))
        .build()
        .await;

    assert!(
        test_cluster
            .get_object_from_fullnode_store(&IOTA_CLAIM_REGISTRY_OBJECT_ID)
            .await
            .is_none(),
        "ClaimRegistry must NOT exist at genesis (protocol v{PRE})"
    );

    let system_state = test_cluster.wait_for_epoch(Some(1)).await;
    assert_eq!(
        system_state.protocol_version(),
        POST,
        "Expected protocol version {POST} after epoch 1"
    );

    let reg = test_cluster
        .get_object_from_fullnode_store(&IOTA_CLAIM_REGISTRY_OBJECT_ID)
        .await
        .expect("ClaimRegistry must exist after upgrade to protocol v{POST}");
    assert!(
        matches!(reg.owner(), Owner::Shared { .. }),
        "ClaimRegistry must be a shared object; got {:?}",
        reg.owner()
    );
}
