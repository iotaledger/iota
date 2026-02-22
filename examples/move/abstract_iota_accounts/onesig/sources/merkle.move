// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//
// Portions of this file are derived from
// https://github.com/dakaii/sui-merkle-verifier
// Copyright (c) 2025 dakaii
// SPDX-License-Identifier: MIT

module onesig::merkle;

use iota::hash;

#[error(code = 0)]
const EInvalidHashLength: vector<u8> = b"A hash length is invalid.";
#[error(code = 1)]
const EInvalidPositionsLength: vector<u8> = b"A positions vector length is invalid.";

const HASH_LENGTH: u64 = 32;

/// Verify with pre-hashed 32-byte leaf and lexicographic concatenation (sorted-pair Merkle trees).
public fun verify_sorted_keccak(
    leaf_hash: &vector<u8>,
    root: &vector<u8>,
    proof: &vector<vector<u8>>,
): bool {
    assert!(root.length() == HASH_LENGTH, EInvalidHashLength);
    let mut idx = 0;
    let proof_len = proof.length();
    while (idx < proof_len) {
        let sib = proof.borrow(idx);
        assert!(sib.length() == HASH_LENGTH, EInvalidHashLength);
        idx = idx + 1;
    };

    let mut cur = *leaf_hash;
    let mut i = 0;
    while (i < proof_len) {
        let sib = proof.borrow(i);
        cur = hash_pair_sorted(&cur, sib);
        i = i + 1;
    };
    cur.length() == HASH_LENGTH && bytes_equal(&cur, root)
}

/// Convenience: raw leaf bytes -> keccak256 before proof processing.
public fun verify_sorted_keccak_from_leaf_bytes(
    leaf_raw: &vector<u8>,
    root: &vector<u8>,
    proof: &vector<vector<u8>>,
): bool {
    let leaf_hash = hash::keccak256(leaf_raw);
    verify_sorted_keccak(&leaf_hash, root, proof)
}

/// Verify using explicit left/right positions (false = sibling on left, true = sibling on right).
public fun verify_with_positions_keccak(
    leaf_hash: &vector<u8>,
    root: &vector<u8>,
    proof: &vector<vector<u8>>,
    positions: &vector<bool>,
): bool {
    assert!(root.length() == HASH_LENGTH, EInvalidHashLength);
    let proof_len = proof.length();
    assert!(positions.length() == proof_len, EInvalidPositionsLength);

    let mut cur = *leaf_hash;
    let mut i = 0;
    while (i < proof_len) {
        let sib = proof.borrow(i);
        assert!(sib.length() == HASH_LENGTH, EInvalidHashLength);
        let right = *positions.borrow(i);
        let pair = if (right) { concat(&cur, sib) } else { concat(sib, &cur) };
        cur = hash::keccak256(&pair);
        i = i + 1;
    };
    bytes_equal(&cur, root)
}

/// Hash a pair of byte vectors with lexicographic sorting (sorted-pair Merkle trees).
public fun hash_pair_sorted(left: &vector<u8>, right: &vector<u8>): vector<u8> {
    let pair = if (bytes_lt(left, right)) {
        vector::flatten(vector[*left, *right])
    } else {
        vector::flatten(vector[*right, *left])
    };
    hash::keccak256(&pair)
}

/// Lexicographic bytes compare: returns true if a < b.
public fun bytes_lt(a: &vector<u8>, b: &vector<u8>): bool {
    let la = a.length();
    let lb = b.length();
    let mut i = 0;
    let min = if (la < lb) { la } else { lb };
    while (i < min) {
        let a_element = *a.borrow(i);
        let b_element = *b.borrow(i);
        if (a_element < b_element) return true;
        if (a_element > b_element) return false;
        i = i + 1;
    };
    // all equal up to min; shorter one is "less"
    la < lb
}

/// Builds a Merkle tree from an arbitrary number of leaves and returns
/// the root together with one proof per leaf.
public fun build_merkle_tree_with_proofs(
    leaves: vector<vector<u8>>,
): (vector<u8>, vector<vector<vector<u8>>>) {
    let n = leaves.length();
    assert!(n > 0);

    // Hash every leaf and initialise per-leaf bookkeeping.
    let mut current_level: vector<vector<u8>> = vector[];
    let mut proofs: vector<vector<vector<u8>>> = vector[];
    let mut leaf_pos: vector<u64> = vector[];
    let mut i = 0;
    while (i < n) {
        current_level.push_back(hash::keccak256(&leaves[i]));
        proofs.push_back(vector[]);
        leaf_pos.push_back(i);
        i = i + 1;
    };

    // Build the tree bottom-up, collecting proof siblings along the way.
    while (current_level.length() > 1) {
        let level_len = current_level.length();
        let mut next_level: vector<vector<u8>> = vector[];

        // Pair adjacent nodes.
        let mut j = 0;
        while (j + 1 < level_len) {
            next_level.push_back(
                hash_pair_sorted(&current_level[j], &current_level[j + 1]),
            );
            j = j + 2;
        };

        // Carry over an unpaired trailing node.
        if (level_len % 2 == 1) {
            next_level.push_back(current_level[level_len - 1]);
        };

        // For every original leaf, record its sibling and update its position.
        i = 0;
        while (i < n) {
            let pos = leaf_pos[i];
            if (pos % 2 == 0) {
                if (pos + 1 < level_len) {
                    proofs[i].push_back(current_level[pos + 1]);
                };
                // else: unpaired last node, no sibling to record
            } else {
                proofs[i].push_back(current_level[pos - 1]);
            };
            *&mut leaf_pos[i] = pos / 2;
            i = i + 1;
        };

        current_level = next_level;
    };

    (current_level[0], proofs)
}

/// Compare two byte vectors for equality.
fun bytes_equal(a: &vector<u8>, b: &vector<u8>): bool {
    let la = a.length();
    let lb = b.length();
    if (la != lb) return false;

    let mut i = 0;
    while (i < la) {
        if (*a.borrow(i) != *b.borrow(i)) return false;
        i = i + 1;
    };
    true
}

/// Concatenate two borrowed byte vectors: out = a || b.
fun concat(a: &vector<u8>, b: &vector<u8>): vector<u8> {
    let mut out = vector::empty<u8>();
    out.append(*a);
    out.append(*b);
    out
}
