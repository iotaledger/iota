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

use fastcrypto::encoding::{Base64, Encoding};
use iota_types::{
    move_authenticator::MoveAuthenticatorExt,
    object::Object,
    signature::GenericSignature,
    transaction::{SenderSignedData, TransactionData, TransactionDataAPI},
};
use iota_vm_sdk::{
    Chain, ChainContext, ExecuteOptions, ExecutionResult, InMemoryStore, LocalVm, ProtocolVersion,
    SignatureStatus, Store, VmSdkError,
};
#[cfg(feature = "tracing")]
use iota_vm_sdk::{DebugConfig, ProfileOutput, ProfileSink};
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

impl Fixture {
    fn transaction(&self) -> TransactionData {
        bcs::from_bytes(&b64(&self.tx_b64)).expect("decode tx")
    }

    fn decoded_signatures(&self) -> Vec<GenericSignature> {
        self.signatures
            .iter()
            .map(|s| GenericSignature::from_bytes(&b64(s)).expect("decode signature"))
            .collect()
    }

    fn signed(&self) -> SenderSignedData {
        SenderSignedData::new(self.transaction(), self.decoded_signatures())
    }

    fn objects(&self) -> Vec<Object> {
        self.objects
            .iter()
            .map(|obj| bcs::from_bytes(&b64(&obj.bcs_b64)).expect("decode object"))
            .collect()
    }

    fn store(&self) -> InMemoryStore {
        let mut store = InMemoryStore::with_framework();
        for obj in self.objects() {
            store.insert(obj);
        }
        store
    }

    fn vm(&self) -> LocalVm {
        LocalVm::new(chain_context(self), self.store()).expect("build LocalVm")
    }
}

fn b64(s: &str) -> Vec<u8> {
    Base64::decode(s).expect("base64 decode")
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
    f.decoded_signatures()
        .into_iter()
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
/// result. The fixtures carry a real gas coin among their objects; the
/// `MoveAuthenticator` is verified inside the VM in every mode.
fn try_run(name: &str, opts: ExecuteOptions) -> Result<ExecutionResult, VmSdkError> {
    let f = load(name);
    let mut vm = f.vm();

    vm.execute_signed(f.signed(), opts)
}

fn run(name: &str, opts: ExecuteOptions) -> ExecutionResult {
    try_run(name, opts).expect("execute_signed returns Ok (auth outcome is carried in the result)")
}

/// Run a fixture in dev-inspect mode and return the status / signature-status
/// pair.
fn replay(name: &str) -> (iota_sdk_types::ExecutionStatus, SignatureStatus) {
    let result = run(name, ExecuteOptions::dev_inspect());
    (result.status, result.signature_status)
}

/// Run a fixture's signed transaction through the pre-consensus signing check
/// and return the reported signature status.
fn signing_check(name: &str) -> SignatureStatus {
    let f = load(name);
    let vm = f.vm();

    vm.check_signing_authentication(f.signed())
        .expect("check_signing_authentication returns Ok (outcome carried in status)")
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
/// authentication failure. This exercises the re-run in
/// `execute_with_move_authenticators`: run the free-access fixture once (the
/// `add_field` body succeeds), seed the created dynamic field back into the
/// store, then replay the identical transaction so `add_field` aborts on the
/// now-existing field while the free-access authenticator still accepts.
#[test]
fn move_authenticator_accepts_but_aborting_body_stays_verified() {
    let f = load("move_auth_free_access_valid.json");
    let mut vm = f.vm();

    // First run: the free-access authenticator accepts and `add_field` succeeds.
    let first = vm
        .execute_signed(f.signed(), ExecuteOptions::dev_inspect())
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
        .execute_signed(f.signed(), ExecuteOptions::dev_inspect())
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

/// The re-run must meter at the same budget the combined run used.
/// In dev-inspect the declared budget is often still `0` (not yet settled) and
/// the combined run meters at the dev-inspect budget instead; a re-run metered
/// at the declared `0` would run the authenticator out of gas and misreport a
/// body abort as `SignatureStatus::Failed`. Same flow as
/// [`move_authenticator_accepts_but_aborting_body_stays_verified`], with the
/// declared budget zeroed (the free-access authenticator ignores the message,
/// so the mutated transaction still authenticates).
#[test]
fn move_authenticator_dev_inspect_rerun_ignores_zero_declared_budget() {
    let f = load("move_auth_free_access_valid.json");
    let mut tx = f.transaction();
    tx.gas_data_mut().budget = 0;
    let mut vm = f.vm();

    // First run: the free-access authenticator accepts and `add_field`
    // succeeds — dev-inspect meters at the dev-inspect budget, not the
    // declared `0`.
    let first = vm
        .execute_signed(
            SenderSignedData::new(tx.clone(), f.decoded_signatures()),
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

    // Second run: the body aborts, triggering the re-run. Metered at
    // the combined run's budget the free-access authenticator still accepts;
    // metered at the declared `0` it would run out of gas and misreport.
    let second = vm
        .execute_signed(
            SenderSignedData::new(tx, f.decoded_signatures()),
            ExecuteOptions::dev_inspect(),
        )
        .expect("second run returns Ok (the abort is carried in the status)");
    assert!(
        !second.status.is_success(),
        "re-adding the field must abort the transaction body"
    );
    assert!(
        matches!(second.signature_status, SignatureStatus::Verified),
        "a body abort must not be reported as a signature failure, got {:?}",
        second.signature_status
    );
}

/// A sponsored transaction whose **sponsor** authorizes via a
/// `MoveAuthenticator` must have that authenticator executed too — not just the
/// sender's. Here the sender uses the accepting free-access authenticator while
/// the sponsor uses the rejecting bogus-ed25519 one (both fixtures are reused
/// as the two signers of one sponsored transaction). The sponsor's rejection
/// must surface as `SignatureStatus::Failed`.
#[test]
fn sponsor_move_authenticator_is_executed_and_can_reject() {
    let sender_fx = load("move_auth_free_access_valid.json"); // sender: accepts
    let sponsor_fx = load("move_auth_ed25519_invalid.json"); // sponsor: rejects

    let sender_auth = move_authenticator_sig(&sender_fx);
    let sponsor_auth = move_authenticator_sig(&sponsor_fx);
    let sponsor = match &sponsor_auth {
        GenericSignature::MoveAuthenticator(a) => a.address(),
        _ => unreachable!("move_authenticator_sig returns a MoveAuthenticator"),
    };

    // Take the sender fixture's transaction and re-point its gas to the sponsor:
    // dropping the gas payment mints a mock gas coin for the sponsor, and
    // sender != gas owner makes it a sponsored transaction. The free-access
    // authenticator ignores the message, so the mutated tx still authenticates.
    let mut tx = sender_fx.transaction();
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
    let mut store = sender_fx.store();
    for obj in sponsor_fx.objects() {
        store.insert(obj);
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
#[cfg(feature = "tracing")]
#[test]
fn move_authenticator_run_captures_trace_and_profile() {
    let opts = ExecuteOptions::dev_inspect().with_debug(
        DebugConfig::default()
            .with_tracing()
            .with_profiling(ProfileSink::Capture),
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

    // Only the framework is present — none of the fixture's objects.
    let store = InMemoryStore::with_framework();
    let mut vm = LocalVm::new(chain_context(&f), store).expect("build LocalVm");

    let err = vm
        .execute_signed(f.signed(), ExecuteOptions::dev_inspect())
        .expect_err("a missing referenced object must fail");
    assert!(
        matches!(err, VmSdkError::MissingObject { .. }),
        "expected MissingObject, got {err:?}"
    );
}

/// The deny checks see the transaction's signatures: with
/// `move_authenticator_disabled`, a `MoveAuthenticator`-signed transaction is
/// rejected during preparation, as on a node with that configuration.
#[test]
fn deny_config_disabled_move_authenticator_is_rejected() {
    let deny_config = iota_vm_sdk::TransactionDenyConfigBuilder::new()
        .disable_move_authenticator()
        .build();
    let err = try_run(
        "move_auth_free_access_valid.json",
        ExecuteOptions::dev_inspect().with_deny_config(deny_config),
    )
    .expect_err("a denied MoveAuthenticator signature must be rejected");
    assert!(matches!(err, VmSdkError::Validation(_)), "got {err:?}");
}

/// The pre-consensus signing check admits the free-access authenticator: it
/// does trivial work, so it accepts within the signing gas cap
/// (`max_auth_gas`).
#[test]
fn check_signing_authentication_admits_free_access() {
    let status = signing_check("move_auth_free_access_valid.json");
    assert!(
        matches!(status, SignatureStatus::Verified),
        "free-access authenticator must be admitted at signing, got {status:?}"
    );
}

/// The pre-consensus signing check rejects the bogus-ed25519 authenticator: it
/// aborts inside the VM, so the transaction would not be admitted for signing.
#[test]
fn check_signing_authentication_rejects_bogus_ed25519() {
    let status = signing_check("move_auth_ed25519_invalid.json");
    assert!(
        matches!(status, SignatureStatus::Failed(_)),
        "a rejecting authenticator must fail the signing check, got {status:?}"
    );
}

/// With the coin deny-list check enabled but no on-chain `DenyList` in the
/// store, an ordinary `MoveAuthenticator` transaction (no regulated coins) is
/// unaffected — the check is a no-op over both the transaction and the
/// authenticator inputs.
#[test]
fn coin_deny_list_check_is_a_noop_without_a_deny_list() {
    let result = run(
        "move_auth_free_access_valid.json",
        ExecuteOptions::dry_run().with_coin_deny_list_check(),
    );
    assert!(
        result.status.is_success(),
        "no regulated coins, so the deny-list check must not block the run, got {:?}",
        result.status
    );
    assert!(
        matches!(result.signature_status, SignatureStatus::Verified),
        "expected Verified, got {:?}",
        result.signature_status
    );
}
