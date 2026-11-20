// TEMP benchmark: kernels that each do one kind of work, to find which work
// costs a validator the most time per unit of gas. Not for commit.
module stress::kernels;

use iota::{bls12381, dynamic_field as df, ecdsa_k1, ecvrf, ed25519, event, hash, hmac};
use std::{bcs, hash as std_hash, string};

public struct Parent has key {
    id: UID,
}

public struct Blob has copy, drop {
    data: vector<u8>,
}

public struct Small has key, store {
    id: UID,
    value: u64,
}

const ED25519_MSG: vector<u8> = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
const ED25519_PK: vector<u8> = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
const ED25519_SIG: vector<u8> =
    x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";
const K1_MSG: vector<u8> = b"Hello, world!";
const K1_RECOVERABLE_SIG: vector<u8> =
    x"7e4237ebfbc36613e166bfc5f6229360a9c1949242da97ca04867e4de57b2df30c8340bcb320328cf46d71bda51fcb519e3ce53b348eec62de852e350edbd88600";
const K1_SIG: vector<u8> =
    x"7e4237ebfbc36613e166bfc5f6229360a9c1949242da97ca04867e4de57b2df30c8340bcb320328cf46d71bda51fcb519e3ce53b348eec62de852e350edbd886";
const K1_PK: vector<u8> = x"02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";
const BLS_PK: vector<u8> =
    x"868f005eb8e6e4ca0a47c8a77ceaa5309a47978a7c71bc5cce96366b5d7a569937c529eeda66c7293784a9402801af31";
const BLS_SIG: vector<u8> =
    x"a2cd8577944b84484ef557a7f92f0d5092779497cc470b1b97680b8f7c807d97250d310b801c7c2185c7c8a21032d45403b97530ca87bd8f05d0cf4ffceb4bcb9bf7184fb604967db7e9e6ea555bc51b25a9e41fbd51181f712aa73aaec749fe";
const BLS_PREV_SIG: vector<u8> =
    x"a96aace596906562dc525dba4dff734642d71b334d51324f9c9bcb5a3d6caf14b05cde91d6507bf4615cb4285e5b4efd1358ebc46b80b51e338f9dc46cca17cf2e046765ba857c04101a560887fa81aef101a5bb3b2350884558bd3adc72be37";
const BLS_ROUND: u64 = 2373935;
const VRF_OUTPUT: vector<u8> =
    x"4fad431c7402fa1d4a7652e975aeb9a2b746540eca0b1b1e59c8d19c14a7701918a8249136e355455b8bc73851f7fc62c84f2e39f685b281e681043970026ed8";
const VRF_PK: vector<u8> = x"1ea6f0f467574295a2cd5d21a3fd3a712ade354d520d3bd0fe6088d7b7c2e00e";
const VRF_PROOF: vector<u8> =
    x"d8ad2eafb4f2eaf317447726e541359f26dfce248431fe09984fdc73144abb6ceb006c57a29a742eae5a81dd04239870769e310a81046cbbaff8b0bd27a6d6affee167ebba50549b58ffdf9aa192f506";

const EWrong: u64 = 0;

// === setup ===

public fun create_parent(ctx: &mut TxContext) {
    transfer::share_object(Parent { id: object::new(ctx) });
}

public fun fill(parent: &mut Parent, start: u64, count: u64) {
    let mut i = start;
    while (i < start + count) {
        df::add(&mut parent.id, i, i);
        i = i + 1;
    };
}

// === kernels ===

public fun noop() {}

public fun u256_loop(n: u64): u256 {
    let mut acc: u256 = 7;
    let mut i = 0;
    while (i < n) {
        acc = (acc * 6364136223846793005 + 1442695040888963407) % 340282366920938463463374607431768211297;
        i = i + 1;
    };
    acc
}

public fun sha3(data: vector<u8>, n: u64) {
    let mut i = 0;
    while (i < n) {
        std_hash::sha3_256(data);
        i = i + 1;
    };
}

public fun sha2(data: vector<u8>, n: u64) {
    let mut i = 0;
    while (i < n) {
        std_hash::sha2_256(data);
        i = i + 1;
    };
}

public fun blake2b(data: vector<u8>, n: u64) {
    let mut i = 0;
    while (i < n) {
        hash::blake2b256(&data);
        i = i + 1;
    };
}

public fun keccak(data: vector<u8>, n: u64) {
    let mut i = 0;
    while (i < n) {
        hash::keccak256(&data);
        i = i + 1;
    };
}

public fun hmac_loop(data: vector<u8>, n: u64) {
    let key = b"my key!";
    let mut i = 0;
    while (i < n) {
        hmac::hmac_sha3_256(&key, &data);
        i = i + 1;
    };
}

public fun ed25519_loop(n: u64) {
    let (sig, pk, msg) = (ED25519_SIG, ED25519_PK, ED25519_MSG);
    let mut i = 0;
    while (i < n) {
        assert!(ed25519::ed25519_verify(&sig, &pk, &msg), EWrong);
        i = i + 1;
    };
}

public fun k1_recover_loop(n: u64) {
    let (sig, msg) = (K1_RECOVERABLE_SIG, K1_MSG);
    let mut i = 0;
    while (i < n) {
        ecdsa_k1::secp256k1_ecrecover(&sig, &msg, 0);
        i = i + 1;
    };
}

public fun k1_verify_loop(n: u64) {
    let (sig, pk, msg) = (K1_SIG, K1_PK, K1_MSG);
    let mut i = 0;
    while (i < n) {
        assert!(ecdsa_k1::secp256k1_verify(&sig, &pk, &msg, 0), EWrong);
        i = i + 1;
    };
}

public fun bls_loop(n: u64) {
    let round = BLS_ROUND;
    let mut round_bytes = bcs::to_bytes(&round);
    round_bytes.reverse();
    let mut msg = BLS_PREV_SIG;
    msg.append(round_bytes);
    let digest = std_hash::sha2_256(msg);
    let (sig, pk) = (BLS_SIG, BLS_PK);
    let mut i = 0;
    while (i < n) {
        assert!(bls12381::bls12381_min_pk_verify(&sig, &pk, &digest), EWrong);
        i = i + 1;
    };
}

public fun ecvrf_loop(n: u64) {
    let (output, alpha, pk, proof) = (VRF_OUTPUT, b"Hello, world!", VRF_PK, VRF_PROOF);
    let mut i = 0;
    while (i < n) {
        assert!(ecvrf::ecvrf_verify(&output, &alpha, &pk, &proof), EWrong);
        i = i + 1;
    };
}

public fun g1_mul_loop(n: u64) {
    let scalar = bls12381::scalar_from_u64(123456789);
    let mut point = bls12381::g1_generator();
    let mut i = 0;
    while (i < n) {
        point = bls12381::g1_mul(&scalar, &point);
        i = i + 1;
    };
}

public fun hash_to_g2_loop(data: vector<u8>, n: u64) {
    let mut i = 0;
    while (i < n) {
        bls12381::hash_to_g2(&data);
        i = i + 1;
    };
}

public fun bcs_loop(len: u64, n: u64) {
    let mut v = vector[];
    let mut i = 0;
    while (i < len) {
        v.push_back(i);
        i = i + 1;
    };
    let mut j = 0;
    while (j < n) {
        bcs::to_bytes(&v);
        j = j + 1;
    };
}

public fun utf8_loop(data: vector<u8>, n: u64) {
    let mut i = 0;
    while (i < n) {
        string::utf8(data);
        i = i + 1;
    };
}

public fun df_read(parent: &Parent, start: u64, count: u64): u64 {
    let mut sum = 0;
    let mut i = start;
    while (i < start + count) {
        sum = sum + *df::borrow<u64, u64>(&parent.id, i);
        i = i + 1;
    };
    sum
}

public fun emit(data: vector<u8>, n: u64) {
    let mut i = 0;
    while (i < n) {
        event::emit(Blob { data });
        i = i + 1;
    };
}

public fun create_objects(n: u64, ctx: &mut TxContext) {
    let mut i = 0;
    while (i < n) {
        transfer::public_transfer(Small { id: object::new(ctx), value: i }, ctx.sender());
        i = i + 1;
    };
}

public struct Big has key, store {
    id: UID,
    data: vector<u8>,
}

/// `size` bytes built from one `bcs::to_bytes` of a `u64` vector.
fun bytes(size: u64): vector<u8> {
    let mut words = vector[];
    let mut i = 0;
    while (i < size / 8) {
        words.push_back(i);
        i = i + 1;
    };
    bcs::to_bytes(&words)
}

public fun emit_big(size: u64, n: u64) {
    let data = bytes(size);
    let mut i = 0;
    while (i < n) {
        event::emit(Blob { data });
        i = i + 1;
    };
}

public fun create_big(size: u64, n: u64, ctx: &mut TxContext) {
    let data = bytes(size);
    let mut i = 0;
    while (i < n) {
        transfer::public_transfer(Big { id: object::new(ctx), data }, ctx.sender());
        i = i + 1;
    };
}

public fun sink(_data: vector<u8>) {}
