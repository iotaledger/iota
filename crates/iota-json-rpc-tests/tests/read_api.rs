// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_api::{IndexerApiClient, ReadApiClient};
use iota_json_rpc_types::{IotaObjectDataOptions, IotaObjectResponse, IotaObjectResponseQuery};
use iota_macros::sim_test;
use iota_types::{IOTA_FRAMEWORK_ADDRESS, base_types::ObjectID};
use test_cluster::TestClusterBuilder;

#[tokio::test]
async fn test_get_package_with_display_should_not_fail() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let response = http_client
        .get_object(
            ObjectID::from(IOTA_FRAMEWORK_ADDRESS),
            Some(IotaObjectDataOptions::new().with_display()),
        )
        .await;
    assert!(response.is_ok());
    let response: IotaObjectResponse = response?;
    assert!(
        response
            .into_object()
            .unwrap()
            .display
            .unwrap()
            .data
            .is_none()
    );
    Ok(())
}

#[sim_test]
async fn test_get_object_info() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let objects = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    for obj in objects {
        let oref = obj.into_object().unwrap();
        let result = http_client
            .get_object(
                oref.object_id,
                Some(IotaObjectDataOptions::new().with_owner()),
            )
            .await?;
        assert!(
            matches!(result, IotaObjectResponse { data: Some(object), .. } if oref.object_id == object.object_id && object.owner.unwrap().get_owner_address()? == address)
        );
    }
    Ok(())
}

#[sim_test]
async fn test_get_objects() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;

    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let objects = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new(),
            )),
            None,
            None,
        )
        .await?;
    assert_eq!(5, objects.data.len());

    // Multiget objectIDs test
    let object_digests = objects
        .data
        .iter()
        .map(|o| o.object().unwrap().object_id)
        .collect();

    let object_resp = http_client.multi_get_objects(object_digests, None).await?;
    assert_eq!(5, object_resp.len());
    Ok(())
}

#[sim_test]
async fn test_get_object_data_with_content() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let objects = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new().with_content().with_owner(),
            )),
            None,
            None,
        )
        .await?
        .data;

    for obj in objects {
        let oref = obj.into_object().unwrap();
        let result = http_client
            .get_object(
                oref.object_id,
                Some(IotaObjectDataOptions::new().with_content().with_owner()),
            )
            .await?;
        assert!(
            matches!(result, IotaObjectResponse { data: Some(object), .. } if oref.object_id == object.object_id && object.owner.unwrap().get_owner_address()? == address)
        );
    }
    Ok(())
}
