// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Happy path: `test_claim_account` exercises the full claim flow from a PTB
// without publishing a separate module.  It creates a `DummyAccount` object
// owned by the sender.  The object ID equals the sender's address because
// `claim` derives the UID via `new_uid_from_hash(sender_address)`.
// Key 7f51... was chosen so that Blake2b256(key) == address(A) in the test framework.

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs object(0x10) x"007f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::claim_registry::test_claim_account(Input(0), Result(0));

//# view-object 1,0
