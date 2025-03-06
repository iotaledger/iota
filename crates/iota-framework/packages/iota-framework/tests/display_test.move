// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::display_tests;

use iota::display;
use iota::package;
use iota::test_scenario as test;
use std::string::String;

#[allow(unused_field)]
/// An example object.
/// Purely for visibility.
public struct IotestNft has key {
    id: UID,
    name: String,
}

/// Test witness type to create a Publisher object.
public struct IOTESTNFT has drop {}

#[test]
fun nft_test_init() {
    let mut test = test::begin(@0x2);
    let pub = package::test_claim(IOTESTNFT {}, test.ctx());

    // create a new display object
    let mut display = display::new<IotestNft>(&pub, test.ctx());

    display.add(b"name".to_string(), b"IOTEST Nft {name}".to_string());
    display.add(b"link".to_string(), b"https://iotestnft.com/nft/{id}".to_string());
    display.add(b"image".to_string(), b"https://api.iotestnft.com/nft/{id}/svg".to_string());
    display.add(b"description".to_string(), b"One of many Iotest Nfts".to_string());

    pub.burn_publisher();
    transfer::public_transfer(display, @0x2);
    test.end();
}
