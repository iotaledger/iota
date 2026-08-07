// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(msim))]
use std::fs::{read_to_string, write};
use std::{fs::read_dir, net::SocketAddr, path::Path, time::Duration};

use iota_config::{
    IOTA_CLIENT_CONFIG, IOTA_FULLNODE_CONFIG, IOTA_GENESIS_FILENAME, IOTA_KEYSTORE_FILENAME,
    IOTA_NETWORK_CONFIG, PersistedConfig,
};
use iota_keys::keystore::AccountKeystore;
#[cfg(feature = "indexer")]
use iota_localnet::commands::IndexerFeatureArgs;
use iota_localnet::commands::{LocalnetCommand, parse_host_port};
use iota_macros::sim_test;
use iota_sdk::iota_client_config::IotaClientConfig;
use iota_swarm_config::{
    genesis_config::DEFAULT_NUMBER_OF_AUTHORITIES, network_config::NetworkConfigLight,
    node_config_override::NodeConfigOverride,
};

/// A `genesis` command that leaves every field but the ones passed in at the
/// value the CLI defaults to.
fn genesis_command(working_dir: &Path, committee_size: usize) -> LocalnetCommand {
    LocalnetCommand::Genesis {
        working_dir: Some(working_dir.to_path_buf()),
        write_config: None,
        force: false,
        from_config: None,
        epoch_duration_ms: None,
        benchmark_ips: None,
        with_faucet: false,
        committee_size,
        num_additional_gas_accounts: None,
        chain_start_timestamp_ms: None,
        admin_interface_address: None,
    }
}

/// A `start` command that leaves every field but the ones passed in at the
/// value the CLI defaults to.
fn start_command(
    config_dir: &Path,
    disable_fullnode_pruning: bool,
    node_config_override: Vec<NodeConfigOverride>,
) -> LocalnetCommand {
    LocalnetCommand::Start {
        #[cfg(feature = "indexer")]
        data_ingestion_dir: None,
        config_dir: Some(config_dir.to_path_buf()),
        no_full_node: false,
        disable_fullnode_pruning,
        node_config_override,
        force_regenesis: false,
        with_faucet: None,
        faucet_amount: None,
        faucet_coin_count: None,
        with_grpc: None,
        fullnode_rpc_port: 9000,
        committee_size: None,
        epoch_duration_ms: None,
        #[cfg(feature = "indexer")]
        indexer_feature_args: Box::new(IndexerFeatureArgs::for_testing()),
    }
}

#[sim_test]
async fn test_genesis() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    // Genesis
    genesis_command(working_dir, DEFAULT_NUMBER_OF_AUTHORITIES)
        .execute()
        .await?;

    // Get all the new file names
    let files = read_dir(working_dir)?
        .flat_map(|r| r.map(|file| file.file_name().to_str().unwrap().to_owned()))
        .collect::<Vec<_>>();

    assert_eq!(9, files.len());
    assert!(files.contains(&IOTA_CLIENT_CONFIG.to_string()));
    assert!(files.contains(&IOTA_NETWORK_CONFIG.to_string()));
    assert!(files.contains(&IOTA_FULLNODE_CONFIG.to_string()));
    assert!(files.contains(&IOTA_GENESIS_FILENAME.to_string()));
    assert!(files.contains(&IOTA_KEYSTORE_FILENAME.to_string()));

    // Check network config
    let network_conf =
        PersistedConfig::<NetworkConfigLight>::read(&working_dir.join(IOTA_NETWORK_CONFIG))?;
    assert_eq!(4, network_conf.validator_configs().len());

    // Check wallet config
    let wallet_conf =
        PersistedConfig::<IotaClientConfig>::read(&working_dir.join(IOTA_CLIENT_CONFIG))?;

    assert!(!wallet_conf.envs().is_empty());

    assert_eq!(5, wallet_conf.keystore().addresses().len());

    // Genesis 2nd time should fail
    let result = genesis_command(working_dir, DEFAULT_NUMBER_OF_AUTHORITIES)
        .execute()
        .await;
    assert!(matches!(result, Err(..)));

    tmp_dir.close()?;
    Ok(())
}

#[sim_test]
async fn test_genesis_rejects_a_zero_committee_size() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();

    let err = genesis_command(tmp_dir.path(), 0)
        .execute()
        .await
        .unwrap_err();

    let err = format!("{err:#}");
    assert!(err.contains("Committee size must be at least 1."), "{err}");

    tmp_dir.close()?;
    Ok(())
}

#[sim_test]
async fn test_start_reports_a_failing_node_config_override() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();

    let err = start_command(
        tmp_dir.path(),
        false,
        // One character off, so the override fails when it is applied to
        // the built config.
        vec!["fullnode:enable-index-processng=false".parse()?],
    )
    .execute()
    .await
    .unwrap_err();

    let err = format!("{err:#}");
    assert!(err.contains("enable-index-processng"), "{err}");

    tmp_dir.close()?;
    Ok(())
}

#[sim_test]
async fn test_start() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    if let Ok(res) = tokio::time::timeout(
        Duration::from_secs(10),
        start_command(
            working_dir,
            true,
            vec![
                // Exercises the override path through the CLI; the values
                // match what --disable-fullnode-pruning and the defaults
                // already set, so the started network is unaffected.
                "fullnode:authority-store-pruning-config.num-epochs-to-retain=18446744073709551615"
                    .parse()?,
                "validator:enable-soft-locking=true".parse()?,
            ],
        )
        .execute(),
    )
    .await
    {
        res.unwrap();
    };

    // Get all the new file names
    let files = read_dir(working_dir)?
        .flat_map(|r| r.map(|file| file.file_name().to_str().unwrap().to_owned()))
        .collect::<Vec<_>>();
    assert!(files.contains(&IOTA_CLIENT_CONFIG.to_string()));
    assert!(files.contains(&IOTA_NETWORK_CONFIG.to_string()));
    assert!(files.contains(&IOTA_FULLNODE_CONFIG.to_string()));
    assert!(files.contains(&IOTA_GENESIS_FILENAME.to_string()));
    assert!(files.contains(&IOTA_KEYSTORE_FILENAME.to_string()));

    // Check network config
    let network_conf =
        PersistedConfig::<NetworkConfigLight>::read(&working_dir.join(IOTA_NETWORK_CONFIG))?;
    assert_eq!(1, network_conf.validator_configs().len());

    // Check wallet config
    let wallet_conf =
        PersistedConfig::<IotaClientConfig>::read(&working_dir.join(IOTA_CLIENT_CONFIG))?;

    assert!(!wallet_conf.envs().is_empty());

    assert_eq!(5, wallet_conf.keystore().addresses().len());

    tmp_dir.close()?;
    Ok(())
}

// A plain tokio test: `start` rejects the config before it launches a node, so
// it needs no simulator and must also run where `#[sim_test]`s are skipped. It
// is excluded under msim because the start path calls
// `tokio::task::spawn_blocking`, which the simulator runs on the current
// simulator node — a plain tokio test has none.
#[cfg(not(msim))]
#[tokio::test]
async fn test_start_rejects_a_persisted_config_no_node_could_start_with()
-> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    genesis_command(working_dir, 1).execute().await?;

    // An enabled gRPC API without a config: `iota-node` refuses to start with
    // it, so the localnet must refuse it too instead of filling in a default.
    let fullnode_config_path = working_dir.join(IOTA_FULLNODE_CONFIG);
    let mut fullnode_config: serde_yaml::Value =
        serde_yaml::from_str(&read_to_string(&fullnode_config_path)?)?;
    let fields = fullnode_config.as_mapping_mut().unwrap();
    fields.insert("enable-grpc-api".into(), true.into());
    fields.insert("grpc-api-config".into(), serde_yaml::Value::Null);
    write(
        &fullnode_config_path,
        serde_yaml::to_string(&fullnode_config)?,
    )?;

    let err = tokio::time::timeout(
        Duration::from_secs(30),
        start_command(working_dir, false, vec![]).execute(),
    )
    .await
    .expect("start must reject the config instead of launching a network")
    .unwrap_err();

    let err = format!("{err:#}");
    assert!(err.contains(IOTA_FULLNODE_CONFIG), "{err}");
    assert!(err.contains("`grpc-api-config` is missing"), "{err}");

    tmp_dir.close()?;
    Ok(())
}

// A plain tokio test for the same reason as the test above.
#[cfg(not(msim))]
#[tokio::test]
async fn test_start_rejects_a_persisted_fullnode_config_with_a_consensus_config()
-> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    genesis_command(working_dir, 1).execute().await?;

    // A `consensus-config` turns the file into a validator config, which the
    // fullnode checks are not written for: `validate` skips the gRPC API rule
    // for validators, so the localnet must reject the file up front.
    let fullnode_config_path = working_dir.join(IOTA_FULLNODE_CONFIG);
    let mut fullnode_config: serde_yaml::Value =
        serde_yaml::from_str(&read_to_string(&fullnode_config_path)?)?;
    let fields = fullnode_config.as_mapping_mut().unwrap();
    fields.insert(
        "consensus-config".into(),
        serde_yaml::from_str("db-path: consensus-db")?,
    );
    fields.insert("enable-grpc-api".into(), true.into());
    fields.insert("grpc-api-config".into(), serde_yaml::Value::Null);
    write(
        &fullnode_config_path,
        serde_yaml::to_string(&fullnode_config)?,
    )?;

    let err = tokio::time::timeout(
        Duration::from_secs(30),
        start_command(working_dir, false, vec![]).execute(),
    )
    .await
    .expect("start must reject the config instead of launching a network")
    .unwrap_err();

    let err = format!("{err:#}");
    assert!(err.contains(IOTA_FULLNODE_CONFIG), "{err}");
    assert!(err.contains("which makes it a validator config"), "{err}");

    tmp_dir.close()?;
    Ok(())
}

#[tokio::test]
async fn test_parse_host_port() {
    let input = "127.0.0.0";
    let result = parse_host_port(input.to_string(), 9123).unwrap();
    assert_eq!(result, "127.0.0.0:9123".parse::<SocketAddr>().unwrap());

    let input = "127.0.0.5:9124";
    let result = parse_host_port(input.to_string(), 9123).unwrap();
    assert_eq!(result, "127.0.0.5:9124".parse::<SocketAddr>().unwrap());

    let input = "9090";
    let result = parse_host_port(input.to_string(), 9123).unwrap();
    assert_eq!(result, "0.0.0.0:9090".parse::<SocketAddr>().unwrap());

    let input = "";
    let result = parse_host_port(input.to_string(), 9123).unwrap();
    assert_eq!(result, "0.0.0.0:9123".parse::<SocketAddr>().unwrap());

    let result = parse_host_port("localhost".to_string(), 9899).unwrap();
    assert_eq!(result, "127.0.0.1:9899".parse::<SocketAddr>().unwrap());

    let input = "asg";
    assert!(parse_host_port(input.to_string(), 9123).is_err());
    let input = "127.0.0:900";
    assert!(parse_host_port(input.to_string(), 9123).is_err());
    let input = "127.0.0";
    assert!(parse_host_port(input.to_string(), 9123).is_err());
    let input = "127.";
    assert!(parse_host_port(input.to_string(), 9123).is_err());
    let input = "127.9.0.1:asb";
    assert!(parse_host_port(input.to_string(), 9123).is_err());
}
