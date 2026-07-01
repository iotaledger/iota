// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end test for the attestor registry lifecycle.
//!
//! Enables `enable_validator_attestation` (+ its required `enable_pcool_flow`).
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
