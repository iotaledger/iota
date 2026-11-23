// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::smart_account_builtin_authenticator_tests;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::bcs;
use iota::builtin_authenticator_functions::{
    Self,
    PublicKeyAttached,
    PublicKeyDetached,
    PublicKeyRotated
};
use iota::event;
use iota::protocol_config;
use iota::public_key::{Self, PublicKey};
use iota::signature_scheme;
use iota::smart_account::{Self, SmartAccount};
use iota::smart_account_builtin_authenticator;
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
    b"check: 0x2::smart_account_builtin_authenticator::verify_key_proof\nnetwork: 0000000000000000000000000000000000000000000000000000000000000000\naccount: 0x0000000000000000000000000000000000000000000000000000000000000000\nnew account: true\nnonce: 6e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f6e63656e6f\nvalid until epoch: 5";

/// K's proof for a new account created by `@0x0`.
const NEW_ACCOUNT_SIGNATURE: vector<u8> =
    x"009c697c2a7f818f54b536370b76ea0b4e21aa1fbb5b4943a000eb745933452b439c87bbbe0dcca5760e97dff622f37b0eb892b292ac2ef365448142137667ff06b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
/// K's proof for the existing account `ACCOUNT`.
const ACCOUNT_SIGNATURE: vector<u8> =
    x"006033459bc0830d6a78539367f4313fc276d2ac9ddd2f89291e97c46d37302fc23fefc479d2308d36187dc3dc721287acc8b0c4c33c0f931207367a0139235302b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
/// K2's proof for the existing account `ACCOUNT`.
const OTHER_KEY_ACCOUNT_SIGNATURE: vector<u8> =
    x"000120ea8d639a59e21da8310d455328ce60cac2ff58ec885c9228cee6bb78f8ac5a73bcbbe5e3c592e5dec83ec90b769e14eb4e81ad8d542b1604c0a410ebe901aef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";
/// K2's proof for a new account created by `ACCOUNT`.
const OTHER_KEY_NEW_ACCOUNT_SIGNATURE: vector<u8> =
    x"00087f0fbbe78f02b361682a2524a6ddd1261c854e58a16f3ae7b84aa5a8ffdbbbffd0bc423e04b83185157674b1705b84a688e03c704e643a92ac013069270503aef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";
/// K2's proof for the existing account `@0x1`.
const OTHER_KEY_OTHER_ACCOUNT_SIGNATURE: vector<u8> =
    x"0091f1e4a388e34d20accc8c1748c960098c4cb39568981a6d1a04498e45930ebeb5f7b7a41085e7bc2788c37f8ee797c46c9195ceddf3e4c68af8858c7164d602aef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";
/// K's signature over `NEW_ACCOUNT_KEY_PROOF_MESSAGE` with the first line replaced by the check of
/// another package, `check: 0xabc::smart_account_builtin_authenticator::verify_key_proof`.
const OTHER_CHECK_NEW_ACCOUNT_SIGNATURE: vector<u8> =
    x"00ff7d1f61e51c32450245bf75fc32309869915e2f787f0ea6e33cd4c4d0c20dc99c0a2850c6bcb977cd741a9336cb6c184e3663e179e8556257e4b0058ceb9401b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";

// === builder_v1 ===

#[test]
fun builder_v1_attaches_public_key_and_builtin_authenticator() {
    builtin_account_view_test!(|account| {
        assert_ref_eq(smart_account_builtin_authenticator::borrow_public_key(account), &key());
        assert_eq(smart_account_builtin_authenticator::has_builtin_authenticator(account), true);
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &builtin_authenticator());
    });
}

#[test]
fun builder_v1_accepts_extra_fields() {
    let mut scenario = begin_with_nonce(@0x0);

    smart_account_builtin_authenticator::builder_v1(
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
    assert_eq(smart_account_builtin_authenticator::has_public_key(&account), true);

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
fun builder_v1_emits_public_key_attached() {
    let mut scenario = begin_with_nonce(@0x0);

    let account = smart_account_builtin_authenticator::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        scenario.ctx(),
    ).build_v1();

    assert_public_key_event_counts(1, 0, 0);
    assert_eq(
        bcs::to_bytes(&event::events_by_type<PublicKeyAttached>()[0]),
        public_key_event_bytes(object::id_from_address(account), key()),
    );

    scenario.end();
}

#[test]
fun builder_v1_accepts_a_proof_in_its_last_valid_epoch() {
    let mut ctx = ctx_in_epoch(@0x0, VALID_UNTIL_EPOCH);

    smart_account_builtin_authenticator::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EKeyProofExpired)]
fun builder_v1_aborts_for_an_expired_proof() {
    let mut ctx = ctx_in_epoch(@0x0, VALID_UNTIL_EPOCH + 1);

    smart_account_builtin_authenticator::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun builder_v1_aborts_for_a_proof_by_another_key() {
    let mut ctx = ctx_in_epoch(@0x0, 0);

    smart_account_builtin_authenticator::builder_v1(
        other_key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun builder_v1_aborts_for_a_proof_for_another_sender() {
    let mut ctx = ctx_in_epoch(@0x1, 0);

    smart_account_builtin_authenticator::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun builder_v1_aborts_for_a_proof_for_an_existing_account() {
    // The sender is `ACCOUNT`, which `ACCOUNT_SIGNATURE` names as an existing account.
    let mut ctx = ctx_in_epoch(ACCOUNT, 0);

    smart_account_builtin_authenticator::builder_v1(
        key(),
        ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun builder_v1_aborts_for_another_valid_until_epoch() {
    let mut ctx = ctx_in_epoch(@0x0, 0);

    smart_account_builtin_authenticator::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH + 1,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun builder_v1_aborts_for_a_proof_for_another_check() {
    let mut ctx = ctx_in_epoch(@0x0, 0);

    smart_account_builtin_authenticator::builder_v1(
        key(),
        OTHER_CHECK_NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        &mut ctx,
    ).build_v1();
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun builder_v1_aborts_for_a_changed_signature() {
    let mut ctx = ctx_in_epoch(@0x0, 0);
    let mut signature = NEW_ACCOUNT_SIGNATURE;
    *&mut signature[1] = signature[1] ^ 1;

    smart_account_builtin_authenticator::builder_v1(key(), signature, VALID_UNTIL_EPOCH, &mut ctx)
        .build_v1();
}

#[test]
#[expected_failure(abort_code = builtin_authenticator_functions::EBuiltinAuthenticatorsNotEnabled)]
fun builder_v1_aborts_without_builtin_authenticators() {
    protocol_config::set_feature_enabled_for_testing(b"enable_builtin_move_authenticators", false);
    let mut ctx = ctx_in_epoch(@0x0, 0);

    smart_account_builtin_authenticator::builder_v1(
        key(),
        NEW_ACCOUNT_SIGNATURE,
        VALID_UNTIL_EPOCH,
        &mut ctx,
    ).build_v1();
}

// === key_proof_message ===

/// The example of the design: values read from testnet, and the text they give.
const EXAMPLE_CHAIN_IDENTIFIER: vector<u8> =
    x"2304aa976b1765b020886e8b5c7369e169bc779154b9c3efb4dd15c6039c0679";
const EXAMPLE_ACCOUNT: address = @0xf9e4f7f66304ca332d20636b5358b0b9297afbc7428bc107fc5dbc64fb204125;
const EXAMPLE_NONCE: vector<u8> = x"dbf7a1f60789ef83fe154a99c04c583785f44a74760105e17a8e7338c77c6cd1";
const EXAMPLE_MESSAGE: vector<u8> =
    b"check: 0x2::smart_account_builtin_authenticator::verify_key_proof\nnetwork: 2304aa976b1765b020886e8b5c7369e169bc779154b9c3efb4dd15c6039c0679\naccount: 0xf9e4f7f66304ca332d20636b5358b0b9297afbc7428bc107fc5dbc64fb204125\nnew account: true\nnonce: dbf7a1f60789ef83fe154a99c04c583785f44a74760105e17a8e7338c77c6cd1\nvalid until epoch: 730";

#[test]
fun key_proof_message_is_the_text_for_the_transaction() {
    let ctx = ctx_in_epoch(@0x0, 0);

    assert_eq(
        smart_account_builtin_authenticator::key_proof_message_for_testing(
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
        smart_account_builtin_authenticator::key_proof_message_for_testing(
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
#[expected_failure(abort_code = 5, location = iota::smart_account_builtin_authenticator)]
fun key_proof_message_aborts_for_a_short_chain_identifier() {
    smart_account_builtin_authenticator::key_proof_message_for_testing(
        x"2304aa97",
        EXAMPLE_ACCOUNT,
        true,
        EXAMPLE_NONCE,
        730,
    );
}

#[test]
#[expected_failure(abort_code = 5, location = iota::smart_account_builtin_authenticator)]
fun key_proof_message_aborts_for_a_short_nonce() {
    smart_account_builtin_authenticator::key_proof_message_for_testing(
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

    smart_account_builtin_authenticator::claim_account_v1_for_testing(public_key, scenario.ctx());

    next_tx_with_nonce(&mut scenario, sender);
    let account = scenario.take_shared<SmartAccount>();
    assert_eq(account.account_address(), sender);
    assert_ref_eq(smart_account_builtin_authenticator::borrow_public_key(&account), &public_key);
    assert_eq(smart_account_builtin_authenticator::has_builtin_authenticator(&account), true);
    test_scenario::return_shared(account);

    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::claim::EAddressMismatch)]
fun claim_account_v1_aborts_on_address_mismatch() {
    let mut scenario = begin_with_nonce(@0x1);

    smart_account_builtin_authenticator::claim_account_v1_for_testing(key(), scenario.ctx());

    scenario.end();
}

#[test]
#[expected_failure(abort_code = smart_account::ESmartAccountsNotEnabled)]
fun claim_account_v1_aborts_without_smart_accounts() {
    protocol_config::set_feature_enabled_for_testing(b"enable_smart_accounts", false);
    let public_key = key();
    let mut scenario = begin_with_nonce(public_key.to_iota_address());

    smart_account_builtin_authenticator::claim_account_v1_for_testing(public_key, scenario.ctx());

    abort
}

// === View functions ===

#[test]
fun custom_account_has_no_public_key_or_builtin_authenticator() {
    custom_account_view_test!(|account| {
        assert_eq(smart_account_builtin_authenticator::has_public_key(account), false);
        assert_eq(smart_account_builtin_authenticator::has_builtin_authenticator(account), false);
    });
}

#[test]
#[expected_failure(abort_code = builtin_authenticator_functions::EPublicKeyMissing)]
fun borrow_public_key_aborts_without_a_key() {
    custom_account_view_test!(|account| {
        smart_account_builtin_authenticator::borrow_public_key(account);
    });
}

// === rotate_to_builtin_authenticator_v1 ===

#[test]
fun rotate_to_builtin_authenticator_v1_attaches_key_and_sets_builtin_authenticator() {
    custom_account_test!(|account, scenario| {
        let previous = smart_account_builtin_authenticator::rotate_to_builtin_authenticator_v1(
            account,
            key(),
            ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );

        assert_eq(previous, custom_authenticator());
        assert_eq(smart_account_builtin_authenticator::has_builtin_authenticator(account), true);
        assert_ref_eq(smart_account_builtin_authenticator::borrow_public_key(account), &key());
    });
}

#[test]
fun rotate_to_builtin_authenticator_v1_emits_public_key_attached() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_to_builtin_authenticator_v1(
            account,
            key(),
            ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );

        assert_public_key_event_counts(1, 0, 0);
        assert_eq(
            bcs::to_bytes(&event::events_by_type<PublicKeyAttached>()[0]),
            public_key_event_bytes(object::id(account), key()),
        );
    });
}

#[test]
#[expected_failure(abort_code = builtin_authenticator_functions::EBuiltinAuthenticatorsNotEnabled)]
fun rotate_to_builtin_authenticator_v1_aborts_without_builtin_authenticators() {
    custom_account_test!(|account, scenario| {
        protocol_config::set_feature_enabled_for_testing(
            b"enable_builtin_move_authenticators",
            false,
        );

        smart_account_builtin_authenticator::rotate_to_builtin_authenticator_v1(
            account,
            key(),
            ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[
    expected_failure(
        abort_code = iota::smart_account_builtin_authenticator::EBuiltinAuthenticatorAlreadySet,
    ),
]
fun rotate_to_builtin_authenticator_v1_aborts_if_builtin_authenticator_already_set() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_to_builtin_authenticator_v1(
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
fun rotate_to_builtin_authenticator_v1_aborts_if_sender_not_account() {
    custom_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_to_builtin_authenticator_v1(
            account,
            key(),
            ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun rotate_to_builtin_authenticator_v1_aborts_for_a_proof_for_a_new_account() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_to_builtin_authenticator_v1(
            account,
            other_key(),
            OTHER_KEY_NEW_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun rotate_to_builtin_authenticator_v1_aborts_for_a_proof_for_another_account() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_to_builtin_authenticator_v1(
            account,
            other_key(),
            OTHER_KEY_OTHER_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun rotate_to_builtin_authenticator_v1_aborts_for_a_proof_by_another_key() {
    custom_account_test!(|account, scenario| {
        // `ACCOUNT_SIGNATURE` is K's proof for this account, but the key attached is K2.
        smart_account_builtin_authenticator::rotate_to_builtin_authenticator_v1(
            account,
            other_key(),
            ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

// === rotate_to_custom_authenticator_v1 ===

#[test]
fun rotate_to_custom_authenticator_v1_detaches_the_key() {
    builtin_account_test!(|account, scenario| {
        let detached = smart_account_builtin_authenticator::rotate_to_custom_authenticator_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );

        assert_eq(detached, key());
        assert_eq(smart_account_builtin_authenticator::has_public_key(account), false);
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &custom_authenticator());
    });
}

#[test]
fun rotate_to_custom_authenticator_v1_emits_public_key_detached() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_to_custom_authenticator_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );

        assert_public_key_event_counts(0, 1, 0);
        assert_eq(
            bcs::to_bytes(&event::events_by_type<PublicKeyDetached>()[0]),
            public_key_event_bytes(object::id(account), key()),
        );
    });
}

#[test]
#[
    expected_failure(
        abort_code = iota::smart_account_builtin_authenticator::EBuiltinAuthenticatorNotSet,
    ),
]
fun rotate_to_custom_authenticator_v1_aborts_without_the_builtin_authenticator() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_to_custom_authenticator_v1(
            account,
            other_custom_authenticator(),
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EAuthenticatorIsBuiltin)]
fun rotate_to_custom_authenticator_v1_aborts_for_the_builtin_authenticator() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_to_custom_authenticator_v1(
            account,
            builtin_authenticator(),
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_to_custom_authenticator_v1_aborts_if_sender_not_account() {
    builtin_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_to_custom_authenticator_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );
    });
}

#[test]
fun rotating_to_custom_and_back_to_builtin_authenticator_takes_a_new_key() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_to_custom_authenticator_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );
        smart_account_builtin_authenticator::rotate_to_builtin_authenticator_v1(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );

        assert_eq(smart_account_builtin_authenticator::has_builtin_authenticator(account), true);
        assert_ref_eq(
            smart_account_builtin_authenticator::borrow_public_key(account),
            &other_key(),
        );
    });
}

// === rotate_public_key ===

#[test]
fun rotate_public_key_replaces_the_key() {
    builtin_account_test!(|account, scenario| {
        let previous = smart_account_builtin_authenticator::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );

        assert_eq(previous, key());
        assert_ref_eq(
            smart_account_builtin_authenticator::borrow_public_key(account),
            &other_key(),
        );
        assert_eq(smart_account_builtin_authenticator::has_builtin_authenticator(account), true);
    });
}

#[test]
fun rotate_public_key_emits_public_key_rotated() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );

        assert_public_key_event_counts(0, 0, 1);
        assert_eq(
            bcs::to_bytes(&event::events_by_type<PublicKeyRotated>()[0]),
            public_key_rotated_event_bytes(object::id(account), key(), other_key()),
        );
    });
}

#[test]
fun rotate_public_key_accepts_the_same_proof_twice_in_one_transaction() {
    builtin_account_test!(|account, scenario| {
        // K to K2, back to K, then K2 again with the same proof as the first rotation.
        let previous = smart_account_builtin_authenticator::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
        assert_eq(previous, key());

        let previous = smart_account_builtin_authenticator::rotate_public_key(
            account,
            key(),
            ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
        assert_eq(previous, other_key());

        let previous = smart_account_builtin_authenticator::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
        assert_eq(previous, key());
        assert_ref_eq(
            smart_account_builtin_authenticator::borrow_public_key(account),
            &other_key(),
        );
    });
}

#[test]
#[
    expected_failure(
        abort_code = iota::smart_account_builtin_authenticator::EBuiltinAuthenticatorNotSet,
    ),
]
fun rotate_public_key_aborts_without_builtin_authenticator() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_public_key(
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
        smart_account_builtin_authenticator::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun rotate_public_key_aborts_for_a_proof_for_a_new_account() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_NEW_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun rotate_public_key_aborts_for_a_proof_for_another_account() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_public_key(
            account,
            other_key(),
            OTHER_KEY_OTHER_ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun rotate_public_key_aborts_for_a_proof_by_the_current_key() {
    builtin_account_test!(|account, scenario| {
        // `ACCOUNT_SIGNATURE` is a proof by K, the key the account has now, not by the new key K2.
        smart_account_builtin_authenticator::rotate_public_key(
            account,
            other_key(),
            ACCOUNT_SIGNATURE,
            VALID_UNTIL_EPOCH,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_authenticator::EInvalidKeyProof)]
fun rotate_public_key_refuses_a_multisig_key() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_authenticator::rotate_public_key(
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
#[
    expected_failure(
        abort_code = builtin_authenticator_functions::EBuiltinAuthenticatorWithoutPublicKey,
    ),
]
fun core_rotation_refuses_builtin_authenticator_without_a_key() {
    custom_account_test!(|account, scenario| {
        account.rotate_auth_function_ref_v1(builtin_authenticator(), scenario.ctx());
    });
}

#[test]
#[
    expected_failure(
        abort_code = builtin_authenticator_functions::EPublicKeyWithoutBuiltinAuthenticator,
    ),
]
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

/// The built-in authenticator ref, as `smart_account_builtin_authenticator::builder_v1` attaches
/// it.
fun builtin_authenticator(): AuthenticatorFunctionRefV1<SmartAccount> {
    builtin_authenticator_functions::builtin_authenticator_function_ref_v1()
}

/// Asserts how many `PublicKeyAttached`, `PublicKeyDetached` and `PublicKeyRotated` events the
/// current transaction has emitted.
fun assert_public_key_event_counts(attached: u64, detached: u64, rotated: u64) {
    assert_eq(event::events_by_type<PublicKeyAttached>().length(), attached);
    assert_eq(event::events_by_type<PublicKeyDetached>().length(), detached);
    assert_eq(event::events_by_type<PublicKeyRotated>().length(), rotated);
}

// The event fields are private to `builtin_authenticator_functions`, so the tests compare BCS
// bytes: a struct's BCS bytes are those of its fields, in declaration order.

/// Returns the BCS bytes of a `PublicKeyAttached` or `PublicKeyDetached` event.
fun public_key_event_bytes(account_id: ID, public_key: PublicKey): vector<u8> {
    let mut bytes = bcs::to_bytes(&account_id);
    bytes.append(bcs::to_bytes(&public_key));
    bytes
}

/// Returns the BCS bytes of a `PublicKeyRotated` event.
fun public_key_rotated_event_bytes(account_id: ID, from: PublicKey, to: PublicKey): vector<u8> {
    let mut bytes = public_key_event_bytes(account_id, from);
    bytes.append(bcs::to_bytes(&to));
    bytes
}

/// Runs `$f` on a shared `SmartAccount` at `ACCOUNT`, built by
/// `smart_account_builtin_authenticator::builder_v1` with K, where the sender is the account
/// itself.
macro fun builtin_account_test($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = begin_with_nonce(@0x0);

    let addr = smart_account_builtin_authenticator::builder_v1(
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
/// `smart_account_builtin_authenticator::builder_v1` with K, where the sender is `@0x1` — not the
/// account.
macro fun builtin_account_wrong_sender_test($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = begin_with_nonce(@0x0);

    smart_account_builtin_authenticator::builder_v1(
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
/// `smart_account_builtin_authenticator::builder_v1` with K.
macro fun builtin_account_view_test($f: |&SmartAccount|) {
    let mut scenario = begin_with_nonce(@0x0);
    smart_account_builtin_authenticator::builder_v1(
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
