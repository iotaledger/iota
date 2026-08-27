// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// The TransactionDenyRules object and its accumulated state survive epoch
// changes, and updates keep working in later epochs.

//# init --simulator --deny-rule-governance true

//# advance-epoch --create-deny-rules-object

//# update-deny-rules --added-addresses 0xAA --package-publish-disabled

//# advance-epoch

// The denied address and the switch survived the epoch change.

//# view-object 1,0

//# update-deny-rules --added-addresses 0xBB --removed-addresses 0xAA

// Still one denied address (0xAA replaced by 0xBB); the switch was cleared by
// the second update.

//# view-object 1,0
