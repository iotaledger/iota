// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module onesig::account_tests;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::hash;
use iota::hex;
use iota::test_scenario::{Self, Scenario};
use onesig::account::{Self, Account};
use onesig::merkle;
use std::ascii;

#[test]
fun test_happy_path() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let public_key = x"5ae220b4b2f65e977c12ede61579ff5170b6c22c006168c37b5e7c61af018083";
    let account_address = create_account_for_testing(scenario, public_key);

    let tx1_digest = b"00000000000000000000000000000001";
    let tx2_digest = b"00000000000000000000000000000002";
    let tx3_digest = b"00000000000000000000000000000003";

    let (merkle_root, tx1_proof, tx2_proof, tx3_proof) = create_merkle_tree_for_testing(
        tx1_digest,
        tx2_digest,
        tx3_digest,
    );

    // This signature is used for authenticating all three transactions, as they are all part of the same Merkle tree with the same root.
    let signature =
        x"12f93594a9865bfef88faa1c728829712cf3f52a31bb268ab29cccb5e1db57db051c3650f9f5b8925509c8fcd07182ae41ffc8505e70becf866e72a091279802";
    let signature_hex = hex::encode(signature);

    // Authenticate the first transaction
    test_scenario::next_tx(scenario, account_address);
    {
        let account = test_scenario::take_shared<Account>(scenario);
        let ctx = create_tx_context_for_testing(account_address, tx1_digest);
        let auth_ctx = create_auth_context_for_testing();

        account::authenticate(
            &account,
            merkle_root,
            tx1_proof,
            signature_hex,
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    // Authenticate the second transaction
    test_scenario::next_tx(scenario, account_address);
    {
        let account = test_scenario::take_shared<Account>(scenario);
        let ctx = create_tx_context_for_testing(account_address, tx2_digest);
        let auth_ctx = create_auth_context_for_testing();

        account::authenticate(
            &account,
            merkle_root,
            tx2_proof,
            signature_hex,
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    // Authenticate the third transaction
    test_scenario::next_tx(scenario, account_address);
    {
        let account = test_scenario::take_shared<Account>(scenario);
        let ctx = create_tx_context_for_testing(account_address, tx3_digest);
        let auth_ctx = create_auth_context_for_testing();

        account::authenticate(
            &account,
            merkle_root,
            tx3_proof,
            signature_hex,
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = account::EEd25519VerificationFailed)]
fun test_invalid_signature() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let public_key = x"5ae220b4b2f65e977c12ede61579ff5170b6c22c006168c37b5e7c61af018083";
    let account_address = create_account_for_testing(scenario, public_key);

    let tx1_digest = b"00000000000000000000000000000001";
    let tx2_digest = b"00000000000000000000000000000002";
    let tx3_digest = b"00000000000000000000000000000003";

    let (merkle_root, tx1_proof, _, _) = create_merkle_tree_for_testing(
        tx1_digest,
        tx2_digest,
        tx3_digest,
    );

    // Invalid signature: last byte changed from 0x02 to 0x00
    let signature =
        x"12f93594a9865bfef88faa1c728829712cf3f52a31bb268ab29cccb5e1db57db051c3650f9f5b8925509c8fcd07182ae41ffc8505e70becf866e72a091279800";
    let signature_hex = hex::encode(signature);

    // Authenticate the first transaction with an invalid signature
    test_scenario::next_tx(scenario, account_address);
    {
        let account = test_scenario::take_shared<Account>(scenario);
        let ctx = create_tx_context_for_testing(account_address, tx1_digest);
        let auth_ctx = create_auth_context_for_testing();

        account::authenticate(
            &account,
            merkle_root,
            tx1_proof,
            signature_hex,
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = account::EInvalidMerkleProof)]
fun test_invalid_merkle_proof() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let public_key = x"5ae220b4b2f65e977c12ede61579ff5170b6c22c006168c37b5e7c61af018083";
    let account_address = create_account_for_testing(scenario, public_key);

    let tx1_digest = b"00000000000000000000000000000001";
    let tx2_digest = b"00000000000000000000000000000002";
    let tx3_digest = b"00000000000000000000000000000003";

    let (merkle_root, _, tx2_proof, _) = create_merkle_tree_for_testing(
        tx1_digest,
        tx2_digest,
        tx3_digest,
    );

    let signature =
        x"12f93594a9865bfef88faa1c728829712cf3f52a31bb268ab29cccb5e1db57db051c3650f9f5b8925509c8fcd07182ae41ffc8505e70becf866e72a091279802";
    let signature_hex = hex::encode(signature);

    // Authenticate the first transaction with a wrong proof (tx2_proof instead of tx1_proof)
    test_scenario::next_tx(scenario, account_address);
    {
        let account = test_scenario::take_shared<Account>(scenario);
        let ctx = create_tx_context_for_testing(account_address, tx1_digest);
        let auth_ctx = create_auth_context_for_testing();

        account::authenticate(
            &account,
            merkle_root,
            tx2_proof,
            signature_hex,
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

fun create_account_for_testing(scenario: &mut Scenario, public_key: vector<u8>): address {
    let ctx = test_scenario::ctx(scenario);

    let authenticator = create_authenticator_function_ref_v1_for_testing();

    account::create(public_key, authenticator, ctx);

    test_scenario::next_tx(scenario, @0x0);

    let account = test_scenario::take_shared<Account>(scenario);
    let account_address = account.account_address();
    test_scenario::return_shared(account);

    account_address
}

fun create_authenticator_function_ref_v1_for_testing(): AuthenticatorFunctionRefV1<Account> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x1,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

fun create_tx_context_for_testing(sender: address, digest: vector<u8>): TxContext {
    tx_context::new(sender, digest, 0, 0, 0)
}

fun create_auth_context_for_testing(): AuthContext {
    auth_context::new_with_tx_inputs(vector::empty(), vector::empty(), vector::empty())
}

fun create_merkle_tree_for_testing(
    tx1_digest: vector<u8>,
    tx2_digest: vector<u8>,
    tx3_digest: vector<u8>,
): (vector<u8>, vector<vector<u8>>, vector<vector<u8>>, vector<vector<u8>>) {
    let tx1_hash = hash::keccak256(&tx1_digest);
    let tx2_hash = hash::keccak256(&tx2_digest);
    let tx3_hash = hash::keccak256(&tx3_digest);

    let h12 = merkle::hash_pair_sorted(&tx1_hash, &tx2_hash);
    let merkle_root = merkle::hash_pair_sorted(&h12, &tx3_hash);

    let tx1_proof = vector[tx2_hash, tx3_hash];
    let tx2_proof = vector[tx1_hash, tx3_hash];
    let tx3_proof = vector[h12];

    (merkle_root, tx1_proof, tx2_proof, tx3_proof)
}
