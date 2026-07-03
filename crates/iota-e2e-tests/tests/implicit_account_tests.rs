// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Implicit account tests under the P-COOL (white-flag) flow.
//!
//! These tests run with `with_pcool_enabled()`, where transactions are
//! sequenced through consensus before execution and built-in account objects
//! participate in shared-object version assignment. They exercise the two
//! mechanisms the implicit-account determinism fix (issue #11900) targets:
//!
//! - IMPLICIT-ACCOUNT PINNING: an implicit account (a plain-signed sender with
//!   no on-chain object) is assigned a per-transaction read-only version, so
//!   its read stays implicit and deterministic across validators even when a
//!   claim of the same address lands in a different commit.
//! - WHITE-FLAG CLAIM-CONFLICT INVALIDATION: when a `smart_account::build_v1`
//!   claim of an address and a plain-signed transaction from that same address
//!   are sequenced into the same consensus commit, the claim is validated first
//!   and the racing plain transaction is deterministically dropped with
//!   `AccountClaimConflict` before it executes — every validator agrees, so the
//!   network does not fork and the dropped transaction is retryable later.
//!
//! Transactions are submitted via the fullnode orchestrator
//! (`execute_transaction` / `wallet.execute_transaction_may_fail`), which
//! routes through the `TransactionDriver` under P-COOL; the legacy
//! `handle_transaction` / authority-aggregator helpers are disabled in this
//! flow.
//!
//! The test functions come first; the signing actor and transaction helpers
//! live at the bottom of the file.

use fastcrypto::{ed25519::Ed25519KeyPair, traits::KeyPair as FastcryptoKeyPair};
use iota_macros::sim_test;
use iota_sdk_types::{
    Address, Identifier, ObjectId, Owner, ProgrammableTransaction,
    crypto::{Intent, IntentMessage},
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::ObjectRef,
    crypto::{IotaKeyPair, Signature as IotaSignature},
    move_authenticator::MoveAuthenticator,
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    transaction::{CallArg, SharedObjectRef, Transaction, TransactionData},
};
use rand::{SeedableRng, rngs::StdRng};
use test_cluster::{TestCluster, TestClusterBuilder};

const GAS_AMOUNT: u64 = 20_000_000_000;
const SMART_ACCOUNT_MODULE: &str = "smart_account";
const PUBLIC_KEY_MODULE: &str = "public_key";

// ---------------------------------------------------
// --- P-COOL (white-flag) flow -----------------------
// ---------------------------------------------------

/// Implicit built-in authentication under P-COOL. A fresh key has no on-chain
/// account, so it is assigned no version (Weak) and the read stays implicit on
/// every validator.
#[sim_test]
async fn test_implicit_builtin_ed25519_pcool() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().with_pcool_enabled().build().await;
    let mut actor = Actor::ed25519(40);
    let sender = actor.address();

    let gas = fund(&test_cluster, sender).await;
    let tx_data = transfer_tx_data(&test_cluster, sender, gas).await;
    let sig = actor.sign(&tx_data).await;
    // `execute_transaction` asserts execution success internally.
    test_cluster
        .execute_transaction(Transaction::from_generic_sig_data(tx_data, vec![sig]))
        .await;

    assert!(
        account_object(&test_cluster, sender).await.is_none(),
        "implicit authentication must not create an account object"
    );
    Ok(())
}

/// Claim → explicit transition under P-COOL. The claim creates the account;
/// afterwards a plain-signed transfer is rejected (claimed accounts require a
/// `MoveAuthenticator`).
#[sim_test]
async fn test_claim_then_plain_pcool() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().with_pcool_enabled().build().await;
    let mut actor = Actor::ed25519(41);
    let sender = actor.address();

    // Claim the sender's address (the claim itself authenticates implicitly).
    let gas = fund(&test_cluster, sender).await;
    let registry = claim_registry_arg(&test_cluster).await;
    let pt = claim_ptb(registry, actor.prefixed_pk_bytes(), false)?;
    let tx_data = ptb_tx_data(&test_cluster, sender, gas, pt).await;
    let sig = actor.sign(&tx_data).await;
    test_cluster
        .execute_transaction(Transaction::from_generic_sig_data(tx_data, vec![sig]))
        .await;
    assert!(
        account_object(&test_cluster, sender).await.is_some(),
        "the account object must exist after the claim"
    );

    // A follow-up plain transfer from the now-claimed account, in a later
    // commit, is rejected: a claimed account can only be authenticated via a
    // `MoveAuthenticator`.
    let gas = fund(&test_cluster, sender).await;
    let tx_data = transfer_tx_data(&test_cluster, sender, gas).await;
    let sig = actor.sign(&tx_data).await;
    let res = test_cluster
        .wallet
        .execute_transaction_may_fail(Transaction::from_generic_sig_data(tx_data, vec![sig]))
        .await;
    let succeeded = res.as_ref().ok().and_then(|r| r.status_ok()) == Some(true);
    assert!(
        !succeeded,
        "a plain signature on a claimed account must be rejected, got {res:?}"
    );
    Ok(())
}

/// Determinism race (claim-then-plain): a `claim` of A and a plain transfer
/// from A, both signed with A's key, submitted concurrently and sequenced into
/// the same consensus commit.
///
/// Under the current white-flag fix (issue #11900) the claim is validated first
/// and the racing plain transfer — whose sender matches the claimed address —
/// is deterministically dropped with `AccountClaimConflict` before it executes.
/// Every validator agrees, so the network does not fork; the dropped transfer
/// never reaches finality and is retryable in a later commit.
#[sim_test]
async fn test_claim_plain_race_pcool() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().with_pcool_enabled().build().await;
    let mut actor = Actor::ed25519(42);
    let sender = actor.address();

    // Independent gas coins so the two transactions do not conflict on gas.
    let claim_gas = fund(&test_cluster, sender).await;
    let transfer_gas = fund(&test_cluster, sender).await;

    let registry = claim_registry_arg(&test_cluster).await;
    let claim_pt = claim_ptb(registry, actor.prefixed_pk_bytes(), false)?;
    let claim_data = ptb_tx_data(&test_cluster, sender, claim_gas, claim_pt).await;
    let claim_sig = actor.sign(&claim_data).await;
    let claim_tx = Transaction::from_generic_sig_data(claim_data, vec![claim_sig]);

    let transfer_data = transfer_tx_data(&test_cluster, sender, transfer_gas).await;
    let transfer_sig = actor.sign(&transfer_data).await;
    let transfer_tx = Transaction::from_generic_sig_data(transfer_data, vec![transfer_sig]);

    let (claim_res, transfer_res) = tokio::join!(
        test_cluster.wallet.execute_transaction_may_fail(claim_tx),
        test_cluster
            .wallet
            .execute_transaction_may_fail(transfer_tx),
    );

    // The claim is a white-flag "claimer": validated first, never dropped.
    assert_eq!(claim_res?.status_ok(), Some(true), "claim must succeed");

    // The racing plain transfer shares the claimed sender, so it is dropped
    // (`AccountClaimConflict`) and never finalizes.
    let transfer_succeeded = transfer_res.as_ref().ok().and_then(|r| r.status_ok()) == Some(true);
    assert!(
        !transfer_succeeded,
        "plain transfer racing the claim must be dropped, not finalized, got {transfer_res:?}"
    );

    assert!(
        account_object(&test_cluster, sender).await.is_some(),
        "the account must exist after the claim"
    );
    Ok(())
}

/// Determinism race with an IMMUTABLE claim: a `claim` of A finalized with
/// `build_immutable_v1` and a plain transfer from A, submitted concurrently into
/// the same consensus commit. A claim is detected by its use of the
/// `ClaimRegistry` (not the finalizer name), so the immutable claim is
/// recognized and the racing transfer is dropped with `AccountClaimConflict`.
/// Regression for the previous `build_v1`-name detection, under which an
/// immutable claim went undetected and the racing transfer would finalize.
#[sim_test]
async fn test_claim_immutable_plain_race_pcool() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().with_pcool_enabled().build().await;
    let mut actor = Actor::ed25519(45);
    let sender = actor.address();

    // Independent gas coins so the two transactions do not conflict on gas.
    let claim_gas = fund(&test_cluster, sender).await;
    let transfer_gas = fund(&test_cluster, sender).await;

    let registry = claim_registry_arg(&test_cluster).await;
    // `true` => finalize the claim with `build_immutable_v1`.
    let claim_pt = claim_ptb(registry, actor.prefixed_pk_bytes(), true)?;
    let claim_data = ptb_tx_data(&test_cluster, sender, claim_gas, claim_pt).await;
    let claim_sig = actor.sign(&claim_data).await;
    let claim_tx = Transaction::from_generic_sig_data(claim_data, vec![claim_sig]);

    let transfer_data = transfer_tx_data(&test_cluster, sender, transfer_gas).await;
    let transfer_sig = actor.sign(&transfer_data).await;
    let transfer_tx = Transaction::from_generic_sig_data(transfer_data, vec![transfer_sig]);

    let (claim_res, transfer_res) = tokio::join!(
        test_cluster.wallet.execute_transaction_may_fail(claim_tx),
        test_cluster
            .wallet
            .execute_transaction_may_fail(transfer_tx),
    );

    assert_eq!(
        claim_res?.status_ok(),
        Some(true),
        "immutable claim must succeed"
    );

    // The racing plain transfer shares the claimed sender, so it is dropped
    // (`AccountClaimConflict`) and never finalizes — even though the claim was
    // finalized with `build_immutable_v1`.
    let transfer_succeeded = transfer_res.as_ref().ok().and_then(|r| r.status_ok()) == Some(true);
    assert!(
        !transfer_succeeded,
        "plain transfer racing the immutable claim must be dropped, not finalized, got {transfer_res:?}"
    );

    assert!(
        account_object(&test_cluster, sender).await.is_some(),
        "the account must exist after the immutable claim"
    );
    Ok(())
}

/// rotate-then-plain (sequential) under P-COOL: after claiming A and rotating
/// its on-chain key (via a `MoveAuthenticator`), a plain transfer signed with
/// the OLD key is rejected — once claimed, A only accepts a
/// `MoveAuthenticator`.
#[sim_test]
async fn test_rotated_old_key_rejected_pcool() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().with_pcool_enabled().build().await;
    let mut actor = Actor::ed25519(43);
    let sender = actor.address();

    // Claim, then rotate the on-chain key to a fresh one (the rotation is
    // authenticated by the current/old key, so it succeeds).
    let gas = fund(&test_cluster, sender).await;
    let registry = claim_registry_arg(&test_cluster).await;
    let claim_pt = claim_ptb(registry, actor.prefixed_pk_bytes(), false)?;
    let claim_data = ptb_tx_data(&test_cluster, sender, gas, claim_pt).await;
    let claim_sig = actor.sign(&claim_data).await;
    test_cluster
        .execute_transaction(Transaction::from_generic_sig_data(
            claim_data,
            vec![claim_sig],
        ))
        .await;

    let new_key =
        IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([44u8; 32])));
    let rotate_gas = fund(&test_cluster, sender).await;
    let account_arg = shared_account_arg(&test_cluster, sender, true).await;
    let rotate_pt = rotate_pk_ptb(account_arg, prefixed_pk_of(&new_key))?;
    let rotate_data = ptb_tx_data(&test_cluster, sender, rotate_gas, rotate_pt).await;
    // The account is claimed, so the rotation is authenticated via a
    // `MoveAuthenticator` (a plain signature would be rejected).
    let rotate_auth_account = shared_account_arg(&test_cluster, sender, false).await;
    let rotate_auth = move_auth_sign(&mut actor, &rotate_data, rotate_auth_account).await?;
    test_cluster
        .execute_transaction(Transaction::from_generic_sig_data(
            rotate_data,
            vec![rotate_auth],
        ))
        .await;

    // Plain transfer signed with the OLD key (the actor still holds it).
    let transfer_gas = fund(&test_cluster, sender).await;
    let transfer_data = transfer_tx_data(&test_cluster, sender, transfer_gas).await;
    let old_key_sig = actor.sign(&transfer_data).await;
    let res = test_cluster
        .wallet
        .execute_transaction_may_fail(Transaction::from_generic_sig_data(
            transfer_data,
            vec![old_key_sig],
        ))
        .await;

    let succeeded = res
        .as_ref()
        .map(|r| r.status_ok() == Some(true))
        .unwrap_or(false);
    assert!(
        !succeeded,
        "the old key must be rejected after rotation, got {res:?}"
    );
    Ok(())
}

// ---------------------------------------------------
// --- Signing actor ---------------------------------
// ---------------------------------------------------

/// A signer for the built-in Ed25519 scheme. Collapses address derivation,
/// Move-side flag-prefixed public key bytes and transaction signing.
enum Actor {
    Simple(Box<IotaKeyPair>),
}

impl Actor {
    fn ed25519(seed: u8) -> Self {
        Self::Simple(Box::new(IotaKeyPair::Ed25519(Ed25519KeyPair::generate(
            &mut StdRng::from_seed([seed; 32]),
        ))))
    }

    fn address(&self) -> Address {
        match self {
            Self::Simple(kp) => Address::from(&kp.public()),
        }
    }

    /// Flag-prefixed public key bytes as expected by Move's
    /// `public_key::from_prefixed_bytes`.
    fn prefixed_pk_bytes(&self) -> Vec<u8> {
        match self {
            Self::Simple(kp) => {
                let pk = kp.public();
                let mut bytes = vec![pk.scheme().flag()];
                bytes.extend_from_slice(pk.as_ref());
                bytes
            }
        }
    }

    /// Signs `tx_data` and returns the plain (non-MoveAuthenticator)
    /// `GenericSignature` that the node maps to an implicit account object.
    async fn sign(&mut self, tx_data: &TransactionData) -> GenericSignature {
        let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
        match self {
            Self::Simple(kp) => {
                GenericSignature::Signature(IotaSignature::new_secure(&intent_msg, kp.as_ref()))
            }
        }
    }
}

/// Flag-prefixed public key bytes for a single keypair (see
/// `Actor::prefixed_pk_bytes`).
fn prefixed_pk_of(kp: &IotaKeyPair) -> Vec<u8> {
    let pk = kp.public();
    let mut bytes = vec![pk.scheme().flag()];
    bytes.extend_from_slice(pk.as_ref());
    bytes
}

// ---------------------------------------------------
// --- Cluster / transaction helpers -----------------
// ---------------------------------------------------

fn ident(name: &str) -> Identifier {
    Identifier::new(name).expect("valid identifier")
}

async fn fund(test_cluster: &TestCluster, address: Address) -> ObjectRef {
    let rgp = test_cluster.get_reference_gas_price().await;
    test_cluster
        .fund_address_and_return_gas(rgp, Some(GAS_AMOUNT), address)
        .await
}

/// Builds a minimal transfer `TransactionData` for `sender`.
async fn transfer_tx_data(
    test_cluster: &TestCluster,
    sender: Address,
    gas: ObjectRef,
) -> TransactionData {
    let rgp = test_cluster.get_reference_gas_price().await;
    TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(Some(1), Address::ZERO)
        .build()
}

/// Builds a `TransactionData` running `pt` with `sender`'s gas.
async fn ptb_tx_data(
    test_cluster: &TestCluster,
    sender: Address,
    gas: ObjectRef,
    pt: ProgrammableTransaction,
) -> TransactionData {
    let rgp = test_cluster.get_reference_gas_price().await;
    TestTransactionBuilder::new(sender, gas, rgp)
        .programmable(pt)
        .build()
}

/// Returns the object at the implicit account ID derived from `address`,
/// if any.
async fn account_object(test_cluster: &TestCluster, address: Address) -> Option<Object> {
    test_cluster
        .get_object_from_fullnode_store(&ObjectId::from(address))
        .await
}

/// The genesis `ClaimRegistry` shared object as a mutable PTB input.
async fn claim_registry_arg(test_cluster: &TestCluster) -> CallArg {
    let registry = test_cluster
        .get_object_from_fullnode_store(&ObjectId::CLAIM_REGISTRY)
        .await
        .expect("ClaimRegistry must exist at genesis");
    let initial_shared_version = match &registry.owner {
        Owner::Shared(initial_shared_version) => *initial_shared_version,
        owner => panic!("ClaimRegistry must be shared, found {owner:?}"),
    };
    CallArg::Shared(SharedObjectRef::new(
        ObjectId::CLAIM_REGISTRY,
        initial_shared_version,
        true,
    ))
}

/// The shared `SmartAccount` object at `address` as a PTB / authenticator
/// input.
async fn shared_account_arg(
    test_cluster: &TestCluster,
    address: Address,
    mutable: bool,
) -> CallArg {
    let account = account_object(test_cluster, address)
        .await
        .expect("SmartAccount must exist");
    let initial_shared_version = match &account.owner {
        Owner::Shared(initial_shared_version) => *initial_shared_version,
        owner => panic!("SmartAccount must be shared, found {owner:?}"),
    };
    CallArg::Shared(SharedObjectRef::new(
        ObjectId::from(address),
        initial_shared_version,
        mutable,
    ))
}

/// PTB claiming the sender's own address as a `SmartAccount`:
/// `public_key::from_prefixed_bytes` -> `smart_account::claim_builder_v1` ->
/// `smart_account::{build_v1|build_immutable_v1}`.
fn claim_ptb(
    registry: CallArg,
    prefixed_pk: Vec<u8>,
    immutable: bool,
) -> anyhow::Result<ProgrammableTransaction> {
    let mut builder = ProgrammableTransactionBuilder::new();
    let pk_bytes_arg = builder.pure(prefixed_pk)?;
    let pk = builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        ident(PUBLIC_KEY_MODULE),
        ident("from_prefixed_bytes"),
        vec![],
        vec![pk_bytes_arg],
    );
    let registry_arg = builder.obj(registry)?;
    let account_builder = builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        ident(SMART_ACCOUNT_MODULE),
        ident("claim_builder_v1"),
        vec![],
        vec![registry_arg, pk],
    );
    builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        ident(SMART_ACCOUNT_MODULE),
        ident(if immutable {
            "build_immutable_v1"
        } else {
            "build_v1"
        }),
        vec![],
        vec![account_builder],
    );
    Ok(builder.finish())
}

/// PTB rotating the built-in authenticator public key of `account` to
/// `new_prefixed_pk`. The returned previous `PublicKey` is copy+drop, so it is
/// safe to leave unconsumed.
fn rotate_pk_ptb(
    account: CallArg,
    new_prefixed_pk: Vec<u8>,
) -> anyhow::Result<ProgrammableTransaction> {
    let mut builder = ProgrammableTransactionBuilder::new();
    let pk_bytes_arg = builder.pure(new_prefixed_pk)?;
    let pk = builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        ident(PUBLIC_KEY_MODULE),
        ident("from_prefixed_bytes"),
        vec![],
        vec![pk_bytes_arg],
    );
    let account_arg = builder.obj(account)?;
    builder.programmable_move_call(
        ObjectId::FRAMEWORK,
        ident(SMART_ACCOUNT_MODULE),
        ident("rotate_builtin_auth_public_key"),
        vec![],
        vec![account_arg, pk],
    );
    Ok(builder.finish())
}

/// Wraps `GenericSignature` wire bytes in a hand-crafted `MoveAuthenticator`
/// that authenticates against `account`.
fn builtin_move_authenticator(
    wire_bytes: &[u8],
    account: CallArg,
) -> anyhow::Result<GenericSignature> {
    Ok(GenericSignature::MoveAuthenticator(
        MoveAuthenticator::new_v1(
            vec![CallArg::Pure(bcs::to_bytes(&wire_bytes.to_vec())?)],
            vec![],
            account,
        ),
    ))
}

/// Authenticates `tx_data` against a claimed `account` via a hand-crafted
/// `MoveAuthenticator` wrapping `actor`'s signature. Required for any operation
/// on a claimed account, because a plain signature on a claimed account is
/// rejected (`PlainSignatureOnClaimedAccount`).
async fn move_auth_sign(
    actor: &mut Actor,
    tx_data: &TransactionData,
    account: CallArg,
) -> anyhow::Result<GenericSignature> {
    let wire_bytes = actor.sign(tx_data).await.to_bytes();
    builtin_move_authenticator(&wire_bytes, account)
}
