// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, fs::read_dir, net::SocketAddr, time::Duration};

use iota_config::{
    IOTA_CLIENT_CONFIG, IOTA_FULLNODE_CONFIG, IOTA_GENESIS_FILENAME, IOTA_KEYSTORE_FILENAME,
    IOTA_NETWORK_CONFIG, NodeConfig, PersistedConfig,
};
use iota_keys::keystore::AccountKeystore;
#[cfg(feature = "indexer")]
use iota_localnet::commands::IndexerFeatureArgs;
use iota_localnet::commands::{LocalnetCommand, parse_host_port};
use iota_macros::sim_test;
use iota_sdk::iota_client_config::IotaClientConfig;
use iota_swarm_config::{
    genesis_config::DEFAULT_NUMBER_OF_AUTHORITIES, network_config::NetworkConfigLight,
};
use iota_types::traffic_control::PolicyConfig;

#[sim_test]
async fn test_genesis() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    // Genesis
    LocalnetCommand::Genesis {
        working_dir: Some(working_dir.to_path_buf()),
        write_config: None,
        force: false,
        from_config: None,
        epoch_duration_ms: None,
        benchmark_ips: None,
        with_faucet: false,
        committee_size: DEFAULT_NUMBER_OF_AUTHORITIES,
        num_additional_gas_accounts: None,
        chain_start_timestamp_ms: None,
        admin_interface_address: None,
    }
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
    let result = LocalnetCommand::Genesis {
        working_dir: Some(working_dir.to_path_buf()),
        write_config: None,
        force: false,
        from_config: None,
        epoch_duration_ms: None,
        benchmark_ips: None,
        with_faucet: false,
        committee_size: DEFAULT_NUMBER_OF_AUTHORITIES,
        num_additional_gas_accounts: None,
        chain_start_timestamp_ms: None,
        admin_interface_address: None,
    }
    .execute()
    .await;
    assert!(matches!(result, Err(..)));

    tmp_dir.close()?;
    Ok(())
}

/// The port layout documented in
/// `docs/content/developer/references/cli/localnet.mdx`.
#[tokio::test]
async fn genesis_gives_every_node_fixed_ports() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    LocalnetCommand::Genesis {
        working_dir: Some(working_dir.to_path_buf()),
        write_config: None,
        force: false,
        from_config: None,
        epoch_duration_ms: None,
        benchmark_ips: None,
        with_faucet: false,
        committee_size: 4,
        num_additional_gas_accounts: None,
        chain_start_timestamp_ms: None,
        admin_interface_address: None,
    }
    .execute()
    .await?;

    let network_config =
        PersistedConfig::<NetworkConfigLight>::read(&working_dir.join(IOTA_NETWORK_CONFIG))?;
    let mut ports = Vec::new();

    for (i, validator) in network_config.validator_configs().iter().enumerate() {
        let base = 9200 + 10 * i as u16;
        assert_eq!(
            validator.network_address.to_string(),
            format!("/ip4/127.0.0.1/tcp/{base}/http")
        );
        assert_eq!(
            validator
                .p2p_config
                .external_address
                .as_ref()
                .unwrap()
                .to_string(),
            format!("/ip4/127.0.0.1/udp/{}/http", base + 1)
        );
        assert_eq!(
            validator.p2p_config.listen_address,
            SocketAddr::from(([127, 0, 0, 1], base + 1))
        );
        assert_eq!(
            validator.metrics_address,
            SocketAddr::from(([127, 0, 0, 1], base + 2))
        );
        assert_eq!(
            validator.admin_interface_address,
            SocketAddr::from(([127, 0, 0, 1], base + 4))
        );
        ports.extend([base, base + 1, base + 2, base + 4]);
    }

    // The primary address is only kept in the committee metadata of the genesis
    // blob, in an order of its own.
    let genesis = network_config.validator_configs()[0].genesis.genesis()?;
    let primary_addresses = genesis
        .validator_set_for_tooling()
        .iter()
        .map(|validator| validator.verified_metadata().primary_address.to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        primary_addresses,
        (0..4)
            .map(|i| format!("/ip4/127.0.0.1/udp/{}/http", 9203 + 10 * i))
            .collect::<BTreeSet<_>>()
    );
    ports.extend((0..4).map(|i| 9203 + 10 * i));

    let fullnode = PersistedConfig::<NodeConfig>::read(&working_dir.join(IOTA_FULLNODE_CONFIG))?;
    assert_eq!(
        fullnode.metrics_address,
        SocketAddr::from(([127, 0, 0, 1], 9184))
    );
    assert_eq!(
        fullnode.admin_interface_address,
        SocketAddr::from(([127, 0, 0, 1], 9185))
    );
    assert_eq!(
        fullnode
            .p2p_config
            .external_address
            .as_ref()
            .unwrap()
            .to_string(),
        "/ip4/127.0.0.1/udp/9186/http"
    );
    assert_eq!(
        fullnode.p2p_config.listen_address,
        SocketAddr::from(([127, 0, 0, 1], 9186))
    );
    assert_eq!(fullnode.json_rpc_address.port(), 9000);
    ports.extend([9184, 9185, 9186, 9000]);

    assert_eq!(
        ports.iter().collect::<BTreeSet<_>>().len(),
        ports.len(),
        "two nodes share a port: {ports:?}"
    );

    tmp_dir.close()?;
    Ok(())
}

// A node started from a written config gets the default
// denial-of-service protection, matching what an absent `policy-config`
// key deserializes to.
#[tokio::test]
async fn genesis_writes_the_default_traffic_control_policy() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    LocalnetCommand::Genesis {
        working_dir: Some(working_dir.to_path_buf()),
        write_config: None,
        force: false,
        from_config: None,
        epoch_duration_ms: None,
        benchmark_ips: None,
        with_faucet: false,
        committee_size: DEFAULT_NUMBER_OF_AUTHORITIES,
        num_additional_gas_accounts: None,
        chain_start_timestamp_ms: None,
        admin_interface_address: None,
    }
    .execute()
    .await?;

    let expected = format!("{:?}", Some(PolicyConfig::default_dos_protection_policy()));

    let network_conf =
        PersistedConfig::<NetworkConfigLight>::read(&working_dir.join(IOTA_NETWORK_CONFIG))?;
    for validator in network_conf.validator_configs() {
        assert_eq!(format!("{:?}", validator.policy_config), expected);
    }

    let fullnode_conf =
        PersistedConfig::<NodeConfig>::read(&working_dir.join(IOTA_FULLNODE_CONFIG))?;
    assert_eq!(format!("{:?}", fullnode_conf.policy_config), expected);

    tmp_dir.close()?;
    Ok(())
}

/// Waits for the fullnode to execute a checkpoint, which it can only get by
/// syncing from the validators.
#[cfg(not(msim))]
async fn wait_until_the_fullnode_syncs(rpc_port: u16) {
    use iota_sdk::IotaClientBuilder;

    let url = format!("http://127.0.0.1:{rpc_port}");
    let synced = tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            if let Ok(client) = IotaClientBuilder::default().build(&url).await {
                if let Ok(checkpoint) = client
                    .read_api()
                    .get_latest_checkpoint_sequence_number()
                    .await
                {
                    // Checkpoint 0 comes from the genesis blob the fullnode
                    // already has, so only a later one is synced.
                    if checkpoint > 0 {
                        return;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await;

    assert!(synced.is_ok(), "the fullnode executed no checkpoint");
}

/// The simulator gives every node an address of its own and routes loopback
/// addresses back to the caller, so the fullnode's JSON-RPC endpoint, which
/// `start` binds on all interfaces without reporting the address, cannot be
/// reached from the test. Give the network time to come up instead.
#[cfg(msim)]
async fn wait_until_the_fullnode_syncs(_rpc_port: u16) {
    tokio::time::sleep(Duration::from_secs(10)).await;
}

#[sim_test]
async fn test_start() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();
    let fullnode_rpc_port = 9000;

    // `start` runs until the network fails, so race it against the fullnode.
    tokio::select! {
        result = LocalnetCommand::Start {
            #[cfg(feature = "indexer")]
            data_ingestion_dir: None,
            config_dir: Some(working_dir.to_path_buf()),
            no_full_node: false,
            disable_fullnode_pruning: false,
            force_regenesis: false,
            with_faucet: None,
            faucet_amount: None,
            faucet_coin_count: None,
            with_grpc: None,
            node_config_override: vec![],
            fullnode_rpc_port,
            committee_size: None,
            epoch_duration_ms: None,
            #[cfg(feature = "indexer")]
            indexer_feature_args: Box::new(IndexerFeatureArgs::for_testing()),
        }
        .execute() => {
            result?;
            unreachable!("the local network stops on failure only");
        }
        () = wait_until_the_fullnode_syncs(fullnode_rpc_port) => {}
    }

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

/// The simulator panics when two nodes share an IP, so a second validator
/// catches a layout that pins addresses instead of ports.
#[sim_test]
async fn start_works_with_a_committee_of_two() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    if let Ok(res) = tokio::time::timeout(
        Duration::from_secs(10),
        LocalnetCommand::Start {
            #[cfg(feature = "indexer")]
            data_ingestion_dir: None,
            config_dir: Some(working_dir.to_path_buf()),
            no_full_node: false,
            disable_fullnode_pruning: false,
            force_regenesis: false,
            with_faucet: None,
            faucet_amount: None,
            faucet_coin_count: None,
            with_grpc: None,
            node_config_override: vec![],
            fullnode_rpc_port: 9000,
            committee_size: Some(2),
            epoch_duration_ms: None,
            #[cfg(feature = "indexer")]
            indexer_feature_args: Box::new(IndexerFeatureArgs::for_testing()),
        }
        .execute(),
    )
    .await
    {
        res.unwrap();
    };

    let network_conf =
        PersistedConfig::<NetworkConfigLight>::read(&working_dir.join(IOTA_NETWORK_CONFIG))?;
    assert_eq!(2, network_conf.validator_configs().len());

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
