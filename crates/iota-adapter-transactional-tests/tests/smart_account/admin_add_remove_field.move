// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Tests admin field operations on a claimed account. Because `claim_builder_v1`
// assigns the account an object ID equal to the sender's address, sender A can
// exercise the admin functions `add_field` and `remove_field`.
// Note: `borrow_field` returns `&Value` which PTBs cannot use as a command result,
// so it is exercised only in Move unit tests.

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs object(0x10) x"007f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::claim_builder_v1(Input(0), Result(0));
//> 2: iota::smart_account::build_v1(Result(1));

//# programmable --sender A --inputs object(1,3) 7u8 100u64
//> iota::smart_account::add_field<u8, u64>(Input(0), Input(1), Input(2));

//# programmable --sender A --inputs object(1,3) 7u8
//> iota::smart_account::remove_field<u8, u64>(Input(0), Input(1));
