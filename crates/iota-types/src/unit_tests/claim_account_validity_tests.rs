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
use iota_sdk_crypto::ed25519::Ed25519PrivateKey;
use iota_sdk_types::{
    Address, ClaimAccountTransaction, ObjectDigest, ObjectId, ObjectReference,
    SharedObjectReference, SignatureScheme, SmartAccountBuildKind, SmartAccountClaim, Transaction,
    Version,
    crypto::{
        MULTISIG_COMMITTEE_SIZE_MAX, MultisigCommittee, MultisigMember, PublicKey,
        Secp256k1PublicKey,
    },
};
use rand::{SeedableRng, rngs::StdRng};

use crate::{
    account_abstraction::public_key::MovePublicKey,
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
    claim_tx_with_budget(claim, sender, 100_000_000)
}

fn claim_tx_with_budget(claim: SmartAccountClaim, sender: Address, budget: u64) -> Transaction {
    claim_tx_with_budget_and_price(claim, sender, budget, 1)
}

fn claim_tx_with_budget_and_price(
    claim: SmartAccountClaim,
    sender: Address,
    budget: u64,
    gas_price: u64,
) -> Transaction {
    Transaction::new(
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim)),
        sender,
        gas_ref(),
        budget,
        gas_price,
    )
}

/// A claim of a MultiSig address with the largest committee the chain
/// accepts, so its key bytes are the largest a claim can carry.
fn largest_multisig_claim() -> (SmartAccountClaim, Address) {
    let mut rng = StdRng::from_seed([7; 32]);
    let members = (0..MULTISIG_COMMITTEE_SIZE_MAX)
        .map(|_| MultisigMember::new(Ed25519PrivateKey::random_with(&mut rng).public_key(), 1))
        .collect();
    let committee = MultisigCommittee::new(members, 1).expect("a valid committee");
    let claim = SmartAccountClaim::new_multisig(&committee, SmartAccountBuildKind::Mutable);
    let sender = MovePublicKey::new(
        SignatureScheme::Multisig,
        claim.public_key_raw_bytes.clone(),
    )
    .expect("a valid multisig key")
    .address()
    .expect("a multisig key derives an address");
    (claim, sender)
}

/// The smallest budget the validity check accepts for `claim` at `gas_price`,
/// read back from the rejection of a zero budget.
fn floor_for(
    claim: SmartAccountClaim,
    sender: Address,
    gas_price: u64,
    config: &ProtocolConfig,
) -> u64 {
    match claim_tx_with_budget_and_price(claim, sender, 0, gas_price).validity_check(config) {
        Err(UserInputError::GasBudgetTooLow { min_budget, .. }) => min_budget,
        other => panic!("a zero budget must be rejected by the gas floor, got {other:?}"),
    }
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
    let floor = floor_for(claim.clone(), sender, 1, &config);

    // A claim the sequencer schedules must not be able to run out of gas.
    let err = claim_tx_with_budget(claim.clone(), sender, floor - 1)
        .validity_check(&config)
        .expect_err("a budget below the floor must be rejected");
    assert!(matches!(
        err,
        UserInputError::GasBudgetTooLow { min_budget, .. } if min_budget == floor
    ));

    claim_tx_with_budget(claim, sender, floor)
        .validity_check(&config)
        .expect("exactly the floor must pass");
}

#[test]
fn claim_floor_covers_the_computation_bucket_at_the_declared_gas_price() {
    let (claim, sender) = valid_claim();
    let config = config();
    let gas_price = config.max_gas_price();

    // Execution charges at least one rounding bucket of computation units at
    // the transaction's own gas price, so a floor that ignores the price lets
    // a claim at the highest admissible price run out of gas.
    let floor = floor_for(claim, sender, gas_price, &config);
    assert!(
        floor > config.gas_rounding_step() * gas_price,
        "floor {floor} does not cover one computation bucket at gas price {gas_price}"
    );
}

#[test]
fn claim_floor_grows_with_the_key_size() {
    let config = config();
    let (ed25519_claim, ed25519_sender) = valid_claim();
    let (multisig_claim, multisig_sender) = largest_multisig_claim();

    // The key is stored on the account object, so the storage a claim pays for
    // grows with the key bytes it carries.
    let ed25519_floor = floor_for(ed25519_claim, ed25519_sender, 1, &config);
    let multisig_floor = floor_for(multisig_claim.clone(), multisig_sender, 1, &config);
    assert!(
        multisig_floor > ed25519_floor,
        "multisig floor {multisig_floor} must exceed the Ed25519 floor {ed25519_floor}"
    );
    assert!(matches!(
        claim_tx_with_budget(multisig_claim, multisig_sender, ed25519_floor)
            .validity_check(&config),
        Err(UserInputError::GasBudgetTooLow { .. })
    ));
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
