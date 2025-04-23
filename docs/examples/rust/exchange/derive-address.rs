// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::hash::HashFunction;
use hex;

fn main() {
    // Example of deriving an Ed25519 address from a public key
    let mut hasher = iota_types::crypto::DefaultHash::default();
    let example_public_key: &str =
        "0xc5b4eb7d3c7efdeb7f3546fee9971536a24298d0f6cac7a48619c343b3999e11";
    hasher.update(example_public_key);
    let arr = hasher.finalize();
    let iota_address_string = hex::encode(arr);
    println!("Address: 0x{iota_address_string}");

    // Example of deriving a Secp256k1 address from a public key
    let flag = 0x01; // 0x01 = Secp256k1, 0x02 = Secp256r1, 0x03 = multiSig
    // Hash the [flag, public key] bytearray using Blake2b
    let mut hasher = iota_types::crypto::DefaultHash::default();
    let example_public_key: &str =
        "0xc5b4eb7d3c7efdeb7f3546fee9971536a24298d0f6cac7a48619c343b3999e11";
    hasher.update([flag]);
    hasher.update(example_public_key);
    let arr = hasher.finalize();
    let iota_address_string = hex::encode(arr);
    println!("Address: 0x{iota_address_string}");
}
