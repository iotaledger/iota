// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::view_metadata;

use iota::package_metadata::PackageMetadataV1;
use std::ascii;

#[view]
public fun answer(): u64 {
    42
}

public fun assert_view_metadata(metadata: &PackageMetadataV1) {
    let module_name = ascii::string(b"view_metadata");
    let view_function_name = ascii::string(b"answer");
    let module_metadata = metadata.module_metadata(&module_name);
    let view_functions_metadata = module_metadata.borrow_view_functions_metadata_v1();

    assert!(view_functions_metadata.length() == 1, 0);

    let flag = module_metadata.is_view_function_v1(
        &view_function_name,
    );
    assert!(flag, 1);
}

//# run test::view_metadata::assert_view_metadata --sender A --args object(1,4)

//# view-object 1,4

//# view-object 1,0
