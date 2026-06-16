// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// `from_prefixed_bytes` validates that the raw key bytes have the length
// required by the declared scheme. Here the same 32-byte payload used for
// Ed25519 is prefixed with flag 0x01 (Secp256k1), which requires 33 bytes.
// The call must abort with EInvalidPublicKeyLength.

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs x"01cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88"
//> iota::public_key::from_prefixed_bytes(Input(0));
