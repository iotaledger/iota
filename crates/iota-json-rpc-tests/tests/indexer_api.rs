// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(msim))]
use std::str::FromStr;

use iota_json_rpc_api::IndexerApiClient;
use iota_json_rpc_types::{
    IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponseQuery, ObjectsPage,
};
use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_swarm_config::genesis_config::AccountConfig;
use iota_types::{
    base_types::{IotaAddress, MoveObjectType, ObjectID},
    collection_types::VecMap,
    crypto::deterministic_random_account_key,
    digests::TransactionDigest,
    id::UID,
    object::{Data, MoveObject, OBJECT_START_VERSION, ObjectInner, Owner},
    stardust::output::{Irc27Metadata, Nft},
};
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn test_nft_display_object() -> Result<(), anyhow::Error> {
    // Create a cluster
    let (address, _) = deterministic_random_account_key();

    let nft = Nft {
        id: UID::new(ObjectID::random()),
        legacy_sender: Some(IotaAddress::ZERO),
        metadata: Some(String::from("metadata value").into_bytes()),
        tag: Some(String::from("tag value").into_bytes()),
        immutable_issuer: Some(
            IotaAddress::from_str(
                "0x1000000000000000002000000000000003000000000000040000000000000005",
            )
            .unwrap(),
        ),
        immutable_metadata: Irc27Metadata {
            version: String::from("version value"),
            media_type: String::from("media type value"),
            uri: String::from("url value").try_into().unwrap(),
            name: String::from("name value"),
            collection_name: Some(String::from("collection name value")),
            royalties: VecMap { contents: vec![] },
            issuer_name: Some(String::from("issuer name value")),
            description: Some(String::from("description value")),
            attributes: VecMap { contents: vec![] },
            non_standard_fields: VecMap { contents: vec![] },
        },
    };

    let nft_move_object = unsafe {
        MoveObject::new_from_execution(
            MoveObjectType::stardust_nft(),
            true,
            OBJECT_START_VERSION,
            bcs::to_bytes(&nft).unwrap(),
            &ProtocolConfig::get_for_min_version(),
        )
        .unwrap()
    };
    let nft_object = ObjectInner {
        owner: Owner::AddressOwner(address),
        data: Data::Move(nft_move_object),
        previous_transaction: TransactionDigest::genesis_marker(),
        storage_rebate: 0,
    };

    let cluster = TestClusterBuilder::new()
        .with_accounts(
            [AccountConfig {
                address: Some(address),
                gas_amounts: [100_000_000].into(),
            }]
            .into(),
        )
        .with_objects([nft_object.into()])
        .build()
        .await;

    // Check owned objects
    let http_client = cluster.rpc_client();

    let objects: ObjectsPage = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new(
                Some(IotaObjectDataFilter::StructType(Nft::tag())),
                Some(
                    IotaObjectDataOptions::new()
                        .with_type()
                        .with_owner()
                        .with_previous_transaction()
                        .with_display(),
                ),
            )),
            None,
            None,
        )
        .await?;
    assert_eq!(1, objects.data.len());

    // Check the Nft display
    let nft_display = objects.data[0]
        .data
        .as_ref()
        .unwrap()
        .display
        .as_ref()
        .unwrap()
        .data
        .as_ref()
        .unwrap();

    assert_eq!(8, nft_display.len());

    assert_eq!(nft_display["collection_name"], "collection name value");
    assert_eq!(nft_display["creator"], "issuer name value");
    assert_eq!(nft_display["description"], "description value");
    assert_eq!(nft_display["image_url"], "url value");
    assert_eq!(
        nft_display["immutable_issuer"],
        "0x1000000000000000002000000000000003000000000000040000000000000005"
    );
    assert_eq!(nft_display["media_type"], "media type value");
    assert_eq!(nft_display["name"], "name value");
    assert_eq!(nft_display["version"], "version value");

    Ok(())
}
