// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_framework::BuiltInFramework;
use iota_grpc_client::{ReadMask, read_mask_fields::ObjectField};
use iota_macros::sim_test;
use iota_sdk_types::ObjectId;
use iota_types::{IOTA_SYSTEM_ADDRESS, digests::TransactionDigest, object::Object};
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn test_additional_objects() {
    // Test the ability to add additional objects into genesis for test clusters
    let id = ObjectId::random();
    let cluster = TestClusterBuilder::new()
        .with_objects([Object::immutable_with_id_for_testing(id)])
        .build()
        .await;

    let objects = cluster
        .grpc_client()
        .get_objects(&[(id, None)], Some(ReadMask::from(ObjectField::REFERENCE)))
        .await
        .unwrap()
        .into_inner();
    let object_ref = objects
        .first()
        .expect("added object should exist")
        .object_reference()
        .expect("added object should exist");
    assert_eq!(object_ref.object_id, id);
}

#[sim_test]
async fn test_package_override() {
    // `with_objects` can be used to override existing packages.
    let framework_ref = {
        let default_cluster = TestClusterBuilder::new().build().await;
        default_cluster
            .grpc_client()
            .get_objects(
                &[(ObjectId::SYSTEM, None)],
                Some(ReadMask::from(ObjectField::REFERENCE)),
            )
            .await
            .unwrap()
            .into_inner()
            .first()
            .expect("original framework package should exist")
            .object_reference()
            .expect("original framework package should exist")
    };

    let modified_ref = {
        let mut framework_modules = BuiltInFramework::get_package_by_id(&ObjectId::SYSTEM)
            .modules()
            .to_vec();

        // Create an empty module that is pretending to be part of the iota framework.
        let mut test_module = move_binary_format::file_format::empty_module();
        let address_idx = test_module.self_handle().address.0 as usize;
        test_module.address_identifiers[address_idx] = IOTA_SYSTEM_ADDRESS;

        // Add the dummy module to the rest of the iota-frameworks.  We can't replace
        // the framework entirely because we will call into it for genesis.
        framework_modules.push(test_module);

        let package_override = Object::new_package_for_testing(
            &framework_modules,
            TransactionDigest::GENESIS_MARKER,
            [
                BuiltInFramework::get_package_by_id(&ObjectId::STD).genesis_move_package(),
                BuiltInFramework::get_package_by_id(&ObjectId::FRAMEWORK).genesis_move_package(),
            ],
        )
        .unwrap();

        let modified_cluster = TestClusterBuilder::new()
            .with_objects([package_override])
            .build()
            .await;

        modified_cluster
            .grpc_client()
            .get_objects(
                &[(ObjectId::SYSTEM, None)],
                Some(ReadMask::from(ObjectField::REFERENCE)),
            )
            .await
            .unwrap()
            .into_inner()
            .first()
            .expect("modified framework package should exist")
            .object_reference()
            .expect("modified framework package should exist")
    };

    assert_ne!(framework_ref, modified_ref);
}
