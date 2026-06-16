// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Happy path: create a mutable smart_account::SmartAccount backed by the built-in
// Secp256k1 authenticator via `builtin_auth_builder_v1` + `build_v1`.

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs x"0102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c"
//> iota::public_key::from_prefixed_bytes(Input(0));
//> 1: iota::smart_account::builtin_auth_builder_v1(Result(0));
//> 2: iota::smart_account::build_v1(Result(1));
