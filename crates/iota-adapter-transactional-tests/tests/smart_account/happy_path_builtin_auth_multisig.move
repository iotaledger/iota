// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Happy path: create a mutable smart_account::SmartAccount backed by the built-in
// MultiSig authenticator via `builtin_auth_builder_v1` + `build_v1`.
// Minimal BCS-encoded MultiSigPublicKey: 1 Ed25519 signer, weight=1, threshold=1.
// Layout: [0x03] || ULEB128(num_signers=1) | ULEB128(tag=0) | 32-byte key | weight=1 | threshold=1
// `builtin_auth_builder_v1` imposes no address-matching requirement, so sender A
// can supply any public key.  A `claim_builder_v1` equivalent is not possible here:
// it would require a multisig key whose Blake2b256 hash equals address(A), which
// cannot be computed (preimage of Blake2b256).

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs x"030100cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010100"
//> iota::public_key::from_prefixed_bytes(Input(0));
//> 1: iota::smart_account::builtin_auth_builder_v1(Result(0));
//> 2: iota::smart_account::build_v1(Result(1));
