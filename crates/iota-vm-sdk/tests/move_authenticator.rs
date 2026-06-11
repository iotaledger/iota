// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end `MoveAuthenticator` flow against the public `iota-vm-sdk` API.
//!
//! Replays the committed `tests/fixtures/*.json` captures — each one a full
//! `MoveAuthenticator`-signed transaction plus every object the authenticator
//! and the PTB body touch — through [`LocalVm::execute_signed`]. No network, no
//! cluster, no live keys: the same in-process VM that runs the PTB body also
//! resolves the account's `AuthenticatorFunctionRefV1` dynamic field from the
//! store and runs the authenticator function.
//!
//! The fixtures stand in for the publish / create / switch-auth setup steps an
//! author would otherwise run via `ExecuteOptions::execute()`, leaving the
//! authenticator verification itself as the part under test.

use std::{fs, path::PathBuf};

use base64::Engine;
use fastcrypto::traits::ToFromBytes;
use iota_types::{
    object::Object,
    signature::GenericSignature,
    transaction::{SenderSignedData, TransactionData},
};
use iota_vm_sdk::{
    Chain, ChainContext, ExecuteOptions, InMemoryStore, LocalVm, ProtocolVersion, SignatureStatus,
    Store,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    protocol_version: u64,
    reference_gas_price: u64,
    epoch_id: u64,
    epoch_timestamp_ms: u64,
    tx_b64: String,
    signatures: Vec<String>,
    objects: Vec<FixtureObject>,
}

#[derive(Deserialize)]
struct FixtureObject {
    bcs_b64: String,
}

fn b64(s: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .expect("base64 decode")
}

fn load(name: &str) -> Fixture {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name]
        .iter()
        .collect();
    let raw = fs::read_to_string(&path).expect("read fixture");
    serde_json::from_str(&raw).expect("parse fixture")
}

/// Run a fixture's signed transaction in dev-inspect mode and return the
/// resulting status / signature-status pair. Dev-inspect relaxes gas (the
/// fixtures carry no real gas payment) while still verifying the
/// `MoveAuthenticator` inside the VM.
fn replay(name: &str) -> (iota_sdk_types::ExecutionStatus, SignatureStatus) {
    let f = load(name);

    let tx: TransactionData = bcs::from_bytes(&b64(&f.tx_b64)).expect("decode tx");

    let mut store = InMemoryStore::with_framework();
    for obj in &f.objects {
        let object: Object = bcs::from_bytes(&b64(&obj.bcs_b64)).expect("decode object");
        store.insert(object);
    }

    let sigs: Vec<GenericSignature> = f
        .signatures
        .iter()
        .map(|s| GenericSignature::from_bytes(&b64(s)).expect("decode signature"))
        .collect();
    let signed = SenderSignedData::new(tx, sigs);

    let ctx = ChainContext::new(
        ProtocolVersion::new(f.protocol_version),
        f.reference_gas_price,
        f.epoch_id,
        f.epoch_timestamp_ms,
        Chain::Unknown,
    );
    let mut vm = LocalVm::new(ctx, store).expect("build LocalVm");

    let result = vm
        .execute_signed(signed, ExecuteOptions::dev_inspect())
        .expect("execute_signed returns Ok (auth verdict is carried in the result)");
    (result.status, result.signature_status)
}

/// `authenticate_free_access`: the authenticator function unconditionally
/// accepts, so the run succeeds and the signature is reported `Verified`.
#[test]
fn move_authenticator_accepts() {
    let (status, signature_status) = replay("move_auth_free_access_valid.json");
    assert!(
        status.is_success(),
        "free-access authenticator must succeed, got {status:?}"
    );
    assert!(
        matches!(signature_status, SignatureStatus::Verified),
        "expected SignatureStatus::Verified, got {signature_status:?}"
    );
}

/// An accepting `MoveAuthenticator` paired with a transaction body that aborts
/// must still report `SignatureStatus::Verified` — a body failure is not an
/// authentication failure. This exercises the verdict re-run in
/// `execute_with_move_authenticator`: run the free-access fixture once (the
/// `add_field` body succeeds), seed the created dynamic field back into the
/// store, then replay the identical transaction so `add_field` aborts on the
/// now-existing field while the free-access authenticator still accepts.
#[test]
fn move_authenticator_accepts_but_aborting_body_stays_verified() {
    let f = load("move_auth_free_access_valid.json");
    let tx: TransactionData = bcs::from_bytes(&b64(&f.tx_b64)).expect("decode tx");
    let sigs: Vec<GenericSignature> = f
        .signatures
        .iter()
        .map(|s| GenericSignature::from_bytes(&b64(s)).expect("decode signature"))
        .collect();

    let mut store = InMemoryStore::with_framework();
    for obj in &f.objects {
        let object: Object = bcs::from_bytes(&b64(&obj.bcs_b64)).expect("decode object");
        store.insert(object);
    }
    let ctx = ChainContext::new(
        ProtocolVersion::new(f.protocol_version),
        f.reference_gas_price,
        f.epoch_id,
        f.epoch_timestamp_ms,
        Chain::Unknown,
    );
    let mut vm = LocalVm::new(ctx, store).expect("build LocalVm");

    // First run: the free-access authenticator accepts and `add_field` succeeds.
    let first = vm
        .execute_signed(
            SenderSignedData::new(tx.clone(), sigs.clone()),
            ExecuteOptions::dev_inspect(),
        )
        .expect("first run returns Ok");
    assert!(
        first.status.is_success(),
        "the first add_field must succeed, got {:?}",
        first.status
    );

    // Seed the objects the run produced (the new dynamic field) so the same
    // `add_field` aborts the second time around.
    for obj in first.output_objects {
        vm.store_mut().insert(obj);
    }

    // Second run: the body aborts (field already exists) but the free-access
    // authenticator still accepts.
    let second = vm
        .execute_signed(
            SenderSignedData::new(tx, sigs),
            ExecuteOptions::dev_inspect(),
        )
        .expect("second run returns Ok (the abort is carried in the status)");
    assert!(
        !second.status.is_success(),
        "re-adding the field must abort the transaction body"
    );
    assert!(
        matches!(second.signature_status, SignatureStatus::Verified),
        "an accepting authenticator with an aborting body must stay Verified, got {:?}",
        second.signature_status
    );
}

/// `authenticate_ed25519` with a bogus signature: the authenticator function
/// aborts inside the VM. `execute_signed` still returns `Ok` — the rejection is
/// surfaced as a failed status and `SignatureStatus::Failed`, not as a
/// top-level error.
#[test]
fn move_authenticator_rejects() {
    let (status, signature_status) = replay("move_auth_ed25519_invalid.json");
    assert!(
        !status.is_success(),
        "authenticator must reject the bogus signature, got success: {status:?}"
    );
    assert!(
        matches!(signature_status, SignatureStatus::Failed(_)),
        "expected SignatureStatus::Failed, got {signature_status:?}"
    );
}
