// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod alias;
mod nft;
mod reference;
mod signature;

use iota_stardust_types::types::block::{
    Error,
    rand::signature::rand_signature,
    unlock::{AliasUnlock, NftUnlock, ReferenceUnlock, SignatureUnlock, Unlock, Unlocks},
};
use packable::bounded::TryIntoBoundedU16Error;
use pretty_assertions::assert_eq;

#[test]
fn kind() {
    assert_eq!(
        Unlock::from(SignatureUnlock::from(rand_signature())).kind(),
        0
    );
    assert_eq!(Unlock::from(ReferenceUnlock::new(0).unwrap()).kind(), 1);
    assert_eq!(Unlock::from(AliasUnlock::new(0).unwrap()).kind(), 2);
    assert_eq!(Unlock::from(NftUnlock::new(0).unwrap()).kind(), 3);
}

#[test]
fn new_invalid_first_reference() {
    assert!(matches!(
        Unlocks::new([ReferenceUnlock::new(42).unwrap().into()]),
        Err(Error::InvalidUnlockReference(0)),
    ));
}

#[test]
fn new_invalid_self_reference() {
    assert!(matches!(
        Unlocks::new([
            SignatureUnlock::from(rand_signature()).into(),
            ReferenceUnlock::new(1).unwrap().into()
        ]),
        Err(Error::InvalidUnlockReference(1)),
    ));
}

#[test]
fn new_invalid_future_reference() {
    assert!(matches!(
        Unlocks::new([
            SignatureUnlock::from(rand_signature()).into(),
            ReferenceUnlock::new(2).unwrap().into(),
            SignatureUnlock::from(rand_signature()).into(),
        ]),
        Err(Error::InvalidUnlockReference(1)),
    ));
}

#[test]
fn new_invalid_reference_reference() {
    assert!(matches!(
        Unlocks::new([
            SignatureUnlock::from(rand_signature()).into(),
            ReferenceUnlock::new(0).unwrap().into(),
            ReferenceUnlock::new(1).unwrap().into()
        ]),
        Err(Error::InvalidUnlockReference(2)),
    ));
}

#[test]
fn new_invalid_duplicate_signature() {
    let dup = rand_signature();
    assert!(matches!(
        Unlocks::new([
            SignatureUnlock::from(rand_signature()).into(),
            ReferenceUnlock::new(0).unwrap().into(),
            ReferenceUnlock::new(0).unwrap().into(),
            SignatureUnlock::from(rand_signature()).into(),
            SignatureUnlock::from(dup.clone()).into(),
            SignatureUnlock::from(dup).into(),
            SignatureUnlock::from(rand_signature()).into(),
            ReferenceUnlock::new(3).unwrap().into()
        ]),
        Err(Error::DuplicateSignatureUnlock(5)),
    ));
}

#[test]
fn new_invalid_too_many_blocks() {
    assert!(matches!(
        Unlocks::new(vec![ReferenceUnlock::new(0).unwrap().into(); 300]),
        Err(Error::InvalidUnlockCount(TryIntoBoundedU16Error::Invalid(
            300
        ))),
    ));
}

#[test]
fn new_valid() {
    assert!(
        Unlocks::new([
            SignatureUnlock::from(rand_signature()).into(),
            ReferenceUnlock::new(0).unwrap().into(),
            ReferenceUnlock::new(0).unwrap().into(),
            SignatureUnlock::from(rand_signature()).into(),
            SignatureUnlock::from(rand_signature()).into(),
            SignatureUnlock::from(rand_signature()).into(),
            ReferenceUnlock::new(3).unwrap().into(),
            ReferenceUnlock::new(4).unwrap().into(),
            ReferenceUnlock::new(3).unwrap().into(),
            ReferenceUnlock::new(4).unwrap().into(),
            ReferenceUnlock::new(5).unwrap().into(),
            SignatureUnlock::from(rand_signature()).into(),
            ReferenceUnlock::new(11).unwrap().into(),
            SignatureUnlock::from(rand_signature()).into(),
        ])
        .is_ok()
    );
}

#[test]
fn get_none() {
    assert!(
        Unlocks::new([SignatureUnlock::from(rand_signature()).into()])
            .unwrap()
            .get(42)
            .is_none()
    );
}

#[test]
fn get_signature() {
    let signature = Unlock::from(SignatureUnlock::from(rand_signature()));

    assert_eq!(
        Unlocks::new([signature.clone()]).unwrap().first(),
        Some(&signature)
    );
}

#[test]
fn get_signature_through_reference() {
    let signature = Unlock::from(SignatureUnlock::from(rand_signature()));

    assert_eq!(
        Unlocks::new([signature.clone(), ReferenceUnlock::new(0).unwrap().into()])
            .unwrap()
            .get(1),
        Some(&signature)
    );
}

#[test]
fn invalid_alias_0() {
    assert!(matches!(
        Unlocks::new([
            AliasUnlock::new(0).unwrap().into(),
            SignatureUnlock::from(rand_signature()).into(),
        ]),
        Err(Error::InvalidUnlockAlias(0)),
    ));
}

#[test]
fn invalid_alias_index() {
    assert!(matches!(
        Unlocks::new([
            SignatureUnlock::from(rand_signature()).into(),
            AliasUnlock::new(2).unwrap().into(),
        ]),
        Err(Error::InvalidUnlockAlias(1)),
    ));
}

#[test]
fn invalid_nft_0() {
    assert!(matches!(
        Unlocks::new([
            NftUnlock::new(0).unwrap().into(),
            SignatureUnlock::from(rand_signature()).into(),
        ]),
        Err(Error::InvalidUnlockNft(0)),
    ));
}

#[test]
fn invalid_nft_index() {
    assert!(matches!(
        Unlocks::new([
            SignatureUnlock::from(rand_signature()).into(),
            NftUnlock::new(2).unwrap().into(),
        ]),
        Err(Error::InvalidUnlockNft(1)),
    ));
}
