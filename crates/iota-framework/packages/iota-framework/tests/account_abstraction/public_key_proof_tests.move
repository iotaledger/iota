// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::public_key_proof_tests;

use iota::public_key::{Self, PublicKey};
use iota::public_key_proof::{Self, AccountFactory};
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
    x"02770632ba449f7f0f6d7e8173ee8cdeee0c1676a4f02a9c10b877b2c022126a1d";
const SECP256R1_PUBLIC_KEY: vector<u8> =
    x"02187de95d431e456a4a1a6837f732d94c21ceec701ae551025042d2f2e96ae05c";
const MULTISIG_PUBLIC_KEY: vector<u8> =
    x"0200d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f010102770632ba449f7f0f6d7e8173ee8cdeee0c1676a4f02a9c10b877b2c022126a1d010100";

const ACCOUNT: address = @0xACC;
const VALID_UNTIL_EPOCH: u64 = 5;

// Proofs for `ACCOUNT` at key change count 0.
const ED25519_ACCOUNT_SIGNATURE: vector<u8> =
    x"00f89365d2500adf1335d3675510994b042acd4b5bb8ab5e6f693b92b54d32dddd35a4d2fadbbbe32f26055a70b006bfcbf339b3afcac171b8e919c27e0c26ea09d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";
const SECP256K1_ACCOUNT_SIGNATURE: vector<u8> =
    x"0154f0aa9401a2bcbc0fd19b8d90a1fa7618fe7d57fdd8e58a8422bb3f67cdeb16682df6af7e1615ace3479d93eba2580664c216eb3806b5e8c24a3cbea744df8102770632ba449f7f0f6d7e8173ee8cdeee0c1676a4f02a9c10b877b2c022126a1d";
const SECP256R1_ACCOUNT_SIGNATURE: vector<u8> =
    x"02963ac14337f164feaf704482a1f329d4acb8f7b6694c4b9d87616e80e142cebf762057b3c287c1115e28835087a65b8f51147afb054b3f04db9b4b30c225e6cf02187de95d431e456a4a1a6837f732d94c21ceec701ae551025042d2f2e96ae05c";
const MULTISIG_ACCOUNT_SIGNATURE: vector<u8> =
    x"030100a092b93f8f3d5215f72d1f13ef5c09e6007adcb845243c22e535fb371b981b088b0286e2331e82eb8feac2813442bdc046af2db5273d458e38aacda1887bf40b01000200d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f010102770632ba449f7f0f6d7e8173ee8cdeee0c1676a4f02a9c10b877b2c022126a1d010100";

// Ed25519 proofs for a new account created by @0x0 under the owned factory of
// `owned_factory_test`, at sequence 0, and under nonce key 3 of the shared factory of
// `shared_factory_test`, at sequence 0.
const ED25519_OWNED_FACTORY_SIGNATURE: vector<u8> =
    x"003017b11a98ecf60525e2900d8a0ad1e135b0ab74b7275ecd94fef0e89f33b6ee522168ad416774bef36dd8b4cd3053ecf77280878b97558e5c922b644780df0fd04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";
const ED25519_SHARED_FACTORY_SIGNATURE: vector<u8> =
    x"000642bed3c16cec16d10a63148898e35cb55c1b344cfa3160242f6ff4e17174cc11b8f9f436f75b7a983bb68420e649cc4ddf26226eafe834338165ac75208002d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";

// Secp256k1 proof for the account `builder_and_rotation_with_signed_proofs` creates, at key
// change count 1.
const SECP256K1_ROTATION_SIGNATURE: vector<u8> =
    x"016facec2b61884403618123e0038ea940c7490a0175d778ece5352b247625ec10358f31f89ced2c01930db6876ef78b51411e70635070e32f7f2ea776eef9a99f02770632ba449f7f0f6d7e8173ee8cdeee0c1676a4f02a9c10b877b2c022126a1d";

// === Messages ===

#[test]
fun account_proof_message_names_the_key_account_count_and_epoch() {
    let message = public_key_proof::account_proof_message(&ed25519(), ACCOUNT, 7, 9);

    let mut expected = b"0000000000000000000000000000000000000000000000000000000000000002::public_key::PublicKey\npublic key: 00";
    expected.append(iota::hex::encode(ED25519_PUBLIC_KEY));
    expected.append(
        b"\naccount: 0x0000000000000000000000000000000000000000000000000000000000000acc\nkey changes on this account: 7\nvalid until epoch: 9",
    );
    assert_eq(message, expected);
}

#[test]
fun creation_proof_message_names_the_key_factory_sender_nonce_key_sequence_and_epoch() {
    let factory = object::id_from_address(@0xFAC);
    let message = public_key_proof::creation_proof_message(&ed25519(), factory, @0x5E, 3, 4, 9);

    let mut expected = b"0000000000000000000000000000000000000000000000000000000000000002::public_key::PublicKey\npublic key: 00";
    expected.append(iota::hex::encode(ED25519_PUBLIC_KEY));
    expected.append(
        b"\nfactory: 0x0000000000000000000000000000000000000000000000000000000000000fac\nsender: 0x000000000000000000000000000000000000000000000000000000000000005e\nnonce key: 3\nsequence: 4\nvalid until epoch: 9",
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
fun verify_for_account_rejects_a_proof_for_another_key_change_count() {
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

// === verify_for_new_account ===

#[test]
fun owned_factory_test() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_owned_factory(scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<AccountFactory>();
    assert_eq(factory.is_shared(), false);
    assert_eq(factory.next_sequence(@0x0, 0), 0);

    let verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_OWNED_FACTORY_SIGNATURE,
        scenario.ctx(),
    );
    assert_ref_eq(verified.public_key(), &ed25519());
    assert_eq(factory.next_sequence(@0x0, 0), 1);

    smart_account_builtin_auth::builder_v1(verified, scenario.ctx()).build_v1();
    scenario.return_to_sender(factory);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidProofSignature)]
fun owned_factory_proof_works_once() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_owned_factory(scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<AccountFactory>();
    let first = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_OWNED_FACTORY_SIGNATURE,
        scenario.ctx(),
    );
    smart_account_builtin_auth::builder_v1(first, scenario.ctx()).build_v1();
    // The factory now expects sequence 1, so the same signature no longer matches.
    let _replay = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_OWNED_FACTORY_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
fun shared_factory_test() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_shared<AccountFactory>();
    assert_eq(factory.is_shared(), true);

    let verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        3,
        VALID_UNTIL_EPOCH,
        ED25519_SHARED_FACTORY_SIGNATURE,
        scenario.ctx(),
    );
    // Only the sender's sequence under nonce key 3 moved on.
    assert_eq(factory.next_sequence(@0x0, 3), 1);
    assert_eq(factory.next_sequence(@0x0, 0), 0);
    assert_eq(factory.next_sequence(@0xB0B, 3), 0);

    smart_account_builtin_auth::builder_v1(verified, scenario.ctx()).build_v1();
    test_scenario::return_shared(factory);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidProofSignature)]
fun shared_factory_proof_rejects_another_sender() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(scenario.ctx());

    // The proof names sender @0x0; @0xB0B submits it through the same shared factory.
    scenario.next_tx(@0xB0B);
    let mut factory = scenario.take_shared<AccountFactory>();
    let _verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        3,
        VALID_UNTIL_EPOCH,
        ED25519_SHARED_FACTORY_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
fun another_sender_cannot_move_a_senders_sequence() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(scenario.ctx());

    // @0xB0B moves its own sequence under nonce key 3, the key @0x0's proof uses.
    scenario.next_tx(@0xB0B);
    let mut factory = scenario.take_shared<AccountFactory>();
    factory.skip_sequence(3, scenario.ctx());
    assert_eq(factory.next_sequence(@0xB0B, 3), 1);
    assert_eq(factory.next_sequence(@0x0, 3), 0);
    test_scenario::return_shared(factory);

    // @0x0's proof for sequence 0 still works.
    scenario.next_tx(@0x0);
    let mut factory = scenario.take_shared<AccountFactory>();
    let verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        3,
        VALID_UNTIL_EPOCH,
        ED25519_SHARED_FACTORY_SIGNATURE,
        scenario.ctx(),
    );
    smart_account_builtin_auth::builder_v1(verified, scenario.ctx()).build_v1();
    test_scenario::return_shared(factory);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidProofSignature)]
fun skip_sequence_cancels_the_open_proof() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_owned_factory(scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<AccountFactory>();
    factory.skip_sequence(0, scenario.ctx());
    assert_eq(factory.next_sequence(@0x0, 0), 1);
    // The proof names sequence 0.
    let _verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_OWNED_FACTORY_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
fun destroy_owned_factory_deletes_it() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_owned_factory(scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<AccountFactory>();
    factory.skip_sequence(0, scenario.ctx());
    public_key_proof::destroy_owned_factory(factory, vector[0], scenario.ctx());

    scenario.next_tx(@0x0);
    assert_eq(test_scenario::has_most_recent_for_sender<AccountFactory>(&scenario), false);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EFactoryIsShared)]
fun destroy_owned_factory_aborts_for_a_shared_factory() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(scenario.ctx());

    scenario.next_tx(@0x0);
    let factory = scenario.take_shared<AccountFactory>();
    public_key_proof::destroy_owned_factory(factory, vector[], scenario.ctx());

    abort
}

// === End to end through smart_account_builtin_auth ===

#[test]
fun builder_and_rotation_with_signed_proofs() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_owned_factory(scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<AccountFactory>();
    let verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_OWNED_FACTORY_SIGNATURE,
        scenario.ctx(),
    );
    let account_address = smart_account_builtin_auth::builder_v1(verified, scenario.ctx())
        .build_v1();
    scenario.return_to_sender(factory);

    scenario.next_tx(account_address);
    let mut account = scenario.take_shared<SmartAccount>();
    let count = smart_account_builtin_auth::public_key_change_count(&account);
    assert_eq(count, 1);
    let verified = public_key_proof::verify_for_account(
        secp256k1(),
        account_address,
        count,
        VALID_UNTIL_EPOCH,
        SECP256K1_ROTATION_SIGNATURE,
        scenario.ctx(),
    );
    smart_account_builtin_auth::rotate_public_key(&mut account, verified, scenario.ctx());

    assert_ref_eq(smart_account_builtin_auth::borrow_public_key(&account), &secp256k1());
    assert_eq(smart_account_builtin_auth::public_key_change_count(&account), 2);
    test_scenario::return_shared(account);
    scenario.end();
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

/// Checks `$signature`, by `$public_key`, over the proof message for `ACCOUNT` at key change
/// count 0, and attaches the key to an account at `ACCOUNT`.
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
    assert_eq(iota::builtin_authenticator_functions::public_key_change_count(&account_id), 1);

    account_id.delete();
    scenario.end();
}
