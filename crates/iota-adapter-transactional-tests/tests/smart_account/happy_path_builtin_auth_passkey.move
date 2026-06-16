// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Happy path: create a mutable smart_account::SmartAccount backed by the built-in
// Passkey authenticator via `builtin_auth_builder_v1` + `build_v1`.
// Passkey uses a 33-byte compressed secp256r1 (P-256) key.
// Layout: [0x06 (Passkey flag)] || [33-byte compressed key]
// `builtin_auth_builder_v1` imposes no address-matching requirement, so sender A
// can supply any public key.  A `claim_builder_v1` equivalent is not possible here:
// it would require a passkey key whose Blake2b256 hash equals address(A), which
// cannot be computed (preimage of Blake2b256).

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs x"060227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a"
//> iota::public_key::from_prefixed_bytes(Input(0));
//> 1: iota::smart_account::builtin_auth_builder_v1(Result(0));
//> 2: iota::smart_account::build_v1(Result(1));
