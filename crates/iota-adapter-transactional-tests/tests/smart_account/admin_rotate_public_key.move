// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Tests `rotate_builtin_auth_public_key` on a claimed account. After claiming
// with the Ed25519 key that derives to address(A), the admin rotates to a
// different Ed25519 key. `detach_builtin_auth_public_key` +
// `attach_builtin_auth_public_key` is also exercised in a follow-up task.

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs object(0x10) x"007f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::claim_builder_v1(Input(0), Result(0));
//> 2: iota::smart_account::build_v1(Result(1));

//# programmable --sender A --inputs object(1,3) x"00cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88"
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::rotate_builtin_auth_public_key(Input(0), Result(0));

//# programmable --sender A --inputs object(1,3)
//> iota::smart_account::detach_builtin_auth_public_key(Input(0));

//# programmable --sender A --inputs object(1,3) x"007f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::attach_builtin_auth_public_key(Input(0), Result(0));
