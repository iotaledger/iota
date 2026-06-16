// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Happy path: create a mutable smart_account::SmartAccount backed by the built-in
// Ed25519 authenticator via `builtin_auth_builder_v1` + `build_v1`.
// The sender does not need to match the key's derived address because
// `builtin_auth_builder_v1` allocates a fresh UID rather than claiming an
// existing address.

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs x"00cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88"
//> iota::public_key::from_prefixed_bytes(Input(0));
//> 1: iota::smart_account::builtin_auth_builder_v1(Result(0));
//> 2: iota::smart_account::build_v1(Result(1));
