// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::view_metadata;

use iota::package_metadata::{Self, PackageMetadataV1};
use std::ascii;

#[view]
public fun answer(): u64 {
    42
}

public fun assert_view_metadata(metadata: &PackageMetadataV1) {
    let view_metadata = package_metadata::borrow_package_view_functions_metadata(metadata);
    let module_name = ascii::string(b"view_metadata");
    let view_functions = package_metadata::module_view_functions(view_metadata, &module_name);

    assert!(view_functions.length() == 1, 0);
    assert!(view_functions[0] == ascii::string(b"answer"), 1);

    let package_view_functions = package_metadata::view_functions(view_metadata);
    assert!(package_view_functions.size() == 1, 2);
    assert!(*package_view_functions.get(&module_name) == vector[ascii::string(b"answer")], 3);
}

//# run test::view_metadata::assert_view_metadata --sender A --args object(1,1)

//# view-object 1,0

//# view-object 1,1
