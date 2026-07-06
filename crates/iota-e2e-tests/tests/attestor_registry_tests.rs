// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end test for the attestor registry lifecycle.
//!
//! Enables `enable_external_attestation` (+ its required
//! `enable_validator_attestation` and `enable_pcool_flow`).
//! Own binary so the process-wide env override does not race others.

use fastcrypto::encoding::{Encoding, Hex};
use iota_macros::sim_test;
use iota_types::{IOTA_SYSTEM_PACKAGE_ID, transaction::CallArg};
use test_cluster::TestClusterBuilder;

/// Sets protocol-config overrides via process-wide env vars for the duration
/// of the test, clearing them on drop. Must be constructed before the cluster
/// is built.
struct ProtocolEnvOverride {
    keys: Vec<&'static str>,
}

impl ProtocolEnvOverride {
    fn new(overrides: &[(&'static str, &'static str)]) -> Self {
        for (key, val) in overrides {
            #[allow(deprecated)]
            std::env::set_var(key, val);
        }
        Self {
            keys: overrides.iter().map(|(k, _)| *k).collect(),
        }
    }
}

impl Drop for ProtocolEnvOverride {
    fn drop(&mut self) {
        for key in &self.keys {
            #[allow(deprecated)]
            std::env::remove_var(key);
        }
    }
}

/// Registering an attestor lands it pending; after one epoch boundary it is
/// active in the epoch store's `AttestorSet`. Deregistering an active attestor
/// is deferred to the next boundary, after which it is removed from the set.
#[sim_test]
async fn test_attestor_registry_lifecycle() {
    telemetry_subscribers::init_for_testing();

    let _env = ProtocolEnvOverride::new(&[
        ("IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE", "1"),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_PCOOL_FLOW",
            "true",
        ),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_VALIDATOR_ATTESTATION",
            "true",
        ),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_EXTERNAL_ATTESTATION",
            "true",
        ),
    ]);

    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = test_cluster.get_address_0();

    // One gas coin for gas and a separate whole coin as the bond (each default
    // test coin far exceeds MIN_ATTESTOR_JOINING_BOND).
    let gas_objects = test_cluster
        .wallet
        .get_all_gas_objects_owned_by_address(sender)
        .await
        .unwrap();
    assert!(
        gas_objects.len() >= 2,
        "test account needs a separate gas and bond coin"
    );
    let gas = gas_objects[0];
    let bond = gas_objects[1];

    // A real `flag || raw_key` ed25519 public key (on-curve validated).
    let attestor_pubkey =
        Hex::decode("00d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f").unwrap();

    let tx_data = test_cluster
        .test_transaction_builder_with_gas_object(sender, gas)
        .await
        .move_call(
            IOTA_SYSTEM_PACKAGE_ID,
            "iota_system",
            "register_attestor",
            vec![
                CallArg::IOTA_SYSTEM_MUTABLE,
                CallArg::ImmutableOrOwned(bond),
                CallArg::pure(&attestor_pubkey),
            ],
        )
        .build();
    let tx = test_cluster.sign_transaction(&tx_data);
    test_cluster.execute_transaction(tx).await;

    // Pending until the boundary: the current epoch's set is still empty.
    let empty = test_cluster.fullnode_handle.iota_node.with(|node| {
        node.state()
            .epoch_store_for_testing()
            .attestor_set()
            .is_empty()
    });
    assert!(empty, "attestor must not be active before the epoch boundary");

    // Cross the boundary; the snapshot must now contain the attestor at index 0.
    test_cluster.force_new_epoch().await;

    let (len, indexed) = test_cluster.fullnode_handle.iota_node.with(|node| {
        let epoch_store = node.state().epoch_store_for_testing();
        let set = epoch_store.attestor_set();
        let indexed = set
            .by_address(&sender)
            .map(|(i, entry)| (i, entry.attestor_pubkey.clone()));
        (set.len(), indexed)
    });
    assert_eq!(len, 1, "attestor must be active after the boundary");
    let (index, pubkey) = indexed.expect("attestor not found in the active set");
    assert_eq!(index, 0);
    assert_eq!(pubkey, attestor_pubkey);

    // Deregister. For an active attestor this schedules removal at the next
    // boundary rather than taking effect immediately.
    let dereg_tx = test_cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .move_call(
            IOTA_SYSTEM_PACKAGE_ID,
            "iota_system",
            "deregister_attestor",
            vec![CallArg::IOTA_SYSTEM_MUTABLE],
        )
        .build();
    let dereg_tx = test_cluster.sign_transaction(&dereg_tx);
    test_cluster.execute_transaction(dereg_tx).await;

    // Still active this epoch; removal is deferred to the boundary.
    let still_active = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().epoch_store_for_testing().attestor_set().len());
    assert_eq!(
        still_active, 1,
        "deregistering an active attestor is deferred to the next boundary"
    );

    // Cross the boundary; the attestor is now removed.
    test_cluster.force_new_epoch().await;

    let removed = test_cluster.fullnode_handle.iota_node.with(|node| {
        let epoch_store = node.state().epoch_store_for_testing();
        let set = epoch_store.attestor_set();
        set.is_empty() && set.by_address(&sender).is_none()
    });
    assert!(removed, "attestor must be removed after the deregistration boundary");
}

/// With the stats feed live, an attestor whose valid attestations are
/// recorded every epoch keeps its slot, while an idle one is dropped with
/// the inactivity penalty once the window passes.
#[sim_test]
async fn test_attestor_inactivity_drop_via_stats() {
    telemetry_subscribers::init_for_testing();

    let _env = ProtocolEnvOverride::new(&[
        ("IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE", "1"),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_PCOOL_FLOW",
            "true",
        ),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_VALIDATOR_ATTESTATION",
            "true",
        ),
        (
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_EXTERNAL_ATTESTATION",
            "true",
        ),
    ]);

    // Devnet default from ProtocolConfig: attestor_max_inactivity_epochs.
    const MAX_INACTIVITY_EPOCHS: u64 = 7;

    let test_cluster = TestClusterBuilder::new().build().await;
    let active_addr = test_cluster.get_address_0();
    let idle_addr = test_cluster.get_address_1();

    // Register both attestors (same flow as the lifecycle test).
    for (addr, pubkey_hex) in [
        (
            active_addr,
            "00d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f",
        ),
        (
            idle_addr,
            "0102770632ba449f7f0f6d7e8173ee8cdeee0c1676a4f02a9c10b877b2c022126a1d",
        ),
    ] {
        let gas_objects = test_cluster
            .wallet
            .get_all_gas_objects_owned_by_address(addr)
            .await
            .unwrap();
        assert!(gas_objects.len() >= 2);
        let (gas, bond) = (gas_objects[0], gas_objects[1]);
        let pubkey = Hex::decode(pubkey_hex).unwrap();
        let tx_data = test_cluster
            .test_transaction_builder_with_gas_object(addr, gas)
            .await
            .move_call(
                IOTA_SYSTEM_PACKAGE_ID,
                "iota_system",
                "register_attestor",
                vec![
                    CallArg::IOTA_SYSTEM_MUTABLE,
                    CallArg::ImmutableOrOwned(bond),
                    CallArg::pure(&pubkey),
                ],
            )
            .build();
        let tx = test_cluster.sign_transaction(&tx_data);
        test_cluster.execute_transaction(tx).await;
    }

    // Activate both.
    test_cluster.force_new_epoch().await;

    let (active_index, len) = test_cluster.fullnode_handle.iota_node.with(|node| {
        let epoch_store = node.state().epoch_store_for_testing();
        let set = epoch_store.attestor_set();
        (set.by_address(&active_addr).map(|(i, _)| i), set.len())
    });
    assert_eq!(len, 2);
    let active_index = active_index.expect("active attestor must be in the set");

    // Run past the inactivity window, recording valid attestations for the
    // active attestor on every validator each epoch. Injection must be
    // identical on all validators to keep the end-of-epoch args identical.
    for _ in 0..=MAX_INACTIVITY_EPOCHS {
        for handle in test_cluster.swarm.validator_node_handles() {
            handle.with(|node| {
                node.state()
                    .epoch_store_for_testing()
                    .attestor_stats_aggregator()
                    .record_valid_attestation(active_index, 1);
            });
        }
        test_cluster.force_new_epoch().await;
    }

    let (still_active, idle_gone, remaining) =
        test_cluster.fullnode_handle.iota_node.with(|node| {
            let epoch_store = node.state().epoch_store_for_testing();
            let set = epoch_store.attestor_set();
            (
                set.by_address(&active_addr).is_some(),
                set.by_address(&idle_addr).is_none(),
                set.len(),
            )
        });
    assert!(still_active, "attested attestor must keep its slot");
    assert!(idle_gone, "idle attestor must be dropped after the window");
    assert_eq!(remaining, 1);
}
