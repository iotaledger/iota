#[test_only]
module lean_imt_account::lean_imt_account_tests;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::test_scenario::{Self, Scenario};
use lean_imt_account::lean_imt;
use lean_imt_account::lean_imt_account::{Self, LeanIMTAccount};

//const USER: address = @0x6b72f63997aa75e2aff8e7cb119f5507f8b521dade51003fc07c8a4c70f79a70;
const PUB_KEY: vector<u8> = x"b22bae6cc8436d178c3e77d9859017ce48997db30abbe87b5a55221888b4e31c";
const PVK: vector<u8> =
    x"bbebdc7c4023eeb6a81fcbbc37613366f4dac6687cbe9abc5d09e4cef8899b173a2747c5442ddb898d34de2429a9b43f86b12aeea35d58ec1d97a009eb2a9c2d34d3f96ebdb7416fbedf83ff29abee30941a380166aac2b2557476f50ffe2094777610b217740ac57c573cbf6af8bce106f7772241dce3406f0b1f2b845570074dd670d78f1c9e0d29fa7113753e384f56775627c9c64dd899566f90b7813301b20482628c99f957ff584f940965bc6b711d377c76b12921e0816b421cae5c098432218eb209bb104559ddc0ad78173ebde47c918c540b82e7e6b658b52bb19e0300000000000000926536117de81e192a1c9bec13bbdfb102852c05911d2ebb306e09706547dfac4dfc834de8b175761425b0dd4080c5c3700a63a4da9548d08ab47ba968d8d09969cdb618703b45502a39c93ce22bbd739c946fba6fd4e10285806ed6de1acaa0";
const PROOF_POINTS: vector<u8> =
    x"641a8593665c9e415c6f7f2c57ad992566ee5af86d86f00812541e97ef1fa4182493694830c22645274d619dbdb95886a8cbf23c5f3683dd3b1a39a7953898243eab26e677c31f863201c3339c427d46ebc1390203917f39c8479135f275bf17456182c1f59a574d3390206bd51f8373384a56c407f94f677089d3d0ce9999a3";
const ROOT: vector<u8> = x"4b61023a56e6b37edec2ba55c1b3f0cf0f4789431aafd6da10d32de09bb97402";
const DIGEST: vector<u8> = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
// iota keytool sign-raw --address 0x6b72f63997aa75e2aff8e7cb119f5507f8b521dade51003fc07c8a4c70f79a70 --data 315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3
const SIGNATURE: vector<u8> =
    x"c25927379145b06a777bb8c4205aeb584b3162bc29eb45a9b3c93f2ce7c65ca965163731adac03e991196bf2b6dbcf588d3d898fa41f2265bcd8fa96adc2ec09";

#[test]
fun test_lean_imt_account() {
    let leaf = lean_imt::derive_leaf_from_public_key(PUB_KEY);
    lean_imt::test_proof(
        PVK,
        PROOF_POINTS,
        ROOT,
        leaf,
    );
}

#[test]
fun test_authenticate_with_secret() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let authenticator = create_authenticate_with_secret_function_ref();
    let account_address = create_lean_imt_account_with_root_for_testing(
        scenario,
        ROOT,
        authenticator,
    );

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<LeanIMTAccount>();
        let ctx = create_tx_context_for_testing(account_address, DIGEST);
        let auth_ctx = create_auth_context_for_testing();

        let leaf = lean_imt::derive_leaf_from_public_key(PUB_KEY);

        lean_imt_account::secret_ed25519_authenticator(
            &account,
            SIGNATURE,
            PUB_KEY,
            leaf,
            PVK,
            PROOF_POINTS,
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun test_authenticate_with_public_key() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let authenticator = create_authenticate_with_public_key_function_ref();
    let account_address = create_lean_imt_account_with_root_for_testing(
        scenario,
        ROOT,
        authenticator,
    );

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<LeanIMTAccount>();
        let ctx = create_tx_context_for_testing(account_address, DIGEST);
        let auth_ctx = create_auth_context_for_testing();

        lean_imt_account::public_key_ed25519_authenticator(
            &account,
            SIGNATURE,
            PUB_KEY,
            PVK,
            PROOF_POINTS,
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

fun create_lean_imt_account_with_root_for_testing(
    scenario: &mut Scenario,
    root: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<LeanIMTAccount>,
): address {
    let ctx = test_scenario::ctx(scenario);

    lean_imt_account::create(root, authenticator, ctx);

    scenario.next_tx(@0x0);

    let account = scenario.take_shared<LeanIMTAccount>();
    let account_address = account.account_address();

    test_scenario::return_shared(account);

    account_address
}

fun create_authenticate_with_secret_function_ref(): AuthenticatorFunctionRefV1<LeanIMTAccount> {
    // The exact values don't matter in these tests.
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x1,
        std::ascii::string(b"lean_imt_account"),
        std::ascii::string(b"secret_ed25519_authenticator"),
    )
}

fun create_authenticate_with_public_key_function_ref(): AuthenticatorFunctionRefV1<LeanIMTAccount> {
    // The exact values don't matter in these tests.
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x1,
        std::ascii::string(b"lean_imt_account"),
        std::ascii::string(b"public_key_ed25519_authenticator"),
    )
}

fun create_tx_context_for_testing(sender: address, digest: vector<u8>): TxContext {
    tx_context::new(sender, digest, 0, 0, 0)
}

fun create_auth_context_for_testing(): AuthContext {
    auth_context::new_with_tx_inputs(
        b"00000000000000000000000000000000",
        vector::empty(),
        vector::empty(),
    )
}
