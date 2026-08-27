// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Re-applying a delta is a no-op: adding an already denied entry or removing
// an absent one is tolerated, and the update transaction still succeeds and
// emits its event.

//# init --simulator --deny-rule-governance true

//# advance-epoch --create-deny-rules-object

//# update-deny-rules --added-addresses 0xAA --added-packages 0x2B

// The exact same delta again: both adds are already present.

//# update-deny-rules --added-addresses 0xAA --added-packages 0x2B

// Removing entries that were never added is tolerated too.

//# update-deny-rules --removed-addresses 0xBB --removed-objects 0x1A

// One denied address and one denied package, from the first update.

//# view-object 1,0
