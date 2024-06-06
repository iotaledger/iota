// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// tests TransferObject should fail for a package

//# init --accounts A B --addresses test=0x0

//# publish --sender A

module test::m {}


//# view-object 1,0

//# transfer-object 1,0 --sender A --recipient B

//# view-object 1,0
