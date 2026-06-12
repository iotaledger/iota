// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `claim_registry` module.

#[cfg(msim)]
use iota_macros::sim_test;
#[cfg(msim)]
use iota_sdk_types::{ObjectId, Owner};
#[cfg(msim)]
use test_cluster::TestClusterBuilder;

// ---------------------------------------------------------------------------
// Feature-flag test (msim only)
// ---------------------------------------------------------------------------

/// Verify that `ClaimRegistry` creation is gated by the `enable_claim_registry`
/// feature flag, driving the flag at runtime rather than through a protocol
/// version upgrade.
///
/// While the flag is disabled the registry must not exist. Once enabled, the
/// `ClaimRegistry` is created by the `EndOfEpochTransaction` of the first epoch
/// that runs with the flag on, becoming visible at the start of the following
/// epoch.
#[cfg(msim)]
#[sim_test]
async fn test_claim_registry_created_when_flag_enabled() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use iota_protocol_config::ProtocolConfig;

    telemetry_subscribers::init_for_testing();

    // The override is re-applied whenever an epoch store is (re)created, so
    // flipping this flag at runtime takes effect from the next epoch onwards.
    let enable_claim_registry = Arc::new(AtomicBool::new(false));
    let _guard = {
        let enable_claim_registry = enable_claim_registry.clone();
        ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
            config.set_enable_claim_registry_for_testing(
                enable_claim_registry.load(Ordering::SeqCst),
            );
            config
        })
    };

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(20000)
        .build()
        .await;

    // Disabled: the registry must not exist at genesis...
    assert!(
        test_cluster
            .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
            .await
            .is_none(),
        "ClaimRegistry must NOT exist at genesis while the flag is disabled"
    );

    // ...nor after a full epoch has run with the flag still disabled.
    test_cluster.wait_for_epoch(Some(1)).await;
    assert!(
        test_cluster
            .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
            .await
            .is_none(),
        "ClaimRegistry must NOT exist while the flag is disabled"
    );

    // Enable the flag. The next epoch store picks up the new config, and the
    // registry is created by that epoch's end-of-epoch transaction, becoming
    // visible at the start of the following epoch.
    enable_claim_registry.store(true, Ordering::SeqCst);

    let mut registry = None;
    for target_epoch in 2..=5 {
        test_cluster.wait_for_epoch(Some(target_epoch)).await;
        if let Some(object) = test_cluster
            .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
            .await
        {
            registry = Some(object);
            break;
        }
    }

    let registry = registry.expect("ClaimRegistry must be created once the flag is enabled");
    assert!(
        matches!(registry.owner(), Owner::Shared { .. }),
        "ClaimRegistry must be a shared object; got {:?}",
        registry.owner()
    );
}
