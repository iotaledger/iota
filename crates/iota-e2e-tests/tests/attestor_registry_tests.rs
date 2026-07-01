// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the attestor registry.
//!
//! `register_attestor` is gated on `enable_validator_attestation`. This test
//! verifies the gate end-to-end with the feature disabled (its default).
//!
//! The happy path (registration activating across a reconfiguration and
//! surfacing in the epoch store's `AttestorSet`) is not exercised here:
//! enabling `enable_validator_attestation` also requires `enable_pcool_flow`
//! and switches on `TotalComputationUnits` per-object congestion control,
//! which cancels writes to the shared system-state object (0x5) in a test
//! cluster. That path is covered by the Move unit tests and will be
//! integration-tested once the feature flag is enabled in a protocol version.

use fastcrypto::encoding::{Encoding, Hex};
use iota_macros::sim_test;
use iota_types::{IOTA_SYSTEM_PACKAGE_ID, transaction::CallArg};
use test_cluster::TestClusterBuilder;

/// With `enable_validator_attestation` off (the default), `register_attestor`
/// must abort with `EFeatureNotEnabled`.
#[sim_test]
async fn test_register_attestor_rejected_when_feature_disabled() {
    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = test_cluster.get_address_0();

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

    // A real `flag || raw_key` ed25519 public key (flag 0x00 + 32 bytes); the
    // native does on-curve validation, so arbitrary bytes are rejected.
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

    let response = test_cluster
        .wallet
        .execute_transaction_may_fail(tx)
        .await
        .unwrap();
    assert_eq!(
        response.status_ok(),
        Some(false),
        "registration must abort while enable_validator_attestation is off"
    );
}
