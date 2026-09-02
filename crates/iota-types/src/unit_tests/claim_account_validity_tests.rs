// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Validity checks that must reject a `ClaimAccount` before it is sequenced.
//!
//! The sequencer stages a claim entry for the address before the claim
//! executes, so a claim it schedules must not be able to abort — the address
//! would be treated as explicit with no account object behind it. Everything
//! the constrained pipeline could abort on is therefore rejected here, from the
//! transaction bytes alone.

use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_sdk_types::{
    Address, ClaimAccountTransaction, ObjectDigest, ObjectId, ObjectReference,
    SharedObjectReference, SmartAccountBuildKind, SmartAccountClaim, Transaction, Version,
    crypto::{PublicKey, Secp256k1PublicKey},
};

use crate::{
    crypto::{AccountPrivateKey, get_key_pair},
    error::UserInputError,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, TransactionAPI, TransactionKind},
};

/// v36 on a non-testnet/mainnet chain is where the claim feature and its gas
/// floor are enabled. The claim kind additionally requires the P-COOL flow,
/// since the sequencer rules it depends on are only sound there.
fn config() -> ProtocolConfig {
    let mut config = ProtocolConfig::get_for_version(ProtocolVersion::MAX, Chain::Unknown);
    config.set_enable_pcool_flow_for_testing(true);
    config
}

fn gas_ref() -> ObjectReference {
    ObjectReference::new(ObjectId::random(), Version::from(1), ObjectDigest::random())
}

/// A claim whose public key derives its sender, with an ample gas budget.
fn valid_claim() -> (SmartAccountClaim, Address) {
    let (sender, private_key): (Address, AccountPrivateKey) = get_key_pair();
    let claim = SmartAccountClaim::new(
        &PublicKey::Ed25519(private_key.public_key()),
        SmartAccountBuildKind::Mutable,
    );
    (claim, sender)
}

fn claim_tx(claim: SmartAccountClaim, sender: Address) -> Transaction {
    claim_tx_with_budget(claim, sender, 10_000_000)
}

fn claim_tx_with_budget(claim: SmartAccountClaim, sender: Address, budget: u64) -> Transaction {
    Transaction::new(
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim)),
        sender,
        gas_ref(),
        budget,
        1,
    )
}

#[test]
fn valid_claim_passes() {
    let (claim, sender) = valid_claim();
    claim_tx(claim, sender)
        .validity_check(&config())
        .expect("a well-formed claim must pass");
}

#[test]
fn claim_whose_key_does_not_derive_the_sender_is_rejected() {
    let (claim, _) = valid_claim();
    let (other_sender, _): (Address, AccountPrivateKey) = get_key_pair();

    // Move asserts this too, but only at execution - too late, once the
    // sequencer has staged the entry.
    assert!(matches!(
        claim_tx(claim, other_sender).validity_check(&config()),
        Err(UserInputError::IncorrectUserSignature { .. })
    ));
}

#[test]
fn claim_with_malformed_key_bytes_is_rejected() {
    let (_, sender) = valid_claim();
    // Length-correct for secp256k1 but not a valid curve point. The kind-level
    // check validates the key bytes against their declared scheme.
    let claim = SmartAccountClaim::new(
        &PublicKey::Secp256k1(Secp256k1PublicKey::new([7u8; 33])),
        SmartAccountBuildKind::Mutable,
    );

    assert!(matches!(
        claim_tx(claim, sender).validity_check(&config()),
        Err(UserInputError::Unsupported(_))
    ));
}

#[test]
fn claim_below_the_gas_floor_is_rejected() {
    let (claim, sender) = valid_claim();
    let config = config();
    let floor = config.claim_account_min_gas_budget();

    // A claim the sequencer schedules must not be able to run out of gas.
    let err = claim_tx_with_budget(claim, sender, floor - 1)
        .validity_check(&config)
        .expect_err("a budget below the floor must be rejected");
    assert!(matches!(
        err,
        UserInputError::GasBudgetTooLow { min_budget, .. } if min_budget == floor
    ));

    let (claim, sender) = valid_claim();
    claim_tx_with_budget(claim, sender, floor)
        .validity_check(&config)
        .expect("exactly the floor must pass");
}

#[test]
fn declared_initial_shared_version_must_be_valid() {
    let (sender, _): (Address, AccountPrivateKey) = get_key_pair();
    let shared_id = ObjectId::random();

    // A sentinel version would otherwise seed the epoch's version chain
    // verbatim and reach the version-assignment walk, which unwraps a lamport
    // increment that errors on an invalid version.
    let mut builder = ProgrammableTransactionBuilder::new();
    builder
        .input(CallArg::Shared(SharedObjectReference::new(
            shared_id,
            Version::CANCELED_READ,
            true,
        )))
        .unwrap();
    let tx = Transaction::new(
        TransactionKind::Programmable(builder.finish()),
        sender,
        gas_ref(),
        10_000_000,
        1,
    );
    assert!(matches!(
        tx.validity_check(&config()),
        Err(UserInputError::InvalidSequenceNumber)
    ));

    // A real initial version is accepted.
    let mut builder = ProgrammableTransactionBuilder::new();
    builder
        .input(CallArg::Shared(SharedObjectReference::new(
            shared_id,
            Version::from(3),
            true,
        )))
        .unwrap();
    let tx = Transaction::new(
        TransactionKind::Programmable(builder.finish()),
        sender,
        gas_ref(),
        10_000_000,
        1,
    );
    tx.validity_check(&config())
        .expect("a valid initial shared version must pass");
}
