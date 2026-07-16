// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, str::FromStr};

use iota_json::call_arg;
use iota_json_rpc_api::{
    IndexerApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    IotaExecutionStatus, IotaMoveValue, IotaObjectDataOptions, IotaObjectResponseQuery,
    IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions, ObjectChange, TransactionBlockBytes,
};
use iota_macros::sim_test;
use iota_move_build::BuildConfig;
use iota_sdk_types::{ObjectId, Owner, TransactionKind};
use iota_simulator::fastcrypto::encoding::Base64;
use iota_types::{
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
};
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn test_dev_inspect_transaction_block() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let other_address = cluster.get_address_1();

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

    let obj = objects
        .clone()
        .first()
        .unwrap()
        .object()
        .unwrap()
        .object_ref();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_object(other_address, obj).unwrap();
        builder.finish()
    };
    let kind = TransactionKind::new_programmable(pt);

    let devinspect_response = http_client
        .dev_inspect_transaction_block(
            address,
            Base64::from_bytes(&bcs::to_bytes(&kind).unwrap()),
            None,
            None,
            None,
        )
        .await?;

    assert_eq!(
        *devinspect_response.effects.status(),
        IotaExecutionStatus::Success
    );
    let tx_effect_obj_reassigned = &devinspect_response
        .effects
        .mutated()
        .iter()
        .find(|o| o.reference.object_id == obj.object_id)
        .unwrap();
    assert_eq!(
        tx_effect_obj_reassigned.owner,
        Owner::Address(other_address)
    );

    let actual_object_info = http_client
        .get_object(
            obj.object_id,
            Some(IotaObjectDataOptions {
                show_owner: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    assert_eq!(
        actual_object_info.data.unwrap().owner.unwrap(),
        Owner::Address(address)
    );

    Ok(())
}

/// Uses the test smart contract under `tests/data/view_functions`.
#[sim_test]
async fn test_view_function_call() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    // Publish the test package containing a #[view] function.
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "data", "view_functions"]);
    let compiled_package = BuildConfig::new_for_testing()
        .with_allow_view_function()
        .build(&path)?;
    let compiled_modules_bytes =
        compiled_package.get_package_base64(/* with_unpublished_deps */ false);
    let dependencies = compiled_package.get_dependency_storage_package_ids();

    let transaction_bytes: TransactionBlockBytes = http_client
        .publish(
            address,
            compiled_modules_bytes,
            dependencies,
            None,
            100_000_000.into(),
        )
        .await?;
    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();
    let tx_response: IotaTransactionBlockResponse = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_object_changes(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution.into()),
        )
        .await?;
    assert_eq!(tx_response.status_ok(), Some(true));

    let object_changes = tx_response.object_changes.unwrap();
    let package_id = object_changes
        .iter()
        .find_map(|change| match change {
            ObjectChange::Published { package_id, .. } => Some(*package_id),
            _ => None,
        })
        .unwrap();
    let counter_id = object_changes
        .iter()
        .find_map(|change| match change {
            ObjectChange::Created {
                object_id,
                owner: Owner::Shared(_),
                ..
            } => Some(*object_id),
            _ => None,
        })
        .unwrap();

    // A #[view] function can be called.
    let results = http_client
        .view_function_call(
            format!("{package_id}::counter::value"),
            None,
            vec![call_arg!(counter_id)?],
        )
        .await?;
    assert!(results.error().is_none(), "{results:?}");
    assert_eq!(
        results.into_return_values(),
        vec![IotaMoveValue::String("42".into())]
    );

    // A public function without the #[view] attribute is rejected.
    let err = http_client
        .view_function_call(
            format!("{package_id}::counter::value_not_view"),
            None,
            vec![call_arg!(counter_id)?],
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("is not declared as a #[view] function"),
        "{err}"
    );

    // A module without function attributes has no view functions metadata, so
    // calls to any of its functions are rejected.
    let err = http_client
        .view_function_call(format!("{package_id}::plain::forty"), None, vec![])
        .await
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("is not declared as a #[view] function"),
        "{err}"
    );

    // System packages have no view functions metadata, so calls to any of their
    // functions are rejected, public or not.
    let err = http_client
        .view_function_call(
            "0x2::clock::timestamp_ms".to_string(),
            None,
            vec![call_arg!(ObjectId::CLOCK)?],
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("is not declared as a #[view] function"),
        "{err}"
    );
    let err = http_client
        .view_function_call(
            "0x2::random::load_inner".to_string(),
            None,
            vec![call_arg!(ObjectId::RANDOMNESS_STATE)?],
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("is not declared as a #[view] function"),
        "{err}"
    );

    // A non-public function in the same metadata-less module is rejected the
    // same way as its public sibling above; visibility plays no role once
    // metadata membership is the only check.
    let err = http_client
        .view_function_call(format!("{package_id}::plain::private_forty"), None, vec![])
        .await
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("is not declared as a #[view] function"),
        "{err}"
    );

    // A nonexistent module is rejected.
    let err = http_client
        .view_function_call(format!("{package_id}::nonexistent::value"), None, vec![])
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("Module not found in package"),
        "{err}"
    );

    // A nonexistent function in an existing module is rejected with a distinct
    // error from a real but non-view function.
    let err = http_client
        .view_function_call(format!("{package_id}::counter::nonexistent"), None, vec![])
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not found in module"), "{err}");

    Ok(())
}
