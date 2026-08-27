// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Each update pins all six global switches to the values carried by the
// transaction: switches set by one update are cleared by the next one that
// does not carry them.

//# init --simulator --deny-rule-governance true

//# advance-epoch --create-deny-rules-object

//# update-deny-rules --package-publish-disabled --shared-object-disabled --move-authenticator-disabled

//# view-object 1,0

// The next update carries different switches; the previous ones are cleared.

//# update-deny-rules --user-transaction-disabled

//# view-object 1,0
