// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --accounts A --addresses Test=0x0 --package-metadata-v2 true

//# publish
module Test::M {
    use iota::package_metadata::PackageMetadataV2;
    use std::ascii;

    const ENotViewFunction: u64 = 0;

    #[view]
    public fun get_value(): u64 {
        42
    }

    public fun assert_view_metadata(metadata: &PackageMetadataV2) {
        let module_name = ascii::string(b"M");
        let function_name = ascii::string(b"get_value");
        let module_metadata = metadata.modules_metadata_v2(&module_name);
        assert!(module_metadata.is_view_function_v1(&function_name), ENotViewFunction);
        let view_metadata = module_metadata.view_function_metadata_v1(&function_name);
        assert!(view_metadata.view_function_name_v1() == function_name, ENotViewFunction);
    }
}

//# run Test::M::assert_view_metadata --sender A --args object(1,1)
