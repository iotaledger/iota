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

// Proofs for `ACCOUNT` while it has no public key attached.
const ED25519_ACCOUNT_SIGNATURE: vector<u8> =
    x"00fca305caebe906f5e30803afdebec1c8a5271677be8c0c60c17d64a19ed9ea447a01a6fdc7ccd6011dd3d6ebf9982f8212b7464be1d9e85a5f0668ecf2da8500d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";
const SECP256K1_ACCOUNT_SIGNATURE: vector<u8> =
    x"015b9d0bc68ed61e9102a2573ebf4dab9f7043aa1a1725323c4927d771f9114e1054fafef357764c5816a1a8a90d133e2d42e8ba66fc76d8b5e68546fb6a00a36902770632ba449f7f0f6d7e8173ee8cdeee0c1676a4f02a9c10b877b2c022126a1d";
const SECP256R1_ACCOUNT_SIGNATURE: vector<u8> =
    x"02983d95126b400500b7bc857e1603e67c3317ea99c2006913839c958d57aaa4a760673eaf8c2035a25431abb3f62d26a9c05f5986589cf3d3b1e0ca97111d1e8202187de95d431e456a4a1a6837f732d94c21ceec701ae551025042d2f2e96ae05c";
const MULTISIG_ACCOUNT_SIGNATURE: vector<u8> =
    x"030100c6ec9cf6af8a11945755996e89ae1d5e7a1ab6747b0dc7cf98885109688fbae7b03c04326e0225005122ea049f0aed63d12539e3e2d9ccc1c97e66292255590901000200d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f010102770632ba449f7f0f6d7e8173ee8cdeee0c1676a4f02a9c10b877b2c022126a1d010100";

// Ed25519 proofs for a new account under the factory each test creates first, at sequence 0:
// created by @0x0 under nonce key 0 of an owned factory, and by @0x0 or @0xB0B under nonce key 3
// of a shared factory.
const ED25519_OWNED_FACTORY_SIGNATURE: vector<u8> =
    x"003017b11a98ecf60525e2900d8a0ad1e135b0ab74b7275ecd94fef0e89f33b6ee522168ad416774bef36dd8b4cd3053ecf77280878b97558e5c922b644780df0fd04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";
const ED25519_SHARED_FACTORY_SIGNATURE: vector<u8> =
    x"000642bed3c16cec16d10a63148898e35cb55c1b344cfa3160242f6ff4e17174cc11b8f9f436f75b7a983bb68420e649cc4ddf26226eafe834338165ac75208002d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";
const ED25519_SHARED_FACTORY_BOB_SIGNATURE: vector<u8> =
    x"0003787b782b267bc790fc393ed17901e1a37ace5c5dce2c4c5226a8b51bac871a5121bc14f064a3e9cf2e633fc9172a2f3cb12e2c141db68db43af7db1784cb0fd04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";

// Secp256k1 proof for the account `create_and_rotate` creates, while its Ed25519 key is attached.
const SECP256K1_ROTATION_SIGNATURE: vector<u8> =
    x"012637239c9ad89ca334bed32a604cc928c49c04f578d82f201f844f51718b9adc2fd13b262326a0e268889d1a63ec051079a81a4818f585bbe83cb9ed47bab48a02770632ba449f7f0f6d7e8173ee8cdeee0c1676a4f02a9c10b877b2c022126a1d";

// === Messages ===

#[test]
fun account_proof_message_names_the_key_account_current_key_address_and_epoch() {
    let message = public_key_proof::account_proof_message(&ed25519(), ACCOUNT, option::none(), 9);

    let mut expected = message_head_ed25519();
    expected.append(
        b"\naccount: 0x0000000000000000000000000000000000000000000000000000000000000acc\ncurrent key address: none\nvalid until epoch: 9",
    );
    assert_eq(message, expected);

    let message = public_key_proof::account_proof_message(
        &ed25519(),
        ACCOUNT,
        option::some(@0xC0DE),
        9,
    );

    let mut expected = message_head_ed25519();
    expected.append(
        b"\naccount: 0x0000000000000000000000000000000000000000000000000000000000000acc\ncurrent key address: 0x000000000000000000000000000000000000000000000000000000000000c0de\nvalid until epoch: 9",
    );
    assert_eq(message, expected);
}

#[test]
fun creation_proof_message_names_the_key_factory_sender_nonce_key_sequence_and_epoch() {
    let factory = object::id_from_address(@0xFAC);
    let message = public_key_proof::creation_proof_message(&ed25519(), factory, @0x5E, 3, 4, 9);

    let mut expected = message_head_ed25519();
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
        option::none(),
        VALID_UNTIL_EPOCH,
        ED25519_ACCOUNT_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidProofSignature)]
fun verify_for_account_rejects_a_proof_for_another_current_key() {
    let mut scenario = test_scenario::begin(@0x0);
    let _verified = public_key_proof::verify_for_account(
        ed25519(),
        ACCOUNT,
        option::some(secp256k1().to_iota_address()),
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
        option::none(),
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
        option::none(),
        VALID_UNTIL_EPOCH,
        ED25519_ACCOUNT_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

// === Owned factory ===

#[test]
fun owned_factory_test() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_owned_factory(scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<AccountFactory>();
    assert_eq(factory.is_shared(), false);
    assert_eq(factory.nonce_keys(), 1);
    assert_eq(factory.next_sequence(0), 0);

    let verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        0,
        VALID_UNTIL_EPOCH,
        ED25519_OWNED_FACTORY_SIGNATURE,
        scenario.ctx(),
    );
    assert_ref_eq(verified.public_key(), &ed25519());
    assert_eq(factory.next_sequence(0), 1);

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
#[expected_failure(abort_code = iota::public_key_proof::ENonceKeyOutOfRange)]
fun owned_factory_has_only_nonce_key_0() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_owned_factory(scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<AccountFactory>();
    let _verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        1,
        VALID_UNTIL_EPOCH,
        ED25519_OWNED_FACTORY_SIGNATURE,
        scenario.ctx(),
    );

    abort
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidProofSignature)]
fun skip_sequence_cancels_the_open_proof() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_owned_factory(scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_from_sender<AccountFactory>();
    factory.skip_sequence(0);
    assert_eq(factory.next_sequence(0), 1);
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
    let factory = scenario.take_from_sender<AccountFactory>();
    public_key_proof::destroy_owned_factory(factory);

    scenario.next_tx(@0x0);
    assert_eq(test_scenario::has_most_recent_for_sender<AccountFactory>(&scenario), false);
    scenario.end();
}

// === Shared factory ===

#[test]
fun shared_factory_test() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(4, scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_shared<AccountFactory>();
    assert_eq(factory.is_shared(), true);
    assert_eq(factory.nonce_keys(), 4);

    let verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        3,
        VALID_UNTIL_EPOCH,
        ED25519_SHARED_FACTORY_SIGNATURE,
        scenario.ctx(),
    );
    // Only the sequence of nonce key 3 moved on.
    assert_eq(factory.next_sequence(3), 1);
    assert_eq(factory.next_sequence(0), 0);

    smart_account_builtin_auth::builder_v1(verified, scenario.ctx()).build_v1();
    test_scenario::return_shared(factory);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidProofSignature)]
fun shared_factory_proof_rejects_another_sender() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(4, scenario.ctx());

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
#[expected_failure(abort_code = iota::public_key_proof::EInvalidProofSignature)]
fun another_sender_moves_a_shared_sequence() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(4, scenario.ctx());

    // @0xB0B creates an account under nonce key 3 first, with its own proof.
    scenario.next_tx(@0xB0B);
    let mut factory = scenario.take_shared<AccountFactory>();
    let verified = public_key_proof::verify_for_new_account(
        ed25519(),
        &mut factory,
        3,
        VALID_UNTIL_EPOCH,
        ED25519_SHARED_FACTORY_BOB_SIGNATURE,
        scenario.ctx(),
    );
    smart_account_builtin_auth::builder_v1(verified, scenario.ctx()).build_v1();
    test_scenario::return_shared(factory);

    // @0x0's proof names sequence 0 under nonce key 3, which is now used.
    scenario.next_tx(@0x0);
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
#[expected_failure(abort_code = iota::public_key_proof::ENonceKeyOutOfRange)]
fun shared_factory_rejects_a_nonce_key_past_its_count() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(3, scenario.ctx());

    scenario.next_tx(@0x0);
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
fun new_shared_factory_accepts_1024_nonce_keys() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(1024, scenario.ctx());

    scenario.next_tx(@0x0);
    let factory = scenario.take_shared<AccountFactory>();
    assert_eq(factory.nonce_keys(), 1024);
    assert_eq(factory.next_sequence(1023), 0);
    test_scenario::return_shared(factory);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidNonceKeyCount)]
fun new_shared_factory_rejects_0_nonce_keys() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(0, scenario.ctx());

    abort
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EInvalidNonceKeyCount)]
fun new_shared_factory_rejects_more_than_1024_nonce_keys() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(1025, scenario.ctx());

    abort
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EFactoryIsShared)]
fun skip_sequence_aborts_for_a_shared_factory() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(4, scenario.ctx());

    scenario.next_tx(@0x0);
    let mut factory = scenario.take_shared<AccountFactory>();
    factory.skip_sequence(3);

    abort
}

#[test]
#[expected_failure(abort_code = iota::public_key_proof::EFactoryIsShared)]
fun destroy_owned_factory_aborts_for_a_shared_factory() {
    let mut scenario = test_scenario::begin(@0x0);
    public_key_proof::new_shared_factory(4, scenario.ctx());

    scenario.next_tx(@0x0);
    let factory = scenario.take_shared<AccountFactory>();
    public_key_proof::destroy_owned_factory(factory);

    abort
}

// === End to end through smart_account_builtin_auth ===

#[test]
fun builder_and_rotation_with_signed_proofs() {
    let mut scenario = test_scenario::begin(@0x0);
    let account_address = create_and_rotate(&mut scenario);

    scenario.next_tx(account_address);
    let account = scenario.take_shared<SmartAccount>();
    assert_ref_eq(smart_account_builtin_auth::borrow_public_key(&account), &secp256k1());
    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
#[expected_failure(
    abort_code = iota::builtin_authenticator_functions::EProofForAnotherCurrentPublicKey,
)]
fun rotation_proof_works_once() {
    let mut scenario = test_scenario::begin(@0x0);
    let account_address = create_and_rotate(&mut scenario);

    // The proof still verifies, but names the Ed25519 key, which is no longer attached.
    scenario.next_tx(account_address);
    let mut account = scenario.take_shared<SmartAccount>();
    let verified = public_key_proof::verify_for_account(
        secp256k1(),
        account_address,
        option::some(ed25519().to_iota_address()),
        VALID_UNTIL_EPOCH,
        SECP256K1_ROTATION_SIGNATURE,
        scenario.ctx(),
    );
    smart_account_builtin_auth::rotate_public_key(&mut account, verified, scenario.ctx());

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

/// Creates an account with the Ed25519 key through an owned factory, then rotates it to the
/// Secp256k1 key. Returns the account's address.
fun create_and_rotate(scenario: &mut test_scenario::Scenario): address {
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
    let current_key_address = option::some(
        smart_account_builtin_auth::borrow_public_key(&account).to_iota_address(),
    );
    let verified = public_key_proof::verify_for_account(
        secp256k1(),
        account_address,
        current_key_address,
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

/// Checks `$signature`, by `$public_key`, over the proof message for `ACCOUNT` while it has no
/// key attached, and attaches the key to an account at `ACCOUNT`.
macro fun account_proof_test($public_key: PublicKey, $signature: vector<u8>) {
    let public_key = $public_key;
    let mut scenario = test_scenario::begin(@0x0);
    let verified = public_key_proof::verify_for_account(
        public_key,
        ACCOUNT,
        option::none(),
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
