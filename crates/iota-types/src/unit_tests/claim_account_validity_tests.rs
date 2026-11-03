// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Validity checks of a `ClaimAccount` that depend on the transaction around
//! the claim: the sender it is sent from and the gas it declares.

use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_sdk_types::{
    Address, ClaimAccountTransaction, ObjectDigest, ObjectId, ObjectReference, SignatureScheme,
    SmartAccountClaim, Transaction, Version,
    crypto::{MULTISIG_COMMITTEE_SIZE_MAX, MultisigCommittee, MultisigMember, PublicKey},
};

use crate::{
    account_abstraction::public_key::MovePublicKey,
    crypto::{AccountPrivateKey, get_key_pair},
    error::UserInputError,
    transaction::{TransactionAPI, TransactionKind},
};

/// The latest version on a chain other than testnet and mainnet enables the
/// claim kind and the built-in authenticators it requires.
fn config() -> ProtocolConfig {
    ProtocolConfig::get_for_version(ProtocolVersion::MAX, Chain::Unknown)
}

fn gas_ref() -> ObjectReference {
    ObjectReference::new(ObjectId::random(), Version::from(1), ObjectDigest::random())
}

/// An Ed25519 claim and the address its key derives.
fn ed25519_claim() -> (SmartAccountClaim, Address) {
    let (sender, private_key): (Address, AccountPrivateKey) = get_key_pair();
    let claim = SmartAccountClaim::new(&PublicKey::Ed25519(private_key.public_key()));
    (claim, sender)
}

/// A MultiSig claim of a committee with `size` Ed25519 members and threshold
/// one, the address the committee derives, and the address of its first
/// member.
fn multisig_claim_of_size(size: usize) -> (SmartAccountClaim, Address, Address) {
    let keys: Vec<(Address, AccountPrivateKey)> = (0..size).map(|_| get_key_pair()).collect();
    let committee = MultisigCommittee::new(
        keys.iter()
            .map(|(_, key)| MultisigMember::new(key.public_key(), 1))
            .collect(),
        1,
    )
    .expect("a valid committee");
    let claim = SmartAccountClaim::new_multisig(&committee);
    let sender = MovePublicKey::new(
        SignatureScheme::Multisig,
        claim.public_key_raw_bytes.clone(),
    )
    .expect("a valid multisig key")
    .address()
    .expect("a multisig key derives an address");
    (claim, sender, keys[0].0)
}

/// A MultiSig claim, the address its committee derives, and the address of
/// one of its members.
fn multisig_claim() -> (SmartAccountClaim, Address, Address) {
    multisig_claim_of_size(2)
}

/// A claim of a MultiSig address with the largest committee the chain
/// accepts, so its key bytes are the largest a claim can carry.
fn largest_multisig_claim() -> (SmartAccountClaim, Address) {
    let (claim, sender, _) = multisig_claim_of_size(MULTISIG_COMMITTEE_SIZE_MAX);
    (claim, sender)
}

fn claim_tx(claim: SmartAccountClaim, sender: Address) -> Transaction {
    claim_tx_with_budget_and_price(claim, sender, 100_000_000, 1)
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
fn claim_whose_key_derives_the_sender_passes() {
    let (claim, sender) = ed25519_claim();
    claim_tx(claim, sender)
        .validity_check(&config())
        .expect("an Ed25519 claim sent from the derived address must pass");

    let (claim, sender, _) = multisig_claim();
    claim_tx(claim, sender)
        .validity_check(&config())
        .expect("a MultiSig claim sent from the derived address must pass");
}

#[test]
fn claim_whose_key_does_not_derive_the_sender_is_rejected() {
    let (claim, _) = ed25519_claim();
    let (other_sender, _): (Address, AccountPrivateKey) = get_key_pair();
    assert!(matches!(
        claim_tx(claim, other_sender).validity_check(&config()),
        Err(UserInputError::IncorrectUserSignature { .. })
    ));

    // A member's own address is not the committee's address.
    let (claim, _, member_address) = multisig_claim();
    assert!(matches!(
        claim_tx(claim, member_address).validity_check(&config()),
        Err(UserInputError::IncorrectUserSignature { .. })
    ));
}

#[test]
fn disabled_claim_kind_is_reported_before_the_sender_check() {
    let (claim, _) = ed25519_claim();
    let (other_sender, _): (Address, AccountPrivateKey) = get_key_pair();
    let mut config = config();
    config.set_enable_claim_account_transaction_for_testing(false);

    assert!(matches!(
        claim_tx(claim, other_sender).validity_check(&config),
        Err(UserInputError::Unsupported(_))
    ));
}

#[test]
fn claim_below_the_gas_floor_is_rejected() {
    let (claim, sender) = ed25519_claim();
    let config = config();
    let floor = floor_for(claim.clone(), sender, 1, &config);

    let err = claim_tx_with_budget_and_price(claim.clone(), sender, floor - 1, 1)
        .validity_check(&config)
        .expect_err("a budget below the floor must be rejected");
    assert!(matches!(
        err,
        UserInputError::GasBudgetTooLow { min_budget, .. } if min_budget == floor
    ));

    claim_tx_with_budget_and_price(claim, sender, floor, 1)
        .validity_check(&config)
        .expect("exactly the floor must pass");
}

#[test]
fn claim_floor_covers_the_computation_bucket_at_the_declared_gas_price() {
    let (claim, sender) = ed25519_claim();
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
    let (ed25519_claim, ed25519_sender) = ed25519_claim();
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
        claim_tx_with_budget_and_price(multisig_claim, multisig_sender, ed25519_floor, 1)
            .validity_check(&config),
        Err(UserInputError::GasBudgetTooLow { .. })
    ));
}

#[test]
fn disabled_claim_kind_skips_the_gas_floor() {
    let (claim, sender) = ed25519_claim();
    let mut config = config();
    config.set_enable_claim_account_transaction_for_testing(false);

    // The kind is rejected as unsupported, not for its budget.
    assert!(matches!(
        claim_tx_with_budget_and_price(claim, sender, 0, 1).validity_check(&config),
        Err(UserInputError::Unsupported(_))
    ));
}
