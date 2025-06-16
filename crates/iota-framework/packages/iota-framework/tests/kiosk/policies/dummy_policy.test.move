// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
/// Dummy policy which showcases all of the methods.
module iota::dummy_policy;

use iota::coin::Coin;
use iota::iota::IOTA;
use iota::transfer_policy::{Self as policy, TransferPolicy, TransferPolicyCap, TransferRequest};

public struct Rule has drop {}
public struct Config has drop, store {}

public fun set<T>(policy: &mut TransferPolicy<T>, cap: &TransferPolicyCap<T>) {
    policy::add_rule(Rule {}, policy, cap, Config {})
}

public fun pay<T>(
    policy: &mut TransferPolicy<T>,
    request: &mut TransferRequest<T>,
    payment: Coin<IOTA>,
) {
    policy::add_to_balance(Rule {}, policy, payment);
    policy::add_receipt(Rule {}, request);
}
