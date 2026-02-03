// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module dynamic_multisig_account::dynamic_multisig_account_tests;

use dynamic_multisig_account::dynamic_multisig_account::{Self, DynamicMultisigAccount};
use dynamic_multisig_account::members;
use dynamic_multisig_account::transactions;
use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};
use std::ascii;

const TRANSACTION_DIGEST: vector<u8> =
    x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

// --------------------------------------- Basic Scenario ---------------------------------------

#[test]
fun test_account_creation() {
    account_test!(|scenario, account_address| {
        // Check the account after creation.
        scenario.next_tx(@0x0);
        {
            let account = scenario.take_shared<DynamicMultisigAccount>();

            assert_eq(account.members().addresses(), vector[@0x1, @0x2, @0x3]);
            assert_eq(account.members().weights(), vector[1, 2, 3]);
            assert_eq(account.threshold(), 3);
            assert_ref_eq(
                account.authenticator(),
                &create_default_authenticator_function_ref_v1_for_testing(),
            );

            test_scenario::return_shared(account);
        };

        // Propose a transaction.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            // The transaction does not exist
            assert_eq(account.transactions().contains(TRANSACTION_DIGEST), false);
            // and has no approvals yet.
            assert_eq(account.total_approves(TRANSACTION_DIGEST), 0);

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Approve the transaction.
        scenario.next_tx(@0x2);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            let transaction = account.transactions().borrow(TRANSACTION_DIGEST);

            // The transaction now exists
            assert_eq(transaction.digest(), TRANSACTION_DIGEST);
            // and has one approval from the proposer.
            assert_ref_eq(transaction.approves(), &vector[@0x1]);

            // The approval weight of the transaction equals to the weight of the proposer.
            assert_eq(account.total_approves(TRANSACTION_DIGEST), 1);

            account.approve_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Authenticate the transaction.
        scenario.next_tx(account_address);
        {
            let account = scenario.take_shared<DynamicMultisigAccount>();
            let tx_ctx = create_tx_context_for_testing(account_address, TRANSACTION_DIGEST);
            let auth_ctx = create_auth_context_for_testing();

            let transaction = account.transactions().borrow(TRANSACTION_DIGEST);

            // The transaction now has two approvals
            assert_ref_eq(transaction.approves(), &vector[@0x1, @0x2]);
            // with total weight which is enough to reach the threshold.
            assert_eq(account.total_approves(TRANSACTION_DIGEST), 3);

            dynamic_multisig_account::authenticate(&account, &auth_ctx, &tx_ctx);

            test_scenario::return_shared(account);
        };

        // Remove the transaction.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.remove_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            // The transaction is removed.
            assert_eq(account.transactions().contains(TRANSACTION_DIGEST), false);
            assert_eq(account.total_approves(TRANSACTION_DIGEST), 0);

            test_scenario::return_shared(account);
        };
    });
}

// --------------------------------------- Creation Issues ---------------------------------------

#[test]
#[expected_failure(abort_code = members::EMembersComponentsHaveDifferentLengths)]
fun test_account_creation_with_inconsistent_members() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = test_scenario::ctx(scenario);

    // The lengths of addresses and weights are different.
    let members_addresses = vector[@0x1, @0x2, @0x3];
    let members_weights = vector[1, 2];
    let threshold = 3;
    let authenticator = create_default_authenticator_function_ref_v1_for_testing();

    dynamic_multisig_account::create(
        members_addresses,
        members_weights,
        threshold,
        authenticator,
        ctx,
    );

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = members::EMembersMustNotContainDuplicates)]
fun test_account_creation_with_members_duplicate() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = test_scenario::ctx(scenario);

    // The address @0x2 is duplicated.
    let members_addresses = vector[@0x1, @0x2, @0x2];
    let members_weights = vector[1, 2, 3];
    let threshold = 3;
    let authenticator = create_default_authenticator_function_ref_v1_for_testing();

    dynamic_multisig_account::create(
        members_addresses,
        members_weights,
        threshold,
        authenticator,
        ctx,
    );

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = dynamic_multisig_account::EThresholdIsZero)]
fun test_account_creation_with_zero_threshold() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = test_scenario::ctx(scenario);

    let members_addresses = vector[@0x1, @0x2, @0x3];
    let members_weights = vector[1, 2, 3];
    // The threshold can't be zero.
    let threshold = 0;
    let authenticator = create_default_authenticator_function_ref_v1_for_testing();

    dynamic_multisig_account::create(
        members_addresses,
        members_weights,
        threshold,
        authenticator,
        ctx,
    );

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = dynamic_multisig_account::ETotalMembersWeightLessThanThreshold)]
fun test_account_creation_with_inconsistent_threshold() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = test_scenario::ctx(scenario);

    let members_addresses = vector[@0x1, @0x2, @0x3];
    let members_weights = vector[1, 2, 3];
    // The threshold is too high.
    let threshold = 7;
    let authenticator = create_default_authenticator_function_ref_v1_for_testing();

    dynamic_multisig_account::create(
        members_addresses,
        members_weights,
        threshold,
        authenticator,
        ctx,
    );

    test_scenario::end(scenario_val);
}

// --------------------------------------- Transactions ---------------------------------------

#[test]
fun test_transaction_propose_several() {
    account_test!(|scenario, _| {
        let transaction_digest_1 =
            x"1111111111111111111111111111111111111111111111111111111111111111";
        let transaction_digest_2 =
            x"2222222222222222222222222222222222222222222222222222222222222222";
        let transaction_digest_3 =
            x"3333333333333333333333333333333333333333333333333333333333333333";

        // Propose a transaction.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(transaction_digest_1, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Propose a second transaction.
        scenario.next_tx(@0x2);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(transaction_digest_2, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Propose a third transaction.
        scenario.next_tx(@0x3);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(transaction_digest_3, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Check the transactions.
        scenario.next_tx(@0x0);
        {
            let account = scenario.take_shared<DynamicMultisigAccount>();

            let transaction_1 = account.transactions().borrow(transaction_digest_1);
            let transaction_2 = account.transactions().borrow(transaction_digest_2);
            let transaction_3 = account.transactions().borrow(transaction_digest_3);

            assert_eq(transaction_1.digest(), transaction_digest_1);
            assert_eq(transaction_2.digest(), transaction_digest_2);
            assert_eq(transaction_3.digest(), transaction_digest_3);

            assert_ref_eq(transaction_1.approves(), &vector[@0x1]);
            assert_ref_eq(transaction_2.approves(), &vector[@0x2]);
            assert_ref_eq(transaction_3.approves(), &vector[@0x3]);

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = members::EMemberIsNotFound)]
fun test_transaction_not_member_proposal() {
    account_test!(|scenario, _| {
        // Propose a transaction by not a member.
        scenario.next_tx(@0xA);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = members::EMemberIsNotFound)]
fun test_transaction_not_member_approve() {
    account_test!(|scenario, _| {
        // Propose a transaction.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Approve the transaction by not a member.
        scenario.next_tx(@0xA);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.approve_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = dynamic_multisig_account::ETransactionSenderIsNotTheAccount)]
fun test_transaction_remove_not_by_account() {
    account_test!(|scenario, _| {
        // Propose a transaction.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Remove the transaction by not the account.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.remove_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = transactions::ETransactionIsAlreadyApprovedByTheMember)]
fun test_transaction_approve_by_proposer() {
    account_test!(|scenario, _| {
        // Propose a transaction.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Approve the transaction with the same sender.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.approve_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = transactions::ETransactionAlreadyExists)]
fun test_transaction_double_proposal() {
    account_test!(|scenario, _| {
        // Propose a transaction.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Propose the transaction once again.
        scenario.next_tx(@0x2);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = transactions::ETransactionIsAlreadyApprovedByTheMember)]
fun test_transaction_double_approve() {
    account_test!(|scenario, _| {
        // Propose a transaction.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Approve the transaction.
        scenario.next_tx(@0x2);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.approve_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Approve the transaction with the same sender.
        scenario.next_tx(@0x2);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.approve_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = transactions::ETransactionDoesNotExist)]
fun test_transaction_remove_non_existent() {
    account_test!(|scenario, account_address| {
        // Remove the not-existent transaction.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.remove_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = transactions::ETransactionDoesNotExist)]
fun test_transaction_double_remove() {
    account_test!(|scenario, account_address| {
        // Propose a transaction.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Remove the transaction.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.remove_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Remove the transaction again.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.remove_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };
    });
}

// --------------------------------------- Update Account ---------------------------------------

#[test]
fun test_account_updating() {
    account_test!(|scenario, account_address| {
        // Update the account data.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            let ctx = test_scenario::ctx(scenario);

            let members_addresses = vector[@0xA, @0xB, @0xC];
            let members_weights = vector[4, 5, 6];
            // The threshold equals to the total weight of all the members; it is the maximum possible value.
            let threshold = 15;
            let authenticator = create_authenticator_function_ref_v1_for_testing(b"function2");

            account.update_account_data(
                members_addresses,
                members_weights,
                threshold,
                authenticator,
                ctx,
            );

            assert_eq(account.members().addresses(), members_addresses);
            assert_eq(account.members().weights(), members_weights);
            assert_eq(account.threshold(), threshold);
            assert_ref_eq(account.authenticator(), &authenticator);

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = dynamic_multisig_account::ETransactionSenderIsNotTheAccount)]
fun test_account_updating_with_not_account() {
    account_test!(|scenario, _| {
        // Update the account data by not the account.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            let ctx = test_scenario::ctx(scenario);

            let members_addresses = vector[@0xA, @0xB, @0xC];
            let members_weights = vector[4, 5, 6];
            let threshold = 10;
            let authenticator = create_authenticator_function_ref_v1_for_testing(b"function2");

            account.update_account_data(
                members_addresses,
                members_weights,
                threshold,
                authenticator,
                ctx,
            );

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = members::EMembersComponentsHaveDifferentLengths)]
fun test_account_updating_with_inconsistent_members() {
    account_test!(|scenario, account_address| {
        // Update the account data.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            let ctx = test_scenario::ctx(scenario);

            // The lengths of addresses and weights are different.
            let members_addresses = vector[@0xA, @0xB];
            let members_weights = vector[4, 5, 6];
            let threshold = 10;
            let authenticator = create_authenticator_function_ref_v1_for_testing(b"function2");

            account.update_account_data(
                members_addresses,
                members_weights,
                threshold,
                authenticator,
                ctx,
            );

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = members::EMembersMustNotContainDuplicates)]
fun test_account_updating_with_members_duplicate() {
    account_test!(|scenario, account_address| {
        // Update the account data.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            let ctx = test_scenario::ctx(scenario);

            // The address @0xA is duplicated.
            let members_addresses = vector[@0xA, @0xA, @0xC];
            let members_weights = vector[4, 5, 6];
            let threshold = 10;
            let authenticator = create_authenticator_function_ref_v1_for_testing(b"function2");

            account.update_account_data(
                members_addresses,
                members_weights,
                threshold,
                authenticator,
                ctx,
            );

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = dynamic_multisig_account::EThresholdIsZero)]
fun test_account_updating_with_zero_threshold() {
    account_test!(|scenario, account_address| {
        // Update the account data.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            let ctx = test_scenario::ctx(scenario);

            let members_addresses = vector[@0xA, @0xB, @0xC];
            let members_weights = vector[4, 5, 6];
            // The threshold can't be zero.
            let threshold = 0;
            let authenticator = create_authenticator_function_ref_v1_for_testing(b"function2");

            account.update_account_data(
                members_addresses,
                members_weights,
                threshold,
                authenticator,
                ctx,
            );

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[expected_failure(abort_code = dynamic_multisig_account::ETotalMembersWeightLessThanThreshold)]
fun test_account_updating_with_inconsistent_threshold() {
    account_test!(|scenario, account_address| {
        // Update the account data.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            let ctx = test_scenario::ctx(scenario);

            let members_addresses = vector[@0xA, @0xB, @0xC];
            let members_weights = vector[4, 5, 6];
            // The threshold is too high.
            let threshold = 16;
            let authenticator = create_authenticator_function_ref_v1_for_testing(b"function2");

            account.update_account_data(
                members_addresses,
                members_weights,
                threshold,
                authenticator,
                ctx,
            );

            test_scenario::return_shared(account);
        };
    });
}

// --------------------------------------- Authentication ---------------------------------------

#[test]
#[expected_failure(abort_code = dynamic_multisig_account::ETransactionSenderIsNotTheAccount)]
fun test_authenticate_by_not_account() {
    account_test!(|scenario, _| {
        // Propose a transaction.
        scenario.next_tx(@0x3);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            assert_eq(account.threshold(), 3);
            // The transaction has enough approves weight and can be executed.
            assert_eq(account.total_approves(TRANSACTION_DIGEST), 3);

            test_scenario::return_shared(account);
        };

        // Authenticate the transaction by not the account.
        scenario.next_tx(@0x3);
        {
            let account = scenario.take_shared<DynamicMultisigAccount>();
            let tx_ctx = create_tx_context_for_testing(@0x3, TRANSACTION_DIGEST);
            let auth_ctx = create_auth_context_for_testing();

            dynamic_multisig_account::authenticate(&account, &auth_ctx, &tx_ctx);

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[
    expected_failure(
        abort_code = dynamic_multisig_account::ETransactionDoesNotHaveSufficientApprovals,
    ),
]
fun test_authenticate_not_enough_total_weight() {
    account_test!(|scenario, account_address| {
        // Propose a transaction.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            assert_eq(account.threshold(), 3);
            // The transaction has not enough approves to be executed.
            assert_eq(account.total_approves(TRANSACTION_DIGEST), 1);

            test_scenario::return_shared(account);
        };

        // Authenticate the transaction.
        scenario.next_tx(account_address);
        {
            let account = scenario.take_shared<DynamicMultisigAccount>();
            let tx_ctx = create_tx_context_for_testing(account_address, TRANSACTION_DIGEST);
            let auth_ctx = create_auth_context_for_testing();

            dynamic_multisig_account::authenticate(&account, &auth_ctx, &tx_ctx);

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[
    expected_failure(
        abort_code = dynamic_multisig_account::ETransactionDoesNotHaveSufficientApprovals,
    ),
]
fun test_authenticate_not_enough_total_weight_after_update() {
    account_test!(|scenario, account_address| {
        // Propose a transaction.
        scenario.next_tx(@0x3);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            assert_eq(account.threshold(), 3);
            // The transaction has enough approves weight and can be executed.
            assert_eq(account.total_approves(TRANSACTION_DIGEST), 3);

            test_scenario::return_shared(account);
        };

        // Update the account data.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            let ctx = test_scenario::ctx(scenario);

            let members_addresses = vector[@0x1, @0x2, @0x3];
            // The @0x3 weight is reduced, so it does not have enough weight to reach the threshold after the update.
            let members_weights = vector[1, 2, 2];
            let threshold = 3;
            let authenticator = create_authenticator_function_ref_v1_for_testing(b"function2");

            account.update_account_data(
                members_addresses,
                members_weights,
                threshold,
                authenticator,
                ctx,
            );

            test_scenario::return_shared(account);
        };

        // Authenticate the transaction.
        scenario.next_tx(account_address);
        {
            let account = scenario.take_shared<DynamicMultisigAccount>();
            let tx_ctx = create_tx_context_for_testing(account_address, TRANSACTION_DIGEST);
            let auth_ctx = create_auth_context_for_testing();

            dynamic_multisig_account::authenticate(&account, &auth_ctx, &tx_ctx);

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[
    expected_failure(
        abort_code = dynamic_multisig_account::ETransactionDoesNotHaveSufficientApprovals,
    ),
]
fun test_authenticate_member_removed_during_update() {
    account_test!(|scenario, account_address| {
        // Propose a transaction.
        scenario.next_tx(@0x1);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            test_scenario::return_shared(account);
        };

        // Approve the transaction.
        scenario.next_tx(@0x2);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.approve_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            assert_eq(account.threshold(), 3);
            // The transaction has enough approves weight and can be executed.
            assert_eq(account.total_approves(TRANSACTION_DIGEST), 3);

            test_scenario::return_shared(account);
        };

        // Update the account data.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            let ctx = test_scenario::ctx(scenario);

            // The member @0x1 is removed, so the transaction does not have enough weight to reach the threshold after the update.
            let members_addresses = vector[@0x2, @0x3];
            let members_weights = vector[2, 3];
            let threshold = 3;
            let authenticator = create_authenticator_function_ref_v1_for_testing(b"function2");

            account.update_account_data(
                members_addresses,
                members_weights,
                threshold,
                authenticator,
                ctx,
            );

            test_scenario::return_shared(account);
        };

        // Authenticate the transaction.
        scenario.next_tx(account_address);
        {
            let account = scenario.take_shared<DynamicMultisigAccount>();
            let tx_ctx = create_tx_context_for_testing(account_address, TRANSACTION_DIGEST);
            let auth_ctx = create_auth_context_for_testing();

            dynamic_multisig_account::authenticate(&account, &auth_ctx, &tx_ctx);

            test_scenario::return_shared(account);
        };
    });
}

#[test]
#[
    expected_failure(
        abort_code = dynamic_multisig_account::ETransactionDoesNotHaveSufficientApprovals,
    ),
]
fun test_authenticate_threshold_changed_during_update() {
    account_test!(|scenario, account_address| {
        // Propose a transaction.
        scenario.next_tx(@0x3);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            account.propose_transaction(TRANSACTION_DIGEST, test_scenario::ctx(scenario));

            assert_eq(account.threshold(), 3);
            // The transaction has enough approves weight and can be executed.
            assert_eq(account.total_approves(TRANSACTION_DIGEST), 3);

            test_scenario::return_shared(account);
        };

        // Update the account data.
        scenario.next_tx(account_address);
        {
            let mut account = scenario.take_shared<DynamicMultisigAccount>();

            let ctx = test_scenario::ctx(scenario);

            let members_addresses = vector[@0x1, @0x2, @0x3];
            let members_weights = vector[1, 2, 3];
            // The threshold is increased, so the transaction does not have enough weight to reach the threshold after the update.
            let threshold = 4;
            let authenticator = create_authenticator_function_ref_v1_for_testing(b"function2");

            account.update_account_data(
                members_addresses,
                members_weights,
                threshold,
                authenticator,
                ctx,
            );

            test_scenario::return_shared(account);
        };

        // Authenticate the transaction.
        scenario.next_tx(account_address);
        {
            let account = scenario.take_shared<DynamicMultisigAccount>();
            let tx_ctx = create_tx_context_for_testing(account_address, TRANSACTION_DIGEST);
            let auth_ctx = create_auth_context_for_testing();

            dynamic_multisig_account::authenticate(&account, &auth_ctx, &tx_ctx);

            test_scenario::return_shared(account);
        };
    });
}

// --------------------------------------- Test Utilities ---------------------------------------

fun create_default_authenticator_function_ref_v1_for_testing(): AuthenticatorFunctionRefV1<
    DynamicMultisigAccount,
> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x1,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

fun create_authenticator_function_ref_v1_for_testing(
    function: vector<u8>,
): AuthenticatorFunctionRefV1<DynamicMultisigAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x1,
        ascii::string(function),
        ascii::string(b"function"),
    )
}

fun create_account_for_testing(scenario: &mut Scenario): address {
    let ctx = test_scenario::ctx(scenario);

    let members_addresses = vector[@0x1, @0x2, @0x3];
    let members_weights = vector[1, 2, 3];
    let threshold = 3;
    let authenticator = create_default_authenticator_function_ref_v1_for_testing();

    dynamic_multisig_account::create(
        members_addresses,
        members_weights,
        threshold,
        authenticator,
        ctx,
    );

    scenario.next_tx(@0x0);

    let account = scenario.take_shared<DynamicMultisigAccount>();
    let account_address = account.get_address();

    test_scenario::return_shared(account);

    account_address
}

fun create_tx_context_for_testing(sender: address, digest: vector<u8>): TxContext {
    tx_context::new(sender, digest, 0, 0, 0)
}

fun create_auth_context_for_testing(): AuthContext {
    auth_context::new_with_tx_inputs(vector::empty(), vector::empty(), vector::empty())
}

macro fun account_test($f: |&mut Scenario, address|) {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_account_for_testing(scenario);

    $f(scenario, account_address);

    test_scenario::end(scenario_val);
}
