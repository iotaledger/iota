// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// TransactionDenyRulesUpdate transactions apply add/remove deltas that
// accumulate in the TransactionDenyRules object across updates.

//# init --simulator --deny-rule-governance true

//# advance-epoch --create-deny-rules-object

// First delta: two addresses, one object, two packages.

//# update-deny-rules --added-addresses 0xAA,0xBB --added-objects 0x1A --added-packages 0x2B,0x2C

// Second delta: remove one address, the object and one package, add a new
// address.

//# update-deny-rules --added-addresses 0xCC --removed-addresses 0xAA --removed-objects 0x1A --removed-packages 0x2B

// The inner object: two denied addresses, no denied objects, one denied
// package left.

//# view-object 1,0

// One of the surviving table entries from the first delta.

//# view-object 2,0

// The entry added by the second delta.

//# view-object 3,0
