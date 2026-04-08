// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Verify that the ClaimRegistry singleton is created during genesis at @0x10
// and is visible as a shared object.

//# init --accounts A --addresses test=0x0

//# view-object 0x10
