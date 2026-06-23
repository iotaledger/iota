// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::ed25519;

/// @param public_key: 32-byte candidate Ed25519 public key.
///
/// Returns true if `public_key` is a valid point on the Ed25519 curve: the encoded
/// y-coordinate must have a corresponding x-coordinate on the curve (i.e. x² must be
/// a quadratic residue mod p) and y must be less than p. Returns false otherwise.
/// Approximately half of all 32-byte inputs fail this check.
public native fun ed25519_validate_pubkey(public_key: &vector<u8>): bool;

/// @param signature: 64-byte Ed25519 signature associated with the Ed25519 elliptic curve.
/// @param public_key: 32-byte public key that represents a point on the Ed25519 elliptic curve.
/// @param msg: The message that we test the signature against.
///
/// If the signature is a valid Ed25519 signature of the message and public key, return true.
/// Otherwise, return false.
public native fun ed25519_verify(
    signature: &vector<u8>,
    public_key: &vector<u8>,
    msg: &vector<u8>,
): bool;
