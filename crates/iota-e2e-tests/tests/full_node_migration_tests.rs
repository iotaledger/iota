use std::{env::current_dir, path::PathBuf, str::FromStr};

use iota_genesis_builder::{
    IF_STARDUST_ADDRESS, SnapshotSource,
    stardust::{
        parse::HornetSnapshotParser,
        test_outputs::{STARDUST_TOTAL_SUPPLY_SHIMMER_MICRO, add_snapshot_test_outputs, to_nanos},
    },
};
use iota_json_rpc_types::{IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions};
use iota_macros::sim_test;
use iota_sdk_features::types::block::address::Address;
use iota_test_transaction_builder::batch_make_transfer_transactions;
use iota_types::{
    quorum_driver_types::ExecuteTransactionRequestType, stardust::coin_type::CoinType,
};
use jsonrpsee::{core::client::ClientT, rpc_params};
use test_cluster::TestClusterBuilder;

const MIGRATION_DATA_PATH: &str = "tests/migration/stardust_object_snapshot.bin";
#[sim_test]
async fn test_full_node_load_migration_data() -> Result<(), anyhow::Error> {
    let snapshot_source = SnapshotSource::Local(PathBuf::from_str(MIGRATION_DATA_PATH).unwrap());
    telemetry_subscribers::init_for_testing();
    let mut test_cluster = TestClusterBuilder::new()
        .with_migration_data(vec![snapshot_source])
        .build()
        .await;

    let context = &mut test_cluster.wallet;
    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;

    let txn_count = 4;
    let mut txns = batch_make_transfer_transactions(context, txn_count).await;
    assert!(
        txns.len() >= txn_count,
        "Expect at least {} txns. Do we generate enough gas objects during genesis?",
        txn_count,
    );

    let txn = txns.swap_remove(0);
    let tx_digest = txn.digest();

    // Test request with ExecuteTransactionRequestType::WaitForLocalExecution
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    let params = rpc_params![
        tx_bytes,
        signatures,
        IotaTransactionBlockResponseOptions::new(),
        ExecuteTransactionRequestType::WaitForLocalExecution
    ];
    let response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_executeTransactionBlock", params)
        .await
        .unwrap();

    let IotaTransactionBlockResponse {
        digest,
        confirmed_local_execution,
        ..
    } = response;
    assert_eq!(&digest, tx_digest);
    assert!(confirmed_local_execution.unwrap());

    Ok(())
}
