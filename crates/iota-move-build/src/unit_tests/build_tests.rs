// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use move_compiler::editions::Edition;

use crate::BuildConfig;

#[test]
fn generate_struct_layouts() {
    // build the IOTA framework and generate struct layouts to make sure nothing
    // crashes
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
        .join("iota-framework")
        .join("packages")
        .join("iota-framework");
    let pkg = BuildConfig::new_for_testing().build(&path).unwrap();
    let registry = pkg.generate_struct_layouts();
    // check for a couple of types that aren't likely to go away
    assert!(registry.contains_key(
        "0000000000000000000000000000000000000000000000000000000000000001::string::String"
    ));
    assert!(registry.contains_key(
        "0000000000000000000000000000000000000000000000000000000000000002::object::UID"
    ));
    assert!(registry.contains_key(
        "0000000000000000000000000000000000000000000000000000000000000002::tx_context::TxContext"
    ));
}

#[test]
fn published_size_matches_move_package_size() {
    use std::collections::BTreeMap;

    use iota_protocol_config::ProtocolConfig;
    use iota_sdk_types::{
        ObjectId, Version,
        move_package::{MovePackage, UpgradeInfo},
    };
    use iota_types::move_package::MovePackageExt;

    // A dependency-free package: its linkage table is empty, so the on-chain
    // `MovePackage` can be built with no transitive dependencies and compared
    // directly against `published_size`.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("unit_tests")
        .join("data")
        .join("no_deps");
    let pkg = BuildConfig::new_for_testing().build(&path).unwrap();
    assert!(pkg.get_published_dependencies_ids().is_empty());

    let protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
    let modules = pkg.get_dependency_sorted_modules(false);
    let empty_deps: Vec<&MovePackage> = Vec::new();
    let on_chain = MovePackage::new_initial(&modules, &protocol_config, empty_deps).unwrap();

    // Version tag + module map + type origin table must match exactly.
    assert_eq!(pkg.published_size(false, 0), on_chain.size() as u64);

    // Each linkage-table entry adds a fixed number of bytes; make sure our
    // per-dependency term matches `MovePackage::size` too.
    let make_pkg = |linkage: BTreeMap<ObjectId, UpgradeInfo>| {
        MovePackage::new(
            ObjectId::new([0; 32]),
            Version::default(),
            BTreeMap::new(),
            u64::MAX,
            vec![],
            linkage,
        )
        .unwrap()
    };
    let per_dep = make_pkg(BTreeMap::from([(
        ObjectId::new([1; 32]),
        UpgradeInfo {
            upgraded_id: ObjectId::new([2; 32]),
            upgraded_version: Version::default(),
        },
    )]))
    .size()
        - make_pkg(BTreeMap::new()).size();
    assert_eq!(
        pkg.published_size(false, 1) - pkg.published_size(false, 0),
        per_dep as u64,
    );
}

#[test]
fn development_mode_not_allowed() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_path_buf()
        .join("src")
        .join("unit_tests")
        .join("data")
        .join("no_development_mode");
    let err = BuildConfig::new_for_testing()
        .build(&path)
        .expect_err("Should have failed due to unsupported edition");
    assert!(
        err.to_string()
            .contains(&Edition::DEVELOPMENT.unknown_edition_error().to_string())
    );
}
