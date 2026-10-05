// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// TransactionDenyRulesUpdate transactions apply add/remove deltas that
// accumulate in the TransactionDenyRules object across updates.

// Entries created in the same update with the same type are numbered in object
// id order, which changes with every protocol version. Such entries must be
// treated the same way by every later update, so the snapshot does not depend
// on that order.

//# init --simulator --deny-rule-governance true

//# advance-epoch --create-deny-rules-object

// First delta: the entries that survive until the end.

//# update-deny-rules --added-addresses 0xBB --added-packages 0x2C

// Second delta: one address, one object and one package.

//# update-deny-rules --added-addresses 0xAA --added-objects 0x1A --added-packages 0x2B

// Third delta: remove everything the second delta added and add a new
// address.

//# update-deny-rules --added-addresses 0xCC --removed-addresses 0xAA --removed-objects 0x1A --removed-packages 0x2B

// The inner object: two denied addresses, no denied objects, one denied
// package left.

//# view-object 1,0

// One of the surviving table entries from the first delta.

//# view-object 2,0

// The entry added by the third delta.

//# view-object 4,0
