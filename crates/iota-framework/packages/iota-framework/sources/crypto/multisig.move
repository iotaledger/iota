// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::multisig;

/// @param public_key: BCS-encoded `MultiSigPublicKey` bytes — the MultiSig raw key material.
///
/// Returns `true` if `public_key` deserializes into a well-formed MultiSig committee that
/// passes the canonical validation rules enforced by the node: between 1 and 10 distinct
/// members, every member key a valid point on its curve, every member weight greater than
/// zero, the threshold greater than zero, and the total member weight at least the threshold.
/// Returns `false` if the bytes fail to deserialize, contain trailing data, or violate any of
/// these rules.
public native fun multisig_validate_pubkey(public_key: &vector<u8>): bool;
