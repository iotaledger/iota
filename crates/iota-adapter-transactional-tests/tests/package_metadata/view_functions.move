// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::view_metadata;

use iota::package_metadata::PackageMetadataV2;
use std::ascii;

#[view]
public fun answer(): u64 {
    42
}

public fun assert_view_metadata(metadata: &PackageMetadataV2) {
    let module_name = ascii::string(b"view_metadata");
    let view_function_name = ascii::string(b"answer");
    let module_metadata = metadata.modules_metadata_v2(&module_name);
    let view_functions_metadata = module_metadata.view_functions_metadata();

    assert!(view_functions_metadata.length() == 1, 0);

    let view_function_metadata = module_metadata.view_function_metadata(&view_function_name);
    assert!(*view_function_metadata.view_function_name() == view_function_name, 1);
}

//# run test::view_metadata::assert_view_metadata --sender A --args object(1,1)

//# view-object 1,0

//# view-object 1,1
