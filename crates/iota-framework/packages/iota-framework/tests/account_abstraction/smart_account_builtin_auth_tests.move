// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::smart_account_builtin_auth_tests;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::builtin_authenticator_functions;
use iota::protocol_config;
use iota::public_key::{Self, PublicKey};
use iota::signature_scheme;
use iota::smart_account::{Self, SmartAccount};
use iota::smart_account_builtin_auth;
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};
use std::ascii;

// === Fixtures ===
//
// Ed25519 signatures of IOTA personal messages, in `UserSignature` wire format, made with
// `iota-sdk-crypto` (`sign_personal_message`) by the keys `StdRng::from_seed([0; 32])` gives
// first and second. Each signs the key proof text for the chain identifier these tests see,
// `TX_NONCE` and `VALID_UNTIL_EPOCH`.

/// The raw Ed25519 key K.
const PUBLIC_KEY: vector<u8> = x"b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
/// The raw Ed25519 key K2.
const OTHER_PUBLIC_KEY: vector<u8> = x"aef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";

const VALID_UNTIL_EPOCH: u64 = 5;

/// The nonce of every transaction in these tests.
const TX_NONCE: vector<u8> = x"6e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f";

/// The address of the first account a test creates, as `begin_with_nonce(@0x0)` derives it.
const ACCOUNT: address = @0x034401905bebdf8c04f3cd5f04f442a39372c8dc321c29edfb4f9cb30b23ab96;

/// What the holder of a key signs to attach it to a new account created by `@0x0`.
const NEW_ACCOUNT_KEY_PROOF_MESSAGE: vector<u8> =
    b"check: 0x2::smart_account_builtin_auth::verify_key_proof\nnetwork: 0000000000000000000000000000000000000000000000000000000000000000\naccount: 0x0000000000000000000000000000000000000000000000000000000000000000\nnew account: true\nnonce: 6e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f\nvalid until epoch: 5";

/// K's proof for a new account created by `@0x0`.
const NEW_ACCOUNT_SIGNATURE: vector<u8> =
    x"007c0472624dc80860931677c30e55293066e3e23d783e3d06e607b2e3c263a9a11c9b973db99845e41d796ce9b1c9daed9748f18f6272e34cd53e73efded15309b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
/// K's proof for the existing account `ACCOUNT`.
const ACCOUNT_SIGNATURE: vector<u8> =
    x"001a2873125a7a0e2e992b5d42937492ce282e3995bf85ae854b1e475c2f3cda84d0234160c1b7537994587bdc1349cc117041744c770e84a943dbc6e9e2cfb90db9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
/// K2's proof for the existing account `ACCOUNT`.
const OTHER_KEY_ACCOUNT_SIGNATURE: vector<u8> =
    x"00cf26fe79c2e58746b41563509a8a0025c989d074ff6e9bc7087e3ffa37aa6422fe519749054cdec397867ec9625a7fb254cdab806a111f8371b7abc9d7c8ca02aef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";
/// K2's proof for a new account created by `ACCOUNT`.
const OTHER_KEY_NEW_ACCOUNT_SIGNATURE: vector<u8> =
    x"0097ff8299e4866f7242e470bb5d0bad2e07847bf91031143056c1007bf5396689a44cfdfa4d71d4ec474b580b33f96e627536cdc5cda6c454d6f896270669a60caef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";
/// K2's proof for the existing account `@0x1`.
const OTHER_KEY_OTHER_ACCOUNT_SIGNATURE: vector<u8> =
    x"00403a3896d7b465c7120f8b42070ac894f3f45441bf5a5a3d6e61c247f62da9ceb6ccc4c1cb6a89d4c9703bf8daf3a1c9cd99f3fee6caa2bc3ede130bcfd8100faef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";

// === builder_v1 ===

#[test]
fun builder_v1_attaches_public_key_and_builtin_auth() {
    builtin_account_view_test!(|account| {
        assert_ref_eq(smart_account_builtin_auth::borrow_public_key(account), &key());
        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &builtin_authenticator());
    });
}

#[test]
fun builder_v1_accepts_extra_fields() {
    let mut scenario = begin_with_nonce(@0x0);

    smart_account_builtin_auth::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        scenario.ctx(),
    )
        .with_field(b"answer", 42u64)
        .build_v1();

    next_tx_with_nonce(&mut scenario, @0x0);
    let account = scenario.take_shared<SmartAccount>();

    assert_ref_eq(account.borrow_field<_, u64>(b"answer"), &42u64);
    assert_eq(smart_account_builtin_auth::has_public_key(&account), true);

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
fun builder_v1_accepts_a_proof_in_its_last_valid_epoch() {
    let mut ctx = ctx_in_epoch(@0x0, VALID_UNTIL_EPOCH);

    smart_account_builtin_auth::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EKeyProofExpired)]
fun builder_v1_aborts_for_an_expired_proof() {
    let mut ctx = ctx_in_epoch(@0x0, VALID_UNTIL_EPOCH + 1);

    smart_account_builtin_auth::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun builder_v1_aborts_for_a_proof_by_another_key() {
    let mut ctx = ctx_in_epoch(@0x0, 0);

    smart_account_builtin_auth::builder_v1(
        other_key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun builder_v1_aborts_for_a_proof_for_another_sender() {
    let mut ctx = ctx_in_epoch(@0x1, 0);

    smart_account_builtin_auth::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun builder_v1_aborts_for_a_proof_for_an_existing_account() {
    // The sender is `ACCOUNT`, which `ACCOUNT_SIGNATURE` names as an existing account.
    let mut ctx = ctx_in_epoch(ACCOUNT, 0);

    smart_account_builtin_auth::builder_v1(key(), ACCOUNT_SIGNATURE, VALID_UNTIL_EPOCH, &mut ctx)
        .build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun builder_v1_aborts_for_another_valid_until_epoch() {
    let mut ctx = ctx_in_epoch(@0x0, 0);

    smart_account_builtin_auth::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH + 1,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun builder_v1_aborts_for_a_changed_signature() {
    let mut ctx = ctx_in_epoch(@0x0, 0);
    let mut signature = NEW_ACCOUNT_SIGNATURE;
    *&mut signature[1] = signature[1] ^ 1;

    smart_account_builtin_auth::builder_v1(key(), signature, VALID_UNTIL_EPOCH, &mut ctx)
        .build_v1();
}

// === key_proof_message ===

/// The example of the design: values read from testnet, and the text they give.
const EXAMPLE_CHAIN_IDENTIFIER: vector<u8> =
    x"2304aa976b1765b020886e8b5c7369e169bc779154b9c3efb4dd15c6039c0679";
const EXAMPLE_ACCOUNT: address = @0xf9e4f7f66304ca332d20636b5358b0b9297afbc7428bc107fc5dbc64fb204125;
const EXAMPLE_NONCE: vector<u8> = x"dbf7a1f60789ef83fe154a99c04c583785f44a74760105e17a8e7338c77c6cd1";
const EXAMPLE_MESSAGE: vector<u8> =
    b"check: 0x2::smart_account_builtin_auth::verify_key_proof\nnetwork: 2304aa976b1765b020886e8b5c7369e169bc779154b9c3efb4dd15c6039c0679\naccount: 0xf9e4f7f66304ca332d20636b5358b0b9297afbc7428bc107fc5dbc64fb204125\nnew account: true\nnonce: dbf7a1f60789ef83fe154a99c04c583785f44a74760105e17a8e7338c77c6cd1\nvalid until epoch: 730";

#[test]
fun key_proof_message_is_the_text_for_the_transaction() {
    let ctx = ctx_in_epoch(@0x0, 0);

    assert_eq(
        smart_account_builtin_auth::key_proof_message_for_testing(
            ctx.chain_identifier(),
            @0x0,
            true,
            ctx.tx_nonce(),
            VALID_UNTIL_EPOCH,
        ),
        NEW_ACCOUNT_KEY_PROOF_MESSAGE,
    );
}

#[test]
fun key_proof_message_matches_the_example_of_the_design() {
    assert_eq(
        smart_account_builtin_auth::key_proof_message_for_testing(
            EXAMPLE_CHAIN_IDENTIFIER,
            EXAMPLE_ACCOUNT,
            true,
            EXAMPLE_NONCE,
            730,
        ),
        EXAMPLE_MESSAGE,
    );
}

#[test]
#[expected_failure(abort_code = 5, location = iota::smart_account_builtin_auth)]
fun key_proof_message_aborts_for_a_short_chain_identifier() {
    smart_account_builtin_auth::key_proof_message_for_testing(
        x"2304aa97",
        EXAMPLE_ACCOUNT,
        true,
        EXAMPLE_NONCE,
        730,
    );
}

#[test]
#[expected_failure(abort_code = 5, location = iota::smart_account_builtin_auth)]
fun key_proof_message_aborts_for_a_short_nonce() {
    smart_account_builtin_auth::key_proof_message_for_testing(
        EXAMPLE_CHAIN_IDENTIFIER,
        EXAMPLE_ACCOUNT,
        true,
        x"dbf7a1f6",
        730,
    );
}

// === claim_account_v1 ===

#[test]
fun claim_account_v1_creates_shared_account_at_sender_address() {
    let public_key = key();
    let sender = public_key.to_iota_address();
    let mut scenario = begin_with_nonce(sender);

    smart_account_builtin_auth::claim_account_v1_for_testing(public_key, scenario.ctx());

    next_tx_with_nonce(&mut scenario, sender);
    let account = scenario.take_shared<SmartAccount>();
    assert_eq(account.account_address(), sender);
    assert_ref_eq(smart_account_builtin_auth::borrow_public_key(&account), &public_key);
    assert_eq(smart_account_builtin_auth::has_builtin_auth(&account), true);
    test_scenario::return_shared(account);

    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::claim::EAddressMismatch)]
fun claim_account_v1_aborts_on_address_mismatch() {
    let mut scenario = begin_with_nonce(@0x1);

    smart_account_builtin_auth::claim_account_v1_for_testing(key(), scenario.ctx());

    scenario.end();
}

#[test]
#[expected_failure(abort_code = smart_account::ESmartAccountsNotEnabled)]
fun claim_account_v1_aborts_without_smart_accounts() {
    protocol_config::set_feature_enabled_for_testing(b"enable_smart_accounts", false);
    let public_key = key();
    let mut scenario = begin_with_nonce(public_key.to_iota_address());

    smart_account_builtin_auth::claim_account_v1_for_testing(public_key, scenario.ctx());

    abort
}

// === View functions ===

#[test]
fun custom_account_has_no_public_key_or_builtin_auth() {
    custom_account_view_test!(|account| {
        assert_eq(smart_account_builtin_auth::has_public_key(account), false);
        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), false);
    });
}

#[test]
#[expected_failure(abort_code = builtin_authenticator_functions::EPublicKeyMissing)]
fun borrow_public_key_aborts_without_a_key() {
    custom_account_view_test!(|account| {
        smart_account_builtin_auth::borrow_public_key(account);
    });
}

// === rotate_to_builtin_auth_v1 ===

#[test]
fun rotate_to_builtin_auth_v1_attaches_key_and_sets_builtin_auth() {
    custom_account_test!(|account, scenario| {
        let previous = smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            key(),
            ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );

        assert_eq(previous, custom_authenticator());
        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
        assert_ref_eq(smart_account_builtin_auth::borrow_public_key(account), &key());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EBuiltinAuthAlreadySet)]
fun rotate_to_builtin_auth_v1_aborts_if_builtin_auth_already_set() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_to_builtin_auth_v1_aborts_if_sender_not_account() {
    custom_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            key(),
            ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun rotate_to_builtin_auth_v1_aborts_for_a_proof_for_a_new_account() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            other_key(),
            OTHER_KEY_NEW_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun rotate_to_builtin_auth_v1_aborts_for_a_proof_for_another_account() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            other_key(),
            OTHER_KEY_OTHER_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun rotate_to_builtin_auth_v1_aborts_for_a_proof_by_another_key() {
    custom_account_test!(|account, scenario| {
        // `ACCOUNT_SIGNATURE` is K's proof for this account, but the key attached is K2.
        smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            other_key(),
            ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

// === rotate_to_custom_auth_v1 ===

#[test]
fun rotate_to_custom_auth_v1_detaches_the_key() {
    builtin_account_test!(|account, scenario| {
        let detached = smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );

        assert_eq(detached, key());
        assert_eq(smart_account_builtin_auth::has_public_key(account), false);
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &custom_authenticator());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EBuiltinAuthNotSet)]
fun rotate_to_custom_auth_v1_aborts_without_the_builtin_authenticator() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            other_custom_authenticator(),
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EAuthenticatorIsBuiltin)]
fun rotate_to_custom_auth_v1_aborts_for_the_builtin_authenticator() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            builtin_authenticator(),
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_to_custom_auth_v1_aborts_if_sender_not_account() {
    builtin_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );
    });
}

#[test]
fun rotating_to_custom_and_back_to_builtin_auth_takes_a_new_key() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );
        smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );

        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
        assert_ref_eq(smart_account_builtin_auth::borrow_public_key(account), &other_key());
    });
}

// === rotate_public_key ===

#[test]
fun rotate_public_key_replaces_the_key() {
    builtin_account_test!(|account, scenario| {
        let previous = smart_account_builtin_auth::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );

        assert_eq(previous, key());
        assert_ref_eq(smart_account_builtin_auth::borrow_public_key(account), &other_key());
        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
    });
}

#[test]
fun rotate_public_key_accepts_the_same_proof_twice_in_one_transaction() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
        let previous = smart_account_builtin_auth::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );

        assert_eq(previous, other_key());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EBuiltinAuthNotSet)]
fun rotate_public_key_aborts_without_builtin_auth() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_public_key_aborts_if_sender_not_account() {
    builtin_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun rotate_public_key_aborts_for_a_proof_for_a_new_account() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_NEW_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun rotate_public_key_aborts_for_a_proof_for_another_account() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_OTHER_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun rotate_public_key_aborts_for_a_proof_by_the_current_key() {
    builtin_account_test!(|account, scenario| {
        // `ACCOUNT_SIGNATURE` is a proof by K, the key the account has now, not by the new key K2.
        smart_account_builtin_auth::rotate_public_key(
            account,
            other_key(),
            ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EInvalidKeyProof)]
fun rotate_public_key_refuses_a_multisig_key() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_public_key(
            account,
            multisig_public_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

// === Core rotation ===

#[test]
#[expected_failure(abort_code = builtin_authenticator_functions::EBuiltinAuthWithoutPublicKey)]
fun core_rotation_refuses_builtin_auth_without_a_key() {
    custom_account_test!(|account, scenario| {
        account.rotate_auth_function_ref_v1(builtin_authenticator(), scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = builtin_authenticator_functions::EPublicKeyWithoutBuiltinAuth)]
fun core_rotation_refuses_a_custom_authenticator_while_the_key_is_attached() {
    builtin_account_test!(|account, scenario| {
        account.rotate_auth_function_ref_v1(custom_authenticator(), scenario.ctx());
    });
}

#[test]
fun core_rotation_between_custom_authenticators_works() {
    custom_account_test!(|account, scenario| {
        account.rotate_auth_function_ref_v1(other_custom_authenticator(), scenario.ctx());

        assert_ref_eq(account.borrow_auth_function_ref_v1(), &other_custom_authenticator());
    });
}

// === Helpers ===

fun key(): PublicKey {
    public_key::create(signature_scheme::ed25519(), PUBLIC_KEY)
}

fun other_key(): PublicKey {
    public_key::create(signature_scheme::ed25519(), OTHER_PUBLIC_KEY)
}

/// Returns the context of a transaction sent by `sender` and executed in `epoch`, with
/// `TX_NONCE`.
fun ctx_in_epoch(sender: address, epoch: u64): TxContext {
    let mut ctx = tx_context::new_from_hint(sender, 0, epoch, 0, 0);
    ctx.set_tx_nonce_for_testing(TX_NONCE);
    ctx
}

/// Begins a scenario whose first transaction, sent by `sender`, has `TX_NONCE`.
fun begin_with_nonce(sender: address): Scenario {
    let mut scenario = test_scenario::begin(sender);
    scenario.ctx().set_tx_nonce_for_testing(TX_NONCE);
    scenario
}

/// Starts the next transaction of `scenario`, sent by `sender`, with `TX_NONCE`.
fun next_tx_with_nonce(scenario: &mut Scenario, sender: address) {
    scenario.next_tx(sender);
    scenario.ctx().set_tx_nonce_for_testing(TX_NONCE);
}

fun multisig_public_key(): PublicKey {
    // 1-of-1 MultiSig over the zero Ed25519 key.
    public_key::create(
        signature_scheme::multisig(),
        x"01000000000000000000000000000000000000000000000000000000000000000000010100",
    )
}

fun custom_authenticator(): AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0xABC,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

fun other_custom_authenticator(): AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0xDEF,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

/// The built-in authenticator ref, as `smart_account_builtin_auth::builder_v1` attaches it.
fun builtin_authenticator(): AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x2,
        ascii::string(b"builtin_authenticator_functions"),
        ascii::string(b"builtin_authenticator_v1"),
    )
}

/// Runs `$f` on a shared `SmartAccount` at `ACCOUNT`, built by
/// `smart_account_builtin_auth::builder_v1` with K, where the sender is the account itself.
macro fun builtin_account_test($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = begin_with_nonce(@0x0);

    let addr = smart_account_builtin_auth::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        scenario.ctx(),
    ).build_v1();

    next_tx_with_nonce(&mut scenario, addr);
    let mut account = scenario.take_shared<SmartAccount>();

    $f(&mut account, &mut scenario);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` on a shared `SmartAccount` at `ACCOUNT`, built by
/// `smart_account_builtin_auth::builder_v1` with K, where the sender is `@0x1` — not the account.
macro fun builtin_account_wrong_sender_test($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = begin_with_nonce(@0x0);

    smart_account_builtin_auth::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        scenario.ctx(),
    ).build_v1();

    next_tx_with_nonce(&mut scenario, @0x1);
    let mut account = scenario.take_shared<SmartAccount>();

    $f(&mut account, &mut scenario);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` on a shared `SmartAccount` at `ACCOUNT`, built by
/// `smart_account_builtin_auth::builder_v1` with K.
macro fun builtin_account_view_test($f: |&SmartAccount|) {
    let mut scenario = begin_with_nonce(@0x0);
    smart_account_builtin_auth::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        scenario.ctx(),
    ).build_v1();

    next_tx_with_nonce(&mut scenario, @0x0);
    let account = scenario.take_shared<SmartAccount>();

    $f(&account);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` on a shared `SmartAccount` at `ACCOUNT`, built with a custom authenticator and no
/// key, where the sender is the account itself.
macro fun custom_account_test($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = begin_with_nonce(@0x0);

    let addr = smart_account::builder_v1(custom_authenticator(), scenario.ctx()).build_v1();

    next_tx_with_nonce(&mut scenario, addr);
    let mut account = scenario.take_shared<SmartAccount>();

    $f(&mut account, &mut scenario);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` on a shared `SmartAccount` at `ACCOUNT`, built with a custom authenticator and no
/// key.
macro fun custom_account_view_test($f: |&SmartAccount|) {
    let mut scenario = begin_with_nonce(@0x0);
    smart_account::builder_v1(custom_authenticator(), scenario.ctx()).build_v1();

    next_tx_with_nonce(&mut scenario, @0x0);
    let account = scenario.take_shared<SmartAccount>();

    $f(&account);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` on a shared `SmartAccount` built with a custom authenticator, where the sender is
/// `@0x1` — not the account.
macro fun custom_account_wrong_sender_test($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = begin_with_nonce(@0x0);

    smart_account::builder_v1(custom_authenticator(), scenario.ctx()).build_v1();

    next_tx_with_nonce(&mut scenario, @0x1);
    let mut account = scenario.take_shared<SmartAccount>();

    $f(&mut account, &mut scenario);

    test_scenario::return_shared(account);
    scenario.end();
}
