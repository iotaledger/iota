// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Happy-path: Ed25519 claim creates IotaDefaultAccount as a shared object.
// Account A's deterministic Ed25519 public key in the transactional test runner:
//   7f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928
// Derived address (Blake2b256(pk)):
//   0x8cca4e1ce0ba5904cea61df9242da2f7d29e3ef328fb7ec07c086b3bf47ca61a

//# init --accounts A --addresses test=0x0

//# run iota::claim_registry::claim_ed25519 --sender A --args object(0x11) x"7f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"

//# view-object 1,0

//# view-object 1,2

//# view-object @A