// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the attestor registry lifecycle and the external
//! attestation ingress (`ValidatorV2::SubmitExternallyAttestedTx`).
//!
//! Enables `enable_external_attestation` (+ its required
//! `enable_validator_attestation` and `enable_pcool_flow`).
//! Own binary so the process-wide env override does not race others.

use std::{net::SocketAddr, time::Duration};

use iota_core::authority_client::{NetworkAuthorityClient, validator_v2::ValidatorV2API};
use iota_macros::sim_test;
use iota_sdk_crypto::{ed25519::Ed25519PrivateKey, simple::SimpleKeypair};
use iota_sdk_types::{Address, ExecutionStatus, TransactionDigest, TransactionEffects};
use iota_types::{
    IOTA_SYSTEM_PACKAGE_ID,
    attestation::{Attestation, AttestationData, AttestedTransaction},
    crypto::get_key_pair_from_rng,
    effects::TransactionEffectsAPI,
    error::IotaError,
    iota_system_state::attestor_registry::{
        attestor_pubkey_bytes, generate_attestor_proof_of_possession, get_attestor_metadata,
    },
    messages_grpc::TxStatusUpdate,
    transaction::CallArg,
};
use rand::{SeedableRng, rngs::StdRng};
use test_cluster::{TestCluster, TestClusterBuilder};

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

/// Enables external attestation and its prerequisites for every node of a
/// cluster built while the guard is alive.
fn enable_external_attestation_env() -> ProtocolEnvOverride {
    ProtocolEnvOverride::new(&[
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
    ])
}

fn keypair_from_seed(seed: u8) -> SimpleKeypair {
    SimpleKeypair::from(
        get_key_pair_from_rng::<Ed25519PrivateKey, _>(&mut StdRng::from_seed([seed; 32])).1,
    )
}

/// A signed transfer from the cluster's second account, attested by
/// `attestor_address` with `keypair`, claiming the protocol floor in units.
async fn attested_transfer(
    test_cluster: &TestCluster,
    amount: u64,
    attestor_address: Address,
    keypair: &SimpleKeypair,
) -> AttestedTransaction {
    let sender = test_cluster.get_addresses()[1];
    let tx_data = test_cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .transfer_iota(Some(amount), Address::random())
        .build();
    let tx = test_cluster.sign_transaction(&tx_data);
    let floor = test_cluster.fullnode_handle.iota_node.with(|node| {
        let protocol_config = node
            .state()
            .epoch_store_for_testing()
            .protocol_config()
            .clone();
        protocol_config
            .base_tx_cost_fixed()
            .min(protocol_config.gas_rounding_step())
    });
    let payload = AttestationData::V1 {
        computation_units: floor,
        object_versions: vec![],
    };
    let attestation = Attestation::new_explicit(tx.digest(), payload, attestor_address, keypair);
    AttestedTransaction::new(tx, attestation)
}

/// The V2 client of the first validator, as the fullnode's orchestrator sees
/// it.
fn first_validator_client(test_cluster: &TestCluster) -> NetworkAuthorityClient {
    test_cluster.fullnode_handle.iota_node.with(|node| {
        node.transaction_orchestrator()
            .expect("fullnode without transaction orchestrator")
            .clone_authority_aggregator()
            .authority_clients
            .values()
            .next()
            .expect("no authority clients")
            .authority_client()
            .clone()
    })
}

/// Waits for `digest` to be checkpointed on the fullnode and returns its
/// effects.
async fn wait_for_effects(
    test_cluster: &TestCluster,
    digest: TransactionDigest,
) -> TransactionEffects {
    test_cluster
        .fullnode_handle
        .iota_node
        .with_async(|node| async move {
            node.state()
                .wait_for_checkpoint_inclusion(&[digest], Duration::from_secs(60))
                .await
                .unwrap();
            node.state()
                .get_transaction_cache_reader()
                .try_get_executed_effects(&digest)
                .unwrap()
                .expect("checkpointed transaction has effects")
        })
        .await
}

/// A transaction attested by an active attestor goes through the ingress,
/// the block verifier and post-consensus validation, and executes; one
/// attested by an unregistered key is rejected at the ingress.
#[sim_test]
async fn test_externally_attested_tx_is_sequenced_and_executed() {
    telemetry_subscribers::init_for_testing();
    let _env = enable_external_attestation_env();
    let test_cluster = TestClusterBuilder::new().build().await;

    let attestor_address = test_cluster.get_address_0();
    let keypair = keypair_from_seed(7);
    test_cluster
        .register_attestor(attestor_address, &keypair)
        .await;
    test_cluster.force_new_epoch().await;

    let client = first_validator_client(&test_cluster);
    let client_addr = Some(SocketAddr::new([127, 0, 0, 1].into(), 0));

    let attested = attested_transfer(&test_cluster, 1_000, attestor_address, &keypair).await;
    let digest = *attested.digest();
    let statuses = client
        .submit_externally_attested_tx(vec![attested], client_addr)
        .await
        .unwrap();
    assert!(
        matches!(
            statuses.as_slice(),
            [(
                _,
                TxStatusUpdate::Submitted | TxStatusUpdate::Executed { .. }
            )]
        ),
        "unexpected statuses: {statuses:?}"
    );
    let effects = wait_for_effects(&test_cluster, digest).await;
    assert!(
        matches!(effects.status(), ExecutionStatus::Success),
        "transaction failed: {:?}",
        effects.status()
    );

    let attested = attested_transfer(
        &test_cluster,
        2_000,
        Address::random(),
        &keypair_from_seed(8),
    )
    .await;
    let statuses = client
        .submit_externally_attested_tx(vec![attested], client_addr)
        .await
        .unwrap();
    assert!(
        matches!(
            statuses.as_slice(),
            [(
                _,
                TxStatusUpdate::Rejected {
                    error: IotaError::ExplicitAttestationUnknownAttestor { .. }
                }
            )]
        ),
        "unexpected statuses: {statuses:?}"
    );
}

/// Registering an attestor lands it pending; after one epoch boundary it is
/// active in the epoch store's `AttestorSet`. Deregistering an active attestor
/// is deferred to the next boundary, after which it is removed from the set.
#[sim_test]
async fn test_attestor_registry_lifecycle() {
    telemetry_subscribers::init_for_testing();

    let _env = enable_external_attestation_env();

    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = test_cluster.get_address_0();

    // One gas coin for gas and a separate whole coin as the bond (each default
    // test coin far exceeds the joining bond derived from the protocol config).
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
    let attestor_pubkey = attestor_pubkey_bytes(&attestor_keypair);
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
