// Copyright (c) ronanyeah
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module lean_imt_account::lean_imt;

use iota::address;
use iota::groth16;
use iota::hash;
use iota::poseidon;

public(package) fun verify_proof(
    pvk: vector<u8>,
    proof_points: vector<u8>,
    root: vector<u8>,
    leaf: vector<u8>,
) {
    let pvk = groth16::prepare_verifying_key(&groth16::bn254(), &pvk);

    let mut public_input_bytes = root;
    public_input_bytes.append(leaf);

    let proof_points = groth16::proof_points_from_bytes(proof_points);
    let public_inputs = groth16::public_proof_inputs_from_bytes(public_input_bytes);
    assert!(groth16::verify_groth16_proof(&groth16::bn254(), &pvk, &public_inputs, &proof_points));
}

public(package) fun hash_address(addr: address): vector<u8> {
    hash_bytes(addr.to_bytes())
}

public(package) fun hash_bytes(bts: vector<u8>): vector<u8> {
    let mut v1: vector<u256> = vector::empty();
    let mut v2: vector<u256> = vector::empty();

    let mut i = 0;
    while (i < 16) {
        let byte = *vector::borrow(&bts, i);
        vector::push_back(&mut v1, (byte as u256));
        i = i + 1;
    };
    while (i < 32) {
        let byte = *vector::borrow(&bts, i);
        vector::push_back(&mut v2, (byte as u256));
        i = i + 1;
    };

    let res1 = poseidon::poseidon_bn254(&v1);
    let res2 = poseidon::poseidon_bn254(&v2);

    let final_hash = poseidon::poseidon_bn254(&vector[res1, res2]);

    iota::bcs::to_bytes(&final_hash)
}

public fun derive_leaf_from_public_key(public_key: vector<u8>): vector<u8> {
    let address = address::from_bytes(hash::blake2b256(&public_key));
    let address_bytes = hash_address(address);
    hash_bytes(address_bytes)
}

/// test

#[test_only]
public fun test_proof(
    pvk: vector<u8>,
    proof_points: vector<u8>,
    root: vector<u8>,
    leaf: vector<u8>,
) {
    verify_proof(pvk, proof_points, root, leaf);
}
