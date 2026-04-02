// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Claiming with a public key that does not derive to the transaction sender
// address must abort with EAddressMismatch (code 0).
// The Ed25519 public key below derives to a specific address that is
// NOT account A's address in the test environment.

//# init --accounts A --addresses test=0x0

//# run iota::claim_registry::claim_ed25519 --sender A --args object(0x11) x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88"
