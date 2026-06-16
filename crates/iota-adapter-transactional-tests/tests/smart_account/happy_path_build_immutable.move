// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Happy path: create an immutable smart_account::SmartAccount via
// `builtin_auth_builder_v1` + `build_immutable_v1`.
// Immutable accounts are frozen at creation — neither the authenticator nor
// any dynamic fields can be changed afterwards.

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs x"00cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88"
//> iota::public_key::from_prefixed_bytes(Input(0));
//> 1: iota::smart_account::builtin_auth_builder_v1(Result(0));
//> 2: iota::smart_account::build_immutable_v1(Result(1));
