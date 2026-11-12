// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::multisig;

/// @param public_key: BCS-encoded `MultiSigPublicKey` bytes — the MultiSig raw key material.
///
/// Returns `true` if `public_key` deserializes into a well-formed MultiSig committee that
/// passes the committee validation of the SDK: between 1 and 10 distinct members, every member
/// weight greater than zero, the threshold greater than zero, the total member weight at
/// least the threshold, and no Ed25519 member together with members of other schemes (an
/// Ed25519 member carries no scheme flag in the address preimage, so mixed committees could
/// share an address). Member keys are not checked against their curves. Returns `false` if
/// the bytes fail to deserialize, contain trailing data, or violate any of these rules.
public native fun multisig_validate_pubkey(public_key: &vector<u8>): bool;
