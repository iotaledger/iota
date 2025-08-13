// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod alias;
mod bech32;
mod ed25519;
mod nft;

use core::str::FromStr;

use iota_stardust_types::types::block::{
    Error,
    address::{Address, AliasAddress, Ed25519Address, NftAddress},
};
use pretty_assertions::assert_eq;

const ED25519_ADDRESS: &str = "0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655";
const ALIAS_ID: &str = "0xe9ba80ad1561e437b663a1f1efbfabd544b0d7da7bb33e0a62e99b20ee450bee";
const NFT_ID: &str = "0xa9ede98a7f0223fa7a49fbc586f7a88bb4f0d152f282b19bcebd05c9e8a02370";
const ED25519_ADDRESS_INVALID: &str =
    "0x52fdfc072182654f163f5f0f9a621d729566c74d10037c4d7bbb0407d1e2c64x";

#[test]
fn invalid_bech32() {
    let address = Address::try_from_bech32(ED25519_ADDRESS_INVALID);

    assert!(matches!(address, Err(Error::InvalidAddress)));
}

#[test]
fn debug() {
    let address = Address::from(Ed25519Address::from_str(ED25519_ADDRESS).unwrap());

    assert_eq!(
        format!("{address:?}"),
        "Ed25519Address(0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655)"
    );

    let address = Address::from(AliasAddress::from_str(ALIAS_ID).unwrap());

    assert_eq!(
        format!("{address:?}"),
        "AliasAddress(0xe9ba80ad1561e437b663a1f1efbfabd544b0d7da7bb33e0a62e99b20ee450bee)"
    );

    let address = Address::from(NftAddress::from_str(NFT_ID).unwrap());

    assert_eq!(
        format!("{address:?}"),
        "NftAddress(0xa9ede98a7f0223fa7a49fbc586f7a88bb4f0d152f282b19bcebd05c9e8a02370)"
    );
}

#[test]
fn is_valid_bech32() {
    assert!(Address::is_valid_bech32(
        "rms1qpllaj0pyveqfkwxmnngz2c488hfdtmfrj3wfkgxtk4gtyrax0jaxzt70zy"
    ));

    assert!(!Address::is_valid_bech32(
        "rms1qpllaj0pyveqfkwxmnngz2c488hfdtmfrj3wfkgxtk4gtyrax0jaxzt70zY"
    ));
}
