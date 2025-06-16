// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module bridge::test_token;

use iota::address;
use iota::coin::{CoinMetadata, TreasuryCap, create_currency};
use iota::hex;
use iota::package::{UpgradeCap, test_publish};
use iota::test_utils::create_one_time_witness;
use std::ascii;
use std::type_name;

public struct TEST_TOKEN has drop {}

public fun create_bridge_token(
    ctx: &mut TxContext,
): (UpgradeCap, TreasuryCap<TEST_TOKEN>, CoinMetadata<TEST_TOKEN>) {
    let otw = create_one_time_witness<TEST_TOKEN>();
    let (treasury_cap, metadata) = create_currency(
        otw,
        8,
        b"tst",
        b"test",
        b"bridge test token",
        option::none(),
        ctx,
    );

    let type_name = type_name::get<TEST_TOKEN>();
    let address_bytes = hex::decode(
        ascii::into_bytes(type_name::get_address(&type_name)),
    );
    let coin_id = address::from_bytes(address_bytes).to_id();
    let upgrade_cap = test_publish(coin_id, ctx);

    (upgrade_cap, treasury_cap, metadata)
}
