// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Enables `enable_external_attestation` (+ its required
//! `enable_validator_attestation` and `enable_pcool_flow`). Nextest runs one
//! test per process, so the process-wide protocol-config env override below
//! is safe.

use iota_keys::keypair_file::read_keypair_from_file;
use iota_types::crypto::SignatureScheme;
use test_cluster::TestClusterBuilder;

use crate::attestor_commands::{IotaAttestorCommand, IotaAttestorCommandResponse};

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

/// Registering via the CLI lands the attestor pending; after one epoch
/// boundary it shows active in `iota attestor display`. update-name,
/// rotate-key and deposit-bond each succeed against an active attestor;
/// rotate-key requires the existing `attestor.key` to be moved aside first
/// and then writes a new keypair to it. Deregistering leaves it active
/// (with removal scheduled) until the following boundary, after which
/// display reports it as not registered.
#[tokio::test]
async fn test_attestor_cli_lifecycle() {
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

    let mut test_cluster = TestClusterBuilder::new().build().await;
    let address = test_cluster.get_address_0();

    // `min_attestor_joining_bond` from the devnet protocol config; register
    // below uses the default bond, so display math builds on this value.
    let min_joining_bond: u64 = 2_000_000_000_000;

    // Same derivation attestor_commands.rs uses for the key file location:
    // next to the wallet's client config.
    let attestor_key_path = test_cluster
        .wallet
        .config()
        .path()
        .parent()
        .expect("client config path has a parent directory")
        .join("attestor.key");

    let resp = IotaAttestorCommand::Register {
        name: "cli-attestor".into(),
        description: "made by the cli test".into(),
        url: "https://example.com".into(),
        logo: "https://example.com/logo.png".into(),
        bond: None,
        key_scheme: SignatureScheme::Ed25519,
        gas_budget: None,
    }
    .execute(&mut test_cluster.wallet)
    .await
    .unwrap();
    let IotaAttestorCommandResponse::Register(register_response) = resp else {
        panic!("expected Register response");
    };
    assert_eq!(
        register_response.status_ok(),
        Some(true),
        "register transaction must succeed"
    );
    assert!(
        attestor_key_path.exists(),
        "register must write attestor.key"
    );
    let registered_pubkey = read_keypair_from_file(&attestor_key_path)
        .unwrap()
        .public();

    // Pending until the epoch boundary.
    let IotaAttestorCommandResponse::Display(out) = IotaAttestorCommand::Display { address: None }
        .execute(&mut test_cluster.wallet)
        .await
        .unwrap()
    else {
        panic!("expected Display response");
    };
    assert!(out.contains("cli-attestor"), "display must show the name");
    assert!(
        out.contains("pending"),
        "attestor must be pending before the epoch boundary: {out}"
    );

    // Cross the boundary; the attestor becomes active.
    test_cluster.force_new_epoch().await;
    let IotaAttestorCommandResponse::Display(out) = IotaAttestorCommand::Display { address: None }
        .execute(&mut test_cluster.wallet)
        .await
        .unwrap()
    else {
        panic!("expected Display response");
    };
    assert!(out.contains("cli-attestor"), "display must show the name");
    assert!(
        out.contains("active"),
        "attestor must be active after the epoch boundary: {out}"
    );

    // update-name, rotate-key and deposit-bond each succeed against the
    // active attestor.
    let resp = IotaAttestorCommand::UpdateName {
        name: "cli-attestor-renamed".into(),
        gas_budget: None,
    }
    .execute(&mut test_cluster.wallet)
    .await
    .unwrap();
    let IotaAttestorCommandResponse::UpdateMetadata(update_name_response) = resp else {
        panic!("expected UpdateMetadata response");
    };
    assert_eq!(
        update_name_response.status_ok(),
        Some(true),
        "update-name transaction must succeed"
    );

    // rotate-key refuses to run while `attestor.key` is present (the current key stays
    // valid on-chain until the rotation lands, so the operator must move it aside first).
    let attestor_key_backup_path = attestor_key_path.with_extension("key.bak");
    std::fs::rename(&attestor_key_path, &attestor_key_backup_path).unwrap();

    let resp = IotaAttestorCommand::RotateKey {
        key_scheme: SignatureScheme::Ed25519,
        gas_budget: None,
    }
    .execute(&mut test_cluster.wallet)
    .await
    .unwrap();
    let IotaAttestorCommandResponse::RotateKey(rotate_key_response) = resp else {
        panic!("expected RotateKey response");
    };
    assert_eq!(
        rotate_key_response.status_ok(),
        Some(true),
        "rotate-key transaction must succeed"
    );
    let rotated_pubkey = read_keypair_from_file(&attestor_key_path)
        .unwrap()
        .public();
    assert_ne!(
        registered_pubkey, rotated_pubkey,
        "rotate-key must write a new keypair to attestor.key"
    );
    std::fs::remove_file(&attestor_key_backup_path).unwrap();

    let resp = IotaAttestorCommand::DepositBond {
        amount: 1_000_000_000,
        gas_budget: None,
    }
    .execute(&mut test_cluster.wallet)
    .await
    .unwrap();
    let IotaAttestorCommandResponse::DepositBond(deposit_bond_response) = resp else {
        panic!("expected DepositBond response");
    };
    assert_eq!(
        deposit_bond_response.status_ok(),
        Some(true),
        "deposit-bond transaction must succeed"
    );

    // The rename and the deposit are effective immediately: display shows the
    // new name and the increased bond.
    let IotaAttestorCommandResponse::Display(out) = IotaAttestorCommand::Display {
        address: Some(address),
    }
    .execute(&mut test_cluster.wallet)
    .await
    .unwrap()
    else {
        panic!("expected Display response");
    };
    assert!(
        out.contains("cli-attestor-renamed"),
        "update-name must be visible in display: {out}"
    );
    assert!(
        out.contains(&(min_joining_bond + 1_000_000_000).to_string()),
        "deposit-bond must raise the displayed bond: {out}"
    );

    // Deregister. For an active attestor this schedules removal at the next
    // boundary rather than taking effect immediately.
    let resp = IotaAttestorCommand::Deregister { gas_budget: None }
        .execute(&mut test_cluster.wallet)
        .await
        .unwrap();
    let IotaAttestorCommandResponse::Deregister {
        response: deregister_response,
        refund_deferred,
    } = resp
    else {
        panic!("expected Deregister response");
    };
    assert_eq!(
        deregister_response.status_ok(),
        Some(true),
        "deregister transaction must succeed"
    );
    assert!(
        refund_deferred,
        "deregistering an active attestor must defer the refund to the next boundary"
    );

    let IotaAttestorCommandResponse::Display(out) = IotaAttestorCommand::Display {
        address: Some(address),
    }
    .execute(&mut test_cluster.wallet)
    .await
    .unwrap()
    else {
        panic!("expected Display response");
    };
    assert!(
        out.contains("removal scheduled at next epoch boundary"),
        "deregistering an active attestor is deferred to the next boundary: {out}"
    );

    // Cross the boundary; the attestor is now removed.
    test_cluster.force_new_epoch().await;
    let IotaAttestorCommandResponse::Display(out) = IotaAttestorCommand::Display {
        address: Some(address),
    }
    .execute(&mut test_cluster.wallet)
    .await
    .unwrap()
    else {
        panic!("expected Display response");
    };
    assert!(
        out.contains("not registered"),
        "attestor must be removed after the deregistration boundary: {out}"
    );
}
