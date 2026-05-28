// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Both claims are in a single PTB: the first succeeds (UID in Result(1)),
// the second aborts with EAlreadyClaimed. The whole transaction rolls back,
// so the unconsumed UID from Result(1) is never committed.

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs object(0x10) x"007f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: iota::claim_registry::claim(Input(0), Result(0));
//> 2: iota::claim_registry::claim(Input(0), Result(0));
