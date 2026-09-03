// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end test for the attestor registry lifecycle.
//!
//! Enables `enable_external_attestation` (+ its required
//! `enable_validator_attestation` and `enable_pcool_flow`).
//! Own binary so the process-wide env override does not race others.

use iota_macros::sim_test;
use iota_sdk_crypto::{ed25519::Ed25519PrivateKey, simple::SimpleKeypair};
use iota_types::{
    IOTA_SYSTEM_PACKAGE_ID,
    crypto::{PublicKey, get_key_pair_from_rng},
    iota_system_state::attestor_registry::{
        generate_attestor_proof_of_possession, get_attestor_metadata,
    },
    transaction::CallArg,
};
use rand::{SeedableRng, rngs::StdRng};
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

    // A dedicated attestor signing key with a proof of possession bound to
    // the registering account.
    let attestor_keypair = SimpleKeypair::from(
        get_key_pair_from_rng::<Ed25519PrivateKey, _>(&mut StdRng::from_seed([7; 32])).1,
    );
    let pk = PublicKey::from(&attestor_keypair);
    let mut attestor_pubkey = vec![pk.flag()];
    attestor_pubkey.extend_from_slice(pk.as_ref());
    let proof_of_possession = generate_attestor_proof_of_possession(&attestor_keypair, sender);

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
                CallArg::pure(&proof_of_possession),
                CallArg::pure(&b"attestor-one".to_vec()),
                CallArg::pure(&b"an attestor".to_vec()),
                CallArg::pure(&b"https://example.com".to_vec()),
                CallArg::pure(&b"https://example.com/logo.png".to_vec()),
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

    let metadata = test_cluster.fullnode_handle.iota_node.with(|node| {
        get_attestor_metadata(node.state().get_object_store().as_ref(), sender).unwrap()
    });
    let metadata = metadata.expect("registered attestor must have metadata");
    assert_eq!(metadata.name, "attestor-one");
    assert_eq!(metadata.url, "https://example.com");

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
    assert!(
        removed,
        "attestor must be removed after the deregistration boundary"
    );

    let metadata_gone = test_cluster.fullnode_handle.iota_node.with(|node| {
        get_attestor_metadata(node.state().get_object_store().as_ref(), sender)
            .unwrap()
            .is_none()
    });
    assert!(metadata_gone, "metadata must be removed with the attestor");
}
