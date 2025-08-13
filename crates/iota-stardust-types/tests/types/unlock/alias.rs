// Copyright 2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_stardust_types::types::block::{Error, unlock::AliasUnlock};
use packable::{PackableExt, bounded::InvalidBoundedU16};
use pretty_assertions::assert_eq;

#[test]
fn kind() {
    assert_eq!(AliasUnlock::KIND, 2);
}

#[test]
fn new_valid_min_index() {
    assert_eq!(AliasUnlock::new(0).unwrap().index(), 0);
}

#[test]
fn new_valid_max_index() {
    assert_eq!(AliasUnlock::new(126).unwrap().index(), 126);
}

#[test]
fn new_invalid_more_than_max_index() {
    assert!(matches!(
        AliasUnlock::new(128),
        Err(Error::InvalidAliasIndex(InvalidBoundedU16(128)))
    ));
}

#[test]
fn try_from_valid() {
    assert_eq!(AliasUnlock::try_from(0).unwrap().index(), 0);
}

#[test]
fn try_from_invalid() {
    assert!(matches!(
        AliasUnlock::try_from(128),
        Err(Error::InvalidAliasIndex(InvalidBoundedU16(128)))
    ));
}

#[test]
fn packed_len() {
    let reference = AliasUnlock::new(0).unwrap();

    assert_eq!(reference.packed_len(), 2);
    assert_eq!(reference.pack_to_vec().len(), 2);
}

#[test]
fn pack_unpack_valid() {
    let unlock_1 = AliasUnlock::new(42).unwrap();
    let unlock_2 = AliasUnlock::unpack_verified(unlock_1.pack_to_vec().as_slice(), &()).unwrap();

    assert_eq!(unlock_1, unlock_2);
}
