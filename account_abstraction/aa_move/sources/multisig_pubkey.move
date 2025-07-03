// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module account_abstraction::multisig_pubkey;

const EInvalidMultiSigPublicKey: u64 = 0;
const EInputPublicKey: u64 = 1;

const MAX_SIGNER_IN_MULTISIG: u64 = 10;

/// Holds the raw bytes of a MultiSig Public Key, including the list public keys,
/// their weights, and the threshold for signing.
public struct MultiSigPublicKey has copy, drop, store {
    pks: vector<vector<u8>>,
    weights: vector<u8>,
    threshold: u16,
}

/// Create a new MultiSigPublicKey with the given public keys, weights, and threshold.
public fun new(pks: vector<vector<u8>>, weights: vector<u8>, threshold: u16): MultiSigPublicKey {
    assert!(
        pks.length() > 0 
        && weights.length() > 0 
        && threshold > 0 
        && pks.length() == weights.length() 
        && pks.length() <= MAX_SIGNER_IN_MULTISIG
        && weights.contains(&0) == false
        && weights.fold!(0 as u16, |acc, e| acc + (e as u16)) >= threshold,
        EInvalidMultiSigPublicKey,
    );
    pks.zip_do_ref!(&vector::tabulate!(pks.length(), |i| i), |a, b| {
        let mut i = *b + 1;
        while (i < pks.length()) {
            assert!(a != pks[i], EInvalidMultiSigPublicKey);
            i = i + 1;
        }
    });

    MultiSigPublicKey {
        pks,
        weights,
        threshold,
    }
}

/// Check if the given public key is part of the MultiSigPublicKey.
public fun contains(multisig: &MultiSigPublicKey, pk: &vector<u8>): bool {
    multisig.pks.contains(pk)
}

/// Check if the given public keys are part of the MultiSigPublicKey and if their total weight meets the threshold.
public fun check_threshold(multisig: &MultiSigPublicKey, pks: &vector<vector<u8>>): bool {
    let mut total_weight = 0;
    pks.do_ref!(|pk| {
        let (res, i) = multisig.pks.index_of(pk);
        assert!(res, EInputPublicKey);
        total_weight = total_weight + multisig.weights[i];
    });

    total_weight as u16 >= multisig.threshold
}
