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
    Address, ClaimAccountTransaction, ObjectId, SmartAccountBuildKind, SmartAccountClaim,
    SmartAccountField, TypeTag,
    crypto::{Ed25519PublicKey, PublicKey, Secp256k1PublicKey},
};

use crate::{
    base_types::{ObjectRef, SequenceNumber},
    crypto::{AccountKeyPair, KeypairTraits, get_key_pair},
    digests::ObjectDigest,
    error::UserInputError,
    transaction::{TransactionData, TransactionDataAPI, TransactionKind},
};

/// v30 on a non-testnet/mainnet chain is where the claim feature and its gas
/// floor are enabled. The claim kind additionally requires the P-COOL flow,
/// since the sequencer rules it depends on are only sound there.
fn config() -> ProtocolConfig {
    let mut config = ProtocolConfig::get_for_version(ProtocolVersion::MAX, Chain::Unknown);
    config.set_enable_pcool_flow_for_testing(true);
    config
}

fn gas_ref() -> ObjectRef {
    ObjectRef::new(
        ObjectId::random(),
        SequenceNumber::from_u64(1),
        ObjectDigest::random(),
    )
}

/// A claim whose public key derives its sender, with an ample gas budget.
fn valid_claim() -> (SmartAccountClaim, Address) {
    let (sender, keypair): (Address, AccountKeyPair) = get_key_pair();
    let claim = SmartAccountClaim {
        public_key: PublicKey::Ed25519(Ed25519PublicKey::new(
            keypair.public().as_ref().try_into().unwrap(),
        )),
        claim_registry_initial_shared_version: 0,
        fields: vec![],
        build_kind: SmartAccountBuildKind::Mutable,
    };
    (claim, sender)
}

fn claim_tx(claim: SmartAccountClaim, sender: Address) -> TransactionData {
    claim_tx_with_budget(claim, sender, 10_000_000)
}

fn claim_tx_with_budget(claim: SmartAccountClaim, sender: Address, budget: u64) -> TransactionData {
    TransactionData::new(
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
fn claim_with_fields_is_rejected() {
    let (mut claim, sender) = valid_claim();
    claim.fields = vec![SmartAccountField {
        name_type: TypeTag::U64,
        name_bcs: bcs::to_bytes(&1u64).unwrap(),
        value_type: TypeTag::U64,
        value_bcs: bcs::to_bytes(&2u64).unwrap(),
    }];

    // Fields would make the pipeline's shape - and so its cost and its abort
    // surface - depend on user-supplied type arguments and BCS bytes.
    assert!(matches!(
        claim_tx(claim, sender).validity_check(&config()),
        Err(UserInputError::Unsupported(_))
    ));
}

#[test]
fn claim_whose_key_does_not_derive_the_sender_is_rejected() {
    let (claim, _) = valid_claim();
    let (other_sender, _): (Address, AccountKeyPair) = get_key_pair();

    // Move asserts this too, but only at execution - too late, once the
    // sequencer has staged the entry.
    assert!(matches!(
        claim_tx(claim, other_sender).validity_check(&config()),
        Err(UserInputError::IncorrectUserSignature { .. })
    ));
}

#[test]
fn claim_with_malformed_key_bytes_is_rejected() {
    let (mut claim, sender) = valid_claim();
    // Length-correct for secp256k1 but not a valid curve point, and declared
    // under a scheme whose flag does not match the sender's derivation either.
    claim.public_key = PublicKey::Secp256k1(Secp256k1PublicKey::new([7u8; 33]));

    assert!(matches!(
        claim_tx(claim, sender).validity_check(&config()),
        Err(UserInputError::IncorrectUserSignature { .. })
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
