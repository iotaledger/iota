// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// `with_field` attaches a dynamic field to the AccountBuilder before finalizing.
// The resulting account object carries one extra child object compared to the
// baseline claim path (additional u64 dynamic field).

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs object(0x10) x"007f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928" 42u8 99u64
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::claim_builder_v1(Input(0), Result(0));
//> 2: iota::smart_account::with_field<u8, u64>(Result(1), Input(2), Input(3));
//> 3: iota::smart_account::build_v1(Result(2));
