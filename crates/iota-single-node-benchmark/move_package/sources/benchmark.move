// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module move_benchmark::benchmark {
    use std::ascii;
    use std::ascii::String;
    use iota::coin::Coin;
    use iota::dynamic_field;
    use iota::iota::IOTA;

    public fun transfer_coin(coin: Coin<IOTA>, ctx: &TxContext) {
        transfer::public_transfer(coin, tx_context::sender(ctx));
    }

    // === compute-heavy workload ===

    public fun run_computation(mut num: u64) {
        // Store all numbers in an array to exercise memory consumption.
        let mut results = vector<u64>[];
        vector::push_back(&mut results, 1);
        vector::push_back(&mut results, 1);
        while (num > 0) {
            let len = vector::length(&results);
            let last = vector::borrow(&results, len - 1);
            let second_last = vector::borrow(&results, len - 2);
            let mut sum = *last + *second_last;
            if (sum >= 1_000_000_000_000_000_000u64) {
                sum = sum % 1_000_000_000_000_000_000u64;
            };
            vector::push_back(&mut results, sum);
            num = num - 1;
        }
    }

    // === dynamic field workload ===

    public struct RootObject has key {
        id: UID,
        child_count: u64,
    }

    public struct Child has store {
        field1: u64,
        field2: String,
          payload: vector<u8>,
    }

    public entry fun generate_dynamic_fields(num: u64, payload_size: u64, ctx: &mut TxContext) {
        let mut payload = vector<u8>[];
        let mut j = 0;
        while (j < payload_size) {
            vector::push_back(&mut payload, 7u8);
            j = j + 1;
        };
        let mut root = RootObject {
            id: object::new(ctx),
            child_count: num,
        };
        let mut i = 0;
        while (i < num) {
            let child = Child {
                field1: i,
                field2: ascii::string(b"a string"),
                payload: copy payload,
            };
            dynamic_field::add(&mut root.id, i, child);
            i = i + 1;
        };
        transfer::transfer(root, tx_context::sender(ctx));
    }

    /// Delete `num` of the dynamic-field children created by
    /// `generate_dynamic_fields`: real deletions of pre-existing objects, the
    /// data source for the tombstone (compaction) constant.
    public fun delete_dynamic_fields(root: &mut RootObject, num: u64) {
        assert!(num <= root.child_count, 0);
        let mut i = 0;
        while (i < num) {
            let Child { field1: _, field2: _, payload: _ } = dynamic_field::remove(&mut root.id, i);
            i = i + 1;
        };
        root.child_count = root.child_count - num;
    }

    public fun read_dynamic_fields(root: &RootObject) {
        let mut i = 0;
        while (i < root.child_count) {
            let child: &Child = dynamic_field::borrow(&root.id, i);
            assert!(child.field1 == i, 0);
            i = i + 1;
        }
    }

    // === shared object workload ===

    public struct SharedCounter has key {
        id: UID,
        count: u64,
    }

    public fun create_shared_counter(ctx: &mut TxContext) {
        let counter = SharedCounter {
            id: object::new(ctx),
            count: 0,
        };
        transfer::share_object(counter);
    }

    public fun increment_shared_counter(counter: &mut SharedCounter) {
        counter.count = counter.count + 1;
    }

    // === mint workload ===

    public struct NFT has key {
        id: UID,
        // mimic NFT's of arbitrary size
        contents: vector<u8>,
    }

    /// Create one NFT, send it to `recipient`
    public fun mint_one(recipient: address, contents: vector<u8>, ctx: &mut TxContext) {
        let nft = NFT { id: object::new(ctx), contents };
        transfer::transfer(nft, recipient)
    }

    /// Create one NFT, send it to each of the `recipients`
    public fun batch_mint(recipients: vector<address>, contents: vector<u8>, ctx: &mut TxContext) {
        let mut i = 0;
        let len = recipients.length();
        while (i < len) {
            let nft = NFT { id: object::new(ctx), contents };
            transfer::transfer(nft, recipients[i]);
            i = i + 1
        }
    }

    // === interpreter cost components, de-correlated ===
    // Three shapes that separate the interpreter's cost components, which the
    // Fibonacci loop above drives together: instruction count, operand-stack
    // pushes, and operand-stack byte flow.

    /// Tight scalar arithmetic: high instruction count, minimal stack bytes,
    /// no allocation.
    public fun scalar_arithmetic(mut iterations: u64) {
        let mut acc = 1u64;
        while (iterations > 0) {
            acc = acc * 31 + iterations;
            acc = acc ^ (acc >> 7);
            acc = acc % 1_000_000_007;
            iterations = iterations - 1;
        };
        assert!(acc != 0, 0);
    }

    public struct Wide has drop {
        a: u64,
        b: u64,
        c: u64,
        d: u64,
        e: u64,
        f: u64,
        g: u64,
        h: u64,
    }

    /// Pack/unpack-heavy: an unpack pushes all eight fields in one
    /// instruction, so pushes are maximized relative to instruction count,
    /// with small values.
    public fun push_pop(mut iterations: u64) {
        while (iterations > 0) {
            let w = Wide { a: 1, b: 2, c: 3, d: 4, e: 5, f: 6, g: 7, h: 8 };
            let Wide { a, b: _, c: _, d: _, e: _, f: _, g: _, h: _ } = w;
            iterations = iterations - a;
        }
    }

    /// Large-value moves: each move of the vector pushes its full abstract
    /// size, so stack byte flow is maximized relative to instruction count.
    public fun vector_move(mut iterations: u64, size: u64) {
        let mut v = vector<u8>[];
        let mut i = 0;
        while (i < size) {
            vector::push_back(&mut v, ((i % 256) as u8));
            i = i + 1;
        };
        while (iterations > 0) {
            let w = v;
            v = w;
            iterations = iterations - 1;
        };
        assert!(vector::length(&v) == size, 0);
    }

    // === working-memory shapes ===

    /// Grow a vector held in a local to `bytes` and keep it live: drives the
    /// locals-memory peak with little other work.
    public fun vector_in_locals(bytes: u64) {
        let mut v = vector<u8>[];
        let mut i = 0;
        while (i < bytes) {
            vector::push_back(&mut v, 7u8);
            i = i + 1;
        };
        assert!(vector::length(&v) == bytes, 0);
    }

    // Move forbids recursive struct types, so tree depth comes from distinct
    // per-level types (up to 4 levels); `width` scales freely, giving
    // width^depth leaves.

    public struct Leaf has drop {
        payload: vector<u8>,
    }

    public struct Level1 has drop {
        children: vector<Leaf>,
    }

    public struct Level2 has drop {
        children: vector<Level1>,
    }

    public struct Level3 has drop {
        children: vector<Level2>,
    }

    public struct Level4 has drop {
        children: vector<Level3>,
    }

    /// Build a tree of nested structs, `depth` levels (1..=4) with `width`
    /// children per node: a branched heap shape held live, in contrast to
    /// the flat vector above.
    public fun struct_tree(depth: u64, width: u64) {
        assert!(depth >= 1 && depth <= 4, 0);
        if (depth == 1) {
            let t = build_level1(width);
            assert!(vector::length(&t.children) == width, 0);
        } else if (depth == 2) {
            let t = build_level2(width);
            assert!(vector::length(&t.children) == width, 0);
        } else if (depth == 3) {
            let t = build_level3(width);
            assert!(vector::length(&t.children) == width, 0);
        } else {
            let t = build_level4(width);
            assert!(vector::length(&t.children) == width, 0);
        }
    }

    fun build_level1(width: u64): Level1 {
        let mut children = vector<Leaf>[];
        let mut i = 0;
        while (i < width) {
            vector::push_back(&mut children, Leaf { payload: b"benchmark-node-payload!" });
            i = i + 1;
        };
        Level1 { children }
    }

    fun build_level2(width: u64): Level2 {
        let mut children = vector<Level1>[];
        let mut i = 0;
        while (i < width) {
            vector::push_back(&mut children, build_level1(width));
            i = i + 1;
        };
        Level2 { children }
    }

    fun build_level3(width: u64): Level3 {
        let mut children = vector<Level2>[];
        let mut i = 0;
        while (i < width) {
            vector::push_back(&mut children, build_level2(width));
            i = i + 1;
        };
        Level3 { children }
    }

    fun build_level4(width: u64): Level4 {
        let mut children = vector<Level3>[];
        let mut i = 0;
        while (i < width) {
            vector::push_back(&mut children, build_level3(width));
            i = i + 1;
        };
        Level4 { children }
    }

    // === native families ===
    // One looping entry point per native family; signature fixtures are
    // generated by the transaction generator and must be valid (the asserts
    // abort the transaction on an invalid fixture).

    public fun sha2_256_loop(mut calls: u64, input: vector<u8>) {
        while (calls > 0) {
            let digest = std::hash::sha2_256(copy input);
            assert!(vector::length(&digest) == 32, 0);
            calls = calls - 1;
        }
    }

    public fun sha3_256_loop(mut calls: u64, input: vector<u8>) {
        while (calls > 0) {
            let digest = std::hash::sha3_256(copy input);
            assert!(vector::length(&digest) == 32, 0);
            calls = calls - 1;
        }
    }

    public fun keccak256_loop(mut calls: u64, input: vector<u8>) {
        while (calls > 0) {
            let digest = iota::hash::keccak256(&input);
            assert!(vector::length(&digest) == 32, 0);
            calls = calls - 1;
        }
    }

    public fun blake2b256_loop(mut calls: u64, input: vector<u8>) {
        while (calls > 0) {
            let digest = iota::hash::blake2b256(&input);
            assert!(vector::length(&digest) == 32, 0);
            calls = calls - 1;
        }
    }

    public fun hmac_sha3_256_loop(mut calls: u64, key: vector<u8>, msg: vector<u8>) {
        while (calls > 0) {
            let digest = iota::hmac::hmac_sha3_256(&key, &msg);
            assert!(vector::length(&digest) == 32, 0);
            calls = calls - 1;
        }
    }

    public fun ed25519_verify_loop(
        mut calls: u64,
        signature: vector<u8>,
        public_key: vector<u8>,
        msg: vector<u8>,
    ) {
        while (calls > 0) {
            assert!(iota::ed25519::ed25519_verify(&signature, &public_key, &msg), 0);
            calls = calls - 1;
        }
    }

    public fun bls12381_min_sig_verify_loop(
        mut calls: u64,
        signature: vector<u8>,
        public_key: vector<u8>,
        msg: vector<u8>,
    ) {
        while (calls > 0) {
            assert!(iota::bls12381::bls12381_min_sig_verify(&signature, &public_key, &msg), 0);
            calls = calls - 1;
        }
    }

    public fun bls12381_min_pk_verify_loop(
        mut calls: u64,
        signature: vector<u8>,
        public_key: vector<u8>,
        msg: vector<u8>,
    ) {
        while (calls > 0) {
            assert!(iota::bls12381::bls12381_min_pk_verify(&signature, &public_key, &msg), 0);
            calls = calls - 1;
        }
    }

    /// The 1 selects sha256 as the message hash, matching how the fixture
    /// signs.
    public fun secp256k1_verify_loop(
        mut calls: u64,
        signature: vector<u8>,
        public_key: vector<u8>,
        msg: vector<u8>,
    ) {
        while (calls > 0) {
            assert!(iota::ecdsa_k1::secp256k1_verify(&signature, &public_key, &msg, 1), 0);
            calls = calls - 1;
        }
    }

    /// The 1 selects sha256 as the message hash, matching how the fixture
    /// signs.
    public fun secp256r1_verify_loop(
        mut calls: u64,
        signature: vector<u8>,
        public_key: vector<u8>,
        msg: vector<u8>,
    ) {
        while (calls > 0) {
            assert!(iota::ecdsa_r1::secp256r1_verify(&signature, &public_key, &msg, 1), 0);
            calls = calls - 1;
        }
    }

    public fun ecvrf_verify_loop(
        mut calls: u64,
        hash: vector<u8>,
        alpha_string: vector<u8>,
        public_key: vector<u8>,
        proof: vector<u8>,
    ) {
        while (calls > 0) {
            assert!(iota::ecvrf::ecvrf_verify(&hash, &alpha_string, &public_key, &proof), 0);
            calls = calls - 1;
        }
    }

    /// Verify a fixed valid Groth16 proof `calls` times; `curve_id` 0 is
    /// BLS12-381, anything else BN254. The prepared-verifying-key parts are
    /// passed pre-prepared so the loop measures only the verify native.
    public fun groth16_verify_loop(
        mut calls: u64,
        curve_id: u8,
        vk_bytes: vector<u8>,
        alpha_bytes: vector<u8>,
        gamma_bytes: vector<u8>,
        delta_bytes: vector<u8>,
        inputs_bytes: vector<u8>,
        proof_bytes: vector<u8>,
    ) {
        let curve = if (curve_id == 0) {
            iota::groth16::bls12381()
        } else {
            iota::groth16::bn254()
        };
        let pvk = iota::groth16::pvk_from_bytes(vk_bytes, alpha_bytes, gamma_bytes, delta_bytes);
        let inputs = iota::groth16::public_proof_inputs_from_bytes(inputs_bytes);
        let proof = iota::groth16::proof_points_from_bytes(proof_bytes);
        while (calls > 0) {
            assert!(iota::groth16::verify_groth16_proof(&curve, &pvk, &inputs, &proof), 0);
            calls = calls - 1;
        }
    }

    public fun poseidon_loop(mut calls: u64, num_inputs: u64) {
        let mut inputs = vector<u256>[];
        let mut i = 0;
        while (i < num_inputs) {
            vector::push_back(&mut inputs, (i as u256) + 1);
            i = i + 1;
        };
        while (calls > 0) {
            let digest = iota::poseidon::poseidon_bn254(&inputs);
            assert!(digest != 0, 0);
            calls = calls - 1;
        }
    }

    // BLS12-381 group operations (the `0x2::group_ops` native tag), one loop
    // per operation so each can be swept alone: addition and scalar
    // multiplication (cheap/medium, fixed-size inputs), hash-to-curve (per
    // message byte), and pairing (the expensive one).

    public fun bls12381_g1_add_loop(mut calls: u64) {
        let g = iota::bls12381::g1_generator();
        while (calls > 0) {
            let _ = iota::bls12381::g1_add(&g, &g);
            calls = calls - 1;
        }
    }

    public fun bls12381_g1_mul_loop(mut calls: u64) {
        let s = iota::bls12381::scalar_from_u64(12345);
        let g = iota::bls12381::g1_generator();
        while (calls > 0) {
            let _ = iota::bls12381::g1_mul(&s, &g);
            calls = calls - 1;
        }
    }

    public fun bls12381_hash_to_g1_loop(mut calls: u64, msg: vector<u8>) {
        while (calls > 0) {
            let _ = iota::bls12381::hash_to_g1(&msg);
            calls = calls - 1;
        }
    }

    public fun bls12381_pairing_loop(mut calls: u64) {
        let g1 = iota::bls12381::g1_generator();
        let g2 = iota::bls12381::g2_generator();
        while (calls > 0) {
            let _ = iota::bls12381::pairing(&g1, &g2);
            calls = calls - 1;
        }
    }

    // === event workload ===

    public struct BenchEvent has copy, drop {
        payload: vector<u8>,
    }

    public fun emit_events(mut count: u64, size: u64) {
        let mut payload = vector<u8>[];
        let mut i = 0;
        while (i < size) {
            vector::push_back(&mut payload, 7u8);
            i = i + 1;
        };
        while (count > 0) {
            iota::event::emit(BenchEvent { payload: copy payload });
            count = count - 1;
        }
    }

    // === tie-breaking workloads ===
    // Shapes that decouple counters the block-composed workloads leave
    // perfectly correlated (see the calibration plan, Phase 2 work item 6).

    /// Mutate an owned object in place: a written object with no creation
    /// and no per-object native call.
    public fun mutate_object(nft: &mut NFT) {
        if (vector::length(&nft.contents) > 0) {
            let first = vector::borrow_mut(&mut nft.contents, 0);
            *first = (((*first as u64) + 1) % 256) as u8;
        };
    }

    /// Delete an owned object: a tombstone with no dynamic-field access.
    public fun burn_object(nft: NFT) {
        let NFT { id, contents: _ } = nft;
        object::delete(id);
    }

    // === interpreter push-ratio shapes ===
    // An unpack pushes all 32 fields in one instruction; the store/branch
    // loop pushes on only half its instructions. Together they span the
    // pushes-per-instruction range a stack machine allows.

    public struct Wide32 has drop {
        f0: u64,
        f1: u64,
        f2: u64,
        f3: u64,
        f4: u64,
        f5: u64,
        f6: u64,
        f7: u64,
        f8: u64,
        f9: u64,
        f10: u64,
        f11: u64,
        f12: u64,
        f13: u64,
        f14: u64,
        f15: u64,
        f16: u64,
        f17: u64,
        f18: u64,
        f19: u64,
        f20: u64,
        f21: u64,
        f22: u64,
        f23: u64,
        f24: u64,
        f25: u64,
        f26: u64,
        f27: u64,
        f28: u64,
        f29: u64,
        f30: u64,
        f31: u64,
    }

    /// High pushes-per-instruction: repeated unpack/repack of a 32-field
    /// struct.
    public fun high_push_ratio(mut iterations: u64) {
        let mut w = Wide32 { f0: 0, f1: 1, f2: 2, f3: 3, f4: 4, f5: 5, f6: 6, f7: 7, f8: 8, f9: 9, f10: 10, f11: 11, f12: 12, f13: 13, f14: 14, f15: 15, f16: 16, f17: 17, f18: 18, f19: 19, f20: 20, f21: 21, f22: 22, f23: 23, f24: 24, f25: 25, f26: 26, f27: 27, f28: 28, f29: 29, f30: 30, f31: 31 };
        while (iterations > 0) {
            let Wide32 { f0: g0, f1: g1, f2: g2, f3: g3, f4: g4, f5: g5, f6: g6, f7: g7, f8: g8, f9: g9, f10: g10, f11: g11, f12: g12, f13: g13, f14: g14, f15: g15, f16: g16, f17: g17, f18: g18, f19: g19, f20: g20, f21: g21, f22: g22, f23: g23, f24: g24, f25: g25, f26: g26, f27: g27, f28: g28, f29: g29, f30: g30, f31: g31 } = w;
            w = Wide32 { f0: g0, f1: g1, f2: g2, f3: g3, f4: g4, f5: g5, f6: g6, f7: g7, f8: g8, f9: g9, f10: g10, f11: g11, f12: g12, f13: g13, f14: g14, f15: g15, f16: g16, f17: g17, f18: g18, f19: g19, f20: g20, f21: g21, f22: g22, f23: g23, f24: g24, f25: g25, f26: g26, f27: g27, f28: g28, f29: g29, f30: g30, f31: g31 };
            iterations = iterations - 1;
        };
        let Wide32 { f0: keep, f1: _, f2: _, f3: _, f4: _, f5: _, f6: _, f7: _, f8: _, f9: _, f10: _, f11: _, f12: _, f13: _, f14: _, f15: _, f16: _, f17: _, f18: _, f19: _, f20: _, f21: _, f22: _, f23: _, f24: _, f25: _, f26: _, f27: _, f28: _, f29: _, f30: _, f31: _ } = w;
        assert!(keep == 0, 0);
    }

    /// Low pushes-per-instruction: move/store chains and branches.
    public fun low_push_ratio(mut iterations: u64) {
        let mut a = 1u64;
        while (iterations > 0) {
            let b = a;
            let c = b;
            let d = c;
            let e = d;
            let f = e;
            let g = f;
            let h = g;
            a = h;
            iterations = iterations - 1;
        };
        assert!(a == 1, 0);
    }
}
