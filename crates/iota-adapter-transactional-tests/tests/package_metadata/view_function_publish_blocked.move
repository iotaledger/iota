// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// While `package_metadata_with_dynamic_module_metadata` is disabled
// (`--module-metadata-dynamic false`), the `View` runtime attribute is not yet
// supported by the protocol. Publishing a package whose bytecode carries it
// must be rejected at verification so that a not-yet-upgraded validator (whose
// binary cannot even deserialize the variant) agrees with an upgraded one.

//# init --addresses test=0x0 --accounts A --module-metadata-dynamic false

//# publish --sender A
module test::view_blocked;

#[view]
public fun answer(): u64 {
    42
}
