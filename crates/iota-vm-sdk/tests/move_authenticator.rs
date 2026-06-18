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
use iota_types::{
    object::Object,
    signature::GenericSignature,
    transaction::{SenderSignedData, TransactionData, TransactionDataAPI},
};
use iota_vm_sdk::{
    Chain, ChainContext, DebugConfig, ExecuteOptions, ExecutionResult, InMemoryStore, LocalVm,
    ProfileOutput, ProfileSink, ProtocolVersion, SignatureStatus, Store, VmSdkError,
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

/// The [`ChainContext`] described by a fixture.
fn chain_context(f: &Fixture) -> ChainContext {
    ChainContext::new(ProtocolVersion::new(f.protocol_version), Chain::Unknown)
        .with_reference_gas_price(f.reference_gas_price)
        .with_epoch_id(f.epoch_id)
        .with_epoch_timestamp_ms(f.epoch_timestamp_ms)
}

/// The fixture's `MoveAuthenticator` signature.
fn move_authenticator_sig(f: &Fixture) -> GenericSignature {
    f.signatures
        .iter()
        .map(|s| GenericSignature::from_bytes(&b64(s)).expect("decode signature"))
        .find(|s| matches!(s, GenericSignature::MoveAuthenticator(_)))
        .expect("fixture carries a MoveAuthenticator signature")
}

fn load(name: &str) -> Fixture {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name]
        .iter()
        .collect();
    let raw = fs::read_to_string(&path).expect("read fixture");
    serde_json::from_str(&raw).expect("parse fixture")
}

/// Run a fixture's signed transaction in the given mode and return the full
/// result. The fixtures carry no real gas payment, so a mock gas coin is
/// minted; the `MoveAuthenticator` is verified inside the VM in every mode.
fn run(name: &str, opts: ExecuteOptions) -> ExecutionResult {
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

    let mut vm = LocalVm::new(chain_context(&f), store).expect("build LocalVm");

    vm.execute_signed(signed, opts)
        .expect("execute_signed returns Ok (auth verdict is carried in the result)")
}

/// Run a fixture in dev-inspect mode and return the status / signature-status
/// pair.
fn replay(name: &str) -> (iota_sdk_types::ExecutionStatus, SignatureStatus) {
    let result = run(name, ExecuteOptions::dev_inspect());
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
    let mut vm = LocalVm::new(chain_context(&f), store).expect("build LocalVm");

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

/// A sponsored transaction whose **sponsor** authorizes via a
/// `MoveAuthenticator` must have that authenticator executed too — not just the
/// sender's. Here the sender uses the accepting free-access authenticator while
/// the sponsor uses the rejecting bogus-ed25519 one (both fixtures are reused
/// as the two signers of one sponsored transaction). The sponsor's rejection
/// must surface as `SignatureStatus::Failed`. Before the sponsor authenticator
/// was wired up, only the sender's ran and this was wrongly reported
/// `Verified`.
#[test]
fn sponsor_move_authenticator_is_executed_and_can_reject() {
    let sender_fx = load("move_auth_free_access_valid.json"); // sender: accepts
    let sponsor_fx = load("move_auth_ed25519_invalid.json"); // sponsor: rejects

    let sender_auth = move_authenticator_sig(&sender_fx);
    let sponsor_auth = move_authenticator_sig(&sponsor_fx);
    let sponsor = match &sponsor_auth {
        GenericSignature::MoveAuthenticator(a) => a.address().expect("sponsor auth address"),
        _ => unreachable!("move_authenticator_sig returns a MoveAuthenticator"),
    };

    // Take the sender fixture's transaction and re-point its gas to the sponsor:
    // dropping the gas payment mints a mock gas coin for the sponsor, and
    // sender != gas owner makes it a sponsored transaction. The free-access
    // authenticator ignores the message, so the mutated tx still authenticates.
    let mut tx: TransactionData = bcs::from_bytes(&b64(&sender_fx.tx_b64)).expect("decode tx");
    {
        let gas = tx.gas_data_mut();
        gas.objects = vec![];
        gas.owner = sponsor;
    }
    assert!(
        tx.is_sponsored_tx(),
        "tx must be sponsored (sender != sponsor)"
    );

    let signed = SenderSignedData::new(tx, vec![sender_auth, sponsor_auth]);

    // The store needs both accounts' objects (each authenticator resolves its
    // own `AuthenticatorFunctionRefV1` dynamic field).
    let mut store = InMemoryStore::with_framework();
    for obj in sender_fx.objects.iter().chain(sponsor_fx.objects.iter()) {
        let object: Object = bcs::from_bytes(&b64(&obj.bcs_b64)).expect("decode object");
        store.insert(object);
    }

    let mut vm = LocalVm::new(chain_context(&sender_fx), store).expect("build LocalVm");

    let result = vm
        .execute_signed(signed, ExecuteOptions::dev_inspect())
        .expect("execute_signed returns Ok (the rejection is carried in the status)");
    assert!(
        matches!(result.signature_status, SignatureStatus::Failed(_)),
        "the sponsor's rejecting authenticator must surface as Failed, got {:?}",
        result.signature_status
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

/// In `DryRun`/`Execute`, the body of a `MoveAuthenticator`-signed transaction
/// must be metered at the full transaction gas budget, not the much smaller
/// authenticator budget (`max_auth_gas`). The free-access authenticator accepts
/// and the `add_field` body costs more than `max_auth_gas`, so the run succeeds
/// only when the body is given the full budget.
#[test]
fn move_authenticator_dry_run_meters_body_at_full_budget() {
    let result = run(
        "move_auth_free_access_valid.json",
        ExecuteOptions::dry_run(),
    );
    assert!(
        result.status.is_success(),
        "the body must be metered at the full budget in dry-run, got {:?}",
        result.status
    );
    assert!(
        matches!(result.signature_status, SignatureStatus::Verified),
        "free-access authenticator must report Verified, got {:?}",
        result.signature_status
    );
}

/// The authenticator path threads a trace builder and a gas profiler through
/// the engine, so a run with both enabled returns the captured artifacts.
#[test]
fn move_authenticator_run_captures_trace_and_profile() {
    let opts = ExecuteOptions::dev_inspect().with_debug(
        DebugConfig::default()
            .with_trace()
            .with_profile(ProfileSink::Capture),
    );
    let result = run("move_auth_free_access_valid.json", opts);
    let debug = result
        .debug
        .expect("debug artifacts present when capture was requested");
    assert!(
        debug.trace.is_some(),
        "authenticator run must capture a trace"
    );
    assert!(
        matches!(debug.profile, Some(ProfileOutput::Json(ref bytes)) if !bytes.is_empty()),
        "capture sink must yield non-empty profile JSON, got {:?}",
        debug.profile
    );
}

/// A `MoveAuthenticator`-signed transaction against a store missing the objects
/// it references must surface a clean [`VmSdkError::MissingObject`], not a
/// panic.
#[test]
fn move_authenticator_missing_object_is_reported() {
    let f = load("move_auth_free_access_valid.json");
    let tx: TransactionData = bcs::from_bytes(&b64(&f.tx_b64)).expect("decode tx");
    let sigs: Vec<GenericSignature> = f
        .signatures
        .iter()
        .map(|s| GenericSignature::from_bytes(&b64(s)).expect("decode signature"))
        .collect();
    let signed = SenderSignedData::new(tx, sigs);

    // Only the framework is present — none of the fixture's objects.
    let store = InMemoryStore::with_framework();
    let mut vm = LocalVm::new(chain_context(&f), store).expect("build LocalVm");

    let err = vm
        .execute_signed(signed, ExecuteOptions::dev_inspect())
        .expect_err("a missing referenced object must fail");
    assert!(
        matches!(err, VmSdkError::MissingObject { .. }),
        "expected MissingObject, got {err:?}"
    );
}
