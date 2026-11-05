// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// An upgradeable publish creates two objects, the package and its upgrade
// capability, which is enough to need the naming that a dry run cannot take
// from storage. The real publish that follows names its two the same way.

//# init --addresses Test=0x0 --accounts A

//# publish --upgradeable --sender A --dry-run

module Test::M1 {
    public fun f1() { }
}

//# publish --upgradeable --sender A

module Test::M2 {
    public fun f2() { }
}

//# view-object 2,1
