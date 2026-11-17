// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The values here are also the fixtures of
//! `smart_account_builtin_auth_tests.move`, so the Move and Rust messages
//! are checked against the same bytes. The chain identifier and the nonce
//! are the values the framework's tests see.

use iota_sdk_crypto::{IotaSigner, ed25519::Ed25519PrivateKey, simple::SimpleKeypair};
use iota_sdk_types::Address;
use rand::{SeedableRng, rngs::StdRng};

use crate::account_abstraction::key_proof::KeyProofMessage;

/// The account the Move tests build first.
const ACCOUNT: &str = "0x034401905bebdf8c04f3cd5f04f442a39372c8dc321c29edfb4f9cb30b23ab96";

const VALID_UNTIL_EPOCH: u64 = 5;

/// `KEY_PROOF_BYTES` of the Move tests: a new account created by `@0x0`.
const NEW_ACCOUNT_BYTES: &str = "6d303030303030303030303030303030303030303030303030303030303030303030303030303030303030303030303030303030303030303030303030303030323a3a736d6172745f6163636f756e745f6275696c74696e5f617574683a3a4b657950726f6f664d657373616765200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001206e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f0500000000000000";

#[test]
fn signing_bytes_are_the_type_name_and_the_bcs_message() {
    let message = message(Address::ZERO, true);

    assert_eq!(hex::encode(message.signing_bytes()), NEW_ACCOUNT_BYTES);
}

#[test]
fn signatures_match_the_move_fixtures() {
    let mut rng = StdRng::from_seed([0; 32]);
    let key = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let other_key = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let account = Address::from_hex(ACCOUNT).unwrap();

    let cases = [
        // NEW_ACCOUNT_SIGNATURE
        (
            &key,
            Address::ZERO,
            true,
            "00e6e96c42782e06b1117c2bb80e8a580d5fb0e53b1fc353e37bd5b2e55945fe36fbc91111fc0bc54e3ef1d53273c6068dcb5a789ec9b642af6dcd646db1f0aa0fb9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200",
        ),
        // ACCOUNT_SIGNATURE
        (
            &key,
            account,
            false,
            "00d960bc4aa80853cf25313b435c6ee9cae0d9a4f2c16dbaaaac740820c029d8b46cf0645ba09125f407e373a23ef6912697b13e856a82acc6c187d96ba20c590bb9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200",
        ),
        // OTHER_KEY_ACCOUNT_SIGNATURE
        (
            &other_key,
            account,
            false,
            "00387dfe109083bd7c9639f4ea9e7d49eb0c61d9ff9d6c13738273bfaf3237873ad6826bd7732f1c7f380c9183f97ea2cf863f6cda1470eeb7e8d65ca2f5dd180eaef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab",
        ),
        // OTHER_KEY_NEW_ACCOUNT_SIGNATURE
        (
            &other_key,
            account,
            true,
            "004c9050a8579131ead5e9b576fb36bcb343cf868e4f144c9124cd8f02a0afc29f125b0e0eff24065dcca9d79b2a6b6e474d35b8ffae02aa7749421773059a8b0caef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab",
        ),
    ];

    for (signer, account, new_account, expected) in cases {
        let signature = signer
            .sign_personal_message(&message(account, new_account).personal_message())
            .unwrap();
        assert_eq!(hex::encode(signature.to_bytes()), expected);
    }
}

fn message(account: Address, new_account: bool) -> KeyProofMessage {
    KeyProofMessage {
        chain_identifier: vec![0; 32],
        account,
        new_account,
        nonce: b"noncenoncenoncenoncenoncenoncenonce"[..32].to_vec(),
        valid_until_epoch: VALID_UNTIL_EPOCH,
    }
}
