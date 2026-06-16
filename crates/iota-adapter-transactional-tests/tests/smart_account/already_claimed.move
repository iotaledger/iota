// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Double-claiming check: the first `claim_builder_v1` call succeeds and the
// Account is created. A second `claim_builder_v1` for the same address in a
// separate transaction must abort with `EAlreadyClaimed` (claim_registry
// error code 1).

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs object(0x10) x"007f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::claim_builder_v1(Input(0), Result(0));
//> 2: iota::smart_account::build_v1(Result(1));

//# programmable --sender A --inputs object(0x10) x"007f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::smart_account::claim_builder_v1(Input(0), Result(0));
//> 2: iota::smart_account::build_v1(Result(1));
