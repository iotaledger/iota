// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example uses the Read API to get owned objects of an address, the
//! dynamic fields of an object, past objects, information about the chain and
//! the protocol configuration, the transaction data after executing a
//! transaction, and finally, the number of transaction blocks known to the
//! server.
//!
//! cargo run --example read_api

#[path = "../utils.rs"]
mod utils;

use iota_sdk::{
    rpc_types::{IotaGetPastObjectRequest, IotaObjectDataOptions},
    types::base_types::ObjectID,
};
use utils::setup_for_write;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, active_address, _) = setup_for_write().await?;

    // Owned Objects
    let owned_objects = client
        .read_api()
        .get_owned_objects(active_address, None, None, 5)
        .await?;
    println!(" *** Owned Objects ***");
    println!("{owned_objects:?}");
    println!(" *** Owned Objects ***\n");

    // Dynamic Fields
    let parent_object_id = ObjectID::from_address(active_address.into());
    let dynamic_fields = client
        .read_api()
        .get_dynamic_fields(parent_object_id, None, None)
        .await?;
    println!(" *** Dynamic Fields ***");
    println!("{dynamic_fields:?}");
    println!(" *** Dynamic Fields ***\n");
    if let Some(dynamic_field_info) = dynamic_fields.data.into_iter().next() {
        println!(" *** First Dynamic Field ***");
        let dynamic_field = client
            .read_api()
            .get_dynamic_field_object(parent_object_id, dynamic_field_info.name.clone())
            .await?;
        println!("{dynamic_field:?}");
        println!(" *** First Dynamic Field ***\n");

        println!(" *** First Dynamic Field BCS***");
        let dynamic_field = client
            .read_api()
            .get_dynamic_field_object_v2(
                parent_object_id,
                dynamic_field_info.name,
                IotaObjectDataOptions::new().with_bcs(),
            )
            .await?;
        println!("{:?}", dynamic_field.data.expect("missing data").bcs);
        println!(" *** First Dynamic Field BCS***\n");
    }

    let object = owned_objects
        .data
        .first()
        .unwrap_or_else(|| panic!("No object data for this address {active_address}"));
    let object_data = object
        .data
        .as_ref()
        .unwrap_or_else(|| panic!("No object data for this IotaObjectResponse {object:?}"));
    let object_id = object_data.object_id;
    let version = object_data.version;

    let iota_data_options = IotaObjectDataOptions {
        show_type: true,
        show_owner: true,
        show_previous_transaction: true,
        show_display: true,
        show_content: true,
        show_bcs: true,
        show_storage_rebate: true,
    };

    let past_object = client
        .read_api()
        .try_get_parsed_past_object(object_id, version, iota_data_options.clone())
        .await?;
    println!(" *** Past Object *** ");
    println!("{past_object:?}");
    println!(" *** Past Object ***\n");

    let past_object = client
        .read_api()
        .try_get_object_before_version(object_id, version)
        .await?;
    println!(" *** Past Object *** ");
    println!("{past_object:?}");
    println!(" *** Past Object ***\n");

    let iota_get_past_object_request = past_object.clone().into_object()?;
    let multi_past_object = client
        .read_api()
        .try_multi_get_parsed_past_object(
            vec![IotaGetPastObjectRequest {
                object_id: iota_get_past_object_request.object_id,
                version: iota_get_past_object_request.version,
            }],
            iota_data_options.clone(),
        )
        .await?;
    println!(" *** Multi Past Object *** ");
    println!("{multi_past_object:?}");
    println!(" *** Multi Past Object ***\n");

    // Object with options
    let object_with_options = client
        .read_api()
        .get_object_with_options(iota_get_past_object_request.object_id, iota_data_options)
        .await?;

    println!(" *** Object with Options *** ");
    println!("{object_with_options:?}");
    println!(" *** Object with Options ***\n");

    println!(" *** Chain identifier *** ");
    println!("{:?}", client.read_api().get_chain_identifier().await?);
    println!(" *** Chain identifier ***\n ");

    println!(" *** Protocol Config *** ");
    println!("{:?}", client.read_api().get_protocol_config(None).await?);
    println!(" *** Protocol Config ***\n ");

    let tx_blocks = client.read_api().get_total_transaction_blocks().await?;
    println!("Total transaction blocks {tx_blocks}");

    Ok(())
}
