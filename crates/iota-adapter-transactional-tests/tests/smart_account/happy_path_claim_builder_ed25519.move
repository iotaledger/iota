// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Happy path: claim an existing address via `claim_builder_v1` (Ed25519) and
// build a mutable smart_account::SmartAccount.  `ClaimRegistry` records each
// address once to prevent double-claiming and ensures the new Account object's
// ID equals the sender's address.  Key 7f51... was chosen so that
// Blake2b256(key) == address(A) in the test framework (same key used in
// claim_registry/happy_path_custom_account.move).

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs object(0x10) x"007f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::claim_builder_v1(Input(0), Result(0));
//> 2: iota::smart_account::build_v1(Result(1));

//# view-object 1,0
