// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// With `--module-metadata-dynamic false` the legacy (inline) PackageMetadataV1
// layout is produced: a single frozen PackageMetadataV1 object whose
// `modules_metadata` VecMap is populated inline (no dynamic fields), and view
// function metadata is not recorded.

//# init --addresses test=0x0 --accounts A --module-metadata-dynamic false

//# publish --sender A
module test::no_module_dynamic_metadata;

use iota::package_metadata::PackageMetadataV1;
use std::ascii;
use std::type_name;

public struct Account has key {
    id: UID,
}

#[authenticator]
public fun authenticate(_account: &Account, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

// In the inline V1 layout this `#[view]` function is ignored (view metadata is
// only captured by the dynamic layout).
#[view]
public fun answer(): u64 {
    42
}

#[allow(deprecated_usage)]
public fun assert_inline_metadata(metadata: &PackageMetadataV1) {
    let module_name = ascii::string(b"no_module_dynamic_metadata");
    let auth_function_name = ascii::string(b"authenticate");
    let module_metadata = metadata.modules_metadata_v1(&module_name);
    let authenticator_metadata = module_metadata.authenticator_metadata_v1(&auth_function_name);
    assert!(authenticator_metadata.account_type() == type_name::get<Account>(), 0);
    assert!(*authenticator_metadata.function_name() == auth_function_name, 1);
}

//# run test::no_module_dynamic_metadata::assert_inline_metadata --sender A --args object(1,0)

//# view-object 1,1

//# view-object 1,0
