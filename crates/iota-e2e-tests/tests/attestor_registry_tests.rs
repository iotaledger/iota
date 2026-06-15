// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the attestor registry lifecycle.
//!
//! This file is intentionally separate from `attestation_tests.rs`: that file
//! sets `enable_white_flag_flow` process-wide via `ProtocolEnvOverride`, which
//! would leak into these tests (env vars are process-global) and route
//! execution through the WFF path that lacks a `QuorumDriverHandler`. Keeping
//! the registry tests in their own test binary isolates them from that env
//! mutation. The registry feature gate is currently a mock that is always
//! enabled, so no protocol overrides are needed here.

use iota_macros::sim_test;
use iota_types::{base_types::ObjectID, transaction::CallArg};
use test_cluster::TestClusterBuilder;

/// Registering an attestor lands it pending; after one epoch boundary it is
/// active and exposed through the epoch store's `AttestorSet` at index 0.
#[sim_test]
async fn test_attestor_registry_lifecycle() {
    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = test_cluster.get_address_0();

    // Use one gas coin for gas and a separate whole coin as the bond (each
    // default test coin far exceeds MIN_ATTESTOR_JOINING_BOND).
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

    // flag byte (ed25519 = 0x00) || 32-byte key
    let mut attestor_pubkey = vec![0u8];
    attestor_pubkey.extend_from_slice(&[0xCD; 32]);

    let tx_data = test_cluster
        .test_transaction_builder_with_gas_object(sender, gas)
        .await
        .move_call(
            ObjectID::SYSTEM,
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
    // Asserts success internally.
    test_cluster.execute_transaction(tx).await;

    // Pending until the boundary: the current epoch's set is still empty.
    let empty = test_cluster.fullnode_handle.iota_node.with(|node| {
        node.state()
            .epoch_store_for_testing()
            .attestor_set()
            .is_empty()
    });
    assert!(
        empty,
        "attestor must not be active before the epoch boundary"
    );

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
}
