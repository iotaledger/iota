// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, str::FromStr};

use iota_json::call_arg;
use iota_json_rpc_api::{
    IndexerApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    DevInspectArgs, IotaExecutionStatus, IotaMoveValue, IotaObjectDataOptions,
    IotaObjectResponseQuery, IotaTransactionBlockDataAPI, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ObjectChange,
    TransactionBlockBytes,
};
use iota_macros::sim_test;
use iota_move_build::BuildConfig;
use iota_sdk_types::{
    GasPayment, ObjectId, Owner, TransactionExpiration, TransactionKind, TransactionV1,
};
use iota_simulator::fastcrypto::encoding::Base64;
use iota_types::{
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{TransactionData, TransactionDataAPI},
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

/// A dev inspect fills a zero gas price and a zero gas budget in from the
/// epoch, rather than metering against those zeroes and failing with
/// `GasPriceUnderRGP` or `InsufficientGas`. This holds whether or not the
/// caller asks for the checks to be skipped: `DevInspectArgs` models both
/// fields as optional, so leaving them out has to work in either mode.
#[sim_test]
async fn test_dev_inspect_transaction_block_zero_gas_price_and_budget() -> Result<(), anyhow::Error>
{
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let other_address = cluster.get_address_1();

    let objects = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new().with_owner(),
            )),
            None,
            None,
        )
        .await?
        .data;
    let obj = objects.first().unwrap().object().unwrap().object_ref();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_object(other_address, obj).unwrap();
        builder.finish()
    };
    let tx_bytes = Base64::from_bytes(&bcs::to_bytes(&TransactionKind::new_programmable(pt))?);
    let reference_gas_price = cluster.get_reference_gas_price().await;

    // A zero budget on its own, with a gas price the epoch would accept as-is.
    let devinspect_response = http_client
        .dev_inspect_transaction_block(
            address,
            tx_bytes.clone(),
            Some(reference_gas_price.into()),
            None,
            Some(DevInspectArgs {
                gas_budget: Some(0),
                ..Default::default()
            }),
        )
        .await?;
    assert_eq!(
        *devinspect_response.effects.status(),
        IotaExecutionStatus::Success
    );

    // A zero price and a zero budget together.
    let devinspect_response = http_client
        .dev_inspect_transaction_block(
            address,
            tx_bytes.clone(),
            Some(0.into()),
            None,
            Some(DevInspectArgs {
                gas_budget: Some(0),
                ..Default::default()
            }),
        )
        .await?;
    assert_eq!(
        *devinspect_response.effects.status(),
        IotaExecutionStatus::Success
    );

    // Both gas fields omitted entirely, with and without the checks. GraphQL's
    // `dryRunTransactionBlock` issues exactly the `skip_checks: false` shape when
    // it is given a `txMeta` carrying only a sender.
    for skip_checks in [true, false] {
        let devinspect_response = http_client
            .dev_inspect_transaction_block(
                address,
                tx_bytes.clone(),
                None,
                None,
                Some(DevInspectArgs {
                    skip_checks: Some(skip_checks),
                    ..Default::default()
                }),
            )
            .await
            .unwrap_or_else(|e| panic!("dev inspect with skip_checks={skip_checks} failed: {e}"));
        assert_eq!(
            *devinspect_response.effects.status(),
            IotaExecutionStatus::Success
        );
    }

    Ok(())
}

/// A caller that declares no gas budget is asking what the transaction costs,
/// so the reported transaction carries the cost the simulation charged rather
/// than the zero that was sent. Matches gRPC `simulate_transactions`.
#[sim_test]
async fn test_zero_gas_budget_is_reported_as_the_gas_used() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let other_address = cluster.get_address_1();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(other_address, Some(1_000));
        builder.finish()
    };

    // No gas payment, no price and no budget: the simulation fills all of it in.
    let transaction = TransactionData::V1(TransactionV1 {
        kind: TransactionKind::new_programmable(pt.clone()),
        sender: address,
        gas_payment: GasPayment {
            objects: vec![],
            owner: address,
            price: 0,
            budget: 0,
        },
        expiration: TransactionExpiration::None,
    });

    let response = http_client
        .dry_run_transaction_block(Base64::from_bytes(&bcs::to_bytes(&transaction)?))
        .await?;
    assert_eq!(*response.effects.status(), IotaExecutionStatus::Success);

    let gas_used = response.effects.gas_cost_summary().gas_used();
    let reference_gas_price = cluster.get_reference_gas_price().await;
    assert_ne!(gas_used, 0, "a successful transfer has to cost something");
    assert_eq!(
        response.input.gas_data().budget,
        gas_used,
        "the reported budget should be the cost the simulation charged"
    );
    // Only the computation half of the cost scales with the price, so the estimate
    // above cannot be read without the price it was charged at.
    assert_eq!(
        response.input.gas_data().price,
        reference_gas_price,
        "the reported price should be the one the simulation charged at"
    );
    // A mock gas coin was minted, so the reported payment names it rather than
    // staying empty.
    assert_eq!(response.input.gas_data().payment.len(), 1);

    // The same for the raw transaction a dev inspect hands back.
    let raw_txn_data = http_client
        .dev_inspect_transaction_block(
            address,
            Base64::from_bytes(&bcs::to_bytes(&TransactionKind::new_programmable(pt))?),
            None,
            None,
            Some(DevInspectArgs {
                show_raw_txn_data_and_effects: Some(true),
                ..Default::default()
            }),
        )
        .await?
        .raw_txn_data;
    let reported: TransactionData = bcs::from_bytes(&raw_txn_data)?;
    assert_ne!(reported.gas_budget(), 0);
    assert_eq!(reported.gas_price(), reference_gas_price);

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
