// Copyright 2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crypto::{
    keys::{
        bip39::{Passphrase, mnemonic_to_seed, wordlist},
        slip10::{Seed, Segment},
    },
    signatures::ed25519,
};

use super::bytes::{rand_bytes, rand_bytes_array};
use crate::types::block::signature::{Ed25519Signature, Signature};

pub fn rand_ed25519_signature() -> Ed25519Signature {
    let mnemonic = wordlist::encode(&rand_bytes_array::<32>(), &wordlist::ENGLISH).unwrap();
    let seed = Seed::from(mnemonic_to_seed(&mnemonic, &Passphrase::default()));
    let chain = [0; 5];
    let private_key = seed
        .derive::<ed25519::SecretKey, _>(chain.into_iter().map(Segment::harden))
        .secret_key();
    let public_key = private_key.public_key();
    let signature = private_key.sign(&rand_bytes(64));

    Ed25519Signature::new(public_key, signature)
}

pub fn rand_signature() -> Signature {
    Signature::from(rand_ed25519_signature())
}
