// Copyright 2020-2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::str::FromStr;

use iota_stardust_types::types::block::{
    Error,
    address::{Address, Bech32Address, Ed25519Address, Hrp},
};
use packable::PackableExt;
use pretty_assertions::assert_eq;

const ED25519_ADDRESS: &str = "0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655";
const ED25519_BECH32: &str = "rms1qr47gz3xxjqpjrwd0yu5glhqrth6w0t08npney8000ust2lcw2r92j5a8rt";

#[test]
fn debug() {
    let bech32_address = Bech32Address::from_str(ED25519_BECH32).unwrap();

    assert_eq!(
        format!("{bech32_address:?}"),
        "Bech32Address(rms1qr47gz3xxjqpjrwd0yu5glhqrth6w0t08npney8000ust2lcw2r92j5a8rt)"
    );
}

#[test]
fn ctors() {
    let ed25519_address = ED25519_ADDRESS.parse::<Ed25519Address>().unwrap();
    let address = Address::Ed25519(ed25519_address);
    let bech32_address = Bech32Address::try_new("rms", address).unwrap();
    assert_eq!(bech32_address.inner(), &address);
    assert_eq!(bech32_address.hrp(), "rms");

    // This makes sure that the custom `try_from_str` method does the same as
    // `FromStr::from_str` trait impl.
    assert_eq!(
        bech32_address,
        ED25519_BECH32.parse::<Bech32Address>().unwrap()
    );
    assert_eq!(
        bech32_address,
        Bech32Address::try_from_str(ED25519_BECH32).unwrap()
    );
}

#[test]
fn hrp_from_str() {
    Hrp::from_str("rms").unwrap();

    assert!(matches!(
        Hrp::from_str("中國"),
        Err(Error::InvalidBech32Hrp(hrp)) if hrp == "中國"
    ));
}

#[test]
fn hrp_packed_len() {
    let hrp = Hrp::from_str("rms").unwrap();

    assert_eq!(hrp.packed_len(), 1 + 3);
    assert_eq!(hrp.pack_to_vec().len(), 1 + 3);
}

#[test]
fn hrp_pack_unpack() {
    let hrp = Hrp::from_str("rms").unwrap();
    let packed_hrp = hrp.pack_to_vec();

    assert_eq!(
        hrp,
        Hrp::unpack_verified(packed_hrp.as_slice(), &()).unwrap()
    );
}

#[test]
fn bech32_into_inner() {
    let address = Address::try_from_bech32(ED25519_BECH32).unwrap();
    let bech32_address = Bech32Address::from_str(ED25519_BECH32).unwrap();

    assert_eq!(address, bech32_address.into_inner());
}
