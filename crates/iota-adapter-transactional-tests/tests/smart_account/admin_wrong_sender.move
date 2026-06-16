// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Admin functions require the transaction sender to equal the account's address.
// Here account A is claimed by A but B attempts to call `add_field` on it, which
// must abort with `ETransactionSenderIsNotTheAccount` (error code 0).

//# init --accounts A B --addresses test=0x0

//# programmable --sender A --inputs object(0x10) x"007f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::claim_builder_v1(Input(0), Result(0));
//> 2: iota::smart_account::build_v1(Result(1));

//# programmable --sender B --inputs object(1,3) 0u8 42u64
//> iota::smart_account::add_field<u8, u64>(Input(0), Input(1), Input(2));
