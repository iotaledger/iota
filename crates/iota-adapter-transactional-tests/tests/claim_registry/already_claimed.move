// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Attempting to claim an address that has already been claimed must abort with
// EAlreadyClaimed (code 1).
// Account A in the transactional test runner is deterministic (fixed RNG seed).
// Its Ed25519 public key is:
// 7f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928
// which derives to A's address via Blake2b256(pk_bytes) — Ed25519 has no flag prefix.

//# init --accounts A --addresses test=0x0

// First claim — must succeed: pubkey derives to A's address.

//# run iota::claim_registry::claim_ed25519 --sender A --args object(0x11) x"7f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"

// Second claim — must fail with EAlreadyClaimed (code 1).

//# run iota::claim_registry::claim_ed25519 --sender A --args object(0x11) x"7f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
