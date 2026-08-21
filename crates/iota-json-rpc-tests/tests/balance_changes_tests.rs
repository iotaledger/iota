// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use iota_move_build::{BuildConfig, IotaPackageHooks};
use iota_sdk::IotaClient;
use iota_sdk_types::{Transaction, TransactionKind};
use iota_types::{
    programmable_transaction_builder::ProgrammableTransactionBuilder, transaction::TransactionAPI,
};
use test_cluster::TestClusterBuilder;

#[tokio::test]
async fn test_dry_run_publish_with_mocked_coin() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let context = &cluster.wallet;

    let address = cluster.get_address_0();
    let client: IotaClient = context.get_client().await.unwrap();

    // Publish test coin package
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "data", "dummy_modules_publish"]);
    let compiled_package = BuildConfig::new_for_testing().build(&path)?;
    let compiled_modules_bytes = compiled_package
        .get_package_base64(false)
        .into_iter()
        .map(|b| b.to_vec().unwrap())
        .collect::<Vec<_>>();
    let dependencies = compiled_package.get_dependency_storage_package_ids();

    let mut builder = ProgrammableTransactionBuilder::new();
    builder.publish_immutable(compiled_modules_bytes, dependencies);

    let publish = TransactionKind::new_programmable(builder.finish());
    let transaction_bytes =
        Transaction::new_with_gas_coins(publish, address, vec![], 100000000, 1000);

    let result = client
        .read_api()
        .dry_run_transaction_block(transaction_bytes)
        .await;

    // Dry run balance change should not fail because of mocked coin
    assert!(result.is_ok());

    Ok(())
}

/// A dry run resolves event types against the packages the transaction itself
/// published, not just those already in the store. Exercised through the
/// JSON-RPC layer so it covers the resolver the server actually builds.
#[tokio::test]
async fn test_dry_run_resolves_events_of_newly_published_package() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let context = &cluster.wallet;

    let address = cluster.get_address_0();
    let client: IotaClient = context.get_client().await.unwrap();

    // This package emits a `PublishEvent { foo: "bar" }` from its `init`, so the
    // only way to decode the event is to resolve the type out of the package the
    // dry run just published.
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "data", "publish_with_event"]);
    let compiled_package = BuildConfig::new_for_testing().build(&path)?;
    let compiled_modules_bytes = compiled_package
        .get_package_base64(false)
        .into_iter()
        .map(|b| b.to_vec().unwrap())
        .collect::<Vec<_>>();
    let dependencies = compiled_package.get_dependency_storage_package_ids();

    let mut builder = ProgrammableTransactionBuilder::new();
    builder.publish_immutable(compiled_modules_bytes, dependencies);

    let publish = TransactionKind::new_programmable(builder.finish());
    let transaction_bytes =
        Transaction::new_with_gas_coins(publish, address, vec![], 100000000, 1000);

    let response = client
        .read_api()
        .dry_run_transaction_block(transaction_bytes)
        .await?;

    assert_eq!(response.events.data.len(), 1);
    let event = &response.events.data[0];
    assert_eq!(event.type_tag.name().to_string(), "PublishEvent");
    // An unresolved type would leave the payload undecoded.
    assert_eq!(event.parsed_json, serde_json::json!({ "foo": "bar" }));

    Ok(())
}
