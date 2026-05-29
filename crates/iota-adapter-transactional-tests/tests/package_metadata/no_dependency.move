// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::view_metadata;

use std::ascii;

#[view]
public fun answer(): u64 {
    42
}

//# view-object 1,0

//# view-object 1,1
