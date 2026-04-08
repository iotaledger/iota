// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Claiming with a public key that does not derive to the transaction sender
// address must abort with EAddressMismatch (code 0).

//# init --accounts A --addresses test=0x0

//# programmable --sender A --inputs object(0x10) 0u8 x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88"
//> iota::claim_registry::claim(Input(0), Input(1), Input(2));
