// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::public_key_proof_tests;

use iota::nonce_factory::{Self, NonceFactory};
use iota::public_key::{Self, PublicKey};
use iota::public_key_proof;
use iota::signature_scheme;
use iota::smart_account::SmartAccount;
use iota::smart_account_builtin_auth;
use iota::test_scenario;
use iota::test_utils::{assert_eq, assert_ref_eq};

// === Signed proof fixtures ===

// Keys and personal message signatures in `UserSignature` wire format, over the proof messages
// the tests below build. Generated with `iota-sdk-crypto`.
const ED25519_PUBLIC_KEY: vector<u8> =
    x"d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";
const SECP256K1_PUBLIC_KEY: vector<u8> =
    x"0399829725f2a1a88fff22eb6aef7b9a1aaefdc104f3aa9246554b73dce501d77a";
const SECP256R1_PUBLIC_KEY: vector<u8> =
    x"0324707917ff539890338a4a36e70647333a3ae2e9f078ac6bcc97d0dd9e3c87f5";
const MULTISIG_PUBLIC_KEY: vector<u8> =
    x"0200d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f01010399829725f2a1a88fff22eb6aef7b9a1aaefdc104f3aa9246554b73dce501d77a010100";

const ACCOUNT: address = @0xACC;
const VALID_UNTIL_EPOCH: u64 = 5;

// Proofs for `ACCOUNT` with nonce 0.
const ED25519_ACCOUNT_SIGNATURE: vector<u8> =
    x"007369432aaad142a7e033f6e51e155a433261679f169e613c24597b1a8320dbe9c5fadbe29252cfb7fda607d8a9890b4b9bf2ecd9e2388c4fa3f0d7f7dd6dcc08d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";
const SECP256K1_ACCOUNT_SIGNATURE: vector<u8> =
    x"01d41f452bf8cd47b078edb260bbd39afc4f1f7983fa6bd5f4decd9d3eb403ec8728ab259cfe2823df6b3aff16a2339df8a41b3c06ece3b01fde19adfb635eeb0a0399829725f2a1a88fff22eb6aef7b9a1aaefdc104f3aa9246554b73dce501d77a";
const SECP256R1_ACCOUNT_SIGNATURE: vector<u8> =
    x"02976bf4c31de5f73d094e6c6298c02c4d680db406002249ae4583c1074213bfa638642da5815514a3a95fa7e6ddf1247d37f41e40a558ee8212acb8d7463f22350324707917ff539890338a4a36e70647333a3ae2e9f078ac6bcc97d0dd9e3c87f5";
const MULTISIG_ACCOUNT_SIGNATURE: vector<u8> =
    x"03010062dbbf6c20dc12b5fa0d395762861af9cf7866e822caee322bc511e6f1614a2b0b3a5f2d06ec5c1eb4a5d69a5e4516f325225f0e79e6f5301fafd7b9a0de7a0001000200d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f01010399829725f2a1a88fff22eb6aef7b9a1aaefdc104f3aa9246554b73dce501d77a010100";

// Ed25519 proofs for a new account created by @0x0 with nonce 0 of the factory each test creates
// first: under nonce key 0, and under nonce key 2.
const ED25519_FACTORY_SIGNATURE: vector<u8> =
    x"002fdeb75ffa1c12df92f3ac76baf8eba992c07a66a47154365d9a96023fdc2a82ceb0d4eb88866dbd6573adc641f54cf1db9f847b13405743942b4f90b5844607d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";
const ED25519_FACTORY_NONCE_KEY_2_SIGNATURE: vector<u8> =
    x"007c492ff38409ffaf54272dd50cb72e0b5e1143e8e3dd6ebc672b21da51f90c9b3ded505eff159fa7e829caa780846549e71a8eb15b30ec45cbe94ce30f2e2d05d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";

// Secp256k1 proof with nonce 0 for the account `create_and_rotate` creates.
const SECP256K1_ROTATION_SIGNATURE: vector<u8> =
    x"01fdb8b126ef3063ae5debf8592538b4a5111930a3f2a6ed06e20c8ae338ef27645b23813936c5b9cf591c16cecb4584161cd83f85b4a20e39c67c7dce838828c20399829725f2a1a88fff22eb6aef7b9a1aaefdc104f3aa9246554b73dce501d77a";

// === Messages ===

#[test]
fun account_proof_message_names_the_key_account_nonce_and_epoch() {
    let message = public_key_proof::account_proof_message(&ed25519(), ACCOUNT, 7, 9);

    let mut expected = message_head_ed25519();
    expected.append(
        b"\naccount: 0x0000000000000000000000000000000000000000000000000000000000000acc\nnonce source: account 0x0000000000000000000000000000000000000000000000000000000000000acc\nnonce: 7\nvalid until epoch: 9",
    );
    assert_eq(message, expected);
}

#[test]
fun creation_proof_message_names_the_key_sender_factory_nonce_key_nonce_and_epoch() {
    let factory = object::id_from_address(@0xFAC);
    let message = public_key_proof::creation_proof_message(&ed25519(), @0x5E, factory, 3, 4, 9);

    let mut expected = message_head_ed25519();
    expected.append(
        b"\nsender: 0x000000000000000000000000000000000000000000000000000000000000005e\nnonce source: factory 0x0000000000000000000000000000000000000000000000000000000000000fac\nnonce key: 3\nnonce: 4\nvalid until epoch: 9",
    );
    assert_eq(message, expected);
}

// === verify_for_account ===

#[test]
fun verify_for_account_accepts_an_ed25519_proof() {
    account_proof_test!(ed25519(), ED25519_ACCOUNT_SIGNATURE);
}

#[test]
fun verify_for_account_accepts_a_secp256k1_proof() {
    account_proof_test!(secp256k1(), SECP256K1_ACCOUNT_SIGNATURE);
}

#[test]
fun verify_for_account_accepts_a_secp256r1_proof() {
    account_proof_test!(secp256r1(), SECP256R1_ACCOUNT_SIGNATURE);
}

#[test]
fun verify_for_account_accepts_a_multisig_proof() {
    account_proof_test!(multisig(), MULTISIG_ACCOUNT_SIGNATURE);
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidProofSignature)]
fun verify_for_account_rejects_a_proof_for_another_account() {
    let mut scenario = test_scenario::begin(@0x0);
    let _verified = public_key_proof::verify_for_account(
        ed25519(),
        @0xB0B,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_ACCOUNT_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidProofSignature)]
fun verify_for_account_rejects_a_proof_for_another_nonce() {
    let mut scenario = test_scenario::begin(@0x0);
    let _verified = public_key_proof::verify_for_account(
        ed25519(),
        ACCOUNT,
        1,
        VALID_UNTIL_EPOCH,
        ED25519_ACCOUNT_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidProofSignature)]
fun verify_for_account_rejects_a_proof_signed_by_another_key() {
    let mut scenario = test_scenario::begin(@0x0);
    let _verified = public_key_proof::verify_for_account(
        secp256k1(),
        ACCOUNT,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_ACCOUNT_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EProofExpired)]
fun verify_for_account_rejects_an_expired_proof() {
    let mut scenario = test_scenario::begin(@0x0);
    let mut epoch = 0;
    while (epoch <= VALID_UNTIL_EPOCH) {
        scenario.next_epoch(@0x0);
        epoch = epoch + 1;
    };
    let _verified = public_key_proof::verify_for_account(
        ed25519(),
        ACCOUNT,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_ACCOUNT_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
#[expected_failure(abort_code = iota::account_nonce::EUnexpectedNonce)]
fun an_account_proof_works_once() {
    let mut scenario = test_scenario::begin(@0x0);
    let mut account_id = object::new_uid_from_hash(ACCOUNT);
    let first = public_key_proof::verify_for_account(
        ed25519(),
        ACCOUNT,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_ACCOUNT_SIGNATURE,
        scenario.ctx(),
    );
    iota::builtin_authenticator_functions::attach_public_key(&mut account_id, first);
    iota::builtin_authenticator_functions::detach_public_key(&mut account_id);
    assert_eq(public_key_proof::next_account_nonce(&account_id), 1);

    // The signature still verifies, but the account's counter has moved on to 1.
    let replay = public_key_proof::verify_for_account(
        ed25519(),
        ACCOUNT,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_ACCOUNT_SIGNATURE,
        scenario.ctx(),
    );
    iota::builtin_authenticator_functions::attach_public_key(&mut account_id, replay);

    abort
}

// === verify_for_new_account ===

#[test]
fun verify_for_new_account_takes_the_nonce() {
    let mut scenario = test_scenario::begin(@0x0);
    nonce_factory::new(3, scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<NonceFactory>();
    let verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_FACTORY_SIGNATURE,
        scenario.ctx(),
    );
    assert_ref_eq(verified.public_key(), &ed25519());
    assert_eq(factory.next_value(0), 1);
    assert_eq(factory.next_value(1), 0);
    assert_eq(factory.next_value(2), 0);

    smart_account_builtin_auth::builder_v1(verified, scenario.ctx()).build_v1();
    scenario.return_to_sender(factory);
    scenario.end();
}

#[test]
fun proofs_under_different_nonce_keys_are_used_in_any_order() {
    let mut scenario = test_scenario::begin(@0x0);
    nonce_factory::new(3, scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<NonceFactory>();
    let second = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        2,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_FACTORY_NONCE_KEY_2_SIGNATURE,
        scenario.ctx(),
    );
    smart_account_builtin_auth::builder_v1(second, scenario.ctx()).build_v1();
    let first = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_FACTORY_SIGNATURE,
        scenario.ctx(),
    );
    smart_account_builtin_auth::builder_v1(first, scenario.ctx()).build_v1();

    scenario.return_to_sender(factory);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::nonce_factory::EUnexpectedNonce)]
fun a_factory_proof_works_once() {
    let mut scenario = test_scenario::begin(@0x0);
    nonce_factory::new(1, scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<NonceFactory>();
    let first = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_FACTORY_SIGNATURE,
        scenario.ctx(),
    );
    smart_account_builtin_auth::builder_v1(first, scenario.ctx()).build_v1();
    let _replay = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_FACTORY_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
#[expected_failure(abort_code = iota::nonce_factory::EUnexpectedNonce)]
fun skip_cancels_the_pending_proof() {
    let mut scenario = test_scenario::begin(@0x0);
    nonce_factory::new(1, scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<NonceFactory>();
    factory.skip(0);
    assert_eq(factory.next_value(0), 1);
    let _verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_FACTORY_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidProofSignature)]
fun verify_for_new_account_rejects_another_sender() {
    let mut scenario = test_scenario::begin(@0x0);
    nonce_factory::new(1, scenario.ctx());

    // The proof names sender @0x0.
    scenario.next_tx(@0xB0B);
    let mut factory = scenario.take_from_address<NonceFactory>(@0x0);
    let _verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_FACTORY_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
#[expected_failure(abort_code = iota::nonce_factory::ENonceKeyOutOfRange)]
fun verify_for_new_account_rejects_a_nonce_key_past_the_count() {
    let mut scenario = test_scenario::begin(@0x0);
    nonce_factory::new(1, scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<NonceFactory>();
    let _verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        1,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_FACTORY_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

// === Nonce factory ===

#[test]
fun new_accepts_64_nonce_keys() {
    let mut scenario = test_scenario::begin(@0x0);
    nonce_factory::new(64, scenario.ctx());

    scenario.next_tx(@0x0);
    let factory = scenario.take_from_sender<NonceFactory>();
    assert_eq(factory.nonce_keys(), 64);
    assert_eq(factory.next_value(63), 0);
    scenario.return_to_sender(factory);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::nonce_factory::EInvalidNonceKeyCount)]
fun new_rejects_0_nonce_keys() {
    let mut scenario = test_scenario::begin(@0x0);
    nonce_factory::new(0, scenario.ctx());

    abort
}

#[test]
#[expected_failure(abort_code = iota::nonce_factory::EInvalidNonceKeyCount)]
fun new_rejects_more_than_64_nonce_keys() {
    let mut scenario = test_scenario::begin(@0x0);
    nonce_factory::new(65, scenario.ctx());

    abort
}

#[test]
fun destroy_deletes_the_factory() {
    let mut scenario = test_scenario::begin(@0x0);
    nonce_factory::new(1, scenario.ctx());

    scenario.next_tx(@0x0);
    let factory = scenario.take_from_sender<NonceFactory>();
    factory.destroy();

    scenario.next_tx(@0x0);
    assert_eq(test_scenario::has_most_recent_for_sender<NonceFactory>(&scenario), false);
    scenario.end();
}

// === End to end through smart_account_builtin_auth ===

#[test]
fun builder_and_rotation_with_signed_proofs() {
    let mut scenario = test_scenario::begin(@0x0);
    let account_address = create_and_rotate(&mut scenario);

    scenario.next_tx(account_address);
    let account = scenario.take_shared<SmartAccount>();
    assert_ref_eq(smart_account_builtin_auth::borrow_public_key(&account), &secp256k1());
    assert_eq(smart_account_builtin_auth::next_proof_nonce(&account), 1);
    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::account_nonce::EUnexpectedNonce)]
fun rotation_proof_works_once() {
    let mut scenario = test_scenario::begin(@0x0);
    let account_address = create_and_rotate(&mut scenario);

    // The proof still verifies, but names nonce 0, which the first rotation took.
    scenario.next_tx(account_address);
    let mut account = scenario.take_shared<SmartAccount>();
    let verified = public_key_proof::verify_for_account(
        secp256k1(),
        account_address,
        0,
        VALID_UNTIL_EPOCH,
        SECP256K1_ROTATION_SIGNATURE,
        scenario.ctx(),
    );
    smart_account_builtin_auth::rotate_public_key(&mut account, verified, scenario.ctx());

    abort
}

#[test]
#[expected_failure(abort_code = iota::account_nonce::EUnexpectedNonce)]
fun a_used_rotation_proof_stays_used_when_the_key_comes_back() {
    let mut scenario = test_scenario::begin(@0x0);
    let account_address = create_and_rotate(&mut scenario);

    scenario.next_tx(account_address);
    let mut account = scenario.take_shared<SmartAccount>();
    let back = public_key_proof::verified_for_account_for_testing(ed25519(), account_address, 1);
    smart_account_builtin_auth::rotate_public_key(&mut account, back, scenario.ctx());

    // The Ed25519 key is attached again, as when the Secp256k1 proof was made.
    let replay = public_key_proof::verify_for_account(
        secp256k1(),
        account_address,
        0,
        VALID_UNTIL_EPOCH,
        SECP256K1_ROTATION_SIGNATURE,
        scenario.ctx(),
    );
    smart_account_builtin_auth::rotate_public_key(&mut account, replay, scenario.ctx());

    abort
}

// === Helpers ===

fun ed25519(): PublicKey {
    public_key::create(signature_scheme::ed25519(), ED25519_PUBLIC_KEY)
}

fun secp256k1(): PublicKey {
    public_key::create(signature_scheme::secp256k1(), SECP256K1_PUBLIC_KEY)
}

fun secp256r1(): PublicKey {
    public_key::create(signature_scheme::secp256r1(), SECP256R1_PUBLIC_KEY)
}

fun multisig(): PublicKey {
    public_key::create(signature_scheme::multisig(), MULTISIG_PUBLIC_KEY)
}

/// Creates an account with the Ed25519 key through a nonce factory, then rotates it to the
/// Secp256k1 key. Returns the account's address.
fun create_and_rotate(scenario: &mut test_scenario::Scenario): address {
    nonce_factory::new(1, scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<NonceFactory>();
    let verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_FACTORY_SIGNATURE,
        scenario.ctx(),
    );
    let account_address = smart_account_builtin_auth::builder_v1(verified, scenario.ctx())
        .build_v1();
    scenario.return_to_sender(factory);

    scenario.next_tx(account_address);
    let mut account = scenario.take_shared<SmartAccount>();
    let verified = public_key_proof::verify_for_account(
        secp256k1(),
        account_address,
        smart_account_builtin_auth::next_proof_nonce(&account),
        VALID_UNTIL_EPOCH,
        SECP256K1_ROTATION_SIGNATURE,
        scenario.ctx(),
    );
    smart_account_builtin_auth::rotate_public_key(&mut account, verified, scenario.ctx());
    test_scenario::return_shared(account);
    account_address
}

/// The first two lines of every proof message for `ed25519()`.
fun message_head_ed25519(): vector<u8> {
    let mut head = b"0000000000000000000000000000000000000000000000000000000000000002::public_key::PublicKey\npublic key: 00";
    head.append(iota::hex::encode(ED25519_PUBLIC_KEY));
    head
}

/// Checks `$signature`, by `$public_key`, over the proof message for `ACCOUNT` with nonce 0, and
/// attaches the key to an account at `ACCOUNT`.
macro fun account_proof_test($public_key: PublicKey, $signature: vector<u8>) {
    let public_key = $public_key;
    let mut scenario = test_scenario::begin(@0x0);
    let verified = public_key_proof::verify_for_account(
        public_key,
        ACCOUNT,
        0,
        VALID_UNTIL_EPOCH,
        $signature,
        scenario.ctx(),
    );
    assert_ref_eq(verified.public_key(), &public_key);

    let mut account_id = object::new_uid_from_hash(ACCOUNT);
    iota::builtin_authenticator_functions::attach_public_key(&mut account_id, verified);
    assert_ref_eq(
        iota::builtin_authenticator_functions::borrow_public_key(&account_id),
        &public_key,
    );

    account_id.delete();
    scenario.end();
}
